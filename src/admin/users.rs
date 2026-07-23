//! 用户管理 CRUD

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::admin::auth::AuthContext;
use crate::auth;
use crate::db::dao::UserRow;
use crate::db::Database;
use crate::error::AppError;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub is_admin: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub is_admin: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<UserRow> for UserResponse {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            is_admin: row.is_admin,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// GET /api/admin/users
pub async fn list_users(
    State(db): State<Arc<Database>>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let users = db.list_users().await?;
    Ok(Json(users.into_iter().map(UserResponse::from).collect()))
}

/// POST /api/admin/users
pub async fn create_user(
    State(db): State<Arc<Database>>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let password_hash = auth::hash_password(&req.password)?;

    let row = UserRow {
        id: uuid::Uuid::new_v4().to_string(),
        username: req.username,
        password_hash,
        is_admin: req.is_admin.unwrap_or(false),
        enabled: true,
        created_at: String::new(),
        updated_at: String::new(),
    };

    db.create_user(&row).await?;
    let created = db.get_user_by_id(&row.id).await?.unwrap();
    Ok(Json(created.into()))
}

/// PUT /api/admin/users/:id
pub async fn update_user(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Path(id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let _user = db
        .get_user_by_id(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".into()))?;

    // 非管理员只能修改自己
    if !auth.is_admin && auth.user_id != id {
        return Err(AppError::Message("无权修改其他用户".into()));
    }

    let id2 = id.clone();
    db.with_conn(move |conn| {
        if let Some(ref username) = req.username {
            conn.execute(
                "UPDATE users SET username=?1 WHERE id=?2",
                rusqlite::params![username, id2],
            )?;
        }
        if let Some(ref password) = req.password {
            let hash = auth::hash_password(password)
                .map_err(|_| rusqlite::Error::InvalidParameterName("hash".into()))?;
            conn.execute(
                "UPDATE users SET password_hash=?1 WHERE id=?2",
                rusqlite::params![hash, id2],
            )?;
        }
        if let Some(is_admin) = req.is_admin {
            conn.execute(
                "UPDATE users SET is_admin=?1 WHERE id=?2",
                rusqlite::params![i32::from(is_admin), id2],
            )?;
        }
        if let Some(enabled) = req.enabled {
            conn.execute(
                "UPDATE users SET enabled=?1 WHERE id=?2",
                rusqlite::params![i32::from(enabled), id2],
            )?;
        }
        conn.execute(
            "UPDATE users SET updated_at=datetime('now') WHERE id=?1",
            [&id2],
        )?;
        Ok::<_, AppError>(())
    })
    .await?;

    let _updated = db.get_user_by_id(&id).await?.unwrap();

    let updated = db.get_user_by_id(&id).await?.unwrap();
    Ok(Json(updated.into()))
}

/// DELETE /api/admin/users/:id
pub async fn delete_user(
    State(db): State<Arc<Database>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = db
        .get_user_by_id(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    db.with_conn(move |conn| {
        conn.execute("DELETE FROM users WHERE id = ?1", [&id])?;
        Ok::<_, AppError>(())
    })
    .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
