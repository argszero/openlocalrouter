//! HTTP 请求处理器
//!
//! 核心代理 handler：
//! - `handle_models`: 聚合模型列表（无需认证）
//! - `handle_chat_completions`: `OpenAI Chat Completions`（需 Bearer API Key）
//! - `handle_messages`: Anthropic Messages（需 Bearer API Key）
//! - `handle_responses`: `OpenAI Responses`（需 Bearer API Key）
//!
//! 支持：`SSE` 流式透传 + `OpenAI` ↔ `Anthropic` 协议转换

use super::types::ProxyState;
use super::usage::{TokenUsage, UsageContext};
use crate::db::dao::UsageRecordRow;
use crate::db::Database;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

/// 上游请求重试配置
const MAX_RETRIES: u32 = 3;
const RETRY_BACKOFF_MS: u64 = 500;

/// 表示已通过 API Key 认证的请求上下文
#[derive(Debug, Clone)]
pub(crate) struct ProxyAuth {
    pub(crate) user_id: String,
    pub(crate) key_owner_id: String,
    pub(crate) endpoint_id: String,
    pub(crate) api_key_id: String,
}

/// 从 headers 中提取 Bearer API Key 并验证
pub(crate) async fn verify_api_key(
    db: &Database,
    endpoint_id: &str,
    headers: &HeaderMap,
) -> Result<ProxyAuth, (StatusCode, Json<serde_json::Value>)> {
    let header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = header.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {"message": "未提供 API Key", "code": 401}
            })),
        )
    })?;

    let api_key = db.get_api_key_by_value(token).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {"message": "服务错误", "code": 500}
            })),
        )
    })?;

    let api_key = api_key.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {"message": "无效的 API Key", "code": 401}
            })),
        )
    })?;

    if !api_key.enabled {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {"message": "API Key 已被禁用", "code": 401}
            })),
        ));
    }

    if api_key.endpoint_id != endpoint_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": {"message": "API Key 无权访问此端点", "code": 403}
            })),
        ));
    }

    let _ = db.touch_api_key(&api_key.id).await;

    Ok(ProxyAuth {
        user_id: api_key.assigned_to.clone(),
        key_owner_id: api_key.created_by.clone(),
        endpoint_id: api_key.endpoint_id,
        api_key_id: api_key.id,
    })
}

/// GET /health — 健康检查
pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// 从请求 URL 中提取端点 `listen_path`
pub(crate) async fn extract_endpoint_path(db: &Database, uri: &str) -> Option<String> {
    let path = uri.split('?').next().unwrap_or("");
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    let endpoints = db.list_endpoints().await.ok()?;
    for ep in &endpoints {
        if !ep.enabled {
            continue;
        }
        let ep_segments: Vec<&str> = ep
            .listen_path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        if ep_segments.len() <= segments.len() && segments[..ep_segments.len()] == ep_segments[..] {
            return Some(ep.listen_path.clone());
        }
    }
    None
}

/// 从请求 body 中提取模型名
pub(crate) fn extract_model_from_body(body: &[u8]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(body).ok()?;
    v.get("model")?
        .as_str()
        .map(std::string::ToString::to_string)
}

/// 从 `ModelRow` 解析实际请求上游的模型名
fn resolve_upstream_model(model_row: &crate::db::dao::ModelRow) -> String {
    serde_json::from_str::<serde_json::Value>(&model_row.extra_config)
        .ok()
        .and_then(|v| {
            v.get("model_slug")
                .and_then(|s| s.as_str())
                .map(std::string::ToString::to_string)
        })
        .unwrap_or_else(|| model_row.slug.clone())
}

/// 从请求 body 中提取 stream 标志
fn is_stream_request(body: &[u8]) -> bool {
    let v: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    v.get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// GET /models — 聚合模型列表（无需认证）
pub async fn handle_models(
    State(state): State<ProxyState>,
    req: axum::http::Request<Body>,
) -> impl IntoResponse {
    let uri = req.uri().to_string();

    let Some(endpoint_path) = extract_endpoint_path(&state.db, &uri).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": {"message": "端点未找到", "code": 404}})),
        )
            .into_response();
    };

    let Ok(Some(endpoint)) = state.db.get_endpoint_by_path(&endpoint_path).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": {"message": "端点不存在", "code": 404}})),
        )
            .into_response();
    };

    let models = state
        .db
        .list_models_for_endpoint(&endpoint.id)
        .await
        .unwrap_or_default();

    let data: Vec<serde_json::Value> = models
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.slug,
                "object": "model",
                "created": 0,
                "owned_by": "openlocalrouter",
                "display_name": m.display_name,
                "context_window": m.context_window,
            })
        })
        .collect();

    Json(serde_json::json!({
        "object": "list",
        "data": data,
    }))
    .into_response()
}

/// `POST /chat/completions` — `OpenAI Chat`
pub async fn handle_chat_completions(
    State(state): State<ProxyState>,
    req: axum::http::Request<Body>,
) -> impl IntoResponse {
    proxy_request(state, req, "openai_chat").await
}

/// `POST /responses` — `OpenAI Responses`
pub async fn handle_responses(
    State(state): State<ProxyState>,
    req: axum::http::Request<Body>,
) -> impl IntoResponse {
    proxy_request(state, req, "openai_responses").await
}

/// POST /v1/messages — Anthropic Messages
pub async fn handle_messages(
    State(state): State<ProxyState>,
    req: axum::http::Request<Body>,
) -> impl IntoResponse {
    proxy_request(state, req, "anthropic_messages").await
}

/// 核心代理逻辑
///
/// 流程：
/// 1. 匹配端点（热加载：每次请求都查 DB，无需重启）
/// 2. API Key 认证
/// 3. 解析模型名 → 查找 Provider
/// 4. 协议匹配：同协议直通，不同协议转换
/// 5. 转发：流式（streaming）或非流式
async fn proxy_request(
    state: ProxyState,
    req: axum::http::Request<Body>,
    expected_protocol: &str,
) -> axum::response::Response {
    let (parts, body) = req.into_parts();
    let uri_str = parts.uri.to_string();
    let headers = parts.headers.clone();

    // 1. 匹配端点（每次请求查 DB，实现热加载）
    let Some(endpoint_path) = extract_endpoint_path(&state.db, &uri_str).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": {"message": "端点未找到", "code": 404}})),
        )
            .into_response();
    };

    let Ok(Some(endpoint)) = state.db.get_endpoint_by_path(&endpoint_path).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": {"message": "端点不存在", "code": 404}})),
        )
            .into_response();
    };

    // 2. API Key 认证
    let auth = match verify_api_key(&state.db, &endpoint.id, &headers).await {
        Ok(a) => a,
        Err((status, json)) => return (status, json).into_response(),
    };

    // 3. 读取 body
    let body_bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    serde_json::json!({"error": {"message": format!("读取请求体失败: {e}"), "code": 400}}),
                ),
            )
                .into_response();
        }
    };

    // 4. 从 body 中提取原始模型名，查找 model row 以获取 model_slug
    let raw_model_name = extract_model_from_body(&body_bytes).unwrap_or_default();

    // 构建用量上下文
    let mut usage_ctx = super::usage::UsageContext {
        api_key_id: auth.api_key_id.clone(),
        key_owner_id: auth.key_owner_id.clone(),
        endpoint_id: auth.endpoint_id.clone(),
        user_id: auth.user_id.clone(),
        provider_id: String::new(),
        provider_name: String::new(),
        model: raw_model_name.clone(),
    };

    // Find the model row to resolve the actual upstream model name
    let models = state
        .db
        .list_models_for_endpoint(&endpoint.id)
        .await
        .unwrap_or_default();
    let model_row = models.iter().find(|m| m.slug == raw_model_name);
    let upstream_model_name =
        model_row.map_or_else(|| raw_model_name.clone(), resolve_upstream_model);

    let provider = match state
        .db
        .find_provider_by_model_slug(&raw_model_name, &endpoint.id)
        .await
    {
        Ok(Some(p)) => {
            usage_ctx.provider_id = p.id.clone();
            usage_ctx.provider_name = p.name.clone();
            p
        }
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(
                    serde_json::json!({"error": {"message": format!("模型 {raw_model_name} 未找到可用的 Provider"), "code": 404}}),
                ),
            )
                .into_response();
        }
    };

    // 5. 检查 Provider 支持的协议
    let supported: Vec<&str> = provider
        .api_type
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if supported.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": {"message": "Provider 未配置协议", "code": 400}})),
        )
            .into_response();
    }

    // 6. 决定转发协议和是否需要转换
    let forwarding_protocol = if supported.contains(&expected_protocol) {
        // 直接匹配 — 同协议直通
        expected_protocol
    } else if expected_protocol == "openai_chat" && supported.contains(&"anthropic_messages") {
        // Anthropic → OpenAI Chat: 需要转换请求+响应
        "anthropic_messages"
    } else if expected_protocol == "anthropic_messages" && supported.contains(&"openai_chat") {
        // OpenAI Chat → Anthropic: 需要转换请求+响应
        "openai_chat"
    } else if expected_protocol == "openai_responses" && supported.contains(&"openai_chat") {
        // OpenAI Responses → Chat: 降级到 Chat Completions
        "openai_chat"
    } else if expected_protocol == "openai_responses" && supported.contains(&"anthropic_messages") {
        // OpenAI Responses → Anthropic: 降级到 Anthropic Messages
        "anthropic_messages"
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({
                    "error": {"message": format!("Provider {} 不支持 {expected_protocol} 协议", provider.name), "code": 400}
                }),
            ),
        )
            .into_response();
    };

    // 是否需要协议转换
    let needs_transform = forwarding_protocol != expected_protocol;

    // 7. 计算上游 URL — 先从 api_urls 查找，fallback 到 base_url
    let api_urls: Option<std::collections::HashMap<String, String>> =
        serde_json::from_str(&provider.extra_config)
            .ok()
            .and_then(|v: serde_json::Value| v.get("api_urls").cloned())
            .and_then(|u| serde_json::from_value(u).ok());

    let effective_base_url = api_urls
        .as_ref()
        .and_then(|urls| urls.get(forwarding_protocol))
        .map_or_else(
            || provider.base_url.trim_end_matches('/'),
            |u| u.trim_end_matches('/'),
        );

    let upstream_path = compute_upstream_path(&uri_str, &endpoint_path, forwarding_protocol);
    let upstream_url = format!("{effective_base_url}{upstream_path}");

    log::info!(
        "代理: {} -> {} (provider: {}, model: {}, protocol: {}→{}, transform: {})",
        uri_str,
        upstream_url,
        provider.name,
        upstream_model_name,
        expected_protocol,
        forwarding_protocol,
        needs_transform
    );

    // 8. 转换请求体（如需）
    let upstream_body = if needs_transform {
        transform_request_body(
            &body_bytes,
            expected_protocol,
            forwarding_protocol,
            &upstream_model_name,
            &provider.api_key,
        )
    } else if upstream_model_name != raw_model_name {
        // 替换请求体中的 model 字段
        let mut v: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();
        v["model"] = serde_json::Value::String(upstream_model_name.clone());
        serde_json::to_vec(&v).unwrap_or_else(|_| body_bytes.to_vec())
    } else {
        body_bytes.to_vec()
    };

    let is_stream = is_stream_request(&upstream_body);

    // 9. 构建转发 headers
    let mut fwd_headers = reqwest::header::HeaderMap::new();
    for (k, v) in &headers {
        let k_str = match std::str::from_utf8(k.as_str().as_bytes()) {
            Ok(s) => s.to_string(),
            Err(_) => continue,
        };
        if matches!(
            k_str.to_lowercase().as_str(),
            "host" | "connection" | "transfer-encoding" | "content-length"
        ) {
            continue;
        }
        if k_str.to_lowercase() == "authorization" {
            fwd_headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", provider.api_key))
                    .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static("")),
            );
        } else if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(k_str.as_bytes()),
            reqwest::header::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            fwd_headers.insert(name, value);
        }
    }

    if !headers.contains_key("Authorization")
        && !headers.contains_key("authorization")
        && !provider.api_key.is_empty()
    {
        fwd_headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", provider.api_key))
                .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static("")),
        );
    }

    // 10. 发起请求（带重试）
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .unwrap_or_default();

    let mut last_error = None;
    let mut response = None;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = Duration::from_millis(RETRY_BACKOFF_MS * 2u64.pow(attempt - 1));
            log::info!(
                "上游请求重试 {}/{} (目标: {upstream_url}), 等待 {}ms",
                attempt,
                MAX_RETRIES,
                delay.as_millis()
            );
            tokio::time::sleep(delay).await;
        }

        let upstream_req = client
            .post(&upstream_url)
            .headers(fwd_headers.clone())
            .body(upstream_body.clone())
            .send()
            .await;

        match upstream_req {
            Ok(resp) => {
                let status = resp.status();
                // 5xx 服务端错误可重试，4xx 客户端错误不重试
                if status.is_server_error() && attempt < MAX_RETRIES {
                    log::warn!("上游返回 5xx: {status} (target: {upstream_url}), 将重试");
                    last_error = Some(format!("上游返回 {status}"));
                    continue;
                }
                response = Some(resp);
                break;
            }
            Err(e) => {
                if attempt < MAX_RETRIES {
                    log::warn!("上游连接失败: {e} (target: {upstream_url}), 将重试");
                    last_error = Some(e.to_string());
                } else {
                    last_error = Some(e.to_string());
                }
            }
        }
    }

    if let Some(resp) = response {
        let status = StatusCode::from_u16(resp.status().as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        log::debug!("上游响应状态: {status} (target: {upstream_url})");

        if is_stream && status.is_success() {
            handle_streaming_response(
                resp,
                needs_transform,
                expected_protocol,
                forwarding_protocol,
                usage_ctx,
                &state.db,
            )
        } else if needs_transform && status.is_success() {
            handle_transformed_response(
                resp,
                status,
                expected_protocol,
                forwarding_protocol,
                usage_ctx,
                &state.db,
            )
            .await
        } else {
            handle_passthrough_response(resp, status, usage_ctx, &state.db).await
        }
    } else {
        let err_msg = last_error.unwrap_or_else(|| "未知错误".into());
        log::error!("上游请求失败（已重试 {MAX_RETRIES} 次）: {err_msg} (target: {upstream_url})");
        (
            StatusCode::BAD_GATEWAY,
            Json(
                serde_json::json!({"error": {"message": format!("上游服务不可用: {err_msg}"), "code": 502}}),
            ),
        )
            .into_response()
    }
}

/// 转换请求体
fn transform_request_body(
    body: &[u8],
    from_protocol: &str,
    to_protocol: &str,
    _model_name: &str,
    _api_key: &str,
) -> Vec<u8> {
    let v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return body.to_vec(),
    };

    let transformed = match (from_protocol, to_protocol) {
        ("anthropic_messages", "openai_chat") => super::transform::anthropic_to_openai_chat(&v),
        ("openai_responses", "openai_chat") => {
            super::transform::openai_responses_to_openai_chat(&v)
        }
        ("openai_chat", "anthropic_messages") => {
            // Anthropic API 需要 x-api-key header，这里我们无法修改 header
            // 保留原始 body 但标记需要特殊处理
            // 实际上 Anthropic API 使用 x-api-key 而不是 Authorization: Bearer
            // 所以在构建 header 时已经处理了
            return serde_json::to_vec(&v).unwrap_or_else(|_| body.to_vec());
        }
        _ => return body.to_vec(),
    };

    serde_json::to_vec(&transformed).unwrap_or_else(|_| body.to_vec())
}

/// 处理流式响应（passthrough 或 transform）
fn handle_streaming_response(
    resp: reqwest::Response,
    needs_transform: bool,
    expected_protocol: &str,
    forwarding_protocol: &str,
    usage_ctx: UsageContext,
    db: &Database,
) -> axum::response::Response {
    let resp_headers = resp.headers().clone();
    let resp_status = resp.status();
    let db = Arc::new(db.clone());

    let byte_stream = resp
        .bytes_stream()
        .map(|result| result.map_err(std::io::Error::other));

    if needs_transform {
        match (expected_protocol, forwarding_protocol) {
            ("anthropic_messages", "openai_chat") => {
                // OpenAI SSE → Anthropic SSE
                let transformed = super::streaming::openai_sse_to_anthropic(byte_stream);
                let recording =
                    UsageRecordingStream::new(transformed, forwarding_protocol, usage_ctx, db);
                let body = Body::from_stream(recording);
                let mut response = axum::response::Response::builder().status(StatusCode::OK);
                response = response.header("content-type", "text/event-stream");
                response = response.header("cache-control", "no-cache");
                response = response.header("x-accel-buffering", "no");
                return response.body(body).unwrap_or_else(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": "内部错误"}})),
                    )
                        .into_response()
                });
            }
            ("openai_responses", "openai_chat") => {
                // OpenAI Chat SSE → Responses SSE
                let transformed = super::streaming::openai_sse_to_openai_responses(byte_stream);
                let recording =
                    UsageRecordingStream::new(transformed, forwarding_protocol, usage_ctx, db);
                let body = Body::from_stream(recording);
                let mut response = axum::response::Response::builder().status(StatusCode::OK);
                response = response.header("content-type", "text/event-stream");
                response = response.header("cache-control", "no-cache");
                response = response.header("x-accel-buffering", "no");
                return response.body(body).unwrap_or_else(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": "内部错误"}})),
                    )
                        .into_response()
                });
            }
            _ => {
                // Other transforms — pass through for now
            }
        }
    }

    // 直通流式（或 fallback transform），用 wrapper 抓取 usage
    let recording = UsageRecordingStream::new(byte_stream, forwarding_protocol, usage_ctx, db);
    let body = Body::from_stream(recording);
    let mut response = axum::response::Response::builder()
        .status(StatusCode::from_u16(resp_status.as_u16()).unwrap_or(StatusCode::OK));

    for (k, v) in &resp_headers {
        if let (Ok(name), Ok(value)) = (
            axum::http::HeaderName::from_bytes(k.as_str().as_bytes()),
            axum::http::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            response = response.header(name, value);
        }
    }

    response.body(body).unwrap_or_else(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": {"message": "内部错误"}})),
        )
            .into_response()
    })
}

/// 处理非流式转换响应
async fn handle_transformed_response(
    resp: reqwest::Response,
    status: StatusCode,
    expected_protocol: &str,
    forwarding_protocol: &str,
    usage_ctx: UsageContext,
    db: &Database,
) -> axum::response::Response {
    match resp.bytes().await {
        Ok(data) => {
            // 记录用量
            if let Some(usage) = super::usage::extract_usage_from_body(&data) {
                write_usage_record(db, &usage_ctx, &usage).await;
            }

            let v: serde_json::Value = if let Ok(v) = serde_json::from_slice(&data) {
                v
            } else {
                log::error!("上游响应JSON解析失败: {}", String::from_utf8_lossy(&data));
                return (status, Body::from(data.to_vec())).into_response();
            };

            log::debug!(
                "上游原始响应 ({}→{}): {}",
                expected_protocol,
                forwarding_protocol,
                serde_json::to_string(&v).unwrap_or_default()
            );

            let transformed = match (expected_protocol, forwarding_protocol) {
                ("anthropic_messages", "openai_chat") => {
                    super::transform::openai_chat_to_anthropic(&v)
                }
                ("openai_responses", "openai_chat") => {
                    super::transform::openai_chat_to_openai_responses(&v)
                }
                ("openai_chat", "anthropic_messages") => {
                    return (status, Body::from(data.to_vec())).into_response();
                }
                _ => return (status, Body::from(data.to_vec())).into_response(),
            };

            let body_bytes = serde_json::to_vec(&transformed).unwrap_or_else(|_| data.to_vec());
            let mut response = axum::response::Response::builder().status(status);
            response = response.header("content-type", "application/json");
            response.body(Body::from(body_bytes)).unwrap_or_else(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": {"message": "内部错误"}})),
                )
                    .into_response()
            })
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": {"message": format!("读取上游响应失败: {e}")}})),
        )
            .into_response(),
    }
}

/// 处理直通响应（非流式）
async fn handle_passthrough_response(
    resp: reqwest::Response,
    status: StatusCode,
    usage_ctx: UsageContext,
    db: &Database,
) -> axum::response::Response {
    let resp_headers = resp.headers().clone();

    match resp.bytes().await {
        Ok(data) => {
            // 记录用量
            if let Some(usage) = super::usage::extract_usage_from_body(&data) {
                write_usage_record(db, &usage_ctx, &usage).await;
            }
            let mut response = axum::response::Response::builder().status(status);
            for (k, v) in &resp_headers {
                let k_lower = k.as_str().to_lowercase();
                // Filter hop-by-hop and conflicting headers that would break HTTP
                if matches!(
                    k_lower.as_str(),
                    "transfer-encoding"
                        | "connection"
                        | "keep-alive"
                        | "trailer"
                        | "upgrade"
                        | "content-encoding"
                        | "content-length"
                ) {
                    continue;
                }
                if let (Ok(name), Ok(value)) = (
                    axum::http::HeaderName::from_bytes(k.as_str().as_bytes()),
                    axum::http::HeaderValue::from_bytes(v.as_bytes()),
                ) {
                    response = response.header(name, value);
                }
            }
            response
                .body(Body::from(data.to_vec()))
                .unwrap_or_else(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": {"message": "内部错误"}})),
                    )
                        .into_response()
                })
        }
        Err(e) => {
            log::error!("读取上游响应体失败: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": {"message": format!("读取上游响应失败: {e}")}})),
            )
                .into_response()
        }
    }
}

/// 计算上游路径
///
/// 根据转发协议确定正确的 API 路径，避免用户 URL 中的冗余前缀
/// 被拼接到上游 `base_url` 后产生双重路径（如 `/v1/v1/chat/completions`）。
fn compute_upstream_path(full_uri: &str, listen_path: &str, protocol: &str) -> String {
    let query = full_uri
        .split('?')
        .nth(1)
        .map(|q| format!("?{q}"))
        .unwrap_or_default();

    let upstream_path = match protocol {
        "openai_chat" => "/chat/completions",
        "openai_responses" => "/responses",
        "anthropic_messages" => "/v1/messages",
        _ => {
            let path = full_uri.split('?').next().unwrap_or("");
            if let Some(rest) = path.strip_prefix(listen_path) {
                rest
            } else {
                path
            }
        }
    };

    format!("{upstream_path}{query}")
}

/// 写入用量记录（非流式路径）
async fn write_usage_record(db: &Database, ctx: &UsageContext, usage: &TokenUsage) {
    if usage.is_zero() {
        return;
    }
    let row = UsageRecordRow {
        id: uuid::Uuid::new_v4().to_string(),
        api_key_id: ctx.api_key_id.clone(),
        key_owner_id: ctx.key_owner_id.clone(),
        endpoint_id: ctx.endpoint_id.clone(),
        user_id: ctx.user_id.clone(),
        provider_id: ctx.provider_id.clone(),
        provider_name: ctx.provider_name.clone(),
        model: ctx.model.clone(),
        input_tokens: i64::from(usage.input_tokens),
        output_tokens: i64::from(usage.output_tokens),
        cache_read_tokens: i64::from(usage.cache_read_tokens),
        created_at: String::new(),
    };
    if let Err(e) = db.insert_usage_record(&row).await {
        log::warn!("写入用量记录失败: {e}");
    }
}

/// 流式响应用量记录包装器
///
/// 包装一个 `Pin<Box<dyn Stream>>`，在数据流经时从 SSE chunk 中提取 usage 信息。
/// `Stream` 结束时通过 `tokio::spawn` 异步写入 `DB`。
struct UsageRecordingStream {
    inner: Pin<Box<dyn futures::Stream<Item = Result<Bytes, std::io::Error>> + Send>>,
    protocol: String,
    buffer: Vec<u8>,
    usage: Option<TokenUsage>,
    ctx: UsageContext,
    db: Arc<Database>,
}

impl UsageRecordingStream {
    fn new<S>(inner: S, protocol: &str, ctx: UsageContext, db: Arc<Database>) -> Self
    where
        S: futures::Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
    {
        Self {
            inner: Box::pin(inner),
            protocol: protocol.to_string(),
            buffer: Vec::new(),
            usage: None,
            ctx,
            db,
        }
    }

    /// 从累积的 SSE line buffer 中尝试提取 usage
    fn try_extract_from_line(&mut self, line: &str) {
        // OpenAI Chat SSE: data: {...} where ... contains "usage"
        if self.protocol == "openai_chat" || self.protocol == "openai_responses" {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    return;
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(u) = super::usage::extract_usage_from_chat_sse(&v) {
                        if !u.is_zero() {
                            self.usage = Some(u);
                        }
                    }
                }
            }
        }

        // Anthropic SSE: event: message_delta \n data: {...}
        if self.protocol == "anthropic_messages" {
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(u) = super::usage::extract_usage_from_anthropic_sse(&v) {
                        if !u.is_zero() {
                            self.usage = Some(u);
                        }
                    }
                }
            }
        }
    }
}

impl futures::Stream for UsageRecordingStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                // Accumulate into line buffer for SSE parsing
                self.buffer.extend_from_slice(&bytes);
                // Check for complete SSE lines (double \n)
                while let Some(pos) = self.buffer.windows(2).position(|w| w == b"\n\n") {
                    let line_bytes = self.buffer[..pos].to_vec();
                    if let Ok(line) = std::str::from_utf8(&line_bytes) {
                        self.try_extract_from_line(line);
                    }
                    // Remove up through the double newline
                    let drain_end = (pos + 2).min(self.buffer.len());
                    self.buffer.drain(..drain_end);
                }
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(None) => {
                // Stream ended — write usage record in background
                if let Some(ref usage) = self.usage {
                    if !usage.is_zero() {
                        let db = Arc::clone(&self.db);
                        let ctx = self.ctx.clone();
                        let usage = usage.clone();
                        tokio::spawn(async move {
                            let row = UsageRecordRow {
                                id: uuid::Uuid::new_v4().to_string(),
                                api_key_id: ctx.api_key_id,
                                key_owner_id: ctx.key_owner_id,
                                endpoint_id: ctx.endpoint_id,
                                user_id: ctx.user_id,
                                provider_id: ctx.provider_id,
                                provider_name: ctx.provider_name,
                                model: ctx.model,
                                input_tokens: i64::from(usage.input_tokens),
                                output_tokens: i64::from(usage.output_tokens),
                                cache_read_tokens: i64::from(usage.cache_read_tokens),
                                created_at: String::new(),
                            };
                            if let Err(e) = db.insert_usage_record(&row).await {
                                log::warn!("写入流式用量记录失败: {e}");
                            }
                        });
                    }
                }
                Poll::Ready(None)
            }
            other => other,
        }
    }
}
