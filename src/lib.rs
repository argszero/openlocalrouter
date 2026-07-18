//! OpenLocalRouter — 本机运行的协议无关 AI API 路由网关
//!
//! 核心库，提供代理服务器和管理 API 的启动逻辑。
//! CLI 和 Tauri 入口均通过 `run_backend` 函数使用本库。

mod admin;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
mod router;

/// 启动后端服务（管理 API + 代理共用一个端口），返回 tokio JoinHandle。
pub async fn run_backend(
    config: config::AppConfig,
    db: db::Database,
) -> tokio::task::JoinHandle<()> {
    // 确保默认管理员存在
    ensure_default_admin(&db, &config).await;

    // 启动统一服务（管理 API + 代理路由 + 前端静态文件）
    let handle = tokio::spawn(async move {
        if let Err(e) = admin::serve(db, config).await {
            log::error!("服务异常退出: {e}");
        }
    });

    handle
}

/// 初始化日志
pub fn init_logging(level: &str) {
    let filter = match level {
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(filter.as_str()))
        .init();
}

/// 确保默认管理员用户存在
async fn ensure_default_admin(db: &db::Database, _config: &config::AppConfig) {
    let count = db.count_users().await.unwrap_or(0);
    if count == 0 {
        let password = auth::generate_password();
        let password_hash = auth::hash_password(&password).expect("密码哈希失败");

        let user = db::dao::UserRow {
            id: uuid::Uuid::new_v4().to_string(),
            username: "admin".to_string(),
            password_hash,
            is_admin: true,
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };

        if db.create_user(&user).await.is_ok() {
            log::info!("============================================");
            log::info!("  管理员账号已创建");
            log::info!("  用户名: admin");
            log::info!("  密码:   {password}");
            log::info!("============================================");
        }
    }
}
