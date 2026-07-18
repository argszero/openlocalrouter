//! 代理路由模块
//!
//! HTTP 代理路由，动态注册端点路由，
//! 处理请求转发和协议转换。

pub(crate) mod handler;
pub(crate) mod sse;
pub(crate) mod streaming;
pub(crate) mod transform;
pub(crate) mod usage;
mod types;

use crate::db::Database;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::Request,
    response::IntoResponse,
    routing::get,
    Router,
};
use std::path::PathBuf;
use std::sync::Arc;

/// 构建代理路由（挂载到管理 API 的 Router 上，共享同一端口）
///
/// 返回 Router<()>，可与 admin Router 合并后用 axum::serve 提供服务
pub fn build_proxy_routes(db: Arc<Database>, frontend_dir: PathBuf) -> Router<()> {
    let state = types::ProxyState { db, frontend_dir };

    Router::new()
        .route("/health", get(handler::health_check))
        .fallback(fallback_handler)
        .layer(DefaultBodyLimit::max(200 * 1024 * 1024))
        .with_state(state)
}

async fn fallback_handler(
    State(state): State<types::ProxyState>,
    req: Request<Body>,
) -> axum::response::Response {
    let uri = req.uri().to_string();
    let method = req.method().clone();
    let method_str = method.as_str();

    // API/asset routes are handled by the admin Router
    if uri.starts_with("/api/") || uri.starts_with("/assets/") || uri.starts_with("/public/") {
        return (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": {"message": "not found", "code": 404}})),
        )
            .into_response();
    }

    let is_get = method_str.eq_ignore_ascii_case(axum::http::Method::GET.as_str());
    let is_post = method_str.eq_ignore_ascii_case(axum::http::Method::POST.as_str());

    if is_get {
        if uri.ends_with("/models") || uri.ends_with("/v1/models") {
            handler::handle_models(State(state), req)
                .await
                .into_response()
        } else {
            // SPA fallback: serve index.html for any other GET
            let index_path = state.frontend_dir.join("index.html");
            match tokio::fs::read_to_string(&index_path).await {
                Ok(html) => (
                    axum::http::StatusCode::OK,
                    [("content-type", "text/html; charset=utf-8")],
                    html,
                )
                    .into_response(),
                Err(_) => (
                    axum::http::StatusCode::NOT_FOUND,
                    axum::Json(serde_json::json!({"error": {"message": "未找到", "code": 404}})),
                )
                    .into_response(),
            }
        }
    } else if is_post {
        if uri.contains("/chat/completions") {
            handler::handle_chat_completions(State(state), req)
                .await
                .into_response()
        } else if uri.contains("/responses") {
            handler::handle_responses(State(state), req)
                .await
                .into_response()
        } else if uri.contains("/v1/messages") || uri.ends_with("/messages") {
            handler::handle_messages(State(state), req)
                .await
                .into_response()
        } else {
            (
                axum::http::StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({"error": {"message": "未找到", "code": 404}})),
            )
                .into_response()
        }
    } else {
        (
            axum::http::StatusCode::METHOD_NOT_ALLOWED,
            axum::Json(serde_json::json!({"error": {"message": "方法不允许", "code": 405}})),
        )
            .into_response()
    }
}
