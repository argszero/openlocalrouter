pub mod dao;
mod schema;

use crate::error::AppError;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 数据库包装
///
/// 使用 `tokio::sync::Mutex` 包装 `rusqlite::Connection`，
/// 支持跨线程的异步访问。所有 DAO 方法通过 `with_conn` 执行。
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// 打开或创建数据库文件
    ///
    /// # Errors
    ///
    /// 如果无法创建父目录、打开数据库文件或运行迁移，返回 `AppError`。
    pub fn open(path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA wal_autocheckpoint=256;
             PRAGMA foreign_keys=ON;",
        )?;
        schema::run_migrations(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 在异步上下文中执行同步数据库操作
    pub(crate) async fn with_conn<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&Connection) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.lock().await;
        f(&conn)
    }

    /// 优雅关闭：等待所有进行中的操作完成，执行 WAL checkpoint。
    ///
    /// 调用后可安全丢弃此 `Database` 实例。
    pub async fn close(&self) {
        // Wait for any in-flight operations to finish, then checkpoint
        let conn = self.conn.lock().await;
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        log::info!("WAL checkpoint 完成");
    }
}
