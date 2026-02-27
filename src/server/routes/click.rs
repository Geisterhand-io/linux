use axum::http::StatusCode;
use axum::Json;
use tracing::info;

use crate::accessibility::service as a11y;
use crate::input;
use crate::input::keycode_map;
use crate::models::accessibility::{AccessibilityAction, ElementQuery};
use crate::models::api::*;

pub async fn handle(Json(body): Json<ClickRequest>) -> (StatusCode, Json<serde_json::Value>) {
    match handle_click_inner(body).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}

async fn handle_click_inner(body: ClickRequest) -> anyhow::Result<ClickResponse> {
    let backend = input::get_backend().await?;

    // Move mouse to position
    backend.mouse_move(body.x, body.y)?;
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Hold modifiers if specified
    let modifier_codes: Vec<u16> = body
        .modifiers
        .as_ref()
        .map(|mods| mods.iter().map(|m| keycode_map::modifier_to_code(m).code()).collect())
        .unwrap_or_default();

    for &code in &modifier_codes {
        backend.key_down(code)?;
    }

    // Click
    let count = body.click_count.unwrap_or(1);
    backend.mouse_click(body.button.clone(), count)?;

    // Release modifiers
    for &code in modifier_codes.iter().rev() {
        backend.key_up(code)?;
    }

    info!("Clicked at ({}, {})", body.x, body.y);

    Ok(ClickResponse {
        success: true,
        x: Some(body.x),
        y: Some(body.y),
        button: Some(format!("{:?}", body.button).to_lowercase()),
        error: None,
    })
}

pub async fn handle_element(
    Json(body): Json<ElementClickRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match handle_element_click_inner(body).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}

async fn handle_element_click_inner(body: ElementClickRequest) -> anyhow::Result<ElementClickResponse> {
    // If use_accessibility_action is true, use AT-SPI2 to press the element
    if body.use_accessibility_action.unwrap_or(false) {
        return element_click_via_a11y(&body).await;
    }

    // Otherwise, find the element by searching, get its frame, and click at center
    let query = ElementQuery {
        role: body.role.clone(),
        title: body.title.clone(),
        title_contains: body.title_contains.clone(),
        label_contains: body.label.clone(),
        max_results: Some(1),
        ..Default::default()
    };

    let result = a11y::find_elements(body.pid, query).await;
    if !result.success {
        return Ok(ElementClickResponse {
            success: false,
            element: None,
            error: result.error,
        });
    }

    let elements = result.elements.unwrap_or_default();
    let element = elements.first().ok_or_else(|| {
        anyhow::anyhow!("No matching element found")
    })?;

    let frame = element.frame.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Element has no frame/position — use use_accessibility_action instead")
    })?;

    let (cx, cy) = frame.center();

    // Click at the element's center
    let backend = input::get_backend().await?;
    backend.mouse_move(cx, cy)?;
    std::thread::sleep(std::time::Duration::from_millis(10));

    let button = body.button.clone().unwrap_or_default();
    backend.mouse_click(button, 1)?;

    info!("Clicked element '{}' at ({}, {})", element.role, cx, cy);

    Ok(ElementClickResponse {
        success: true,
        element: Some(ClickedElementInfo {
            role: element.role.clone(),
            title: element.title.clone(),
            frame: Some(ElementFrameInfo {
                x: frame.x,
                y: frame.y,
                width: frame.width,
                height: frame.height,
            }),
            coordinates: Some(ClickCoordinates { x: cx, y: cy }),
        }),
        error: None,
    })
}

async fn element_click_via_a11y(body: &ElementClickRequest) -> anyhow::Result<ElementClickResponse> {
    let query = ElementQuery {
        role: body.role.clone(),
        title: body.title.clone(),
        title_contains: body.title_contains.clone(),
        label_contains: body.label.clone(),
        max_results: Some(1),
        ..Default::default()
    };

    let result = a11y::find_elements(body.pid, query).await;
    if !result.success {
        return Ok(ElementClickResponse {
            success: false,
            element: None,
            error: result.error,
        });
    }

    let elements = result.elements.unwrap_or_default();
    let element = elements.first().ok_or_else(|| {
        anyhow::anyhow!("No matching element found")
    })?;

    // Use accessibility action to press the element
    let action_resp = a11y::perform_action(
        element.path.clone(),
        AccessibilityAction::Press,
        None,
    )
    .await;

    if !action_resp.success {
        return Ok(ElementClickResponse {
            success: false,
            element: None,
            error: action_resp.error,
        });
    }

    info!("Accessibility-clicked element '{}' ({})", element.role, element.title.as_deref().unwrap_or(""));

    Ok(ElementClickResponse {
        success: true,
        element: Some(ClickedElementInfo {
            role: element.role.clone(),
            title: element.title.clone(),
            frame: element.frame.as_ref().map(|f| ElementFrameInfo {
                x: f.x,
                y: f.y,
                width: f.width,
                height: f.height,
            }),
            coordinates: element.frame.as_ref().map(|f| {
                let (cx, cy) = f.center();
                ClickCoordinates { x: cx, y: cy }
            }),
        }),
        error: None,
    })
}
