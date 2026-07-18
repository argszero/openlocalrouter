use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// 统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("已存在: {0}")]
    AlreadyExists(String),

    #[error("验证失败: {0}")]
    Validation(String),

    #[error("请求无效: {0}")]
    BadRequest(String),

    #[error("{0}")]
    Message(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Io(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Serde(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::AlreadyExists(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Message(msg) => {
                // 使用 401 作为认证/授权失败的默认状态码
                let status = if msg.contains("用户名或密码错误") || msg.contains("已被禁用")
                {
                    StatusCode::UNAUTHORIZED
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };
                (status, msg.clone())
            }
        };

        let body = serde_json::json!({
            "error": {
                "message": message,
                "code": status.as_u16()
            }
        });

        (status, axum::Json(body)).into_response()
    }
}
