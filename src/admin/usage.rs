//! 用量查询和聚合 API

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::admin::auth::AuthContext;
use crate::db::dao::{SharedUsageSummary, TimeSeriesBreakdown, TimeSeriesPoint, UsageAggregateRow, UsageRecordRow};
use crate::db::Database;
use crate::error::AppError;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    pub api_key_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct UsageListResponse {
    pub records: Vec<UsageRecordRow>,
    pub total: i64,
}

#[derive(Debug, Deserialize)]
pub struct SummaryQuery {
    pub group_by: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SummaryResponse {
    pub groups: Vec<UsageAggregateRow>,
}

/// GET /api/admin/usage
pub async fn list_usage(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<UsageQuery>,
) -> Result<Json<UsageListResponse>, AppError> {
    let (user_id_filter, api_key_id_filter) = if auth.is_admin {
        (Some(auth.user_id.as_str()), q.api_key_id.as_deref())
    } else {
        (Some(auth.user_id.as_str()), None)
    };

    let (records, total) = db
        .list_usage_records(
            api_key_id_filter,
            q.endpoint_id.as_deref(),
            user_id_filter,
            None, // key_owner_id
            q.from.as_deref(),
            q.to.as_deref(),
            q.limit.unwrap_or(50),
            q.offset.unwrap_or(0),
        )
        .await?;

    Ok(Json(UsageListResponse { records, total }))
}

/// GET /api/admin/usage/summary
pub async fn usage_summary(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<SummaryQuery>,
) -> Result<Json<SummaryResponse>, AppError> {
    let groups = db
        .aggregate_usage(
            None,
            Some(&auth.user_id),
            false,
            &q.group_by,
            q.from.as_deref(),
            q.to.as_deref(),
        )
        .await?;

    Ok(Json(SummaryResponse { groups }))
}

/// GET /api/admin/keys/:id/usage
pub async fn key_usage(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    axum::extract::Path(key_id): axum::extract::Path<String>,
    Query(q): Query<UsageQuery>,
) -> Result<Json<UsageListResponse>, AppError> {
    let user_id = if auth.is_admin {
        None
    } else {
        Some(auth.user_id.as_str())
    };

    let (records, total) = db
        .list_usage_records(
            Some(&key_id),
            q.endpoint_id.as_deref(),
            user_id,
            None,
            q.from.as_deref(),
            q.to.as_deref(),
            q.limit.unwrap_or(50),
            q.offset.unwrap_or(0),
        )
        .await?;

    Ok(Json(UsageListResponse { records, total }))
}

// ── My Usage ──────────────────────────────────────────

/// GET /api/admin/usage/my/summary
pub async fn my_usage_summary(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<SummaryQuery>,
) -> Result<Json<SummaryResponse>, AppError> {
    let groups = db
        .aggregate_usage(
            None,
            Some(&auth.user_id),
            false,
            &q.group_by,
            q.from.as_deref(),
            q.to.as_deref(),
        )
        .await?;

    Ok(Json(SummaryResponse { groups }))
}

/// GET /api/admin/usage/my/trend
pub async fn my_usage_trend(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<TimeSeriesQuery>,
) -> Result<Json<TimeSeriesResponse>, AppError> {
    let points = db
        .time_series(
            Some(&auth.user_id),
            None,
            false,
            q.granularity.as_deref().unwrap_or("day"),
            q.from.as_deref(),
            q.to.as_deref(),
        )
        .await?;

    Ok(Json(TimeSeriesResponse { points }))
}

/// GET /api/admin/usage/my/trend-breakdown?group_by=model|key
pub async fn my_usage_trend_breakdown(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<TrendBreakdownQuery>,
) -> Result<Json<TrendBreakdownResponse>, AppError> {
    let points = db
        .time_series_breakdown(
            &auth.user_id,
            q.group_by.as_deref().unwrap_or("model"),
            q.from.as_deref(),
            q.to.as_deref(),
        )
        .await?;

    Ok(Json(TrendBreakdownResponse { points }))
}

/// GET /api/admin/usage/my/records
pub async fn my_usage_records(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<UsageQuery>,
) -> Result<Json<UsageListResponse>, AppError> {
    let (records, total) = db
        .list_usage_records(
            q.api_key_id.as_deref(),
            q.endpoint_id.as_deref(),
            Some(&auth.user_id),
            None,
            q.from.as_deref(),
            q.to.as_deref(),
            q.limit.unwrap_or(50),
            q.offset.unwrap_or(0),
        )
        .await?;

    Ok(Json(UsageListResponse { records, total }))
}

// ── Shared Usage ───────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TimeSeriesQuery {
    pub granularity: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TimeSeriesResponse {
    pub points: Vec<TimeSeriesPoint>,
}

#[derive(Debug, Deserialize)]
pub struct TrendBreakdownQuery {
    pub group_by: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TrendBreakdownResponse {
    pub points: Vec<TimeSeriesBreakdown>,
}

#[derive(Debug, Deserialize)]
pub struct TopQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub rank_by: Option<String>,
    pub limit: Option<u32>,
}

/// GET /api/admin/usage/shared/summary
pub async fn shared_summary(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<SummaryQuery>,
) -> Result<Json<SharedUsageSummary>, AppError> {
    let summary = db
        .shared_usage_summary(&auth.user_id, q.from.as_deref(), q.to.as_deref())
        .await?;
    Ok(Json(summary))
}

/// GET /api/admin/usage/shared/trend
pub async fn shared_trend(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<TimeSeriesQuery>,
) -> Result<Json<TimeSeriesResponse>, AppError> {
    let points = db
        .time_series(
            None,
            Some(&auth.user_id),
            true,
            q.granularity.as_deref().unwrap_or("day"),
            q.from.as_deref(),
            q.to.as_deref(),
        )
        .await?;

    Ok(Json(TimeSeriesResponse { points }))
}

/// GET /api/admin/usage/shared/top
pub async fn shared_top(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<TopQuery>,
) -> Result<Json<SummaryResponse>, AppError> {
    let group_by = q.rank_by.as_deref().unwrap_or("model");
    let groups = db
        .aggregate_usage(
            Some(&auth.user_id),
            None,
            true, // exclude self
            group_by,
            q.from.as_deref(),
            q.to.as_deref(),
        )
        .await?;

    Ok(Json(SummaryResponse {
        groups: groups.into_iter().take(q.limit.unwrap_or(10) as usize).collect(),
    }))
}

/// GET /api/admin/usage/shared/keys — key health status
pub async fn shared_keys(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
) -> Result<Json<serde_json::Value>, AppError> {
    let keys = db.list_my_keys(&auth.user_id).await?;
    // Return keys with usage info
    let result: Vec<serde_json::Value> = keys
        .into_iter()
        .map(|k| {
            serde_json::json!({
                "id": k.id,
                "name": k.name,
                "assigned_to": k.assigned_to,
                "last_used_at": k.last_used_at,
                "created_by": k.created_by,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "keys": result })))
}

/// GET /api/admin/usage/shared/records
pub async fn shared_records(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
    Query(q): Query<UsageQuery>,
) -> Result<Json<UsageListResponse>, AppError> {
    let (records, total) = db
        .list_usage_records(
            q.api_key_id.as_deref(),
            q.endpoint_id.as_deref(),
            None,
            Some(&auth.user_id),
            q.from.as_deref(),
            q.to.as_deref(),
            q.limit.unwrap_or(50),
            q.offset.unwrap_or(0),
        )
        .await?;

    Ok(Json(UsageListResponse { records, total }))
}
