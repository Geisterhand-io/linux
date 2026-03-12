use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use evdev::Key;
use futures_util::StreamExt;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
use atspi::Role;

use crate::input::backend::InputBackend;
use crate::input::keycode_map;
use crate::models::api::MouseButton;

/// Input backend using the XDG RemoteDesktop portal via D-Bus.
///
/// On GNOME Wayland, this shows a permission dialog once at session creation.
/// Once approved, all input is sent through the portal session without further
/// prompts. This avoids the repeated "Remote Desktop" dialog that XTest triggers
/// and does not require root (unlike uinput).
pub struct PortalBackend {
    conn: zbus::Connection,
    session_handle: String,
    /// The PipeWire stream node ID from ScreenCast, used for absolute mouse positioning.
    /// None if ScreenCast was not available — falls back to relative motion.
    stream_node_id: Option<u32>,
    /// Guards against concurrent D-Bus calls which could interleave.
    lock: Mutex<()>,
}

impl PortalBackend {
    /// Create a new portal backend by establishing a RemoteDesktop session
    /// with ScreenCast for absolute mouse positioning.
    ///
    /// This will:
    /// 1. Connect to the D-Bus session bus
    /// 2. Call CreateSession to get a session handle
    /// 3. Call SelectDevices to request keyboard + pointer access
    /// 4. Call SelectSources to request screen sharing (for absolute mouse)
    /// 5. Call Start to show the combined permission dialog (once)
    ///
    /// The permission dialog is auto-approved via AT-SPI if available.
    pub async fn new() -> Result<Self> {
        let conn = zbus::Connection::session()
            .await
            .context("Failed to connect to D-Bus session bus")?;

        let session_handle = create_session(&conn).await?;
        select_devices(&conn, &session_handle).await?;

        // SelectSources enables absolute mouse positioning via ScreenCast.
        // If it fails (e.g., portal doesn't support combined sessions), we
        // still try Start — absolute positioning may not work but keyboard will.
        match tokio::time::timeout(
            Duration::from_secs(5),
            select_sources(&conn, &session_handle),
        )
        .await
        {
            Ok(Ok(())) => tracing::debug!("ScreenCast SelectSources succeeded"),
            Ok(Err(e)) => tracing::warn!("SelectSources failed (continuing without ScreenCast): {}", e),
            Err(_) => tracing::warn!("SelectSources timed out (continuing without ScreenCast)"),
        }

        let stream_node_id = start_session_with_approval(&conn, &session_handle).await?;

        if let Some(id) = stream_node_id {
            tracing::info!("Portal backend ready (stream_node_id={})", id);
        } else {
            tracing::info!(
                "Portal backend ready (no ScreenCast — using relative mouse positioning)"
            );
        }

        Ok(Self {
            conn,
            session_handle,
            stream_node_id,
            lock: Mutex::new(()),
        })
    }

    fn call_button(&self, button: i32, state: u32) -> Result<()> {
        let _guard = self.lock.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        let session_path = ObjectPath::try_from(self.session_handle.as_str())
            .context("Invalid session handle")?;
        let options: HashMap<String, Value<'_>> = HashMap::new();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.conn
                    .call_method(
                        Some("org.freedesktop.portal.Desktop"),
                        "/org/freedesktop/portal/desktop",
                        Some("org.freedesktop.portal.RemoteDesktop"),
                        "NotifyPointerButton",
                        &(&session_path, &options, button, state),
                    )
                    .await
                    .context("Portal NotifyPointerButton call failed")?;
                Ok::<(), anyhow::Error>(())
            })
        })
    }

    fn call_key(&self, keycode: i32, state: u32) -> Result<()> {
        let _guard = self.lock.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        let session_path = ObjectPath::try_from(self.session_handle.as_str())
            .context("Invalid session handle")?;
        let options: HashMap<String, Value<'_>> = HashMap::new();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.conn
                    .call_method(
                        Some("org.freedesktop.portal.Desktop"),
                        "/org/freedesktop/portal/desktop",
                        Some("org.freedesktop.portal.RemoteDesktop"),
                        "NotifyKeyboardKeycode",
                        &(&session_path, &options, keycode, state),
                    )
                    .await
                    .context("Portal NotifyKeyboardKeycode call failed")?;
                Ok::<(), anyhow::Error>(())
            })
        })
    }
}

impl InputBackend for PortalBackend {
    fn mouse_move(&self, x: f64, y: f64) -> Result<()> {
        let _guard = self.lock.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        let session_path = ObjectPath::try_from(self.session_handle.as_str())
            .context("Invalid session handle")?;
        let options: HashMap<String, Value<'_>> = HashMap::new();

        if let Some(stream) = self.stream_node_id {
            // Absolute positioning via ScreenCast stream
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    self.conn
                        .call_method(
                            Some("org.freedesktop.portal.Desktop"),
                            "/org/freedesktop/portal/desktop",
                            Some("org.freedesktop.portal.RemoteDesktop"),
                            "NotifyPointerMotionAbsolute",
                            &(&session_path, &options, stream, x, y),
                        )
                        .await
                        .context("Portal NotifyPointerMotionAbsolute call failed")?;
                    Ok::<(), anyhow::Error>(())
                })
            })
        } else {
            // Fallback: relative motion. Move far negative to reset to (0,0), then to target.
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    self.conn
                        .call_method(
                            Some("org.freedesktop.portal.Desktop"),
                            "/org/freedesktop/portal/desktop",
                            Some("org.freedesktop.portal.RemoteDesktop"),
                            "NotifyPointerMotion",
                            &(&session_path, &options, -10000.0f64, -10000.0f64),
                        )
                        .await
                        .context("Portal NotifyPointerMotion (reset) failed")?;
                    self.conn
                        .call_method(
                            Some("org.freedesktop.portal.Desktop"),
                            "/org/freedesktop/portal/desktop",
                            Some("org.freedesktop.portal.RemoteDesktop"),
                            "NotifyPointerMotion",
                            &(&session_path, &options, x, y),
                        )
                        .await
                        .context("Portal NotifyPointerMotion (target) failed")?;
                    Ok::<(), anyhow::Error>(())
                })
            })
        }
    }

    fn mouse_click(&self, button: MouseButton, count: u32) -> Result<()> {
        let btn_code = match button {
            MouseButton::Left => 272,   // BTN_LEFT
            MouseButton::Right => 273,  // BTN_RIGHT
            MouseButton::Center => 274, // BTN_MIDDLE
        };

        for _ in 0..count {
            self.call_button(btn_code, 1)?; // press
            thread::sleep(Duration::from_millis(10));
            self.call_button(btn_code, 0)?; // release
            if count > 1 {
                thread::sleep(Duration::from_millis(50));
            }
        }
        Ok(())
    }

    fn scroll(&self, delta_x: f64, delta_y: f64) -> Result<()> {
        if delta_x.abs() < f64::EPSILON && delta_y.abs() < f64::EPSILON {
            return Ok(());
        }

        let _guard = self.lock.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        let session_path = ObjectPath::try_from(self.session_handle.as_str())
            .context("Invalid session handle")?;
        let options: HashMap<String, Value<'_>> = HashMap::new();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                if delta_y.abs() > f64::EPSILON {
                    let steps = delta_y.abs().ceil() as i32;
                    let direction = if delta_y > 0.0 { 1i32 } else { -1i32 };
                    for _ in 0..steps {
                        self.conn
                            .call_method(
                                Some("org.freedesktop.portal.Desktop"),
                                "/org/freedesktop/portal/desktop",
                                Some("org.freedesktop.portal.RemoteDesktop"),
                                "NotifyPointerAxisDiscrete",
                                &(&session_path, &options, 0u32, direction),
                            )
                            .await
                            .context("Portal NotifyPointerAxisDiscrete (vertical) failed")?;
                    }
                }

                if delta_x.abs() > f64::EPSILON {
                    let steps = delta_x.abs().ceil() as i32;
                    let direction = if delta_x > 0.0 { 1i32 } else { -1i32 };
                    for _ in 0..steps {
                        self.conn
                            .call_method(
                                Some("org.freedesktop.portal.Desktop"),
                                "/org/freedesktop/portal/desktop",
                                Some("org.freedesktop.portal.RemoteDesktop"),
                                "NotifyPointerAxisDiscrete",
                                &(&session_path, &options, 1u32, direction),
                            )
                            .await
                            .context("Portal NotifyPointerAxisDiscrete (horizontal) failed")?;
                    }
                }

                Ok::<(), anyhow::Error>(())
            })
        })
    }

    fn key_down(&self, keycode: u16) -> Result<()> {
        self.call_key(keycode as i32, 1)
    }

    fn key_up(&self, keycode: u16) -> Result<()> {
        self.call_key(keycode as i32, 0)
    }

    fn type_text(&self, text: &str, delay_ms: Option<u64>) -> Result<()> {
        let delay = delay_ms.map(Duration::from_millis);

        for ch in text.chars() {
            if let Some((key, needs_shift)) = keycode_map::char_to_key(ch) {
                if needs_shift {
                    self.key_down(Key::KEY_LEFTSHIFT.code())?;
                }
                self.key_press(key.code())?;
                if needs_shift {
                    self.key_up(Key::KEY_LEFTSHIFT.code())?;
                }
                if let Some(d) = delay {
                    thread::sleep(d);
                } else {
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "portal"
    }
}

// ---------------------------------------------------------------------------
// Portal session setup helpers
// ---------------------------------------------------------------------------

/// Generate a unique token for portal handle deduplication.
fn unique_token() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("geisterhand{}", id)
}

/// Compute the expected request object path for a portal call.
/// Portal spec: /org/freedesktop/portal/desktop/request/{sender}/{token}
/// where sender is the bus unique name with ':' removed and '.' replaced by '_'.
fn expected_request_path(conn: &zbus::Connection, token: &str) -> Result<String> {
    let unique = conn
        .unique_name()
        .ok_or_else(|| anyhow::anyhow!("No unique name on D-Bus connection"))?;
    let sender = unique.as_str().trim_start_matches(':').replace('.', "_");
    Ok(format!(
        "/org/freedesktop/portal/desktop/request/{}/{}",
        sender, token
    ))
}

/// Subscribe to the Response signal on the given path and return a message stream.
/// Must be called BEFORE making the portal method call to avoid missing the response.
async fn subscribe_response(
    conn: &zbus::Connection,
    request_path: &str,
) -> Result<zbus::MessageStream> {
    let rule = format!(
        "type='signal',interface='org.freedesktop.portal.Request',member='Response',path='{}'",
        request_path
    );

    conn.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "AddMatch",
        &(&rule),
    )
    .await
    .context("Failed to add match rule for portal response")?;

    Ok(zbus::MessageStream::from(conn))
}

/// Wait for the portal Response signal on an already-subscribed stream.
async fn receive_response(
    stream: &mut zbus::MessageStream,
    request_path: &str,
) -> Result<(u32, HashMap<String, OwnedValue>)> {
    while let Some(msg) = stream.next().await {
        let msg = msg.context("Error receiving D-Bus message")?;

        let header = msg.header();
        if header.message_type() != zbus::message::Type::Signal {
            continue;
        }

        let member = header.member().map(|m| m.as_str().to_string());
        let path = header.path().map(|p| p.as_str().to_string());
        let interface = header.interface().map(|i| i.as_str().to_string());

        if member.as_deref() != Some("Response") {
            continue;
        }
        if interface.as_deref() != Some("org.freedesktop.portal.Request") {
            continue;
        }
        if path.as_deref() != Some(request_path) {
            continue;
        }

        let (response_code, results): (u32, HashMap<String, OwnedValue>) = msg
            .body()
            .deserialize()
            .context("Failed to parse portal response body")?;

        return Ok((response_code, results));
    }

    anyhow::bail!("D-Bus message stream ended without portal response")
}

/// Create a RemoteDesktop session.
async fn create_session(conn: &zbus::Connection) -> Result<String> {
    let token = unique_token();
    let mut options: HashMap<String, Value<'_>> = HashMap::new();
    options.insert("handle_token".to_string(), Value::from(token.as_str()));
    options.insert(
        "session_handle_token".to_string(),
        Value::from(token.as_str()),
    );

    let request_path = expected_request_path(conn, &token)?;
    let mut stream = subscribe_response(conn, &request_path).await?;

    let reply = conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.RemoteDesktop"),
            "CreateSession",
            &(options,),
        )
        .await
        .context("Portal CreateSession call failed")?;

    // Log the returned request path for debugging
    if let Ok(returned_path) = reply.body().deserialize::<ObjectPath>() {
        tracing::debug!("CreateSession returned request path: {}", returned_path);
    }

    let (code, results) = tokio::time::timeout(
        Duration::from_secs(30),
        receive_response(&mut stream, &request_path),
    )
    .await
    .context("CreateSession timed out")??;

    if code != 0 {
        anyhow::bail!("CreateSession failed with response code {}", code);
    }

    let session_handle = results
        .get("session_handle")
        .and_then(|v| {
            let s: Result<String, _> = v.try_clone().unwrap().try_into();
            s.ok()
        })
        .ok_or_else(|| anyhow::anyhow!("No session_handle in CreateSession response"))?;

    tracing::debug!("Portal session created: {}", session_handle);
    Ok(session_handle)
}

/// Select keyboard and pointer devices for the session.
async fn select_devices(conn: &zbus::Connection, session_handle: &str) -> Result<()> {
    let token = unique_token();
    let session_path = ObjectPath::try_from(session_handle)
        .context("Invalid session handle for SelectDevices")?;

    let mut options: HashMap<String, Value<'_>> = HashMap::new();
    options.insert("handle_token".to_string(), Value::from(token.as_str()));
    // types: 1=keyboard, 2=pointer, 3=both
    options.insert("types".to_string(), Value::U32(3));

    let request_path = expected_request_path(conn, &token)?;
    let mut stream = subscribe_response(conn, &request_path).await?;

    let reply = conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.RemoteDesktop"),
            "SelectDevices",
            &(session_path, options),
        )
        .await
        .context("Portal SelectDevices call failed")?;

    // Check for immediate completion
    if let Ok(returned_path) = reply.body().deserialize::<ObjectPath>() {
        if returned_path.as_str() == "/" {
            tracing::debug!("Portal devices selected (immediate)");
            return Ok(());
        }
    }

    let (code, _results) = tokio::time::timeout(
        Duration::from_secs(30),
        receive_response(&mut stream, &request_path),
    )
    .await
    .context("SelectDevices timed out")??;

    if code != 0 {
        anyhow::bail!("SelectDevices failed with response code {}", code);
    }

    tracing::debug!("Portal devices selected (keyboard + pointer)");
    Ok(())
}

/// Select screen sources for absolute mouse positioning (ScreenCast on the same session).
async fn select_sources(conn: &zbus::Connection, session_handle: &str) -> Result<()> {
    let token = unique_token();
    let session_path = ObjectPath::try_from(session_handle)
        .context("Invalid session handle for SelectSources")?;

    let mut options: HashMap<String, Value<'_>> = HashMap::new();
    options.insert("handle_token".to_string(), Value::from(token.as_str()));
    // types: 1=monitor, 2=window, 4=virtual
    options.insert("types".to_string(), Value::U32(1));
    options.insert("multiple".to_string(), Value::Bool(false));

    let request_path = expected_request_path(conn, &token)?;
    let mut stream = subscribe_response(conn, &request_path).await?;

    let reply = conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.ScreenCast"),
            "SelectSources",
            &(session_path, options),
        )
        .await
        .context("Portal SelectSources call failed")?;

    // Check if the portal returned "/" as the request path — means completed immediately
    let body = reply.body();
    let returned_path: ObjectPath = body.deserialize().context("Failed to parse SelectSources reply")?;
    if returned_path.as_str() == "/" {
        tracing::debug!("Portal screen sources selected (immediate)");
        return Ok(());
    }

    let (code, _results) = tokio::time::timeout(
        Duration::from_secs(30),
        receive_response(&mut stream, &request_path),
    )
    .await
    .context("SelectSources timed out")??;

    if code != 0 {
        anyhow::bail!("SelectSources failed with response code {}", code);
    }

    tracing::debug!("Portal screen sources selected");
    Ok(())
}

/// Start the session with automatic dialog approval via AT-SPI.
/// Returns the PipeWire stream node_id for absolute mouse positioning,
/// or None if ScreenCast was not available.
async fn start_session_with_approval(
    conn: &zbus::Connection,
    session_handle: &str,
) -> Result<Option<u32>> {
    let token = unique_token();
    let session_path = ObjectPath::try_from(session_handle)
        .context("Invalid session handle for Start")?;

    let mut options: HashMap<String, Value<'_>> = HashMap::new();
    options.insert("handle_token".to_string(), Value::from(token.as_str()));

    let request_path = expected_request_path(conn, &token)?;
    let mut stream = subscribe_response(conn, &request_path).await?;

    // Spawn AT-SPI auto-approval before making the Start call.
    // The dialog appears when Start is called; we approve it in the background.
    let approval_handle = tokio::spawn(auto_approve_portal_dialog());

    conn.call_method(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        Some("org.freedesktop.portal.RemoteDesktop"),
        "Start",
        &(session_path, "", options),
    )
    .await
    .context("Portal Start call failed")?;

    let (code, results) = tokio::time::timeout(
        Duration::from_secs(120),
        receive_response(&mut stream, &request_path),
    )
    .await
    .context("Portal Start timed out (user may not have responded to permission dialog)")??;

    // Cancel auto-approval task if still running
    approval_handle.abort();

    if code != 0 {
        anyhow::bail!(
            "Portal Start failed with response code {} (user may have denied permission)",
            code
        );
    }

    // Extract the stream node_id from the "streams" key in the response.
    // Format: array of (node_id: u32, properties: dict)
    let stream_node_id = match extract_stream_node_id(&results) {
        Ok(id) => {
            tracing::debug!("Portal session started (stream_node_id={})", id);
            Some(id)
        }
        Err(e) => {
            tracing::warn!(
                "No stream node_id in Start response (absolute mouse positioning unavailable): {}",
                e
            );
            None
        }
    };

    Ok(stream_node_id)
}

/// Extract the PipeWire stream node_id from the Start response.
fn extract_stream_node_id(results: &HashMap<String, OwnedValue>) -> Result<u32> {
    use zbus::zvariant;

    let streams_val = results
        .get("streams")
        .ok_or_else(|| anyhow::anyhow!("No 'streams' in Start response"))?;

    // streams is a(ua{sv}) — array of (uint32, dict)
    // Try to deserialize as array of structures
    let streams_val = streams_val.try_clone()
        .map_err(|e| anyhow::anyhow!("Failed to clone streams value: {}", e))?;

    // Walk the Value structure to extract the first node_id
    if let zvariant::Value::Array(arr) = &*streams_val {
        for item in arr.iter() {
            if let zvariant::Value::Structure(struc) = item {
                let fields = struc.fields();
                if let Some(zvariant::Value::U32(node_id)) = fields.first() {
                    return Ok(*node_id);
                }
            }
        }
    }

    anyhow::bail!(
        "Could not extract stream node_id from Start response. streams value: {:?}",
        streams_val
    )
}

// ---------------------------------------------------------------------------
// AT-SPI automatic dialog approval
// ---------------------------------------------------------------------------

/// Automatically approve the GNOME portal "Remote Desktop" permission dialog.
/// Watches for the xdg-desktop-portal-gnome dialog via AT-SPI, toggles
/// "Allow Remote Interaction", and clicks "Share".
async fn auto_approve_portal_dialog() {
    // Give the dialog a moment to appear
    tokio::time::sleep(Duration::from_millis(500)).await;

    let a11y_conn = match atspi::AccessibilityConnection::new().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("AT-SPI auto-approve: accessibility connection failed: {}", e);
            return;
        }
    };
    let conn = a11y_conn.connection();

    // Poll for the portal dialog for up to 30 seconds
    for attempt in 0..60 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        match try_approve_dialog(conn).await {
            Ok(true) => {
                tracing::info!("Portal permission dialog auto-approved via AT-SPI");
                return;
            }
            Ok(false) => {
                // Dialog not found yet, keep polling
                continue;
            }
            Err(e) => {
                tracing::debug!("AT-SPI auto-approve attempt {}: {}", attempt, e);
                continue;
            }
        }
    }

    tracing::warn!(
        "AT-SPI auto-approve: dialog not found after 30s. \
         User may need to manually approve the 'Remote Desktop' dialog."
    );
}

/// Try to find and approve the portal dialog. Returns Ok(true) if approved,
/// Ok(false) if dialog not found yet, Err on AT-SPI errors.
async fn try_approve_dialog(conn: &zbus::Connection) -> Result<bool> {
    let registry = AccessibleProxy::builder(conn)
        .destination("org.a11y.atspi.Registry")?
        .path("/org/a11y/atspi/accessible/root")?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;

    let apps = registry.get_children().await?;

    for app_ref in &apps {
        let app_proxy: AccessibleProxy<'_> = match app_ref.as_accessible_proxy(conn).await {
            Ok(p) => p,
            Err(_) => continue,
        };
        let app_name = app_proxy.name().await.unwrap_or_default();
        if !app_name.contains("portal-gnome") {
            continue;
        }

        let children = app_proxy.get_children().await.unwrap_or_default();
        if children.is_empty() {
            continue;
        }

        // Found the portal-gnome app with a window — walk the tree
        let window_ref = &children[0];
        let window: AccessibleProxy<'_> = window_ref.as_accessible_proxy(conn).await?;
        let window_name = window.name().await.unwrap_or_default();

        if !window_name.contains("Remote Desktop") {
            continue;
        }

        tracing::debug!("Found portal dialog: {}", window_name);

        // Find and toggle "Allow Remote Interaction" checkbox, then click "Share"
        let mut toggled = false;
        let mut shared = false;

        walk_and_approve(conn, window_ref, &mut toggled, &mut shared).await?;

        if shared {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Recursively walk the AT-SPI tree to find the checkbox and Share button.
fn walk_and_approve<'a>(
    conn: &'a zbus::Connection,
    node_ref: &'a atspi::ObjectRef,
    toggled: &'a mut bool,
    shared: &'a mut bool,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
    let proxy: AccessibleProxy<'_> = match node_ref.as_accessible_proxy(conn).await {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };

    let role = proxy.get_role().await.unwrap_or(Role::Invalid);
    let name = proxy.name().await.unwrap_or_default();

    // Toggle the "Allow Remote Interaction" — could be CheckBox, ToggleButton, or Switch.
    // GTK4/libadwaita may report 0 actions, so try do_action(0) regardless.
    if (role == Role::CheckBox || role == Role::ToggleButton)
        && name.contains("Allow") && !*toggled
    {
        let action = atspi::proxy::action::ActionProxy::builder(conn)
            .destination(proxy.inner().destination().to_owned())?
            .path(proxy.inner().path().to_owned())?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        if action.do_action(0).await.is_ok() {
            tracing::debug!("Toggled: {}", name);
            *toggled = true;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    // Click the "Share" or "Allow" button (depending on GNOME version).
    // We click even if the toggle didn't work (user might have already toggled it).
    if role == Role::Button && (name == "Share" || name == "Allow") && !*shared {
        let action = atspi::proxy::action::ActionProxy::builder(conn)
            .destination(proxy.inner().destination().to_owned())?
            .path(proxy.inner().path().to_owned())?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        if action.do_action(0).await.is_ok() {
            tracing::debug!("Clicked: {}", name);
            *shared = true;
            return Ok(());
        }
    }

    // Recurse into children
    let children = proxy.get_children().await.unwrap_or_default();
    for child_ref in &children {
        walk_and_approve(conn, child_ref, toggled, shared).await?;
        if *shared {
            return Ok(());
        }
    }

    Ok(())
    }) // Box::pin
}
