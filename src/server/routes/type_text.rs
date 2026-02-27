use axum::http::StatusCode;
use axum::Json;
use tracing::info;

use crate::accessibility::service as a11y;
use crate::input;
use crate::models::accessibility::{AccessibilityAction, ElementQuery};
use crate::models::api::*;

pub async fn handle(Json(body): Json<TypeRequest>) -> (StatusCode, Json<serde_json::Value>) {
    match handle_type_inner(body).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}

async fn handle_type_inner(body: TypeRequest) -> anyhow::Result<TypeResponse> {
    // If mode is "accessibility" or a path is given, use AT-SPI2 SetValue
    let use_a11y = body.mode.as_deref() == Some("accessibility")
        || body.path.is_some()
        || body.role.is_some()
        || body.title.is_some()
        || body.title_contains.is_some();

    if use_a11y {
        return type_via_accessibility(&body).await;
    }

    // Otherwise, use keyboard input injection
    let backend = input::get_backend().await?;
    backend.type_text(&body.text, body.delay_ms)?;

    info!("Typed {} characters via keyboard", body.text.len());

    Ok(TypeResponse {
        success: true,
        characters_typed: Some(body.text.len()),
        error: None,
    })
}

async fn type_via_accessibility(body: &TypeRequest) -> anyhow::Result<TypeResponse> {
    // If a path is provided, use it directly
    if let Some(path) = &body.path {
        let resp = a11y::perform_action(
            path.clone(),
            AccessibilityAction::SetValue,
            Some(body.text.clone()),
        )
        .await;

        return Ok(TypeResponse {
            success: resp.success,
            characters_typed: if resp.success { Some(body.text.len()) } else { None },
            error: resp.error,
        });
    }

    // Otherwise, search for the element
    let query = ElementQuery {
        role: body.role.clone(),
        title: body.title.clone(),
        title_contains: body.title_contains.clone(),
        max_results: Some(1),
        ..Default::default()
    };

    let result = a11y::find_elements(body.pid, query).await;
    if !result.success {
        return Ok(TypeResponse {
            success: false,
            characters_typed: None,
            error: result.error,
        });
    }

    let elements = result.elements.unwrap_or_default();
    let element = elements.first().ok_or_else(|| {
        anyhow::anyhow!("No matching element found for typing")
    })?;

    let resp = a11y::perform_action(
        element.path.clone(),
        AccessibilityAction::SetValue,
        Some(body.text.clone()),
    )
    .await;

    Ok(TypeResponse {
        success: resp.success,
        characters_typed: if resp.success { Some(body.text.len()) } else { None },
        error: resp.error,
    })
}
