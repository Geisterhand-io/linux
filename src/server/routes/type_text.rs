use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use tracing::info;

use crate::accessibility::service as a11y;
use crate::input;
use crate::models::accessibility::{AccessibilityAction, ElementQuery};
use crate::models::api::*;
use crate::server::http::AppState;

pub async fn handle(
    State(state): State<AppState>,
    Json(body): Json<TypeRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match handle_type_inner(body, state).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}

async fn handle_type_inner(body: TypeRequest, state: AppState) -> anyhow::Result<TypeResponse> {
    let has_element_target = body.path.is_some()
        || body.role.is_some()
        || body.title.is_some()
        || body.title_contains.is_some()
        || body.placeholder_contains.is_some();

    // mode=keys in run mode falls through to setValue (avoids focus stealing)
    let mode_is_keys = body.mode.as_deref() == Some("keys");
    let in_run_mode = state.target_app.is_some();

    if mode_is_keys && !in_run_mode {
        // Character-by-character keyboard typing (standalone server mode only)
        let backend = input::get_backend().await?;
        backend.type_text(&body.text, body.delay_ms)?;
        info!("Typed {} characters via keyboard (keys mode)", body.text.len());
        return Ok(TypeResponse {
            success: true,
            characters_typed: Some(body.text.len()),
            error: None,
        });
    }

    if has_element_target {
        return type_via_accessibility(&body).await;
    }

    if in_run_mode {
        // In geisterhand-run mode without explicit targeting:
        // Use setValue on the AT-SPI2-focused element (non-disruptive, reliable).
        let pid = state.target_app.as_ref().and_then(|t| t.pid).unwrap_or(0);
        let result = a11y::set_value_on_focused_element(pid, &body.text).await;
        if result.success {
            return Ok(TypeResponse {
                success: true,
                characters_typed: Some(body.text.len()),
                error: None,
            });
        } else {
            return Ok(TypeResponse {
                success: false,
                characters_typed: None,
                error: result.error,
            });
        }
    }

    // Standard keyboard injection (standalone server, no element target)
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
        placeholder_contains: body.placeholder_contains.clone(),
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
