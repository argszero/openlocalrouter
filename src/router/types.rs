use std::path::PathBuf;

/// 代理服务器共享状态
#[derive(Clone)]
pub struct ProxyState {
    /// 数据库连接
    pub db: std::sync::Arc<crate::db::Database>,
    /// 前端静态文件目录
    pub frontend_dir: PathBuf,
}
