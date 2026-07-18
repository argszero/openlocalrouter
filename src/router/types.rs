use crate::db::dao::{EndpointRow, ProviderRow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// API 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiProtocol {
    OpenAiChat,
    OpenAiResponses,
    AnthropicMessages,
}

impl ApiProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiProtocol::OpenAiChat => "openai_chat",
            ApiProtocol::OpenAiResponses => "openai_responses",
            ApiProtocol::AnthropicMessages => "anthropic_messages",
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "openai_chat" => Some(ApiProtocol::OpenAiChat),
            "openai_responses" => Some(ApiProtocol::OpenAiResponses),
            "anthropic_messages" => Some(ApiProtocol::AnthropicMessages),
            _ => None,
        }
    }
}

impl std::fmt::Display for ApiProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 代理服务器共享状态
#[derive(Clone)]
pub struct ProxyState {
    /// 数据库连接
    pub db: std::sync::Arc<crate::db::Database>,
    /// 前端静态文件目录
    pub frontend_dir: PathBuf,
}

/// 匹配到的端点 + Provider 信息
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RouteMatch {
    pub endpoint: EndpointRow,
    pub provider: ProviderRow,
    pub model: String,
}

/// 代理服务器配置
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProxyConfig {
    pub listen_address: String,
    pub listen_port: u16,
}
