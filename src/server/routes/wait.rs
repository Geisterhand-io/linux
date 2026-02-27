use axum::http::StatusCode;
use axum::Json;
use tracing::info;

use crate::accessibility::service as a11y;
use crate::models::accessibility::ElementQuery;
use crate::models::api::*;

pub async fn handle(Json(body): Json<WaitRequest>) -> (StatusCode, Json<serde_json::Value>) {
    match handle_wait_inner(body).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}

async fn handle_wait_inner(body: WaitRequest) -> anyhow::Result<WaitResponse> {
    // Validate that at least one search criterion is provided
    if body.role.is_none()
        && body.title.is_none()
        && body.title_contains.is_none()
        && body.label_contains.is_none()
        && body.value_contains.is_none()
        && body.path.is_none()
    {
        return Ok(WaitResponse {
            success: false,
            condition_met: false,
            time_elapsed_ms: Some(0),
            error: Some("At least one search criterion is required".to_string()),
        });
    }

    // Clamp timeout and poll interval
    let timeout_ms = body.timeout_ms.clamp(1, 60000);
    let poll_interval_ms = body.poll_interval_ms.clamp(1, 5000);

    let query = ElementQuery {
        role: body.role.clone(),
        title: body.title.clone(),
        title_contains: body.title_contains.clone(),
        label_contains: body.label_contains.clone(),
        value_contains: body.value_contains.clone(),
        max_results: Some(1),
    };

    let start = std::time::Instant::now();

    loop {
        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Check condition
        let result = a11y::find_elements(body.pid, query.clone()).await;
        let found = result.success && result.elements.as_ref().is_some_and(|e| !e.is_empty());

        let condition_met = match body.condition {
            WaitCondition::Exists => found,
            WaitCondition::NotExists => !found,
            WaitCondition::Enabled => {
                if let Some(elements) = &result.elements {
                    elements
                        .first()
                        .and_then(|e| e.is_enabled)
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            WaitCondition::Focused => {
                if let Some(elements) = &result.elements {
                    elements
                        .first()
                        .and_then(|e| e.is_focused)
                        .unwrap_or(false)
                } else {
                    false
                }
            }
        };

        if condition_met {
            info!(
                "Wait condition {:?} met after {}ms",
                body.condition, elapsed_ms
            );
            return Ok(WaitResponse {
                success: true,
                condition_met: true,
                time_elapsed_ms: Some(elapsed_ms),
                error: None,
            });
        }

        if elapsed_ms >= timeout_ms {
            return Ok(WaitResponse {
                success: true,
                condition_met: false,
                time_elapsed_ms: Some(elapsed_ms),
                error: Some(format!(
                    "Timeout: condition {:?} not met within {}ms",
                    body.condition, timeout_ms
                )),
            });
        }

        tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)).await;
    }
}
