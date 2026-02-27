use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

use crate::accessibility::service as a11y;
use crate::models::accessibility::{AccessibilityAction, ElementQuery};
use crate::models::api::*;

// JSON-RPC types

#[derive(Deserialize)]
#[allow(dead_code)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

// MCP Tool definitions

fn tool_definitions() -> Value {
    serde_json::json!([
        {
            "name": "screenshot",
            "description": "Take a screenshot of the screen. Returns base64-encoded image data.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "format": {
                        "type": "string",
                        "enum": ["png", "jpeg"],
                        "description": "Image format (default: png)"
                    }
                }
            }
        },
        {
            "name": "click",
            "description": "Click at screen coordinates.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "x": {"type": "number", "description": "X coordinate"},
                    "y": {"type": "number", "description": "Y coordinate"},
                    "button": {"type": "string", "enum": ["left", "right", "center"], "description": "Mouse button (default: left)"},
                    "click_count": {"type": "integer", "description": "Number of clicks (default: 1)"}
                },
                "required": ["x", "y"]
            }
        },
        {
            "name": "click_element",
            "description": "Click a UI element found by title, role, or label. Uses accessibility to find and click the element.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": {"type": "string", "description": "Element title (exact match)"},
                    "title_contains": {"type": "string", "description": "Substring of element title"},
                    "role": {"type": "string", "description": "Accessibility role (e.g. 'button', 'text', 'menu item')"},
                    "pid": {"type": "integer", "description": "Target application PID"},
                    "use_accessibility_action": {"type": "boolean", "description": "Use AT-SPI2 Press action instead of mouse click (default: true)"}
                }
            }
        },
        {
            "name": "type_text",
            "description": "Type text using keyboard input injection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text to type"},
                    "delay_ms": {"type": "integer", "description": "Delay between keystrokes in ms"}
                },
                "required": ["text"]
            }
        },
        {
            "name": "key_press",
            "description": "Press a key combination. Key names: return, escape, tab, space, backspace, delete, up, down, left, right, home, end, page_up, page_down, f1-f12, a-z, 0-9.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Key name"},
                    "modifiers": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["ctrl", "alt", "shift", "super"]},
                        "description": "Modifier keys to hold"
                    }
                },
                "required": ["key"]
            }
        },
        {
            "name": "scroll",
            "description": "Scroll at the current or specified position.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "delta_x": {"type": "number", "description": "Horizontal scroll amount"},
                    "delta_y": {"type": "number", "description": "Vertical scroll amount (positive=down, negative=up)"},
                    "x": {"type": "number", "description": "X coordinate to scroll at"},
                    "y": {"type": "number", "description": "Y coordinate to scroll at"}
                }
            }
        },
        {
            "name": "get_tree",
            "description": "Get the accessibility tree of an application. Returns UI elements with roles, titles, and paths.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": {"type": "integer", "description": "Target application PID (uses frontmost app if omitted)"},
                    "max_depth": {"type": "integer", "description": "Maximum tree depth (default: 10)"}
                }
            }
        },
        {
            "name": "find_elements",
            "description": "Search for UI elements matching criteria in an application's accessibility tree.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": {"type": "integer", "description": "Target application PID"},
                    "role": {"type": "string", "description": "Accessibility role to match"},
                    "title": {"type": "string", "description": "Exact title to match"},
                    "title_contains": {"type": "string", "description": "Substring to match in title"},
                    "max_results": {"type": "integer", "description": "Maximum results (default: 10)"}
                }
            }
        },
        {
            "name": "get_focused",
            "description": "Get the currently focused UI element.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": {"type": "integer", "description": "Target application PID"}
                }
            }
        },
        {
            "name": "perform_action",
            "description": "Perform an accessibility action on a UI element identified by path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": {"type": "integer", "description": "Application PID"},
                    "path": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "Element path (array of child indices from app root)"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["press", "setValue", "focus", "confirm", "cancel", "increment", "decrement", "showMenu", "pick"],
                        "description": "Action to perform"
                    },
                    "value": {"type": "string", "description": "Value for setValue action"}
                },
                "required": ["pid", "path", "action"]
            }
        },
        {
            "name": "wait",
            "description": "Wait for a UI element to appear, disappear, or reach a state.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": {"type": "integer", "description": "Target application PID"},
                    "role": {"type": "string", "description": "Element role to match"},
                    "title": {"type": "string", "description": "Element title to match"},
                    "title_contains": {"type": "string", "description": "Substring in title"},
                    "condition": {
                        "type": "string",
                        "enum": ["exists", "not_exists", "enabled", "focused"],
                        "description": "Condition to wait for (default: exists)"
                    },
                    "timeout_ms": {"type": "integer", "description": "Timeout in ms (default: 5000)"}
                }
            }
        },
        {
            "name": "status",
            "description": "Get the current system status including permissions, display server, and frontmost app.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }
    ])
}

/// Run the MCP server, reading JSON-RPC from stdin, writing to stdout.
pub async fn run_mcp_server() -> anyhow::Result<()> {
    info!("MCP server starting (stdio mode)");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                };
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        };

        let id = request.id.clone().unwrap_or(Value::Null);

        // Handle notifications (no id) — just ignore
        if request.id.is_none() {
            if request.method == "notifications/initialized" {
                // Client acknowledges initialization
                continue;
            }
            continue;
        }

        let response = handle_request(&request).await;
        let resp = match response {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(result),
                error: None,
            },
            Err(e) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(e),
            },
        };

        writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
        stdout.flush()?;
    }

    Ok(())
}

async fn handle_request(req: &JsonRpcRequest) -> Result<Value, JsonRpcError> {
    match req.method.as_str() {
        "initialize" => handle_initialize(),
        "tools/list" => handle_tools_list(),
        "tools/call" => handle_tools_call(&req.params).await,
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("Method not found: {}", req.method),
        }),
    }
}

fn handle_initialize() -> Result<Value, JsonRpcError> {
    Ok(serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "geisterhand",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

fn handle_tools_list() -> Result<Value, JsonRpcError> {
    Ok(serde_json::json!({
        "tools": tool_definitions()
    }))
}

async fn handle_tools_call(params: &Value) -> Result<Value, JsonRpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing tool name".to_string(),
        })?;

    let args = params.get("arguments").cloned().unwrap_or(Value::Object(Default::default()));

    // Screenshot returns image content blocks directly
    if name == "screenshot" {
        return match tool_screenshot(&args).await {
            Ok(content) => Ok(serde_json::json!({ "content": content })),
            Err(e) => Ok(serde_json::json!({
                "content": [{"type": "text", "text": format!("Error: {}", e)}],
                "isError": true
            })),
        };
    }

    let result = match name {
        "click" => tool_click(&args).await,
        "click_element" => tool_click_element(&args).await,
        "type_text" => tool_type_text(&args).await,
        "key_press" => tool_key_press(&args).await,
        "scroll" => tool_scroll(&args).await,
        "get_tree" => tool_get_tree(&args).await,
        "find_elements" => tool_find_elements(&args).await,
        "get_focused" => tool_get_focused(&args).await,
        "perform_action" => tool_perform_action(&args).await,
        "wait" => tool_wait(&args).await,
        "status" => tool_status().await,
        _ => Err(format!("Unknown tool: {}", name)),
    };

    match result {
        Ok(text) => Ok(serde_json::json!({
            "content": [{"type": "text", "text": text}]
        })),
        Err(e) => Ok(serde_json::json!({
            "content": [{"type": "text", "text": format!("Error: {}", e)}],
            "isError": true
        })),
    }
}

// Tool implementations

async fn tool_screenshot(args: &Value) -> Result<Vec<Value>, String> {
    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("png");

    let fmt = crate::screen::ImageFormat::parse(format);
    let shot = crate::screen::capture_screen(fmt)
        .await
        .map_err(|e| e.to_string())?;

    let b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &shot.data,
    );

    let mime_type = match format {
        "jpeg" | "jpg" => "image/jpeg",
        _ => "image/png",
    };

    Ok(vec![
        serde_json::json!({
            "type": "image",
            "data": b64,
            "mimeType": mime_type
        }),
        serde_json::json!({
            "type": "text",
            "text": format!("Screenshot captured: {}x{} {}", shot.width, shot.height, format)
        }),
    ])
}

async fn tool_click(args: &Value) -> Result<String, String> {
    let x = args.get("x").and_then(|v| v.as_f64()).ok_or("Missing x")?;
    let y = args.get("y").and_then(|v| v.as_f64()).ok_or("Missing y")?;
    let button = match args.get("button").and_then(|v| v.as_str()) {
        Some("right") => MouseButton::Right,
        Some("center") => MouseButton::Center,
        _ => MouseButton::Left,
    };
    let count = args.get("click_count").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

    let backend = crate::input::get_backend().await.map_err(|e| e.to_string())?;
    backend.mouse_move(x, y).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(10));
    backend.mouse_click(button, count).map_err(|e| e.to_string())?;

    Ok(format!("Clicked at ({}, {})", x, y))
}

async fn tool_click_element(args: &Value) -> Result<String, String> {
    let query = ElementQuery {
        role: args.get("role").and_then(|v| v.as_str()).map(String::from),
        title: args.get("title").and_then(|v| v.as_str()).map(String::from),
        title_contains: args.get("title_contains").and_then(|v| v.as_str()).map(String::from),
        max_results: Some(1),
        ..Default::default()
    };
    let pid = args.get("pid").and_then(|v| v.as_i64()).map(|p| p as i32);
    let use_a11y = args.get("use_accessibility_action").and_then(|v| v.as_bool()).unwrap_or(true);

    let result = a11y::find_elements(pid, query).await;
    let elements = result.elements.unwrap_or_default();
    let element = elements.first().ok_or("No matching element found")?;

    if use_a11y {
        let resp = a11y::perform_action(element.path.clone(), AccessibilityAction::Press, None).await;
        if resp.success {
            Ok(format!("Clicked element: {} {:?}", element.role, element.title))
        } else {
            Err(resp.error.unwrap_or_else(|| "Action failed".to_string()))
        }
    } else {
        let frame = element.frame.as_ref().ok_or("Element has no frame")?;
        let (cx, cy) = frame.center();
        let backend = crate::input::get_backend().await.map_err(|e| e.to_string())?;
        backend.mouse_move(cx, cy).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(10));
        backend.mouse_click(MouseButton::Left, 1).map_err(|e| e.to_string())?;
        Ok(format!("Clicked element at ({}, {}): {} {:?}", cx, cy, element.role, element.title))
    }
}

async fn tool_type_text(args: &Value) -> Result<String, String> {
    let text = args.get("text").and_then(|v| v.as_str()).ok_or("Missing text")?;
    let delay = args.get("delay_ms").and_then(|v| v.as_u64());

    let backend = crate::input::get_backend().await.map_err(|e| e.to_string())?;
    backend.type_text(text, delay).map_err(|e| e.to_string())?;

    Ok(format!("Typed {} characters", text.len()))
}

async fn tool_key_press(args: &Value) -> Result<String, String> {
    let key_name = args.get("key").and_then(|v| v.as_str()).ok_or("Missing key")?;
    let key = crate::input::keycode_map::key_name_to_code(key_name)
        .ok_or_else(|| format!("Unknown key: '{}'", key_name))?;

    let backend = crate::input::get_backend().await.map_err(|e| e.to_string())?;

    // Handle modifiers
    let modifier_codes: Vec<u16> = args
        .get("modifiers")
        .and_then(|v| v.as_array())
        .map(|mods| {
            mods.iter()
                .filter_map(|m| m.as_str())
                .map(|m| {
                    let km = match m {
                        "ctrl" | "control" => KeyModifier::Ctrl,
                        "alt" => KeyModifier::Alt,
                        "shift" => KeyModifier::Shift,
                        "super" | "cmd" => KeyModifier::Super,
                        _ => KeyModifier::Ctrl,
                    };
                    crate::input::keycode_map::modifier_to_code(&km).code()
                })
                .collect()
        })
        .unwrap_or_default();

    for &code in &modifier_codes {
        backend.key_down(code).map_err(|e| e.to_string())?;
    }
    backend.key_press(key.code()).map_err(|e| e.to_string())?;
    for &code in modifier_codes.iter().rev() {
        backend.key_up(code).map_err(|e| e.to_string())?;
    }

    Ok(format!("Pressed key: {}", key_name))
}

async fn tool_scroll(args: &Value) -> Result<String, String> {
    let backend = crate::input::get_backend().await.map_err(|e| e.to_string())?;

    if let (Some(x), Some(y)) = (
        args.get("x").and_then(|v| v.as_f64()),
        args.get("y").and_then(|v| v.as_f64()),
    ) {
        backend.mouse_move(x, y).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let dx = args.get("delta_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let dy = args.get("delta_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    backend.scroll(dx, dy).map_err(|e| e.to_string())?;

    Ok(format!("Scrolled dx={}, dy={}", dx, dy))
}

async fn tool_get_tree(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_i64()).map(|p| p as i32);
    let max_depth = args.get("max_depth").and_then(|v| v.as_i64()).unwrap_or(10) as i32;

    let result = a11y::get_compact_tree(pid, Some(max_depth), true, None).await;

    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

async fn tool_find_elements(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_i64()).map(|p| p as i32);
    let query = ElementQuery {
        role: args.get("role").and_then(|v| v.as_str()).map(String::from),
        title: args.get("title").and_then(|v| v.as_str()).map(String::from),
        title_contains: args.get("title_contains").and_then(|v| v.as_str()).map(String::from),
        max_results: args.get("max_results").and_then(|v| v.as_u64()).map(|n| n as usize),
        ..Default::default()
    };

    let result = a11y::find_elements(pid, query).await;
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

async fn tool_get_focused(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_i64()).map(|p| p as i32);
    let result = a11y::get_focused_element(pid).await;
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

async fn tool_perform_action(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_i64()).ok_or("Missing pid")? as i32;
    let path: Vec<i32> = args
        .get("path")
        .and_then(|v| v.as_array())
        .ok_or("Missing path")?
        .iter()
        .filter_map(|v| v.as_i64().map(|n| n as i32))
        .collect();

    let action_str = args.get("action").and_then(|v| v.as_str()).ok_or("Missing action")?;
    let action = match action_str {
        "press" => AccessibilityAction::Press,
        "setValue" | "set_value" => AccessibilityAction::SetValue,
        "focus" => AccessibilityAction::Focus,
        "confirm" => AccessibilityAction::Confirm,
        "cancel" => AccessibilityAction::Cancel,
        "increment" => AccessibilityAction::Increment,
        "decrement" => AccessibilityAction::Decrement,
        "showMenu" | "show_menu" => AccessibilityAction::ShowMenu,
        "pick" => AccessibilityAction::Pick,
        _ => return Err(format!("Unknown action: {}", action_str)),
    };

    let value = args.get("value").and_then(|v| v.as_str()).map(String::from);
    let element_path = crate::models::accessibility::ElementPath { pid, path };

    let result = a11y::perform_action(element_path, action, value).await;
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

async fn tool_wait(args: &Value) -> Result<String, String> {
    let pid = args.get("pid").and_then(|v| v.as_i64()).map(|p| p as i32);
    let query = ElementQuery {
        role: args.get("role").and_then(|v| v.as_str()).map(String::from),
        title: args.get("title").and_then(|v| v.as_str()).map(String::from),
        title_contains: args.get("title_contains").and_then(|v| v.as_str()).map(String::from),
        max_results: Some(1),
        ..Default::default()
    };

    let condition = match args.get("condition").and_then(|v| v.as_str()) {
        Some("not_exists") => WaitCondition::NotExists,
        Some("enabled") => WaitCondition::Enabled,
        Some("focused") => WaitCondition::Focused,
        _ => WaitCondition::Exists,
    };

    let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(5000);
    let poll_interval_ms = 100u64;

    let start = std::time::Instant::now();
    loop {
        let elapsed = start.elapsed().as_millis() as u64;
        let result = a11y::find_elements(pid, query.clone()).await;
        let found = result.success && result.elements.as_ref().is_some_and(|e| !e.is_empty());

        let met = match condition {
            WaitCondition::Exists => found,
            WaitCondition::NotExists => !found,
            WaitCondition::Enabled => result.elements.as_ref()
                .and_then(|e| e.first())
                .and_then(|e| e.is_enabled)
                .unwrap_or(false),
            WaitCondition::Focused => result.elements.as_ref()
                .and_then(|e| e.first())
                .and_then(|e| e.is_focused)
                .unwrap_or(false),
        };

        if met {
            return Ok(format!("Condition {:?} met after {}ms", condition, elapsed));
        }
        if elapsed >= timeout_ms {
            return Ok(format!("Timeout: condition {:?} not met within {}ms", condition, timeout_ms));
        }
        tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)).await;
    }
}

async fn tool_status() -> Result<String, String> {
    let display = crate::platform::display::detect_display_server();
    let at_spi2 = crate::platform::permissions::check_at_spi2_available();
    let uinput = crate::platform::permissions::check_uinput_access();
    let screen = crate::platform::permissions::check_screen_capture_available();

    let input_backend = match crate::input::get_backend().await {
        Ok(b) => b.name().to_string(),
        Err(_) => "none".to_string(),
    };

    let frontmost = match a11y::get_frontmost_app().await {
        Ok(Some(app)) => Some(app),
        _ => crate::platform::process::get_frontmost_app(),
    };

    let status = serde_json::json!({
        "display_server": display.to_string(),
        "at_spi2_available": at_spi2,
        "uinput_access": uinput,
        "screen_capture_available": screen,
        "input_backend": input_backend,
        "frontmost_app": frontmost,
    });

    serde_json::to_string_pretty(&status).map_err(|e| e.to_string())
}
