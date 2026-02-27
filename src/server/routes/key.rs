use axum::http::StatusCode;
use axum::Json;
use tracing::info;

use crate::input;
use crate::input::keycode_map;
use crate::models::api::*;

pub async fn handle(Json(body): Json<KeyRequest>) -> (StatusCode, Json<serde_json::Value>) {
    match handle_key_inner(body).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(ErrorResponse::new(e.to_string())).unwrap()),
        ),
    }
}

async fn handle_key_inner(body: KeyRequest) -> anyhow::Result<KeyResponse> {
    let backend = input::get_backend().await?;

    // Resolve the key name to an evdev keycode
    let key = keycode_map::key_name_to_code(&body.key)
        .ok_or_else(|| anyhow::anyhow!("Unknown key: '{}'", body.key))?;

    // Hold modifiers
    let modifier_codes: Vec<u16> = body
        .modifiers
        .as_ref()
        .map(|mods| mods.iter().map(|m| keycode_map::modifier_to_code(m).code()).collect())
        .unwrap_or_default();

    for &code in &modifier_codes {
        backend.key_down(code)?;
    }

    // Press the key
    backend.key_press(key.code())?;

    // Release modifiers (reverse order)
    for &code in modifier_codes.iter().rev() {
        backend.key_up(code)?;
    }

    let mod_names: Option<Vec<String>> = body.modifiers.as_ref().map(|mods| {
        mods.iter().map(|m| format!("{:?}", m).to_lowercase()).collect()
    });

    info!("Key press: {} (modifiers: {:?})", body.key, mod_names);

    Ok(KeyResponse {
        success: true,
        key: Some(body.key),
        modifiers: mod_names,
        error: None,
    })
}
