use std::net::SocketAddr;

use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::net::TcpListener;
use tracing::info;

use crate::models::api::TargetApp;
use crate::server::routes;

#[derive(Debug, Clone)]
pub struct AppState {
    pub target_app: Option<TargetApp>,
}

/// Build the full router with all endpoints.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Index — endpoint listing
        .route("/", get(handle_index))
        // Health check
        .route("/health", get(handle_health))
        // Status
        .route("/status", get(routes::status::handle))
        // Screenshot
        .route("/screenshot", get(routes::screenshot::handle))
        // Input
        .route("/click", post(routes::click::handle))
        .route("/click/element", post(routes::click::handle_element))
        .route("/type", post(routes::type_text::handle))
        .route("/key", post(routes::key::handle))
        .route("/scroll", post(routes::scroll::handle))
        // Wait
        .route("/wait", post(routes::wait::handle))
        // Accessibility
        .route("/accessibility/tree", get(routes::accessibility::handle_tree))
        .route(
            "/accessibility/element",
            get(routes::accessibility::handle_element),
        )
        .route(
            "/accessibility/elements",
            get(routes::accessibility::handle_find_elements),
        )
        .route(
            "/accessibility/focused",
            get(routes::accessibility::handle_focused),
        )
        .route(
            "/accessibility/action",
            post(routes::accessibility::handle_action),
        )
        // Menu
        .route("/menu", get(routes::menu::handle_get))
        .route("/menu", post(routes::menu::handle_trigger))
        // Quit
        .route("/quit", post(handle_quit))
        .with_state(state)
}

/// Start the HTTP server on the given port.
pub async fn start_server(port: u16, target_app: Option<TargetApp>) -> anyhow::Result<()> {
    let state = AppState { target_app };
    let app = build_router(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    info!("Geisterhand server listening on http://{}", addr);

    // Print startup JSON for tooling integration
    let startup_info = serde_json::json!({
        "port": port,
        "host": "127.0.0.1",
        "version": env!("CARGO_PKG_VERSION"),
    });
    println!("{}", serde_json::to_string(&startup_info)?);

    axum::serve(listener, app).await?;
    Ok(())
}

/// Find an available port starting from the preferred one.
pub async fn find_available_port(preferred: u16) -> anyhow::Result<u16> {
    for port in preferred..preferred + 100 {
        if TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
            .await
            .is_ok()
        {
            return Ok(port);
        }
    }
    anyhow::bail!("No available port found in range {}..{}", preferred, preferred + 100)
}

// -- Inline handlers --

async fn handle_index() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "geisterhand",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": "linux",
        "endpoints": [
            {"method": "GET",  "path": "/status",               "description": "Server status and permissions"},
            {"method": "GET",  "path": "/health",               "description": "Health check"},
            {"method": "GET",  "path": "/screenshot",           "description": "Take a screenshot"},
            {"method": "POST", "path": "/click",                "description": "Click at coordinates"},
            {"method": "POST", "path": "/click/element",        "description": "Click a UI element"},
            {"method": "POST", "path": "/type",                 "description": "Type text"},
            {"method": "POST", "path": "/key",                  "description": "Press a key combination"},
            {"method": "POST", "path": "/scroll",               "description": "Scroll"},
            {"method": "POST", "path": "/wait",                 "description": "Wait for a condition"},
            {"method": "GET",  "path": "/accessibility/tree",   "description": "Get UI element tree"},
            {"method": "GET",  "path": "/accessibility/element","description": "Get a specific element"},
            {"method": "GET",  "path": "/accessibility/elements","description": "Find elements by query"},
            {"method": "GET",  "path": "/accessibility/focused","description": "Get focused element"},
            {"method": "POST", "path": "/accessibility/action", "description": "Perform action on element"},
            {"method": "GET",  "path": "/menu",                 "description": "Get menu items"},
            {"method": "POST", "path": "/menu",                 "description": "Trigger a menu item"},
            {"method": "POST", "path": "/quit",                 "description": "Quit the server"},
        ]
    }))
}

async fn handle_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn handle_quit() -> Json<serde_json::Value> {
    // Schedule exit after a short delay to allow response to be sent
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        std::process::exit(0);
    });
    Json(serde_json::json!({"status": "quitting"}))
}
