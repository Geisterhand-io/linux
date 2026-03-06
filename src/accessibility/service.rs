use std::sync::Arc;

use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
use atspi::proxy::proxy_ext::ProxyExt;
use atspi::{AccessibilityConnection, CoordType, Interface, Role, State};
use tokio::sync::OnceCell;
use tracing::debug;
use zbus::fdo::DBusProxy;
use zbus::names::OwnedUniqueName;

use crate::models::accessibility::*;
use crate::models::api::AppInfo;

const DEFAULT_MAX_DEPTH: i32 = 5;
const DEFAULT_MAX_RESULTS: usize = 50;

/// Meaningful roles that should appear in compact tree output even without text.
const MEANINGFUL_ROLES: &[Role] = &[
    Role::Button,
    Role::PushButtonMenu,
    Role::ToggleButton,
    Role::CheckBox,
    Role::RadioButton,
    Role::Entry,
    Role::PasswordText,
    Role::Text,
    Role::Link,
    Role::ComboBox,
    Role::Slider,
    Role::SpinButton,
    Role::PageTabList,
    Role::PageTab,
    Role::Table,
    Role::List,
    Role::ListItem,
    Role::TreeItem,
    Role::MenuItem,
    Role::CheckMenuItem,
    Role::RadioMenuItem,
    Role::Menu,
    Role::MenuBar,
    Role::ToolBar,
    Role::Dialog,
    Role::Window,
    Role::Frame,
    Role::Label,
    Role::Image,
    Role::ScrollPane,
    Role::StatusBar,
    Role::Separator,
];

static ATSPI_CONNECTION: OnceCell<Arc<AccessibilityConnection>> = OnceCell::const_new();

/// Get or initialize the global AT-SPI2 connection.
pub async fn get_connection() -> anyhow::Result<Arc<AccessibilityConnection>> {
    let conn = ATSPI_CONNECTION
        .get_or_try_init(|| async {
            let conn = AccessibilityConnection::new().await?;
            Ok::<_, anyhow::Error>(Arc::new(conn))
        })
        .await?;
    Ok(conn.clone())
}

/// Build an ObjectRef from a proxy's destination and path.
fn object_ref_from_proxy(proxy: &AccessibleProxy<'_>) -> anyhow::Result<atspi::ObjectRef> {
    let dest = proxy.inner().destination();
    let unique_name: OwnedUniqueName = dest
        .as_str()
        .try_into()
        .map_err(|e| anyhow::anyhow!("Invalid bus name '{}': {}", dest, e))?;
    Ok(atspi::ObjectRef {
        name: unique_name,
        path: proxy.inner().path().to_owned().into(),
    })
}

/// Find the AT-SPI2 application root ObjectRef for a given PID.
async fn find_app_by_pid(
    conn: &zbus::Connection,
    target_pid: i32,
) -> anyhow::Result<Option<atspi::ObjectRef>> {
    let registry_acc = AccessibleProxy::builder(conn)
        .destination("org.a11y.atspi.Registry")?
        .path("/org/a11y/atspi/accessible/root")?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;

    let app_refs = registry_acc.get_children().await?;
    let dbus = DBusProxy::new(conn).await?;

    for app_ref in &app_refs {
        let bus_name: zbus::names::BusName = app_ref.name.as_str().try_into()?;
        match dbus.get_connection_unix_process_id(bus_name).await {
            Ok(pid) if pid as i32 == target_pid => {
                return Ok(Some(app_ref.clone()));
            }
            _ => continue,
        }
    }

    Ok(None)
}

/// Get the frontmost (focused) application's PID and name via AT-SPI2.
pub async fn get_frontmost_app() -> anyhow::Result<Option<AppInfo>> {
    let atspi_conn = get_connection().await?;
    let conn = atspi_conn.connection();

    let registry_acc = AccessibleProxy::builder(conn)
        .destination("org.a11y.atspi.Registry")?
        .path("/org/a11y/atspi/accessible/root")?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;

    let app_refs = registry_acc.get_children().await?;
    let dbus = DBusProxy::new(conn).await?;

    // Find the app with active state
    for app_ref in &app_refs {
        let root = match app_ref.as_accessible_proxy(conn).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let states = root.get_state().await.unwrap_or_default();

        if states.contains(State::Active) {
            let name = root.name().await.unwrap_or_default();
            let bus_name: zbus::names::BusName = app_ref.name.as_str().try_into()?;
            let pid = dbus
                .get_connection_unix_process_id(bus_name)
                .await
                .unwrap_or(0) as i32;

            if !name.is_empty() {
                return Ok(Some(AppInfo {
                    name,
                    desktop_file: None,
                    process_identifier: pid,
                }));
            }
        }
    }

    Ok(None)
}

/// Resolve the target PID - uses explicit PID or finds frontmost app.
async fn resolve_pid(
    conn: &zbus::Connection,
    pid: Option<i32>,
) -> anyhow::Result<(i32, atspi::ObjectRef)> {
    if let Some(pid) = pid {
        let app_ref = find_app_by_pid(conn, pid)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No accessible application found for PID {}", pid))?;
        Ok((pid, app_ref))
    } else {
        // Find the active/frontmost app via multiple heuristics
        let registry_acc = AccessibleProxy::builder(conn)
            .destination("org.a11y.atspi.Registry")?
            .path("/org/a11y/atspi/accessible/root")?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;

        let app_refs = registry_acc.get_children().await?;
        let dbus = DBusProxy::new(conn).await?;

        // Strategy 1: Find app with Active state on root
        for app_ref in &app_refs {
            let root = match app_ref.as_accessible_proxy(conn).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            let states = root.get_state().await.unwrap_or_default();
            if states.contains(State::Active) {
                let bus_name: zbus::names::BusName = app_ref.name.as_str().try_into()?;
                let pid = dbus
                    .get_connection_unix_process_id(bus_name)
                    .await
                    .unwrap_or(0) as i32;
                return Ok((pid, app_ref.clone()));
            }
        }

        // Strategy 2: Find app with a Focused or Active child window
        for app_ref in &app_refs {
            let root = match app_ref.as_accessible_proxy(conn).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            let children = root.get_children().await.unwrap_or_default();
            for child_ref in &children {
                let child = match child_ref.as_accessible_proxy(conn).await {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let states = child.get_state().await.unwrap_or_default();
                if states.contains(State::Active) || states.contains(State::Focused) {
                    let bus_name: zbus::names::BusName = app_ref.name.as_str().try_into()?;
                    let pid = dbus
                        .get_connection_unix_process_id(bus_name)
                        .await
                        .unwrap_or(0) as i32;
                    return Ok((pid, app_ref.clone()));
                }
            }
        }

        anyhow::bail!("No active application found. Specify a PID.")
    }
}

/// Navigate from an app root to a specific element by path indices.
async fn navigate_to_element<'a>(
    conn: &zbus::Connection,
    root: &AccessibleProxy<'a>,
    path: &[i32],
) -> anyhow::Result<atspi::ObjectRef> {
    let mut current_ref = atspi::ObjectRef {
        name: {
            let dest = root.inner().destination();
            dest.as_str().try_into().map_err(|e| anyhow::anyhow!("Invalid bus name: {}", e))?
        },
        path: root.inner().path().to_owned().into(),
    };

    for (i, &index) in path.iter().enumerate() {
        let proxy = current_ref.as_accessible_proxy(conn).await?;
        let children = proxy.get_children().await?;
        let idx = index as usize;
        if idx >= children.len() {
            anyhow::bail!(
                "Child index {} out of bounds at path position {} (has {} children)",
                index,
                i,
                children.len()
            );
        }
        current_ref = children[idx].clone();
    }

    Ok(current_ref)
}

/// Build UIElementInfo from an accessible proxy.
async fn build_element_info(
    conn: &zbus::Connection,
    proxy: &AccessibleProxy<'_>,
    pid: i32,
    path: Vec<i32>,
    depth: i32,
    max_depth: i32,
) -> anyhow::Result<UIElementInfo> {
    let role = proxy.get_role().await.unwrap_or(Role::Unknown);
    let name = proxy.name().await.ok().filter(|s| !s.is_empty());
    let description = proxy.description().await.ok().filter(|s| !s.is_empty());
    let states = proxy.get_state().await.unwrap_or_default();
    let ifaces = proxy.get_interfaces().await.unwrap_or_default();

    // Get label (same as description in AT-SPI2 context)
    let label = description.clone();

    // Get value from Text or Value interface
    let value = get_element_value(conn, proxy, &ifaces).await;

    // Get placeholder value from attributes
    let placeholder_value = get_placeholder_value(proxy).await;

    // Get frame from Component interface
    let frame = if ifaces.contains(Interface::Component) {
        get_element_frame(proxy).await
    } else {
        None
    };

    // Get actions
    let actions = if ifaces.contains(Interface::Action) {
        get_element_actions(proxy).await
    } else {
        None
    };

    // Build children if within depth limit
    let children = if depth < max_depth {
        let child_refs = proxy.get_children().await.unwrap_or_default();
        let mut child_infos = Vec::new();
        for (i, child_ref) in child_refs.iter().enumerate() {
            if let Ok(child_proxy) = child_ref.as_accessible_proxy(conn).await {
                let mut child_path = path.clone();
                child_path.push(i as i32);
                match Box::pin(build_element_info(conn, &child_proxy, pid, child_path, depth + 1, max_depth))
                    .await
                {
                    Ok(info) => child_infos.push(info),
                    Err(e) => {
                        debug!("Error building child info at index {}: {}", i, e);
                    }
                }
            }
        }
        if child_infos.is_empty() {
            None
        } else {
            Some(child_infos)
        }
    } else {
        None
    };

    Ok(UIElementInfo {
        path: ElementPath {
            pid,
            path,
        },
        role: role.name().to_string(),
        title: name,
        label,
        value,
        placeholder_value,
        element_description: description,
        frame,
        is_enabled: Some(states.contains(State::Enabled)),
        is_focused: Some(states.contains(State::Focused)),
        actions,
        children,
    })
}

/// Get placeholder text from AT-SPI2 attributes (GTK4 exposes "placeholder-text").
async fn get_placeholder_value(proxy: &AccessibleProxy<'_>) -> Option<String> {
    let attrs = proxy.get_attributes().await.ok()?;
    attrs.get("placeholder-text").cloned().filter(|s| !s.is_empty())
}

/// Get element value from Text or Value interface.
async fn get_element_value(
    _conn: &zbus::Connection,
    proxy: &AccessibleProxy<'_>,
    ifaces: &atspi::InterfaceSet,
) -> Option<String> {
    if ifaces.contains(Interface::Text) {
        if let Ok(mut proxies) = proxy.proxies().await {
            if let Ok(text) = proxies.text() {
                let count = text.character_count().await.unwrap_or(0);
                if count > 0 {
                    if let Ok(content) = text.get_text(0, count).await {
                        if !content.is_empty() {
                            return Some(content);
                        }
                    }
                }
            }
        }
    }

    if ifaces.contains(Interface::Value) {
        if let Ok(mut proxies) = proxy.proxies().await {
            if let Ok(val) = proxies.value() {
                if let Ok(current) = val.current_value().await {
                    return Some(format!("{}", current));
                }
            }
        }
    }

    None
}

/// Get element frame from Component interface.
async fn get_element_frame(proxy: &AccessibleProxy<'_>) -> Option<ElementFrame> {
    let mut proxies = proxy.proxies().await.ok()?;
    let comp = proxies.component().ok()?;
    let (x, y, w, h) = comp.get_extents(CoordType::Screen).await.ok()?;
    Some(ElementFrame {
        x: x as f64,
        y: y as f64,
        width: w as f64,
        height: h as f64,
    })
}

/// Build an ActionProxy directly from an AccessibleProxy's connection info.
async fn build_action_proxy<'a>(
    conn: &'a zbus::Connection,
    proxy: &AccessibleProxy<'_>,
) -> anyhow::Result<atspi::proxy::action::ActionProxy<'a>> {
    Ok(atspi::proxy::action::ActionProxy::builder(conn)
        .destination(proxy.inner().destination().to_owned())?
        .path(proxy.inner().path().to_owned())?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?)
}

/// Get element action names.
async fn get_element_actions(proxy: &AccessibleProxy<'_>) -> Option<Vec<String>> {
    let conn = proxy.inner().connection();
    let action = build_action_proxy(conn, proxy).await.ok()?;

    // Try bulk get_actions first, fall back to individual calls
    if let Ok(actions) = action.get_actions().await {
        let names: Vec<String> = actions.iter().map(|a| a.name.clone()).collect();
        if !names.is_empty() {
            return Some(names);
        }
    }

    // Fallback: use get_name for each action index
    let mut names = Vec::new();
    for i in 0..10 {
        match action.get_name(i).await {
            Ok(name) if !name.is_empty() => names.push(name),
            _ => break,
        }
    }
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

/// Get app info from an ObjectRef.
async fn get_app_info_async(
    conn: &zbus::Connection,
    app_ref: &atspi::ObjectRef,
    pid: i32,
) -> AppInfo {
    let name = match app_ref.as_accessible_proxy(conn).await {
        Ok(root) => root.name().await.unwrap_or_else(|_| "Unknown".to_string()),
        Err(_) => "Unknown".to_string(),
    };

    AppInfo {
        name,
        desktop_file: None,
        process_identifier: pid,
    }
}

// -- Public API methods matching the macOS AccessibilityService --

/// Get the accessibility tree for an application.
pub async fn get_tree(
    pid: Option<i32>,
    max_depth: Option<i32>,
    root_path: Option<Vec<i32>>,
) -> GetTreeResponse {
    match get_tree_inner(pid, max_depth, root_path).await {
        Ok(resp) => resp,
        Err(e) => GetTreeResponse {
            success: false,
            app: None,
            tree: None,
            error: Some(e.to_string()),
        },
    }
}

async fn get_tree_inner(
    pid: Option<i32>,
    max_depth: Option<i32>,
    root_path: Option<Vec<i32>>,
) -> anyhow::Result<GetTreeResponse> {
    let atspi_conn = get_connection().await?;
    let conn = atspi_conn.connection();

    let (pid, app_ref) = resolve_pid(conn, pid).await?;
    let app_info = get_app_info_async(conn, &app_ref, pid).await;

    let root_proxy = app_ref.as_accessible_proxy(conn).await?;
    let max_depth = max_depth.unwrap_or(DEFAULT_MAX_DEPTH);

    // Navigate to root_path if specified
    let (start_proxy_ref, start_path) = if let Some(ref rp) = root_path {
        let obj_ref = navigate_to_element(conn, &root_proxy, rp).await?;
        (obj_ref, rp.clone())
    } else {
        let obj_ref = object_ref_from_proxy(&root_proxy)?;
        (obj_ref, vec![])
    };

    let start_proxy = start_proxy_ref.as_accessible_proxy(conn).await?;
    let tree = build_element_info(conn, &start_proxy, pid, start_path, 0, max_depth).await?;

    Ok(GetTreeResponse {
        success: true,
        app: Some(app_info),
        tree: Some(tree),
        error: None,
    })
}

/// Get the compact (flattened) tree for an application.
pub async fn get_compact_tree(
    pid: Option<i32>,
    max_depth: Option<i32>,
    include_actions: bool,
    root_path: Option<Vec<i32>>,
) -> GetCompactTreeResponse {
    match get_compact_tree_inner(pid, max_depth, include_actions, root_path).await {
        Ok(resp) => resp,
        Err(e) => GetCompactTreeResponse {
            success: false,
            app: None,
            elements: None,
            count: None,
            error: Some(e.to_string()),
        },
    }
}

async fn get_compact_tree_inner(
    pid: Option<i32>,
    max_depth: Option<i32>,
    include_actions: bool,
    root_path: Option<Vec<i32>>,
) -> anyhow::Result<GetCompactTreeResponse> {
    let atspi_conn = get_connection().await?;
    let conn = atspi_conn.connection();

    let (pid, app_ref) = resolve_pid(conn, pid).await?;
    let app_info = get_app_info_async(conn, &app_ref, pid).await;

    let root_proxy = app_ref.as_accessible_proxy(conn).await?;
    let max_depth = max_depth.unwrap_or(DEFAULT_MAX_DEPTH);

    let (start_proxy_ref, start_path) = if let Some(ref rp) = root_path {
        let obj_ref = navigate_to_element(conn, &root_proxy, rp).await?;
        (obj_ref, rp.clone())
    } else {
        let obj_ref = object_ref_from_proxy(&root_proxy)?;
        (obj_ref, vec![])
    };

    let start_proxy = start_proxy_ref.as_accessible_proxy(conn).await?;
    let mut elements = Vec::new();
    collect_compact_elements(
        conn,
        &start_proxy,
        pid,
        start_path,
        0,
        max_depth,
        include_actions,
        &mut elements,
    )
    .await;

    let count = elements.len();
    Ok(GetCompactTreeResponse {
        success: true,
        app: Some(app_info),
        elements: Some(elements),
        count: Some(count),
        error: None,
    })
}

/// Recursively collect compact elements, filtering to meaningful ones.
#[allow(clippy::too_many_arguments)]
async fn collect_compact_elements(
    conn: &zbus::Connection,
    proxy: &AccessibleProxy<'_>,
    pid: i32,
    path: Vec<i32>,
    depth: i32,
    max_depth: i32,
    include_actions: bool,
    elements: &mut Vec<CompactElementInfo>,
) {
    let role = proxy.get_role().await.unwrap_or(Role::Unknown);
    let name = proxy.name().await.ok().filter(|s| !s.is_empty());
    let description = proxy.description().await.ok().filter(|s| !s.is_empty());
    let placeholder_value = get_placeholder_value(proxy).await;
    let ifaces = proxy.get_interfaces().await.unwrap_or_default();

    // Include if has text, placeholder, or is a meaningful role
    let has_text = name.is_some() || description.is_some() || placeholder_value.is_some();
    let is_meaningful = MEANINGFUL_ROLES.contains(&role);

    if has_text || is_meaningful {
        let frame = if ifaces.contains(Interface::Component) {
            get_element_frame(proxy).await
        } else {
            None
        };

        let actions = if include_actions && ifaces.contains(Interface::Action) {
            get_element_actions(proxy).await
        } else {
            None
        };

        elements.push(CompactElementInfo {
            path: ElementPath {
                pid,
                path: path.clone(),
            },
            role: role.name().to_string(),
            title: name,
            label: description,
            placeholder_value,
            frame,
            actions,
            depth,
        });
    }

    // Recurse into children
    if depth < max_depth {
        let child_refs = proxy.get_children().await.unwrap_or_default();
        for (i, child_ref) in child_refs.iter().enumerate() {
            if let Ok(child_proxy) = child_ref.as_accessible_proxy(conn).await {
                let mut child_path = path.clone();
                child_path.push(i as i32);
                Box::pin(collect_compact_elements(
                    conn,
                    &child_proxy,
                    pid,
                    child_path,
                    depth + 1,
                    max_depth,
                    include_actions,
                    elements,
                ))
                .await;
            }
        }
    }
}

/// Find elements matching a query.
pub async fn find_elements(
    pid: Option<i32>,
    query: ElementQuery,
) -> FindElementsResponse {
    match find_elements_inner(pid, query).await {
        Ok(resp) => resp,
        Err(e) => FindElementsResponse {
            success: false,
            elements: None,
            count: None,
            error: Some(e.to_string()),
        },
    }
}

async fn find_elements_inner(
    pid: Option<i32>,
    query: ElementQuery,
) -> anyhow::Result<FindElementsResponse> {
    let atspi_conn = get_connection().await?;
    let conn = atspi_conn.connection();

    let (pid, app_ref) = resolve_pid(conn, pid).await?;
    let root_proxy = app_ref.as_accessible_proxy(conn).await?;
    let max_results = query.max_results.unwrap_or(DEFAULT_MAX_RESULTS);

    let mut results = Vec::new();
    search_elements(
        conn,
        &root_proxy,
        pid,
        vec![],
        &query,
        max_results,
        &mut results,
    )
    .await;

    let count = results.len();
    Ok(FindElementsResponse {
        success: true,
        elements: Some(results),
        count: Some(count),
        error: None,
    })
}

/// DFS search for elements matching a query.
async fn search_elements(
    conn: &zbus::Connection,
    proxy: &AccessibleProxy<'_>,
    pid: i32,
    path: Vec<i32>,
    query: &ElementQuery,
    max_results: usize,
    results: &mut Vec<UIElementInfo>,
) {
    if results.len() >= max_results {
        return;
    }

    let role = proxy.get_role().await.unwrap_or(Role::Unknown);
    let name = proxy.name().await.ok().unwrap_or_default();
    let description = proxy.description().await.ok().unwrap_or_default();
    let ifaces = proxy.get_interfaces().await.unwrap_or_default();
    let value = get_element_value(conn, proxy, &ifaces).await;
    let placeholder_value = get_placeholder_value(proxy).await;

    // Check if this element matches the query
    let matches = element_matches_query(
        &role,
        &name,
        &description,
        value.as_deref(),
        placeholder_value.as_deref(),
        query,
    );

    if matches {
        let states = proxy.get_state().await.unwrap_or_default();
        let frame = if ifaces.contains(Interface::Component) {
            get_element_frame(proxy).await
        } else {
            None
        };
        let actions = if ifaces.contains(Interface::Action) {
            get_element_actions(proxy).await
        } else {
            None
        };

        results.push(UIElementInfo {
            path: ElementPath {
                pid,
                path: path.clone(),
            },
            role: role.name().to_string(),
            title: if name.is_empty() { None } else { Some(name.clone()) },
            label: if description.is_empty() {
                None
            } else {
                Some(description.clone())
            },
            value,
            placeholder_value,
            element_description: if description.is_empty() {
                None
            } else {
                Some(description.clone())
            },
            frame,
            is_enabled: Some(states.contains(State::Enabled)),
            is_focused: Some(states.contains(State::Focused)),
            actions,
            children: None,
        });
    }

    // Recurse into children
    let child_refs = proxy.get_children().await.unwrap_or_default();
    for (i, child_ref) in child_refs.iter().enumerate() {
        if results.len() >= max_results {
            return;
        }
        if let Ok(child_proxy) = child_ref.as_accessible_proxy(conn).await {
            let mut child_path = path.clone();
            child_path.push(i as i32);
            Box::pin(search_elements(
                conn,
                &child_proxy,
                pid,
                child_path,
                query,
                max_results,
                results,
            ))
            .await;
        }
    }
}

/// Check if an element matches a query (case-insensitive substring matching).
fn element_matches_query(
    role: &Role,
    name: &str,
    description: &str,
    value: Option<&str>,
    placeholder_value: Option<&str>,
    query: &ElementQuery,
) -> bool {
    if let Some(ref q_role) = query.role {
        let role_name = role.name();
        if !role_name.eq_ignore_ascii_case(q_role) {
            return false;
        }
    }

    if let Some(ref q_title) = query.title {
        if !name.eq_ignore_ascii_case(q_title) {
            return false;
        }
    }

    if let Some(ref q_title_contains) = query.title_contains {
        if !name.to_lowercase().contains(&q_title_contains.to_lowercase()) {
            return false;
        }
    }

    if let Some(ref q_label_contains) = query.label_contains {
        if !description
            .to_lowercase()
            .contains(&q_label_contains.to_lowercase())
        {
            return false;
        }
    }

    if let Some(ref q_value_contains) = query.value_contains {
        match value {
            Some(v) => {
                if !v.to_lowercase().contains(&q_value_contains.to_lowercase()) {
                    return false;
                }
            }
            None => return false,
        }
    }

    if let Some(ref q_placeholder_contains) = query.placeholder_contains {
        match placeholder_value {
            Some(p) => {
                if !p.to_lowercase().contains(&q_placeholder_contains.to_lowercase()) {
                    return false;
                }
            }
            None => return false,
        }
    }

    // At least one filter must have been specified
    query.role.is_some()
        || query.title.is_some()
        || query.title_contains.is_some()
        || query.label_contains.is_some()
        || query.value_contains.is_some()
        || query.placeholder_contains.is_some()
}

/// Get the focused element.
pub async fn get_focused_element(pid: Option<i32>) -> GetFocusedResponse {
    match get_focused_element_inner(pid).await {
        Ok(resp) => resp,
        Err(e) => GetFocusedResponse {
            success: false,
            element: None,
            error: Some(e.to_string()),
        },
    }
}

async fn get_focused_element_inner(pid: Option<i32>) -> anyhow::Result<GetFocusedResponse> {
    let atspi_conn = get_connection().await?;
    let conn = atspi_conn.connection();

    let (pid, app_ref) = resolve_pid(conn, pid).await?;
    let root_proxy = app_ref.as_accessible_proxy(conn).await?;

    // Search for the focused element in the tree
    if let Some(info) = find_focused_element(conn, &root_proxy, pid, vec![]).await {
        Ok(GetFocusedResponse {
            success: true,
            element: Some(info),
            error: None,
        })
    } else {
        Ok(GetFocusedResponse {
            success: true,
            element: None,
            error: Some("No focused element found".to_string()),
        })
    }
}

/// DFS search for the focused element.
async fn find_focused_element(
    conn: &zbus::Connection,
    proxy: &AccessibleProxy<'_>,
    pid: i32,
    path: Vec<i32>,
) -> Option<UIElementInfo> {
    let states = proxy.get_state().await.unwrap_or_default();
    let ifaces = proxy.get_interfaces().await.unwrap_or_default();

    if states.contains(State::Focused) {
        let role = proxy.get_role().await.unwrap_or(Role::Unknown);
        let name = proxy.name().await.ok().filter(|s| !s.is_empty());
        let description = proxy.description().await.ok().filter(|s| !s.is_empty());
        let value = get_element_value(conn, proxy, &ifaces).await;
        let placeholder_value = get_placeholder_value(proxy).await;
        let frame = if ifaces.contains(Interface::Component) {
            get_element_frame(proxy).await
        } else {
            None
        };
        let actions = if ifaces.contains(Interface::Action) {
            get_element_actions(proxy).await
        } else {
            None
        };

        return Some(UIElementInfo {
            path: ElementPath {
                pid,
                path,
            },
            role: role.name().to_string(),
            title: name,
            label: description.clone(),
            value,
            placeholder_value,
            element_description: description,
            frame,
            is_enabled: Some(states.contains(State::Enabled)),
            is_focused: Some(true),
            actions,
            children: None,
        });
    }

    // Recurse into children (only if this subtree might contain focus)
    let child_refs = proxy.get_children().await.unwrap_or_default();
    for (i, child_ref) in child_refs.iter().enumerate() {
        if let Ok(child_proxy) = child_ref.as_accessible_proxy(conn).await {
            let mut child_path = path.clone();
            child_path.push(i as i32);
            if let Some(info) =
                Box::pin(find_focused_element(conn, &child_proxy, pid, child_path)).await
            {
                return Some(info);
            }
        }
    }

    None
}

/// Get a specific element by path.
pub async fn get_element(
    pid: i32,
    path: Vec<i32>,
    child_depth: Option<i32>,
) -> GetElementResponse {
    match get_element_inner(pid, path, child_depth).await {
        Ok(resp) => resp,
        Err(e) => GetElementResponse {
            success: false,
            element: None,
            error: Some(e.to_string()),
        },
    }
}

async fn get_element_inner(
    pid: i32,
    path: Vec<i32>,
    child_depth: Option<i32>,
) -> anyhow::Result<GetElementResponse> {
    let atspi_conn = get_connection().await?;
    let conn = atspi_conn.connection();

    let app_ref = find_app_by_pid(conn, pid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No accessible application found for PID {}", pid))?;

    let root_proxy = app_ref.as_accessible_proxy(conn).await?;
    let element_ref = navigate_to_element(conn, &root_proxy, &path).await?;
    let element_proxy = element_ref.as_accessible_proxy(conn).await?;

    let max_depth = child_depth.unwrap_or(1);
    let info = build_element_info(conn, &element_proxy, pid, path, 0, max_depth).await?;

    Ok(GetElementResponse {
        success: true,
        element: Some(info),
        error: None,
    })
}

/// Perform an action on an element.
pub async fn perform_action(
    path: ElementPath,
    action: AccessibilityAction,
    value: Option<String>,
) -> ActionResponse {
    match perform_action_inner(path, action, value).await {
        Ok(resp) => resp,
        Err(e) => ActionResponse {
            success: false,
            action: None,
            error: Some(e.to_string()),
        },
    }
}

async fn perform_action_inner(
    path: ElementPath,
    action: AccessibilityAction,
    value: Option<String>,
) -> anyhow::Result<ActionResponse> {
    let atspi_conn = get_connection().await?;
    let conn = atspi_conn.connection();

    let app_ref = find_app_by_pid(conn, path.pid)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No accessible application found for PID {}",
                path.pid
            )
        })?;

    let root_proxy = app_ref.as_accessible_proxy(conn).await?;
    let element_ref = navigate_to_element(conn, &root_proxy, &path.path).await?;
    let element_proxy = element_ref.as_accessible_proxy(conn).await?;
    let ifaces = element_proxy.get_interfaces().await.unwrap_or_default();

    let action_name = match action {
        AccessibilityAction::Press => {
            if !ifaces.contains(Interface::Action) {
                anyhow::bail!("Element does not support actions (interfaces: {:?})", ifaces);
            }
            // Build ActionProxy directly from the element's destination/path
            let action_proxy = atspi::proxy::action::ActionProxy::builder(conn)
                .destination(element_proxy.inner().destination().to_owned())?
                .path(element_proxy.inner().path().to_owned())?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await?;

            // Try to find a specific action, falling back to index 0
            let count = action_proxy.nactions().await.unwrap_or(0);
            let mut idx = 0;
            for i in 0..count {
                if let Ok(name) = action_proxy.get_name(i).await {
                    let name_lower = name.to_lowercase();
                    if name_lower == "click" || name_lower == "press" || name_lower == "activate" {
                        idx = i;
                        break;
                    }
                }
            }
            action_proxy.do_action(idx).await?;
            "press".to_string()
        }

        AccessibilityAction::SetValue => {
            let val = value.ok_or_else(|| anyhow::anyhow!("setValue requires a value"))?;

            if ifaces.contains(Interface::EditableText) {
                let et = atspi::proxy::editable_text::EditableTextProxy::builder(conn)
                    .destination(element_proxy.inner().destination().to_owned())?
                    .path(element_proxy.inner().path().to_owned())?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await?;
                et.set_text_contents(&val).await?;
            } else if ifaces.contains(Interface::Value) {
                let num: f64 = val
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Value must be numeric for Value interface"))?;
                let vp = atspi::proxy::value::ValueProxy::builder(conn)
                    .destination(element_proxy.inner().destination().to_owned())?
                    .path(element_proxy.inner().path().to_owned())?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await?;
                vp.set_current_value(num).await?;
            } else {
                anyhow::bail!("Element does not support EditableText or Value interface");
            }
            "setValue".to_string()
        }

        AccessibilityAction::Focus => {
            if ifaces.contains(Interface::Component) {
                let comp = atspi::proxy::component::ComponentProxy::builder(conn)
                    .destination(element_proxy.inner().destination().to_owned())?
                    .path(element_proxy.inner().path().to_owned())?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await?;
                comp.grab_focus().await?;
            } else {
                anyhow::bail!("Element does not support Component interface for focus");
            }
            "focus".to_string()
        }

        AccessibilityAction::Confirm => {
            do_named_action(conn, &element_proxy, &["activate", "press", "click"]).await?;
            "confirm".to_string()
        }

        AccessibilityAction::Cancel => {
            // Try to find a cancel-like action, or send Escape via action
            do_named_action(conn, &element_proxy, &["cancel", "close"]).await?;
            "cancel".to_string()
        }

        AccessibilityAction::Increment => {
            if ifaces.contains(Interface::Value) {
                let vp = atspi::proxy::value::ValueProxy::builder(conn)
                    .destination(element_proxy.inner().destination().to_owned())?
                    .path(element_proxy.inner().path().to_owned())?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await?;
                let current = vp.current_value().await?;
                let increment = vp.minimum_increment().await.unwrap_or(1.0);
                vp.set_current_value(current + increment).await?;
            } else {
                anyhow::bail!("Element does not support Value interface for increment");
            }
            "increment".to_string()
        }

        AccessibilityAction::Decrement => {
            if ifaces.contains(Interface::Value) {
                let vp = atspi::proxy::value::ValueProxy::builder(conn)
                    .destination(element_proxy.inner().destination().to_owned())?
                    .path(element_proxy.inner().path().to_owned())?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await?;
                let current = vp.current_value().await?;
                let increment = vp.minimum_increment().await.unwrap_or(1.0);
                vp.set_current_value(current - increment).await?;
            } else {
                anyhow::bail!("Element does not support Value interface for decrement");
            }
            "decrement".to_string()
        }

        AccessibilityAction::ShowMenu => {
            do_named_action(conn, &element_proxy, &["showMenu", "menu", "popup"]).await?;
            "showMenu".to_string()
        }

        AccessibilityAction::Pick => {
            do_named_action(conn, &element_proxy, &["activate", "click", "press", "select"]).await?;
            "pick".to_string()
        }
    };

    Ok(ActionResponse {
        success: true,
        action: Some(action_name),
        error: None,
    })
}

/// Try to do one of the named actions on an element.
async fn do_named_action(
    conn: &zbus::Connection,
    proxy: &AccessibleProxy<'_>,
    names: &[&str],
) -> anyhow::Result<()> {
    let action = build_action_proxy(conn, proxy).await?;

    // Try to find a named action
    for i in 0..10 {
        match action.get_name(i).await {
            Ok(name) if !name.is_empty() => {
                for target in names {
                    if name.eq_ignore_ascii_case(target) {
                        action.do_action(i).await?;
                        return Ok(());
                    }
                }
            }
            _ => break,
        }
    }

    // Fallback: do the default action (index 0)
    action.do_action(0).await?;
    Ok(())
}

/// Set a value on the focused element for a given PID (used in run mode typing).
/// Falls back gracefully if no focused element is found.
pub async fn set_value_on_focused_element(pid: i32, value: &str) -> ActionResponse {
    // Find the focused element via AT-SPI2 state search
    let focused = get_focused_element(Some(pid)).await;
    if let Some(element) = focused.element {
        return perform_action(
            element.path,
            AccessibilityAction::SetValue,
            Some(value.to_string()),
        )
        .await;
    }

    ActionResponse {
        success: false,
        action: Some("setValue".to_string()),
        error: Some(
            "No focused element found — use /click on a text field first, or pass role/title to /type".to_string(),
        ),
    }
}

/// Find an element's frame (position + size) by ElementPath.
pub async fn find_element_frame(path: &ElementPath) -> anyhow::Result<Option<ElementFrame>> {
    let atspi_conn = get_connection().await?;
    let conn = atspi_conn.connection();

    let app_ref = find_app_by_pid(conn, path.pid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No accessible application found for PID {}", path.pid))?;

    let root_proxy = app_ref.as_accessible_proxy(conn).await?;
    let element_ref = navigate_to_element(conn, &root_proxy, &path.path).await?;
    let element_proxy = element_ref.as_accessible_proxy(conn).await?;

    Ok(get_element_frame(&element_proxy).await)
}
