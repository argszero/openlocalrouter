//! OpenLocalRouter CLI 入口
//!
//! 纯二进制模式 — 不依赖 Tauri，可直接用于 Docker 部署。

use clap::Parser;
use openlocalrouter_core::config::AppConfig;
use openlocalrouter_core::db;
use openlocalrouter_core::init_logging;
use openlocalrouter_core::run_backend;

#[derive(Parser)]
#[command(
    name = "openlocalrouter",
    version,
    about = "协议无关的本地 AI API 路由网关"
)]
struct Cli {
    /// 重置数据库（删除现有数据，重新初始化）
    #[arg(long)]
    reset_db: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    let config = AppConfig::load();
    init_logging(&config.log_level);

    let db_path = config.database_path();

    if cli.reset_db {
        if db_path.exists() {
            // Also remove WAL and SHM files
            let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
            let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
            std::fs::remove_file(&db_path)?;
            log::info!("数据库已重置: {}", db_path.display());
        } else {
            log::info!("数据库不存在，无需重置");
        }
    }

    let db = db::Database::open(&db_path)?;

    log::info!("数据库已就绪: {}", db_path.display());

    let handle = run_backend(config, db).await;

    handle.await?;
    Ok(())
}
