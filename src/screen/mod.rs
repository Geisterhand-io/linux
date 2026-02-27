pub mod portal;
pub mod x11;

use anyhow::Result;
use tracing::{info, warn};

use crate::platform::display::{detect_display_server, DisplayServer};

/// Screenshot output format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

impl ImageFormat {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "jpeg" | "jpg" => ImageFormat::Jpeg,
            _ => ImageFormat::Png,
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
        }
    }
}

/// Captured screenshot data.
pub struct Screenshot {
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

/// Take a screenshot and return base64-encoded image data.
pub async fn capture_screen(format: ImageFormat) -> Result<Screenshot> {
    let display = detect_display_server();

    // On X11, use direct X11 capture
    if display == DisplayServer::X11 {
        match x11::capture_x11(format) {
            Ok(shot) => {
                info!("Screenshot captured via X11 ({}x{})", shot.width, shot.height);
                return Ok(shot);
            }
            Err(e) => {
                warn!("X11 capture failed: {}", e);
            }
        }
    }

    // On Wayland or X11 fallback, use xdg-desktop-portal
    match portal::capture_portal(format).await {
        Ok(shot) => {
            info!("Screenshot captured via portal ({}x{})", shot.width, shot.height);
            return Ok(shot);
        }
        Err(e) => {
            warn!("Portal capture failed: {:?}", e);
        }
    }

    // Final fallback: try X11 even if not detected (e.g. XWayland)
    if display != DisplayServer::X11 {
        match x11::capture_x11(format) {
            Ok(shot) => {
                info!("Screenshot captured via X11 fallback ({}x{})", shot.width, shot.height);
                return Ok(shot);
            }
            Err(e) => {
                warn!("X11 fallback capture failed: {:?}", e);
            }
        }
    }

    anyhow::bail!(
        "No screenshot backend available. On Wayland, xdg-desktop-portal must be running. \
         On X11, the display must be accessible."
    )
}
