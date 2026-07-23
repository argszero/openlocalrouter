//! Provider 管理 CRUD + Model CRUD

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::admin::auth::AuthContext;
use crate::db::dao::{ModelRow, ProviderRow};
use crate::db::Database;
use crate::error::AppError;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub api_types: Vec<String>,
    pub api_urls: Option<HashMap<String, String>>,
    pub extra_config: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_types: Option<Vec<String>>,
    pub enabled: Option<bool>,
    pub api_urls: Option<HashMap<String, String>>,
    pub extra_config: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateModelRequest {
    pub slug: String,
    pub display_name: String,
    pub context_window: Option<i64>,
    pub model_slug: Option<String>,
    pub visible_endpoint_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ProviderDetail {
    #[serde(flatten)]
    pub provider: ProviderRow,
    pub models: Vec<ModelWithVisibility>,
}

#[derive(Debug, Serialize)]
pub struct ModelWithVisibility {
    #[serde(flatten)]
    pub model: ModelRow,
    pub visible_endpoint_ids: Vec<String>,
}

/// Build `extra_config` JSON string, merging `api_urls` into the provided base config.
fn build_extra_config(
    base_extra_config: Option<&str>,
    api_urls: Option<&HashMap<String, String>>,
) -> String {
    let base: serde_json::Value = base_extra_config
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::json!({}));
    let mut config = match base {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    if let Some(urls) = api_urls {
        if !urls.is_empty() {
            config.insert("api_urls".into(), serde_json::json!(urls));
        } else {
            config.remove("api_urls");
        }
    } else {
        config.remove("api_urls");
    }
    serde_json::to_string(&config).unwrap_or_else(|_| "{}".into())
}

/// Parse `api_urls` from `extra_config` JSON string
fn parse_api_urls(extra_config: &str) -> Option<HashMap<String, String>> {
    let v: serde_json::Value = serde_json::from_str(extra_config).ok()?;
    v.get("api_urls")
        .and_then(|u| serde_json::from_value(u.clone()).ok())
}

/// Enrich a provider JSON response with `api_types` array and `api_urls`
fn enrich_provider_json(p: &ProviderRow) -> serde_json::Value {
    let api_types: Vec<&str> = p.api_type.split(',').filter(|s| !s.is_empty()).collect();
    let mut val = serde_json::to_value(p).unwrap_or_default();
    if let Some(obj) = val.as_object_mut() {
        obj.insert("api_types".into(), serde_json::json!(api_types));
        if let Some(urls) = parse_api_urls(&p.extra_config) {
            obj.insert("api_urls".into(), serde_json::json!(urls));
        }
    }
    val
}

/// GET /api/admin/providers
pub async fn list_providers(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let providers = if auth.is_admin {
        db.list_providers().await?
    } else {
        db.list_providers_for_user(&auth.user_id).await?
    };
    let result: Vec<serde_json::Value> = providers
        .into_iter()
        .map(|p| enrich_provider_json(&p))
        .collect();
    Ok(Json(result))
}

/// POST /api/admin/providers
pub async fn create_provider(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Json(req): Json<CreateProviderRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let extra_config = build_extra_config(req.extra_config.as_deref(), req.api_urls.as_ref());

    let row = ProviderRow {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: auth.user_id.clone(),
        name: req.name,
        base_url: req.base_url,
        api_key: req.api_key,
        api_type: req.api_types.join(","),
        enabled: true,
        extra_config,
        created_at: String::new(),
        updated_at: String::new(),
    };

    db.create_provider(&row).await?;
    let created = db.get_provider(&row.id).await?.unwrap();
    Ok(Json(enrich_provider_json(&created)))
}

/// GET /api/admin/providers/:id
pub async fn get_provider(
    State(db): State<Arc<Database>>,
    Path(id): Path<String>,
) -> Result<Json<ProviderDetail>, AppError> {
    let provider = db
        .get_provider(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("Provider 不存在".into()))?;

    let models = db.list_models_for_provider(&id).await?;
    let models_with_vis: Vec<ModelWithVisibility> =
        futures::future::join_all(models.iter().map(|m| async {
            let vis = db.get_model_visibility(&m.id).await.unwrap_or_default();
            ModelWithVisibility {
                model: m.clone(),
                visible_endpoint_ids: vis,
            }
        }))
        .await;

    Ok(Json(ProviderDetail {
        provider,
        models: models_with_vis,
    }))
}

/// PUT /api/admin/providers/:id
pub async fn update_provider(
    State(db): State<Arc<Database>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProviderRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut p = db
        .get_provider(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("Provider 不存在".into()))?;

    if let Some(name) = req.name {
        p.name = name;
    }
    if let Some(base_url) = req.base_url {
        p.base_url = base_url;
    }
    if let Some(api_key) = req.api_key {
        p.api_key = api_key;
    }
    if let Some(api_types) = req.api_types {
        p.api_type = api_types.join(",");
    }
    if let Some(enabled) = req.enabled {
        p.enabled = enabled;
    }

    // Merge api_urls into extra_config
    if req.api_urls.is_some() || req.extra_config.is_some() {
        p.extra_config = build_extra_config(
            req.extra_config.as_deref().or(Some(&p.extra_config)),
            req.api_urls.as_ref(),
        );
    } else {
        // Preserve existing extra_config and api_urls; just re-serialize to include api_urls
        let current_urls = parse_api_urls(&p.extra_config);
        if current_urls.is_some() {
            p.extra_config = build_extra_config(None, current_urls.as_ref());
        }
    }

    db.update_provider(&p).await?;
    let updated = db.get_provider(&id).await?.unwrap();

    Ok(Json(enrich_provider_json(&updated)))
}

/// DELETE /api/admin/providers/:id
pub async fn delete_provider(
    State(db): State<Arc<Database>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = db
        .get_provider(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("Provider 不存在".into()))?;
    db.delete_provider(&id).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// GET /api/admin/providers/:id/models
pub async fn list_models(
    State(db): State<Arc<Database>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let models = db.list_models_for_provider(&id).await?;
    let data: Vec<serde_json::Value> = futures::future::join_all(models.iter().map(|m| {
        let m_clone = m.clone();
        let vis_db = db.clone();
        async move {
            let vis = vis_db
                .get_model_visibility(&m_clone.id)
                .await
                .unwrap_or_default();
            let mut val = serde_json::to_value(&m_clone).unwrap_or_default();
            if let Some(obj) = val.as_object_mut() {
                obj.insert("visible_endpoint_ids".into(), serde_json::json!(vis));
            }
            val
        }
    }))
    .await;
    Ok(Json(serde_json::json!({
        "models": data,
        "count": data.len(),
    })))
}

/// POST /api/admin/providers/:id/models
pub async fn create_model(
    State(db): State<Arc<Database>>,
    Path(id): Path<String>,
    Json(req): Json<CreateModelRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let extra_config = if let Some(ref ms) = req.model_slug {
        if ms != &req.slug {
            serde_json::json!({"model_slug": ms}).to_string()
        } else {
            "{}".into()
        }
    } else {
        "{}".into()
    };

    let row = ModelRow {
        id: uuid::Uuid::new_v4().to_string(),
        provider_id: id.clone(),
        slug: req.slug,
        display_name: req.display_name,
        context_window: req.context_window.unwrap_or(128000),
        extra_config,
    };

    db.create_model(&row).await?;

    if let Some(ref visible_endpoint_ids) = req.visible_endpoint_ids {
        if !visible_endpoint_ids.is_empty() {
            db.set_model_visibility(&row.id, visible_endpoint_ids)
                .await?;
        }
    }

    let created = db.list_models_for_provider(&id).await?;
    let found = created
        .into_iter()
        .find(|m| m.id == row.id)
        .ok_or_else(|| AppError::Message("创建后查询失败".into()))?;
    Ok(Json(serde_json::json!({
        "id": found.id,
        "provider_id": found.provider_id,
        "slug": found.slug,
        "display_name": found.display_name,
        "context_window": found.context_window,
    })))
}

/// `DELETE /api/admin/providers/:id/models/:model_id`
pub async fn delete_model(
    State(db): State<Arc<Database>>,
    Path((_provider_id, model_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    // delete from models table (visibility cascades)
    db.with_conn(move |conn| {
        conn.execute("DELETE FROM models WHERE id = ?1", [&model_id])?;
        Ok(())
    })
    .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// `PUT /api/admin/providers/:id/models/:model_id/visibility`
#[derive(Debug, Deserialize)]
pub struct SetVisibilityRequest {
    pub endpoint_ids: Vec<String>,
}

pub async fn set_model_visibility(
    State(db): State<Arc<Database>>,
    Path((_provider_id, model_id)): Path<(String, String)>,
    Json(req): Json<SetVisibilityRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    db.set_model_visibility(&model_id, &req.endpoint_ids)
        .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
