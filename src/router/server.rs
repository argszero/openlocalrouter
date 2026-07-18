//! HTTP 代理服务器
//!
//! 基于 axum + hyper 的 HTTP 代理。
//! 使用 catch-all 路由实现热加载：每次请求都从 DB 查询匹配的端点，
//! 新建/启用的端点在请求到来时立即可用，无需重启。

use super::types::ProxyState;
use crate::config::AppConfig;
use axum::{
    extract::{DefaultBodyLimit, State},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;

/// 代理服务器
pub struct ProxyServer {
    config: AppConfig,
    state: ProxyState,
    shutdown_tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
}

impl ProxyServer {
    pub fn new(config: AppConfig, state: ProxyState) -> Self {
        Self {
            config,
            state,
            shutdown_tx: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// 启动代理服务器
    pub async fn start(&self) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
        let addr: SocketAddr =
            format!("{}:{}", self.config.listen_address, self.config.listen_port).parse()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        let app = self.build_router().await;

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        let local_addr = listener.local_addr()?;
        let actual_port = local_addr.port();

        log::info!("代理服务器启动于 {local_addr}");

        let mut shutdown_rx = shutdown_rx;
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let (stream, _remote_addr) = match result {
                            Ok(v) => v,
                            Err(e) => {
                                log::error!("accept 失败: {e}");
                                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                                continue;
                            }
                        };

                        let app = app.clone();
                        tokio::spawn(async move {
                            let service = hyper::service::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                                let mut router = app.clone();
                                async move {
                                    let (parts, body) = req.into_parts();
                                    let body = axum::body::Body::new(body);
                                    let axum_req = http::Request::from_parts(parts, body);
                                    <Router as tower::Service<http::Request<axum::body::Body>>>::call(
                                        &mut router, axum_req,
                                    )
                                    .await
                                }
                            });

                            if let Err(e) = hyper::server::conn::http1::Builder::new()
                                .preserve_header_case(true)
                                .serve_connection(TokioIo::new(stream), service)
                                .await
                            {
                                log::debug!("connection error: {e}");
                            }
                        });
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });

        Ok(actual_port)
    }

    /// 构建路由表
    ///
    /// 为每个已知端点注册显式路由（快速路径），同时挂载 fallback
    /// 处理未预注册的端点（热加载）。新建/启用的端点在请求到来时
    /// 立即可用，无需重启。
    async fn build_router(&self) -> Router {
        // 预注册已知端点路由（性能优化：已知路径直接匹配）
        let mut router = Router::new().route("/health", get(super::handler::health_check));

        if let Ok(endpoints) = self.state.db.list_endpoints().await {
            for endpoint in &endpoints {
                if !endpoint.enabled {
                    continue;
                }
                let path = endpoint.listen_path.as_str();

                router = router
                    .route(
                        &format!("{path}/models"),
                        get(super::handler::handle_models),
                    )
                    .route(
                        &format!("{path}/v1/models"),
                        get(super::handler::handle_models),
                    )
                    .route(
                        &format!("{path}/chat/completions"),
                        post(super::handler::handle_chat_completions),
                    )
                    .route(
                        &format!("{path}/v1/chat/completions"),
                        post(super::handler::handle_chat_completions),
                    )
                    .route(
                        &format!("{path}/responses"),
                        post(super::handler::handle_responses),
                    )
                    .route(
                        &format!("{path}/v1/responses"),
                        post(super::handler::handle_responses),
                    )
                    .route(
                        &format!("{path}/messages"),
                        post(super::handler::handle_messages),
                    )
                    .route(
                        &format!("{path}/v1/messages"),
                        post(super::handler::handle_messages),
                    );
            }
        }

        // Fallback: 处理未预注册的端点（热加载核心机制）
        // axum 0.7 的 fallback 会捕获所有未匹配的方法+路径
        router = router.fallback(fallback_handler);

        router
            .layer(DefaultBodyLimit::max(200 * 1024 * 1024))
            .with_state(self.state.clone())
    }

    /// 停止代理服务器
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(());
            log::info!("代理服务器已停止");
        }
        Ok(())
    }
}

/// Fallback handler — 捕获所有未匹配路由的请求
///
/// 根据 URL 后缀分发到对应的 handler，实现对新建端点
/// （尚未注册路由的）的热加载支持。
async fn fallback_handler(
    State(state): State<ProxyState>,
    req: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let uri = req.uri().to_string();
    let method = req.method().clone();

    if method == axum::http::Method::GET {
        if uri.ends_with("/models") || uri.ends_with("/v1/models") {
            super::handler::handle_models(State(state), req)
                .await
                .into_response()
        } else {
            (
                axum::http::StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({"error": {"message": "未找到", "code": 404}})),
            )
                .into_response()
        }
    } else if method == axum::http::Method::POST {
        if uri.contains("/chat/completions") {
            super::handler::handle_chat_completions(State(state), req)
                .await
                .into_response()
        } else if uri.contains("/responses") {
            super::handler::handle_responses(State(state), req)
                .await
                .into_response()
        } else if uri.contains("/v1/messages") || uri.ends_with("/messages") {
            super::handler::handle_messages(State(state), req)
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
