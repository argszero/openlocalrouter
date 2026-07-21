//! 端点管理 CRUD

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::admin::auth::AuthContext;
use crate::db::dao::EndpointRow;
use crate::db::Database;
use crate::error::AppError;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CreateEndpointRequest {
    pub name: String,
    pub path_prefix: String,
    pub protocol: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEndpointRequest {
    pub name: Option<String>,
    pub path_prefix: Option<String>,
    pub protocol: Option<String>,
    pub enabled: Option<bool>,
}

/// GET /api/admin/endpoints
pub async fn list_endpoints(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
) -> Result<Json<serde_json::Value>, AppError> {
    let my_endpoints = if auth.is_admin {
        db.list_endpoints().await?
    } else {
        db.list_endpoints_for_user(&auth.user_id).await?
    };

    // Get shared endpoints (via keys assigned to me where I'm not the creator)
    let shared: Vec<EndpointRow> = db
        .with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT e.id, e.user_id, e.name, e.listen_path, e.protocol, e.enabled, e.created_at, e.updated_at
                 FROM endpoints e
                 JOIN endpoint_api_keys k ON k.endpoint_id = e.id
                 WHERE k.assigned_to = ?1 AND k.created_by != ?1 AND e.user_id != ?1",
            )?;
            let rows = stmt.query_map([&auth.user_id], |row| {
                Ok(EndpointRow {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    listen_path: row.get(3)?,
                    protocol: row.get(4)?,
                    enabled: row.get::<_, i32>(5)? != 0,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await?;

    let result: Vec<serde_json::Value> = my_endpoints
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "user_id": e.user_id,
                "name": e.name,
                "listen_path": e.listen_path,
                "protocol": e.protocol,
                "enabled": e.enabled,
                "created_at": e.created_at,
                "updated_at": e.updated_at,
                "is_mine": true,
                "shared_by": null,
            })
        })
        .chain(shared.into_iter().map(|e| {
            serde_json::json!({
                "id": e.id,
                "user_id": e.user_id,
                "name": e.name,
                "listen_path": e.listen_path,
                "protocol": e.protocol,
                "enabled": e.enabled,
                "created_at": e.created_at,
                "updated_at": e.updated_at,
                "is_mine": false,
                "shared_by": e.user_id,
            })
        }))
        .collect();

    Ok(Json(serde_json::json!(result)))
}

/// POST /api/admin/endpoints
pub async fn create_endpoint(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Json(req): Json<CreateEndpointRequest>,
) -> Result<Json<EndpointRow>, AppError> {
    // Clean the prefix — strip leading/trailing slashes to get just the last segment
    let prefix = req.path_prefix.trim_matches('/');
    let listen_path = format!("/u/{}/{}", auth.username, prefix);

    let row = EndpointRow {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: auth.user_id.clone(),
        name: req.name,
        listen_path,
        protocol: req.protocol,
        enabled: true,
        created_at: String::new(),
        updated_at: String::new(),
    };

    db.create_endpoint(&row).await?;
    let created = db.get_endpoint(&row.id).await?.unwrap();
    Ok(Json(created))
}

/// GET /api/admin/endpoints/:id
pub async fn get_endpoint(
    State(db): State<Arc<Database>>,
    Path(id): Path<String>,
) -> Result<Json<EndpointRow>, AppError> {
    let ep = db
        .get_endpoint(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("端点不存在".into()))?;
    Ok(Json(ep))
}

/// PUT /api/admin/endpoints/:id
pub async fn update_endpoint(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Path(id): Path<String>,
    Json(req): Json<UpdateEndpointRequest>,
) -> Result<Json<EndpointRow>, AppError> {
    let mut ep = db
        .get_endpoint(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("端点不存在".into()))?;

    if let Some(name) = req.name {
        ep.name = name;
    }
    if let Some(prefix) = req.path_prefix {
        let prefix = prefix.trim_matches('/');
        ep.listen_path = format!("/u/{}/{}", auth.username, prefix);
    }
    if let Some(protocol) = req.protocol {
        ep.protocol = protocol;
    }
    if let Some(enabled) = req.enabled {
        ep.enabled = enabled;
    }

    db.update_endpoint(&ep).await?;
    let updated = db.get_endpoint(&id).await?.unwrap();
    Ok(Json(updated))
}

/// DELETE /api/admin/endpoints/:id
pub async fn delete_endpoint(
    State(db): State<Arc<Database>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = db
        .get_endpoint(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("端点不存在".into()))?;
    db.delete_endpoint(&id).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
