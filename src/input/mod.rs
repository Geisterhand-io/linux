pub mod backend;
pub mod keycode_map;
pub mod portal;
pub mod uinput;
pub mod xtest;

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::OnceCell;
use tracing::{info, warn};

use crate::platform::display::{detect_display_server, DisplayServer};
use backend::InputBackend;

static INPUT_BACKEND: OnceCell<Arc<dyn InputBackend>> = OnceCell::const_new();

/// Get or initialize the input backend.
/// Selection order:
/// - X11: XTest (no special permissions)
/// - Wayland: Portal (one-time permission dialog) > uinput (needs /dev/uinput access)
/// - Fallback: uinput > XTest
pub async fn get_backend() -> Result<Arc<dyn InputBackend>> {
    let backend = INPUT_BACKEND
        .get_or_try_init(|| async { init_backend().await })
        .await?;
    Ok(backend.clone())
}

async fn init_backend() -> Result<Arc<dyn InputBackend>> {
    let display = detect_display_server();

    // On X11, prefer XTest (no special permissions needed)
    if display == DisplayServer::X11 {
        match xtest::XTestBackend::new() {
            Ok(backend) => {
                info!("Using XTest input backend");
                return Ok(Arc::new(backend));
            }
            Err(e) => {
                warn!("XTest backend not available: {}", e);
            }
        }
    }

    // On Wayland, try portal first (one-time permission dialog, no root needed)
    if display == DisplayServer::Wayland {
        match portal::PortalBackend::new().await {
            Ok(backend) => {
                info!("Using portal input backend (XDG RemoteDesktop)");
                return Ok(Arc::new(backend));
            }
            Err(e) => {
                warn!("Portal backend not available: {}", e);
            }
        }
    }

    // Fallback: uinput (works on both X11 and Wayland, needs /dev/uinput access)
    match uinput::UinputBackend::new() {
        Ok(backend) => {
            info!("Using uinput input backend");
            return Ok(Arc::new(backend));
        }
        Err(e) => {
            warn!("uinput backend not available: {}", e);
        }
    }

    // On Wayland, try XTest as last resort (may work under XWayland)
    // On X11, XTest was already tried first
    if display != DisplayServer::X11 {
        match xtest::XTestBackend::new() {
            Ok(backend) => {
                info!("Using XTest input backend (fallback)");
                return Ok(Arc::new(backend));
            }
            Err(e) => {
                warn!("XTest backend not available (fallback): {}", e);
            }
        }
    }

    anyhow::bail!(
        "No input backend available. On Wayland, the portal backend requires \
         xdg-desktop-portal with RemoteDesktop support; alternatively ensure \
         /dev/uinput is accessible (add user to 'input' group). On X11, \
         XTest extension should be available."
    )
}
