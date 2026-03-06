use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use tracing::info;

use crate::accessibility::service as a11y;
use crate::models::accessibility::{AccessibilityAction, ElementQuery};
use crate::models::api::*;
use crate::server::http::AppState;

pub async fn handle_get(
    Query(params): Query<MenuGetRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match handle_get_inner(params).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}

async fn handle_get_inner(params: MenuGetRequest) -> anyhow::Result<MenuResponse> {
    // Find menu bar elements for the given app
    let query = ElementQuery {
        role: Some("menu bar".to_string()),
        max_results: Some(1),
        ..Default::default()
    };

    let result = a11y::find_elements(params.pid, query).await;
    if !result.success {
        return Ok(MenuResponse {
            success: false,
            menu_items: None,
            error: result.error,
        });
    }

    let elements = result.elements.unwrap_or_default();
    if elements.is_empty() {
        return Ok(MenuResponse {
            success: true,
            menu_items: Some(vec![]),
            error: None,
        });
    }

    // Get the menu bar's children (top-level menus)
    let menu_bar = &elements[0];
    let mut menu_items = Vec::new();

    if let Some(children) = &menu_bar.children {
        for child in children {
            let item = build_menu_item(child);
            menu_items.push(item);
        }
    }

    Ok(MenuResponse {
        success: true,
        menu_items: Some(menu_items),
        error: None,
    })
}

fn build_menu_item(element: &crate::models::accessibility::UIElementInfo) -> MenuItemInfo {
    let title = element
        .title
        .clone()
        .unwrap_or_else(|| element.label.clone().unwrap_or_default());

    let children = element.children.as_ref().map(|kids| {
        kids.iter()
            .filter(|c| {
                // Skip separators
                c.role != "separator"
                    && c.title
                        .as_ref()
                        .is_none_or(|t| !t.is_empty())
            })
            .map(build_menu_item)
            .collect()
    });

    let has_children = children
        .as_ref()
        .is_some_and(|c: &Vec<MenuItemInfo>| !c.is_empty());

    MenuItemInfo {
        title,
        enabled: element.is_enabled,
        children: if has_children { children } else { None },
        shortcut: None, // AT-SPI2 doesn't expose shortcuts the same way
    }
}

pub async fn handle_trigger(
    State(state): State<AppState>,
    Json(body): Json<MenuTriggerRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match handle_trigger_inner(body, state).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}

async fn handle_trigger_inner(body: MenuTriggerRequest, state: AppState) -> anyhow::Result<MenuResponse> {
    // In run mode, always trigger menus in the background to avoid activating the app
    let _background = body.background.unwrap_or(state.target_app.is_some());
    if body.path.is_empty() {
        return Ok(MenuResponse {
            success: false,
            menu_items: None,
            error: Some("Menu path is required".to_string()),
        });
    }

    // Search for menu items by walking the menu path
    // First find the menu bar
    let query = ElementQuery {
        role: Some("menu bar".to_string()),
        max_results: Some(1),
        ..Default::default()
    };

    let result = a11y::find_elements(body.pid, query).await;
    if !result.success || result.elements.as_ref().is_none_or(|e| e.is_empty()) {
        return Ok(MenuResponse {
            success: false,
            menu_items: None,
            error: Some("No menu bar found".to_string()),
        });
    }

    let elements = result.elements.unwrap();
    let menu_bar = &elements[0];

    // Navigate the menu path by searching for matching titles at each level
    // The path is like ["File", "New Window"] - we need to find and click each level
    let mut current_path = menu_bar.path.clone();

    for (i, menu_name) in body.path.iter().enumerate() {
        // Search for the menu item at the current level
        let query = ElementQuery {
            title: Some(menu_name.clone()),
            max_results: Some(5),
            ..Default::default()
        };

        let result = a11y::find_elements(body.pid, query).await;
        if !result.success || result.elements.as_ref().is_none_or(|e| e.is_empty()) {
            return Ok(MenuResponse {
                success: false,
                menu_items: None,
                error: Some(format!("Menu item '{}' not found", menu_name)),
            });
        }

        let found_elements = result.elements.unwrap();

        // Find the element whose path starts with current_path
        let matching = found_elements.iter().find(|e| {
            e.path.pid == current_path.pid
                && e.path.path.len() > current_path.path.len()
                && e.path.path.starts_with(&current_path.path)
        });

        if let Some(element) = matching {
            if i == body.path.len() - 1 {
                // Last item in path — click it
                let action_resp = a11y::perform_action(
                    element.path.clone(),
                    AccessibilityAction::Press,
                    None,
                )
                .await;

                info!("Menu trigger: {:?} -> {}", body.path, action_resp.success);

                return Ok(MenuResponse {
                    success: action_resp.success,
                    menu_items: None,
                    error: action_resp.error,
                });
            } else {
                // Intermediate menu — click to open, then continue
                let _ = a11y::perform_action(
                    element.path.clone(),
                    AccessibilityAction::Press,
                    None,
                )
                .await;

                current_path = element.path.clone();
                // Small delay for menu to open
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        } else {
            return Ok(MenuResponse {
                success: false,
                menu_items: None,
                error: Some(format!(
                    "Menu item '{}' not found under current menu path",
                    menu_name
                )),
            });
        }
    }

    Ok(MenuResponse {
        success: false,
        menu_items: None,
        error: Some("Empty menu path".to_string()),
    })
}
