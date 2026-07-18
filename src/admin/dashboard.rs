//! 仪表板统计

use axum::{extract::State, Json};
use serde::Serialize;

use crate::admin::auth::AuthContext;
use crate::db::Database;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub my_providers: i64,
    pub my_endpoints: i64,
    pub my_keys: i64,
    pub keys_assigned_to_others: i64,
    pub keys_assigned_to_me: i64,
    pub shared_endpoints: i64,
    pub today_my_tokens: i64,
    pub today_shared_tokens: i64,
}

/// GET /api/admin/dashboard
pub async fn dashboard_handler(
    State(db): State<Arc<Database>>,
    auth: AuthContext,
) -> Result<Json<DashboardStats>, crate::error::AppError> {
    let user_id = auth.user_id.clone();

    let stats = db
        .with_conn(move |conn| {
            let my_providers: i64 =
                conn.query_row("SELECT COUNT(*) FROM providers WHERE user_id = ?1", [&user_id], |r| r.get(0))?;
            let my_endpoints: i64 =
                conn.query_row("SELECT COUNT(*) FROM endpoints WHERE user_id = ?1", [&user_id], |r| r.get(0))?;
            let my_keys: i64 =
                conn.query_row("SELECT COUNT(*) FROM endpoint_api_keys WHERE created_by = ?1", [&user_id], |r| r.get(0))?;
            let keys_assigned_to_others: i64 =
                conn.query_row("SELECT COUNT(*) FROM endpoint_api_keys WHERE created_by = ?1 AND assigned_to != created_by", [&user_id], |r| r.get(0))?;
            let keys_assigned_to_me: i64 =
                conn.query_row("SELECT COUNT(*) FROM endpoint_api_keys WHERE assigned_to = ?1 AND created_by != assigned_to", [&user_id], |r| r.get(0))?;
            let shared_endpoints: i64 = conn.query_row(
                "SELECT COUNT(DISTINCT e.id) FROM endpoints e JOIN endpoint_api_keys k ON k.endpoint_id = e.id WHERE k.assigned_to = ?1 AND k.created_by != ?1 AND e.user_id != ?1",
                [&user_id], |r| r.get(0),
            )?;
            let today_my_tokens: i64 = conn.query_row(
                "SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM usage_records WHERE user_id = ?1 AND date(created_at) = date('now')",
                [&user_id], |r| r.get(0),
            )?;
            let today_shared_tokens: i64 = conn.query_row(
                "SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM usage_records WHERE key_owner_id = ?1 AND user_id != key_owner_id AND date(created_at) = date('now')",
                [&user_id], |r| r.get(0),
            )?;
            Ok(DashboardStats {
                my_providers,
                my_endpoints,
                my_keys,
                keys_assigned_to_others,
                keys_assigned_to_me,
                shared_endpoints,
                today_my_tokens,
                today_shared_tokens,
            })
        })
        .await?;

    Ok(Json(stats))
}
