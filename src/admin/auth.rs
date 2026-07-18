//! 管理 API 认证
//!
//! 提供 login/logout 端点和 auth 中间件（Bearer session token 验证）。

use axum::{
    extract::State,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::db::Database;
use std::sync::Arc;

// ── Request / Response types ──────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
}

/// 认证后注入的上下文
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: String,
    #[allow(dead_code)]
    pub username: String,
    pub is_admin: bool,
}

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": {"message": "未认证", "code": 401}})),
                )
            })
    }
}

/// Session 有效期（24 小时）
const SESSION_TTL_HOURS: i64 = 24;

// ── Handlers ──────────────────────────────────────────

/// POST /api/admin/login
pub async fn login_handler(
    State(db): State<Arc<Database>>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, crate::error::AppError> {
    let user = db
        .get_user_by_username(&req.username)
        .await?
        .ok_or_else(|| crate::error::AppError::Message("用户名或密码错误".into()))?;

    if !user.enabled {
        return Err(crate::error::AppError::Message("用户已被禁用".into()));
    }

    let valid = auth::verify_password(&req.password, &user.password_hash)?;
    if !valid {
        return Err(crate::error::AppError::Message("用户名或密码错误".into()));
    }

    // 生成 session
    let token = auth::generate_session_token();
    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(SESSION_TTL_HOURS))
        .unwrap()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    db.create_session(&crate::db::dao::SessionRow {
        token: token.clone(),
        user_id: user.id.clone(),
        created_at: String::new(),
        expires_at,
    })
    .await?;

    Ok(Json(LoginResponse {
        token,
        user: UserInfo {
            id: user.id,
            username: user.username,
            is_admin: user.is_admin,
        },
    }))
}

/// POST /api/admin/logout
pub async fn logout_handler(State(_db): State<Arc<Database>>) -> impl IntoResponse {
    // TODO: delete session token from header
    Json(serde_json::json!({ "ok": true }))
}

// ── Auth Middleware ───────────────────────────────────

/// 验证 Bearer session token，注入 AuthContext 到 request extensions。
#[allow(dead_code)]
pub async fn require_auth(
    State(db): State<Arc<Database>>,
    mut request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = header.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": {"message": "未提供认证令牌", "code": 401}})),
        )
    })?;

    let session = db
        .get_session(token)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": {"message": "服务错误", "code": 500}})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": {"message": "无效的认证令牌", "code": 401}})),
            )
        })?;

    // 检查是否过期
    let now = chrono::Utc::now().naive_utc();
    let expires = chrono::NaiveDateTime::parse_from_str(&session.expires_at, "%Y-%m-%d %H:%M:%S")
        .unwrap_or(now);
    if now > expires {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": {"message": "认证令牌已过期", "code": 401}})),
        ));
    }

    // 查询用户信息
    let user = db
        .get_user_by_id(&session.user_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": {"message": "服务错误", "code": 500}})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": {"message": "用户不存在", "code": 401}})),
            )
        })?;

    request.extensions_mut().insert(AuthContext {
        user_id: user.id,
        username: user.username,
        is_admin: user.is_admin,
    });

    Ok(next.run(request).await)
}
