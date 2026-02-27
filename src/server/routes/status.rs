use axum::extract::State;
use axum::Json;

use crate::accessibility::service as a11y;
use crate::input;
use crate::models::api::{PermissionStatus, ScreenSize, StatusResponse};
use crate::platform::{display, permissions, process};
use crate::server::http::AppState;

pub async fn handle(State(state): State<AppState>) -> Json<StatusResponse> {
    let display_server = display::detect_display_server();

    let at_spi2_available = permissions::check_at_spi2_available();
    let uinput_access = permissions::check_uinput_access();
    let screen_capture_available = permissions::check_screen_capture_available();

    // Determine active input backend
    let input_backend = match input::get_backend().await {
        Ok(b) => Some(b.name().to_string()),
        Err(_) => None,
    };

    // Build actionable hints
    let mut hints = Vec::new();
    if !at_spi2_available {
        hints.push(
            "AT-SPI2 not available. Enable accessibility: gsettings set org.gnome.desktop.interface toolkit-accessibility true"
                .to_string(),
        );
    }
    if !uinput_access {
        hints.push(
            "uinput not accessible. For Wayland input: sudo usermod -aG input $USER (then re-login). XTest fallback works on X11/XWayland."
                .to_string(),
        );
    }
    if !screen_capture_available {
        hints.push(
            "Screen capture may be limited. Ensure xdg-desktop-portal-gnome is installed for Wayland screenshots."
                .to_string(),
        );
    }

    let perms = PermissionStatus {
        at_spi2_available,
        uinput_access,
        screen_capture_available,
        input_backend,
        hints: if hints.is_empty() { None } else { Some(hints) },
    };

    let screen_size = display::get_screen_size().map(|(w, h)| ScreenSize {
        width: w,
        height: h,
    });

    // Try AT-SPI2 first for frontmost app, fallback to xdotool-based detection
    let frontmost_app = match a11y::get_frontmost_app().await {
        Ok(Some(app)) => Some(app),
        _ => process::get_frontmost_app(),
    };

    Json(StatusResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        permissions: perms,
        frontmost_app,
        screen_size,
        target_app: state.target_app,
        display_server: Some(display_server.to_string()),
    })
}
