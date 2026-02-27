use serde::{Deserialize, Serialize};

// -- Target App --

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetApp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desktop_file: Option<String>,
}

// -- Status --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: String,
    pub version: String,
    pub permissions: PermissionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmost_app: Option<AppInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_size: Option<ScreenSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_app: Option<TargetApp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub at_spi2_available: bool,
    pub uinput_access: bool,
    pub screen_capture_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desktop_file: Option<String>,
    pub process_identifier: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenSize {
    pub width: f64,
    pub height: f64,
}

// -- Screenshot --

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScreenshotRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotResponse {
    pub success: bool,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// -- Click --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickRequest {
    pub x: f64,
    pub y: f64,
    #[serde(default = "default_mouse_button")]
    pub button: MouseButton,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<Vec<KeyModifier>>,
}

fn default_mouse_button() -> MouseButton {
    MouseButton::Left
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Center,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// -- Element Click --

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ElementClickRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_accessibility_action: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<MouseButton>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementClickResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<ClickedElementInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickedElementInfo {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<ElementFrameInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<ClickCoordinates>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickCoordinates {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementFrameInfo {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

// -- Type --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeRequest {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<super::accessibility::ElementPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub characters_typed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// -- Key --

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KeyModifier {
    Cmd,
    Command,
    Super,
    Ctrl,
    Control,
    Alt,
    Option,
    Shift,
    Fn,
    Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRequest {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<Vec<KeyModifier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<super::accessibility::ElementPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// -- Scroll --

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScrollRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<super::accessibility::ElementPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// -- Wait --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<super::accessibility::ElementPath>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
    #[serde(default)]
    pub condition: WaitCondition,
}

fn default_timeout_ms() -> u64 {
    5000
}

fn default_poll_interval_ms() -> u64 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WaitCondition {
    #[default]
    Exists,
    NotExists,
    Enabled,
    Focused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitResponse {
    pub success: bool,
    pub condition_met: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// -- Menu --

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MenuGetRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuTriggerRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    pub path: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_items: Option<Vec<MenuItemInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuItemInfo {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<MenuItemInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
}

// -- Error --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: None,
        }
    }

    pub fn with_code(error: impl Into<String>, code: i32) -> Self {
        Self {
            error: error.into(),
            code: Some(code),
        }
    }
}
