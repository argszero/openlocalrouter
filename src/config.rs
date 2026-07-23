use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 代理服务器监听地址
    #[serde(default = "default_listen_address")]
    pub listen_address: String,

    /// 代理服务器监听端口
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,

    /// 管理界面端口
    #[serde(default = "default_admin_port")]
    pub admin_port: u16,

    /// 数据目录
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_listen_address() -> String {
    "127.0.0.1".to_string()
}

fn default_listen_port() -> u16 {
    19527
}

fn default_admin_port() -> u16 {
    19528
}

fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("openlocalrouter")
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            listen_address: default_listen_address(),
            listen_port: default_listen_port(),
            admin_port: default_admin_port(),
            data_dir: default_data_dir(),
            log_level: default_log_level(),
        }
    }
}

impl AppConfig {
    /// 从默认路径加载配置，不存在则使用默认值。
    /// 支持环境变量覆盖（`OLR_LISTEN_ADDRESS`, `OLR_LOG_LEVEL`），便于 Docker 部署。
    pub fn load() -> Self {
        let config_path = Self::config_path();
        let mut config = if config_path.exists() {
            std::fs::read_to_string(&config_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            let config = Self::default();
            if let Some(parent) = config_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&config) {
                let _ = std::fs::write(&config_path, json);
            }
            config
        };

        // 环境变量覆盖（Docker / 容器化部署）
        if let Ok(addr) = std::env::var("OLR_LISTEN_ADDRESS") {
            config.listen_address = addr;
        }
        if let Ok(level) = std::env::var("OLR_LOG_LEVEL") {
            config.log_level = level;
        }
        if let Ok(dir) = std::env::var("OLR_DATA_DIR") {
            config.data_dir = PathBuf::from(dir);
        }

        config
    }

    /// 配置文件路径
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openlocalrouter")
            .join("config.json")
    }

    /// 数据库路径
    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("openlocalrouter.db")
    }
}
