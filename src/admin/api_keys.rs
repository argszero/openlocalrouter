//! API Key 管理 CRUD

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::admin::auth::AuthContext;
use crate::auth;
use crate::db::dao::EndpointApiKeyRow;
use crate::db::Database;
use crate::error::AppError;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreatedApiKey {
    pub key: String,
    #[serde(flatten)]
    pub row: EndpointApiKeyRow,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub assigned_to: Option<String>,
}

/// GET /api/admin/endpoints/:id/keys
pub async fn list_api_keys(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Path(endpoint_id): Path<String>,
) -> Result<Json<Vec<EndpointApiKeyRow>>, AppError> {
    // Non-admin users can only see keys they created or are assigned to
    let user_filter = if auth.is_admin {
        None
    } else {
        Some(auth.user_id.as_str())
    };
    let keys = db
        .list_api_keys_for_endpoint(&endpoint_id, user_filter)
        .await?;
    Ok(Json(keys))
}

/// POST /api/admin/endpoints/:id/keys
pub async fn create_api_key(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Path(endpoint_id): Path<String>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<CreatedApiKey>, AppError> {
    let _ = db
        .get_endpoint(&endpoint_id)
        .await?
        .ok_or_else(|| AppError::NotFound("端点不存在".into()))?;

    let assigned_to = req.assigned_to.unwrap_or_else(|| auth.user_id.clone());

    let (raw_key, prefix) = auth::generate_api_key();
    let hash = auth::hash_api_key(&raw_key);

    let row = EndpointApiKeyRow {
        id: uuid::Uuid::new_v4().to_string(),
        endpoint_id: endpoint_id.clone(),
        user_id: assigned_to.clone(), // deprecated, kept for compat
        created_by: auth.user_id.clone(),
        assigned_to,
        name: req.name,
        key_value: raw_key.clone(),
        key_hash: hash,
        key_prefix: prefix,
        enabled: true,
        created_at: String::new(),
        last_used_at: None,
    };

    db.create_api_key(&row).await?;

    Ok(Json(CreatedApiKey {
        key: raw_key,
        row: db
            .list_api_keys_for_endpoint(&endpoint_id, None)
            .await?
            .into_iter()
            .find(|k| k.id == row.id)
            .unwrap(),
    }))
}

/// PUT /api/admin/endpoints/:id/keys/:key_id
pub async fn update_api_key(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Path((_endpoint_id, key_id)): Path<(String, String)>,
    Json(req): Json<UpdateApiKeyRequest>,
) -> Result<Json<EndpointApiKeyRow>, AppError> {
    // Verify ownership: only admin or key creator/assignee can update.
    // Assignees can update name/enabled but NOT reassign to another user.
    let mut is_assignee_only = false;
    if !auth.is_admin {
        let key = db
            .get_api_key_by_id(&key_id)
            .await?
            .ok_or_else(|| AppError::NotFound("API Key 不存在".into()))?;
        if key.created_by != auth.user_id && key.assigned_to != auth.user_id {
            return Err(AppError::Message("无权修改此 API Key".into()));
        }
        if key.created_by != auth.user_id && key.assigned_to == auth.user_id {
            is_assignee_only = true;
        }
    }

    let assigned_to = if is_assignee_only {
        None
    } else {
        req.assigned_to.as_deref()
    };

    db.update_api_key(&key_id, req.name.as_deref(), req.enabled, assigned_to)
        .await?;

    // use direct query since get_api_key_by_value is by key_value
    let keys = db
        .with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, endpoint_id, user_id, created_by, assigned_to, name, key_value, key_hash, key_prefix, enabled,
                    created_at, last_used_at
             FROM endpoint_api_keys WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map([&key_id], |row| {
                Ok(EndpointApiKeyRow {
                    id: row.get(0)?,
                    endpoint_id: row.get(1)?,
                    user_id: row.get::<_, String>(2).unwrap_or_default(),
                    created_by: row.get::<_, String>(3).unwrap_or_default(),
                    assigned_to: row.get::<_, String>(4).unwrap_or_default(),
                    name: row.get::<_, String>(5).unwrap_or_default(),
                    key_value: row.get::<_, String>(6).unwrap_or_default(),
                    key_hash: row.get::<_, String>(7).unwrap_or_default(),
                    key_prefix: row.get::<_, String>(8).unwrap_or_default(),
                    enabled: row.get::<_, i32>(9)? != 0,
                    created_at: row.get::<_, String>(10).unwrap_or_default(),
                    last_used_at: row.get::<_, Option<String>>(11).ok().flatten(),
                })
            })?;
            Ok(rows
                .next()
                .transpose()?
                .ok_or(AppError::NotFound("API Key 不存在".into())))
        })
        .await?;

    Ok(Json(keys?))
}

/// DELETE /api/admin/endpoints/:id/keys/:key_id
pub async fn delete_api_key(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Path((_endpoint_id, key_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify ownership: only admin or key creator/assignee can delete
    if !auth.is_admin {
        let key = db
            .get_api_key_by_id(&key_id)
            .await?
            .ok_or_else(|| AppError::NotFound("API Key 不存在".into()))?;
        if key.created_by != auth.user_id && key.assigned_to != auth.user_id {
            return Err(AppError::Message("无权删除此 API Key".into()));
        }
    }

    db.delete_api_key(&key_id).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
