use axum::http::StatusCode;
use axum::Json;
use tracing::info;

use crate::input;
use crate::models::api::*;

pub async fn handle(Json(body): Json<ScrollRequest>) -> (StatusCode, Json<serde_json::Value>) {
    match handle_scroll_inner(body).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}

async fn handle_scroll_inner(body: ScrollRequest) -> anyhow::Result<ScrollResponse> {
    let backend = input::get_backend().await?;

    // If x/y are provided, move mouse there first
    if let (Some(x), Some(y)) = (body.x, body.y) {
        backend.mouse_move(x, y)?;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let delta_x = body.delta_x.unwrap_or(0.0);
    let delta_y = body.delta_y.unwrap_or(0.0);

    backend.scroll(delta_x, delta_y)?;

    info!("Scrolled dx={}, dy={}", delta_x, delta_y);

    Ok(ScrollResponse {
        success: true,
        x: body.x,
        y: body.y,
        delta_x: Some(delta_x),
        delta_y: Some(delta_y),
        error: None,
    })
}
