use axum::extract::Query;
use axum::http::StatusCode;
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::models::api::ErrorResponse;
use crate::screen;

#[derive(Debug, Deserialize)]
pub struct ScreenshotParams {
    pub format: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScreenshotResponse {
    pub success: bool,
    pub image: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
}

pub async fn handle(
    Query(params): Query<ScreenshotParams>,
) -> (StatusCode, Json<serde_json::Value>) {
    let format = screen::ImageFormat::parse(
        params.format.as_deref().unwrap_or("png"),
    );

    match screen::capture_screen(format).await {
        Ok(shot) => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&shot.data);
            info!(
                "Screenshot captured: {}x{} ({} bytes)",
                shot.width,
                shot.height,
                shot.data.len()
            );

            let resp = ScreenshotResponse {
                success: true,
                image: b64,
                format: match shot.format {
                    screen::ImageFormat::Png => "png".to_string(),
                    screen::ImageFormat::Jpeg => "jpeg".to_string(),
                },
                width: shot.width,
                height: shot.height,
            };
            (
                StatusCode::OK,
                Json(serde_json::to_value(resp).unwrap()),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}
