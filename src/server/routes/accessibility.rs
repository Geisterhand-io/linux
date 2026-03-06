use axum::extract::Query;
use axum::Json;
use serde::Deserialize;

use crate::accessibility::service;
use crate::models::accessibility::{
    ActionRequest, ActionResponse, FindElementsResponse,
    GetElementResponse, GetFocusedResponse,
};

#[derive(Debug, Deserialize, Default)]
pub struct TreeQueryParams {
    pub pid: Option<i32>,
    pub max_depth: Option<i32>,
    pub format: Option<String>,
    pub include_actions: Option<bool>,
    pub root_path: Option<String>,
}

pub async fn handle_tree(Query(params): Query<TreeQueryParams>) -> axum::response::Response {
    let root_path = params.root_path.as_deref().and_then(parse_path);

    let format = params.format.as_deref().unwrap_or("tree");

    if format == "compact" {
        let include_actions = params.include_actions.unwrap_or(false);
        let resp = service::get_compact_tree(params.pid, params.max_depth, include_actions, root_path).await;
        axum::response::IntoResponse::into_response(Json(resp))
    } else {
        let resp = service::get_tree(params.pid, params.max_depth, root_path).await;
        axum::response::IntoResponse::into_response(Json(resp))
    }
}

#[derive(Debug, Deserialize)]
pub struct ElementQueryParams {
    pub pid: i32,
    pub path: String,
    pub child_depth: Option<i32>,
}

pub async fn handle_element(
    Query(params): Query<ElementQueryParams>,
) -> Json<GetElementResponse> {
    let path = parse_path(&params.path).unwrap_or_default();
    let resp = service::get_element(params.pid, path, params.child_depth).await;
    Json(resp)
}

#[derive(Debug, Deserialize, Default)]
pub struct FindElementsQueryParams {
    pub pid: Option<i32>,
    pub role: Option<String>,
    pub title: Option<String>,
    pub title_contains: Option<String>,
    pub label_contains: Option<String>,
    pub value_contains: Option<String>,
    pub placeholder_contains: Option<String>,
    pub max_results: Option<usize>,
}

pub async fn handle_find_elements(
    Query(params): Query<FindElementsQueryParams>,
) -> Json<FindElementsResponse> {
    let query = crate::models::accessibility::ElementQuery {
        role: params.role,
        title: params.title,
        title_contains: params.title_contains,
        label_contains: params.label_contains,
        value_contains: params.value_contains,
        placeholder_contains: params.placeholder_contains,
        max_results: params.max_results,
    };
    let resp = service::find_elements(params.pid, query).await;
    Json(resp)
}

#[derive(Debug, Deserialize, Default)]
pub struct FocusedQueryParams {
    pub pid: Option<i32>,
}

pub async fn handle_focused(Query(params): Query<FocusedQueryParams>) -> Json<GetFocusedResponse> {
    let resp = service::get_focused_element(params.pid).await;
    Json(resp)
}

pub async fn handle_action(Json(body): Json<ActionRequest>) -> Json<ActionResponse> {
    let resp = service::perform_action(body.path, body.action, body.value).await;
    Json(resp)
}

/// Parse a comma-separated path string like "0,1,2" into Vec<i32>.
fn parse_path(s: &str) -> Option<Vec<i32>> {
    if s.is_empty() {
        return Some(vec![]);
    }
    let result: Result<Vec<i32>, _> = s.split(',').map(|p| p.trim().parse::<i32>()).collect();
    result.ok()
}
