use serde::{Deserialize, Serialize};

use super::api::AppInfo;

// -- Element Path --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementPath {
    pub pid: i32,
    pub path: Vec<i32>,
}

// -- Element Frame --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ElementFrame {
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

// -- UI Element Info --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIElementInfo {
    pub path: ElementPath,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<ElementFrame>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_focused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<UIElementInfo>>,
}

// -- Compact Element Info (for flattened tree output) --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactElementInfo {
    pub path: ElementPath,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<ElementFrame>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<String>>,
    pub depth: i32,
}

// -- Element Query --

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ElementQuery {
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
    pub max_results: Option<usize>,
}

// -- Accessibility Actions --

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AccessibilityAction {
    Press,
    SetValue,
    Focus,
    Confirm,
    Cancel,
    Increment,
    Decrement,
    ShowMenu,
    Pick,
}

// -- Request types --

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetTreeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_actions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetElementRequest {
    pub pid: i32,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_depth: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FindElementsRequest {
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
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub path: ElementPath,
    pub action: AccessibilityAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetFocusedRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
}

// -- Response types --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTreeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<AppInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree: Option<UIElementInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCompactTreeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<AppInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<CompactElementInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindElementsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<UIElementInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFocusedResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<UIElementInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetElementResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<UIElementInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
