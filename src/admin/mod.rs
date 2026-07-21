//! 管理 API 模块
//!
//! 提供端点、Provider 和模型的 CRUD REST API，
//! 同时托管前端静态资源和代理路由。
//! 所有服务运行在统一端口（默认 19528）。

pub(crate) mod api_keys;
pub(crate) mod auth;
pub(crate) mod dashboard;
mod endpoints;
pub(crate) mod presets;
mod providers;
pub(crate) mod usage;
mod users;

use crate::config::AppConfig;
use crate::db::Database;
use axum::{
    extract::State,
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::services::ServeDir;

/// 启动管理 API 服务器（统一端口，包含代理路由）
pub async fn serve(
    db: Database,
    config: AppConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db = Arc::new(db);
    let state = AdminState {
        db: db.clone(),
        listen_address: config.listen_address.clone(),
        serve_port: config.admin_port,
    };

    // 需要认证的路由
    let auth_routes = Router::new()
        // Dashboard
        .route("/api/admin/dashboard", get(dashboard::dashboard_handler))
        // Endpoints
        .route(
            "/api/admin/endpoints",
            get(endpoints::list_endpoints).post(endpoints::create_endpoint),
        )
        .route(
            "/api/admin/endpoints/:id",
            get(endpoints::get_endpoint)
                .put(endpoints::update_endpoint)
                .delete(endpoints::delete_endpoint),
        )
        // API Keys (scoped under endpoint)
        .route(
            "/api/admin/endpoints/:id/keys",
            get(api_keys::list_api_keys).post(api_keys::create_api_key),
        )
        .route(
            "/api/admin/endpoints/:id/keys/:key_id",
            put(api_keys::update_api_key).delete(api_keys::delete_api_key),
        )
        // Usage
        .route("/api/admin/usage", get(usage::list_usage))
        .route("/api/admin/usage/summary", get(usage::usage_summary))
        .route("/api/admin/usage/my/summary", get(usage::my_usage_summary))
        .route("/api/admin/usage/my/trend", get(usage::my_usage_trend))
        .route(
            "/api/admin/usage/my/trend-breakdown",
            get(usage::my_usage_trend_breakdown),
        )
        .route("/api/admin/usage/my/records", get(usage::my_usage_records))
        .route(
            "/api/admin/usage/shared/summary",
            get(usage::shared_summary),
        )
        .route("/api/admin/usage/shared/trend", get(usage::shared_trend))
        .route("/api/admin/usage/shared/top", get(usage::shared_top))
        .route("/api/admin/usage/shared/keys", get(usage::shared_keys))
        .route(
            "/api/admin/usage/shared/records",
            get(usage::shared_records),
        )
        .route("/api/admin/keys/:id/usage", get(usage::key_usage))
        // Providers
        .route(
            "/api/admin/providers",
            get(providers::list_providers).post(providers::create_provider),
        )
        .route(
            "/api/admin/providers/:id",
            get(providers::get_provider)
                .put(providers::update_provider)
                .delete(providers::delete_provider),
        )
        // Models (scoped under provider)
        .route(
            "/api/admin/providers/:id/models",
            get(providers::list_models).post(providers::create_model),
        )
        .route(
            "/api/admin/providers/:id/models/:model_id",
            delete(providers::delete_model),
        )
        .route(
            "/api/admin/providers/:id/models/:model_id/visibility",
            put(providers::set_model_visibility),
        )
        // Users
        .route(
            "/api/admin/users",
            get(users::list_users).post(users::create_user),
        )
        .route(
            "/api/admin/users/:id",
            put(users::update_user).delete(users::delete_user),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ));

    // 公开路由（无需认证）
    let public_routes = Router::new()
        .route("/api/admin/status", get(status_handler))
        .route("/api/admin/login", post(auth::login_handler))
        .route("/api/admin/logout", post(auth::logout_handler))
        .route("/api/admin/presets", get(get_presets_handler))
        .route("/api/admin/server-info", get(server_info_handler));

    // 前端静态文件
    let frontend_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("frontend")
        .join("dist");

    log::info!("前端路径: {}", frontend_dir.display());

    let api_app = Router::new()
        .merge(public_routes)
        .merge(auth_routes)
        .with_state(state);

    // Merge proxy routes as Router<()>
    let proxy_app = crate::router::build_proxy_routes(db.clone(), frontend_dir.clone());
    let app = api_app.merge(proxy_app);

    // Serve frontend SPA static assets
    let app = if frontend_dir.join("index.html").exists() {
        log::info!("前端静态文件已挂载");
        app.nest_service("/assets", ServeDir::new(frontend_dir.join("assets")))
            .nest_service("/public", ServeDir::new(frontend_dir.join("public")))
    } else {
        log::warn!("前端目录不存在，仅 API 模式");
        app
    };

    let addr = format!("{}:{}", config.listen_address, config.admin_port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!(
                "无法绑定到 {addr}: {e}。该地址可能已被占用，请检查是否有其他进程在使用此端口。"
            );
            return Err(Box::new(e));
        }
    };
    log::info!("OpenLocalRouter 启动于 http://{addr}");

    let app: Router = app;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
pub struct AdminState {
    pub(crate) db: Arc<Database>,
    pub(crate) listen_address: String,
    pub(crate) serve_port: u16,
}

impl axum::extract::FromRef<AdminState> for Arc<Database> {
    fn from_ref(state: &AdminState) -> Self {
        state.db.clone()
    }
}

/// GET /api/admin/status — 服务状态
async fn status_handler() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "running",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// GET /api/admin/presets — Provider 预设列表
async fn get_presets_handler() -> axum::Json<Vec<presets::ProviderPreset>> {
    axum::Json(presets::get_presets())
}

/// GET /api/admin/server-info — 返回服务器连接信息（无需认证）
async fn server_info_handler(State(state): State<AdminState>) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "listen_address": state.listen_address,
        "proxy_port": state.serve_port,
        "proxy_base_url": format!("http://{}:{}", state.listen_address, state.serve_port),
    }))
}
