use crate::db::Database;
use crate::error::AppError;
use serde::{Deserialize, Serialize};

// ── Types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointRow {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub listen_path: String,
    pub protocol: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRow {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub api_type: String,
    pub enabled: bool,
    pub extra_config: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRow {
    pub id: String,
    pub provider_id: String,
    pub slug: String,
    pub display_name: String,
    pub context_window: i64,
    pub extra_config: String,
}

/// 聚合视图：模型 + 对其可见的端点列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWithVisibility {
    #[serde(flatten)]
    pub model: ModelRow,
    pub visible_endpoint_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRow {
    pub token: String,
    pub user_id: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointApiKeyRow {
    pub id: String,
    pub endpoint_id: String,
    pub user_id: String,     // deprecated, kept for compat
    pub created_by: String,  // who owns this key
    pub assigned_to: String, // who can use this key
    pub name: String,
    pub key_value: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub enabled: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecordRow {
    pub id: String,
    pub api_key_id: String,
    pub key_owner_id: String,
    pub endpoint_id: String,
    pub user_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageAggregateRow {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub count: i64,
}

/// Time-series data point for trend charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub count: i64,
}

/// Time-series breakdown by model or key name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesBreakdown {
    pub timestamp: String,
    pub group_key: String,
    pub total_tokens: i64,
    pub count: i64,
}

/// Shared usage overview stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedUsageSummary {
    pub today_tokens: i64,
    pub yesterday_tokens: i64,
    pub trend_pct: f64,
    pub active_keys: i64,
    pub total_keys: i64,
    pub active_users: i64,
}

// ── Helper: row → EndpointApiKeyRow ──────────────────

fn key_row_from_row(row: &rusqlite::Row) -> rusqlite::Result<EndpointApiKeyRow> {
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
}

// ── User DAO ──────────────────────────────────────────

impl Database {
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn create_user(&self, row: &UserRow) -> Result<(), AppError> {
        let id = row.id.clone();
        let username = row.username.clone();
        let password_hash = row.password_hash.clone();
        let is_admin = i32::from(row.is_admin);
        let enabled = i32::from(row.enabled);

        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO users (id, username, password_hash, is_admin, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![id, username, password_hash, is_admin, enabled],
            )?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<UserRow>, AppError> {
        let username = username.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, username, password_hash, is_admin, enabled, created_at, updated_at
                 FROM users WHERE username = ?1",
            )?;
            let mut rows = stmt.query_map([&username], |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                    is_admin: row.get::<_, i32>(3)? != 0,
                    enabled: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            Ok(rows.next().transpose()?)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn get_user_by_id(&self, id: &str) -> Result<Option<UserRow>, AppError> {
        let id = id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, username, password_hash, is_admin, enabled, created_at, updated_at
                 FROM users WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map([&id], |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                    is_admin: row.get::<_, i32>(3)? != 0,
                    enabled: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            Ok(rows.next().transpose()?)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_users(&self) -> Result<Vec<UserRow>, AppError> {
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, username, password_hash, is_admin, enabled, created_at, updated_at
                 FROM users ORDER BY created_at",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                    is_admin: row.get::<_, i32>(3)? != 0,
                    enabled: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn count_users(&self) -> Result<i64, AppError> {
        self.with_conn(move |conn| {
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
            Ok(count)
        })
        .await
    }

    // ── Session DAO ────────────────────────────────────

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn create_session(&self, row: &SessionRow) -> Result<(), AppError> {
        let token = row.token.clone();
        let user_id = row.user_id.clone();
        let expires_at = row.expires_at.clone();

        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO sessions (token, user_id, expires_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![token, user_id, expires_at],
            )?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn get_session(&self, token: &str) -> Result<Option<SessionRow>, AppError> {
        let token = token.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT token, user_id, created_at, expires_at FROM sessions WHERE token = ?1",
            )?;
            let mut rows = stmt.query_map([&token], |row| {
                Ok(SessionRow {
                    token: row.get(0)?,
                    user_id: row.get(1)?,
                    created_at: row.get(2)?,
                    expires_at: row.get(3)?,
                })
            })?;
            Ok(rows.next().transpose()?)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn delete_session(&self, token: &str) -> Result<(), AppError> {
        let token = token.to_string();
        self.with_conn(move |conn| {
            conn.execute("DELETE FROM sessions WHERE token = ?1", [&token])?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn cleanup_expired_sessions(&self) -> Result<(), AppError> {
        self.with_conn(move |conn| {
            conn.execute(
                "DELETE FROM sessions WHERE expires_at < datetime('now')",
                [],
            )?;
            Ok(())
        })
        .await
    }

    // ── Endpoint API Key DAO ───────────────────────────

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn create_api_key(&self, row: &EndpointApiKeyRow) -> Result<(), AppError> {
        let id = row.id.clone();
        let endpoint_id = row.endpoint_id.clone();
        let user_id = row.user_id.clone();
        let created_by = row.created_by.clone();
        let assigned_to = row.assigned_to.clone();
        let name = row.name.clone();
        let key_value = row.key_value.clone();
        let key_hash = row.key_hash.clone();
        let key_prefix = row.key_prefix.clone();
        let enabled = i32::from(row.enabled);

        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO endpoint_api_keys (id, endpoint_id, user_id, created_by, assigned_to, name, key_value, key_hash, key_prefix, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![id, endpoint_id, user_id, created_by, assigned_to, name, key_value, key_hash, key_prefix, enabled],
            )?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_api_keys_for_endpoint(
        &self,
        endpoint_id: &str,
        user_id_filter: Option<&str>,
    ) -> Result<Vec<EndpointApiKeyRow>, AppError> {
        let endpoint_id = endpoint_id.to_string();
        let user_id_filter = user_id_filter.map(std::string::ToString::to_string);
        self.with_conn(move |conn| {
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(endpoint_id)];
            let mut sql = "SELECT id, endpoint_id, user_id, created_by, assigned_to, name, key_value, key_hash, key_prefix, enabled, created_at, last_used_at
                           FROM endpoint_api_keys WHERE endpoint_id = ?1".to_string();
            if let Some(ref filter) = user_id_filter {
                sql.push_str(" AND (created_by = ?2 OR assigned_to = ?2)");
                params.push(Box::new(filter.clone()));
            }
            sql.push_str(" ORDER BY created_at");

            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(std::convert::AsRef::as_ref).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                key_row_from_row(row)
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn get_api_key_by_value(
        &self,
        key_value: &str,
    ) -> Result<Option<EndpointApiKeyRow>, AppError> {
        let key_value = key_value.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, endpoint_id, user_id, created_by, assigned_to, name, key_value, key_hash, key_prefix, enabled, created_at, last_used_at
                 FROM endpoint_api_keys WHERE key_value = ?1",
            )?;
            let mut rows = stmt.query_map([&key_value], |row| {
                key_row_from_row(row)
            })?;
            match rows.next() {
                Some(r) => Ok(Some(r?)),
                None => Ok(None),
            }
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn get_api_key_by_id(
        &self,
        key_id: &str,
    ) -> Result<Option<EndpointApiKeyRow>, AppError> {
        let key_id = key_id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, endpoint_id, user_id, created_by, assigned_to, name, key_value, key_hash, key_prefix, enabled, created_at, last_used_at
                 FROM endpoint_api_keys WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map([&key_id], |row| {
                key_row_from_row(row)
            })?;
            match rows.next() {
                Some(r) => Ok(Some(r?)),
                None => Ok(None),
            }
        })
        .await
    }

    /// List keys the current user owns OR has been assigned to
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_my_keys(&self, user_id: &str) -> Result<Vec<EndpointApiKeyRow>, AppError> {
        let user_id = user_id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, endpoint_id, user_id, created_by, assigned_to, name, key_value, key_hash, key_prefix, enabled, created_at, last_used_at
                 FROM endpoint_api_keys WHERE created_by = ?1 OR assigned_to = ?1 ORDER BY created_at",
            )?;
            let rows = stmt.query_map([&user_id], |row| {
                key_row_from_row(row)
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn update_api_key(
        &self,
        id: &str,
        name: Option<&str>,
        enabled: Option<bool>,
        assigned_to: Option<&str>,
    ) -> Result<(), AppError> {
        let id = id.to_string();
        let name = name.map(std::string::ToString::to_string);
        let enabled = enabled.map(i32::from);
        let assigned_to = assigned_to.map(std::string::ToString::to_string);

        self.with_conn(move |conn| {
            let mut sets: Vec<String> = Vec::new();
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(ref n) = name {
                sets.push(format!("name = ?{}", params.len() + 1));
                params.push(Box::new(n.clone()));
            }
            if let Some(e) = enabled {
                sets.push(format!("enabled = ?{}", params.len() + 1));
                params.push(Box::new(e));
            }
            if let Some(ref a) = assigned_to {
                sets.push(format!("assigned_to = ?{}", params.len() + 1));
                params.push(Box::new(a.clone()));
            }

            if sets.is_empty() {
                return Ok(());
            }

            params.push(Box::new(id.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();
            let sql = format!(
                "UPDATE endpoint_api_keys SET {} WHERE id = ?{}",
                sets.join(", "),
                params.len()
            );
            conn.execute(&sql, param_refs.as_slice())?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn delete_api_key(&self, id: &str) -> Result<(), AppError> {
        let id = id.to_string();
        self.with_conn(move |conn| {
            conn.execute("DELETE FROM endpoint_api_keys WHERE id = ?1", [&id])?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn touch_api_key(&self, id: &str) -> Result<(), AppError> {
        let id = id.to_string();
        self.with_conn(move |conn| {
            conn.execute(
                "UPDATE endpoint_api_keys SET last_used_at=datetime('now') WHERE id=?1",
                [&id],
            )?;
            Ok(())
        })
        .await
    }

    // ── Endpoint CRUD (with user_id) ────────────────────

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_endpoints(&self) -> Result<Vec<EndpointRow>, AppError> {
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, name, listen_path, protocol, enabled, created_at, updated_at
                 FROM endpoints ORDER BY created_at",
            )?;
            let rows = stmt.query_map([], |row| {
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
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_endpoints_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<EndpointRow>, AppError> {
        let user_id = user_id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, name, listen_path, protocol, enabled, created_at, updated_at
                 FROM endpoints WHERE user_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt.query_map([&user_id], |row| {
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
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn get_endpoint(&self, id: &str) -> Result<Option<EndpointRow>, AppError> {
        let id = id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, name, listen_path, protocol, enabled, created_at, updated_at
                 FROM endpoints WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map([&id], |row| {
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
            Ok(rows.next().transpose()?)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn get_endpoint_by_path(
        &self,
        listen_path: &str,
    ) -> Result<Option<EndpointRow>, AppError> {
        let listen_path = listen_path.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, name, listen_path, protocol, enabled, created_at, updated_at
                 FROM endpoints WHERE listen_path = ?1 AND enabled = 1",
            )?;
            let mut rows = stmt.query_map([&listen_path], |row| {
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
            Ok(rows.next().transpose()?)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn create_endpoint(&self, row: &EndpointRow) -> Result<(), AppError> {
        let id = row.id.clone();
        let user_id = row.user_id.clone();
        let name = row.name.clone();
        let listen_path = row.listen_path.clone();
        let protocol = row.protocol.clone();
        let enabled = i32::from(row.enabled);

        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO endpoints (id, user_id, name, listen_path, protocol, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![id, user_id, name, listen_path, protocol, enabled],
            )?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn update_endpoint(&self, row: &EndpointRow) -> Result<(), AppError> {
        let id = row.id.clone();
        let name = row.name.clone();
        let listen_path = row.listen_path.clone();
        let protocol = row.protocol.clone();
        let enabled = i32::from(row.enabled);

        self.with_conn(move |conn| {
            conn.execute(
                "UPDATE endpoints SET name=?2, listen_path=?3, protocol=?4, enabled=?5,
                 updated_at=datetime('now') WHERE id=?1",
                rusqlite::params![id, name, listen_path, protocol, enabled],
            )?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn delete_endpoint(&self, id: &str) -> Result<(), AppError> {
        let id = id.to_string();
        self.with_conn(move |conn| {
            conn.execute("DELETE FROM endpoints WHERE id = ?1", [&id])?;
            Ok(())
        })
        .await
    }

    // ── Provider CRUD (with user_id) ────────────────────

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_providers(&self) -> Result<Vec<ProviderRow>, AppError> {
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, name, base_url, api_key, api_type, enabled, extra_config, created_at, updated_at
                 FROM providers ORDER BY created_at",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(ProviderRow {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    base_url: row.get(3)?,
                    api_key: row.get(4)?,
                    api_type: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                    extra_config: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(AppError::Database)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_providers_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<ProviderRow>, AppError> {
        let user_id = user_id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, name, base_url, api_key, api_type, enabled, extra_config, created_at, updated_at
                 FROM providers WHERE user_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt.query_map([&user_id], |row| {
                Ok(ProviderRow {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    base_url: row.get(3)?,
                    api_key: row.get(4)?,
                    api_type: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                    extra_config: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(AppError::Database)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn get_provider(&self, id: &str) -> Result<Option<ProviderRow>, AppError> {
        let id = id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, name, base_url, api_key, api_type, enabled, extra_config, created_at, updated_at
                 FROM providers WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map([&id], |row| {
                Ok(ProviderRow {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    base_url: row.get(3)?,
                    api_key: row.get(4)?,
                    api_type: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                    extra_config: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?;
            Ok(rows.next().transpose()?)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn create_provider(&self, row: &ProviderRow) -> Result<(), AppError> {
        let id = row.id.clone();
        let user_id = row.user_id.clone();
        let name = row.name.clone();
        let base_url = row.base_url.clone();
        let api_key = row.api_key.clone();
        let api_type = row.api_type.clone();
        let enabled = i32::from(row.enabled);
        let extra_config = row.extra_config.clone();

        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO providers (id, user_id, name, base_url, api_key, api_type, enabled, extra_config)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![id, user_id, name, base_url, api_key, api_type, enabled, extra_config],
            )?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn update_provider(&self, row: &ProviderRow) -> Result<(), AppError> {
        let id = row.id.clone();
        let name = row.name.clone();
        let base_url = row.base_url.clone();
        let api_key = row.api_key.clone();
        let api_type = row.api_type.clone();
        let enabled = i32::from(row.enabled);
        let extra_config = row.extra_config.clone();

        self.with_conn(move |conn| {
            conn.execute(
                "UPDATE providers SET name=?2, base_url=?3, api_key=?4, api_type=?5,
                 enabled=?6, extra_config=?7, updated_at=datetime('now') WHERE id=?1",
                rusqlite::params![id, name, base_url, api_key, api_type, enabled, extra_config],
            )?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn delete_provider(&self, id: &str) -> Result<(), AppError> {
        let id = id.to_string();
        self.with_conn(move |conn| {
            conn.execute("DELETE FROM providers WHERE id = ?1", [&id])?;
            Ok(())
        })
        .await
    }

    // ── Model CRUD ──────────────────────────────────────

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_models_for_provider(
        &self,
        provider_id: &str,
    ) -> Result<Vec<ModelRow>, AppError> {
        let provider_id = provider_id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, provider_id, slug, display_name, context_window, extra_config
                 FROM models WHERE provider_id = ?1 ORDER BY slug",
            )?;
            let rows = stmt.query_map([&provider_id], |row| {
                Ok(ModelRow {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    slug: row.get(2)?,
                    display_name: row.get(3)?,
                    context_window: row.get(4)?,
                    extra_config: row.get::<_, String>(5).unwrap_or_default(),
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await
    }

    /// 获取端点可用的所有模型（聚合查询）
    /// 直接返回所有 model，不再依赖 `model_endpoint_visibility`
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_models_for_endpoint(
        &self,
        _endpoint_id: &str,
    ) -> Result<Vec<ModelRow>, AppError> {
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT m.id, m.provider_id, m.slug, m.display_name, m.context_window, m.extra_config
                 FROM models m
                 INNER JOIN providers p ON p.id = m.provider_id AND p.enabled = 1
                 ORDER BY m.slug",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(ModelRow {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    slug: row.get(2)?,
                    display_name: row.get(3)?,
                    context_window: row.get(4)?,
                    extra_config: row.get::<_, String>(5).unwrap_or_default(),
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await
    }

    /// 查找提供某个模型的 Provider（按 slug 匹配）
    /// 不再依赖 `model_endpoint_visibility`：model 属于哪个 provider
    /// 是固有关系，不需要再通过 endpoint 二次过滤。
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn find_provider_by_model_slug(
        &self,
        slug: &str,
        _endpoint_id: &str,
    ) -> Result<Option<ProviderRow>, AppError> {
        let slug = slug.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.user_id, p.name, p.base_url, p.api_key, p.api_type, p.enabled,
                        p.extra_config, p.created_at, p.updated_at
                 FROM providers p
                 INNER JOIN models m ON m.provider_id = p.id
                 WHERE m.slug = ?1 AND p.enabled = 1
                 LIMIT 1",
            )?;
            let mut rows = stmt.query_map(rusqlite::params![slug], |row| {
                Ok(ProviderRow {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    base_url: row.get(3)?,
                    api_key: row.get(4)?,
                    api_type: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                    extra_config: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?;
            Ok(rows.next().transpose()?)
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn create_model(&self, row: &ModelRow) -> Result<(), AppError> {
        let id = row.id.clone();
        let provider_id = row.provider_id.clone();
        let slug = row.slug.clone();
        let display_name = row.display_name.clone();
        let context_window = row.context_window;
        let extra_config = row.extra_config.clone();

        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO models (id, provider_id, slug, display_name, context_window, extra_config)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![id, provider_id, slug, display_name, context_window, extra_config],
            )?;
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn delete_models_for_provider(&self, provider_id: &str) -> Result<(), AppError> {
        let provider_id = provider_id.to_string();
        self.with_conn(move |conn| {
            conn.execute("DELETE FROM models WHERE provider_id = ?1", [&provider_id])?;
            Ok(())
        })
        .await
    }

    // ── Visibility CRUD ─────────────────────────────────

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn set_model_visibility(
        &self,
        model_id: &str,
        endpoint_ids: &[String],
    ) -> Result<(), AppError> {
        let model_id = model_id.to_string();
        let endpoint_ids = endpoint_ids.to_vec();
        self.with_conn(move |conn| {
            conn.execute(
                "DELETE FROM model_endpoint_visibility WHERE model_id = ?1",
                [&model_id],
            )?;
            let mut stmt = conn.prepare(
                "INSERT OR IGNORE INTO model_endpoint_visibility (model_id, endpoint_id)
                 VALUES (?1, ?2)",
            )?;
            for endpoint_id in &endpoint_ids {
                stmt.execute(rusqlite::params![model_id, endpoint_id])?;
            }
            Ok(())
        })
        .await
    }

    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn get_model_visibility(&self, model_id: &str) -> Result<Vec<String>, AppError> {
        let model_id = model_id.to_string();
        self.with_conn(move |conn| {
            let mut stmt = conn
                .prepare("SELECT endpoint_id FROM model_endpoint_visibility WHERE model_id = ?1")?;
            let rows = stmt.query_map([&model_id], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await
    }

    // ── Usage Records ──────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn insert_usage_record(&self, row: &UsageRecordRow) -> Result<(), AppError> {
        let id = row.id.clone();
        let api_key_id = row.api_key_id.clone();
        let key_owner_id = row.key_owner_id.clone();
        let endpoint_id = row.endpoint_id.clone();
        let user_id = row.user_id.clone();
        let provider_id = row.provider_id.clone();
        let provider_name = row.provider_name.clone();
        let model = row.model.clone();
        let input_tokens = row.input_tokens;
        let output_tokens = row.output_tokens;
        let cache_read_tokens = row.cache_read_tokens;

        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO usage_records (id, api_key_id, key_owner_id, endpoint_id, user_id, provider_id, provider_name, model,
                 input_tokens, output_tokens, cache_read_tokens)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    id, api_key_id, key_owner_id, endpoint_id, user_id,
                    provider_id, provider_name, model,
                    input_tokens, output_tokens, cache_read_tokens,
                ],
            )?;
            Ok(())
        })
        .await
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn list_usage_records(
        &self,
        api_key_id: Option<&str>,
        endpoint_id: Option<&str>,
        user_id: Option<&str>,
        key_owner_id: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<UsageRecordRow>, i64), AppError> {
        let api_key_id = api_key_id.map(std::string::ToString::to_string);
        let endpoint_id = endpoint_id.map(std::string::ToString::to_string);
        let user_id = user_id.map(std::string::ToString::to_string);
        let key_owner_id = key_owner_id.map(std::string::ToString::to_string);
        let from = from.map(std::string::ToString::to_string);
        let to = to.map(std::string::ToString::to_string);

        self.with_conn(move |conn| {
            let mut conditions = Vec::new();
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(ref v) = api_key_id {
                conditions.push(format!("u.api_key_id = ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = endpoint_id {
                conditions.push(format!("u.endpoint_id = ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = user_id {
                conditions.push(format!("u.user_id = ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = key_owner_id {
                conditions.push(format!("u.key_owner_id = ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = from {
                conditions.push(format!("u.created_at >= ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = to {
                conditions.push(format!(
                    "u.created_at < date(?{}, '+1 day')",
                    params.len() + 1
                ));
                params.push(Box::new(v.clone()));
            }

            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            let count_sql = format!("SELECT COUNT(*) FROM usage_records u {where_clause}");
            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();
            let total: i64 = conn.query_row(&count_sql, params_refs.as_slice(), |r| r.get(0))?;

            let data_sql = format!(
                "SELECT u.id, u.api_key_id, u.key_owner_id, u.endpoint_id, u.user_id,
                 u.provider_id, u.provider_name, u.model,
                 u.input_tokens, u.output_tokens, u.cache_read_tokens, u.created_at,
                 COALESCE(k.name, '') as key_name, COALESCE(k.key_prefix, '') as key_prefix
                 FROM usage_records u
                 LEFT JOIN endpoint_api_keys k ON k.id = u.api_key_id
                 {where_clause}
                 ORDER BY u.created_at DESC
                 LIMIT ?{} OFFSET ?{}",
                params.len() + 1,
                params.len() + 2
            );
            let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = params;
            all_params.push(Box::new(limit));
            all_params.push(Box::new(offset));
            let all_refs: Vec<&dyn rusqlite::types::ToSql> =
                all_params.iter().map(std::convert::AsRef::as_ref).collect();

            let mut stmt = conn.prepare(&data_sql)?;
            let rows = stmt.query_map(all_refs.as_slice(), |row| {
                Ok(UsageRecordRow {
                    id: row.get(0)?,
                    api_key_id: row.get(1)?,
                    key_owner_id: row.get::<_, String>(2).unwrap_or_default(),
                    endpoint_id: row.get(3)?,
                    user_id: row.get(4)?,
                    provider_id: row.get::<_, String>(5).unwrap_or_default(),
                    provider_name: row.get::<_, String>(6).unwrap_or_default(),
                    model: row.get(7)?,
                    input_tokens: row.get(8)?,
                    output_tokens: row.get(9)?,
                    cache_read_tokens: row.get(10)?,
                    created_at: row.get(11)?,
                })
            })?;

            let records = rows.collect::<Result<Vec<_>, _>>()?;
            Ok((records, total))
        })
        .await
    }

    /// `time_series`, from, to, granularity=day|hour
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn time_series(
        &self,
        user_id: Option<&str>,
        key_owner_id: Option<&str>,
        exclude_self: bool,
        granularity: &str,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<Vec<TimeSeriesPoint>, AppError> {
        let user_id = user_id.map(std::string::ToString::to_string);
        let key_owner_id = key_owner_id.map(std::string::ToString::to_string);
        let from = from.map(std::string::ToString::to_string);
        let to = to.map(std::string::ToString::to_string);

        let time_expr = match granularity {
            "hour" => "strftime('%Y-%m-%dT%H:00:00', created_at)",
            _ => "date(created_at)",
        };

        self.with_conn(move |conn| {
            let mut conditions = Vec::new();
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(ref v) = user_id {
                conditions.push(format!("user_id = ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = key_owner_id {
                conditions.push(format!("key_owner_id = ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if exclude_self {
                conditions.push("user_id != key_owner_id".to_string());
            }
            if let Some(ref v) = from {
                conditions.push(format!("created_at >= ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = to {
                conditions.push(format!(
                    "created_at < date(?{}, '+1 day')",
                    params.len() + 1
                ));
                params.push(Box::new(v.clone()));
            }

            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            let sql = format!(
                "SELECT {time_expr} as ts, SUM(input_tokens), SUM(output_tokens), COUNT(*)
                 FROM usage_records {where_clause}
                 GROUP BY ts ORDER BY ts ASC"
            );

            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_refs.as_slice(), |row| {
                Ok(TimeSeriesPoint {
                    timestamp: row.get(0)?,
                    input_tokens: row.get(1)?,
                    output_tokens: row.get(2)?,
                    count: row.get(3)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await
    }

    /// `time_series_breakdown`: time series grouped by model or key name
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn time_series_breakdown(
        &self,
        user_id: &str,
        group_by: &str, // "model" | "key"
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<Vec<TimeSeriesBreakdown>, AppError> {
        let user_id = user_id.to_string();
        let from = from.map(std::string::ToString::to_string);
        let to = to.map(std::string::ToString::to_string);
        let group_by = group_by.to_string();

        self.with_conn(move |conn| {
            let group_col = if group_by == "key" {
                "COALESCE(k.name, u.api_key_id)"
            } else {
                "u.model"
            };
            let join = if group_by == "key" {
                "LEFT JOIN endpoint_api_keys k ON k.id = u.api_key_id"
            } else {
                ""
            };

            let mut conditions = vec![format!("u.user_id = ?1")];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(user_id.clone())];

            if let Some(ref v) = from {
                let idx = params.len() + 1;
                conditions.push(format!("u.created_at >= ?{idx}"));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = to {
                let idx = params.len() + 1;
                conditions.push(format!("u.created_at < date(?{idx}, '+1 day')"));
                params.push(Box::new(v.clone()));
            }

            let where_clause = format!("WHERE {}", conditions.join(" AND "));

            let sql = format!(
                "SELECT date(u.created_at) as ts, {group_col},
                        SUM(u.input_tokens + u.output_tokens), COUNT(*)
                 FROM usage_records u {join}
                 {where_clause}
                 GROUP BY ts, {group_col} ORDER BY ts ASC, total_tokens DESC"
            );

            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_refs.as_slice(), |row| {
                Ok(TimeSeriesBreakdown {
                    timestamp: row.get(0)?,
                    group_key: row.get(1)?,
                    total_tokens: row.get(2)?,
                    count: row.get(3)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await
    }

    /// `aggregate_usage`: `group_by=key|model|day|provider`
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn aggregate_usage(
        &self,
        key_owner_id: Option<&str>,
        user_id: Option<&str>,
        exclude_self: bool, // exclude user_id == key_owner_id (for shared usage)
        group_by: &str,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<Vec<UsageAggregateRow>, AppError> {
        let key_owner_id = key_owner_id.map(std::string::ToString::to_string);
        let user_id = user_id.map(std::string::ToString::to_string);
        let from = from.map(std::string::ToString::to_string);
        let to = to.map(std::string::ToString::to_string);

        let group_col = match group_by {
            "key" => "u.api_key_id",
            "model" => "u.model",
            "day" => "date(u.created_at)",
            "provider" => "u.provider_id",
            _ => return Err(AppError::BadRequest("invalid group_by".into())),
        };
        let is_key_group = group_by == "key";

        self.with_conn(move |conn| {
            let mut conditions = Vec::new();
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(ref v) = key_owner_id {
                conditions.push(format!("u.key_owner_id = ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = user_id {
                conditions.push(format!("u.user_id = ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if exclude_self {
                conditions.push("u.user_id != u.key_owner_id".to_string());
            }
            if let Some(ref v) = from {
                conditions.push(format!("u.created_at >= ?{}", params.len() + 1));
                params.push(Box::new(v.clone()));
            }
            if let Some(ref v) = to {
                conditions.push(format!(
                    "u.created_at < date(?{}, '+1 day')",
                    params.len() + 1
                ));
                params.push(Box::new(v.clone()));
            }

            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            let sql = if is_key_group {
                format!(
                    "SELECT u.api_key_id as grp, COALESCE(k.name, '') as key_name,
                     SUM(u.input_tokens) as total_input,
                     SUM(u.output_tokens) as total_output, COUNT(*) as cnt
                     FROM usage_records u
                     LEFT JOIN endpoint_api_keys k ON k.id = u.api_key_id
                     {where_clause}
                     GROUP BY grp ORDER BY total_input DESC"
                )
            } else {
                format!(
                    "SELECT {group_col} as grp, SUM(u.input_tokens) as total_input,
                     SUM(u.output_tokens) as total_output, COUNT(*) as cnt
                     FROM usage_records u
                     {where_clause}
                     GROUP BY grp ORDER BY total_input DESC"
                )
            };

            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(std::convert::AsRef::as_ref).collect();

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_refs.as_slice(), |row| {
                Ok(UsageAggregateRow {
                    key: row.get(0)?,
                    key_name: if is_key_group {
                        row.get::<_, String>(1).ok()
                    } else {
                        None
                    },
                    total_input_tokens: if is_key_group {
                        row.get(2)?
                    } else {
                        row.get(1)?
                    },
                    total_output_tokens: if is_key_group {
                        row.get(3)?
                    } else {
                        row.get(2)?
                    },
                    count: if is_key_group {
                        row.get(4)?
                    } else {
                        row.get(3)?
                    },
                })
            })?;

            rows.collect::<Result<Vec<_>, _>>()
                .map_err(AppError::Database)
        })
        .await
    }

    /// Shared usage overview summary
    /// # Errors
    /// Returns an [`AppError`] if the underlying database operation fails.
    pub async fn shared_usage_summary(
        &self,
        key_owner_id: &str,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<SharedUsageSummary, AppError> {
        let key_owner_id = key_owner_id.to_string();
        let from = from.map_or_else(
            || String::from("2000-01-01"),
            std::string::ToString::to_string,
        );
        let to = to.map_or_else(
            || String::from("2099-12-31"),
            std::string::ToString::to_string,
        );

        self.with_conn(move |conn| {
            // Today tokens
            let today: i64 = conn.query_row(
                "SELECT COALESCE(SUM(input_tokens + output_tokens), 0)
                 FROM usage_records
                 WHERE key_owner_id = ?1 AND user_id != key_owner_id
                   AND date(created_at) >= date(?2)
                   AND date(created_at) <= date(?3)",
                [&key_owner_id, &from, &to],
                |r| r.get(0),
            )?;

            // Previous period tokens (same duration)
            let prev_tokens: i64 = conn.query_row(
                "SELECT COALESCE(SUM(input_tokens + output_tokens), 0)
                 FROM usage_records
                 WHERE key_owner_id = ?1 AND user_id != key_owner_id
                   AND date(created_at) >= date(?2, ?3)
                   AND date(created_at) < date(?2)",
                rusqlite::params![
                    &key_owner_id,
                    &from,
                    &format!("-{} days", days_between(&from, &to))
                ],
                |r| r.get(0),
            )?;

            let trend_pct = if prev_tokens > 0 {
                (today - prev_tokens) as f64 / prev_tokens as f64 * 100.0
            } else if today > 0 {
                100.0
            } else {
                0.0
            };

            // Active keys in period
            let active_keys: i64 = conn.query_row(
                "SELECT COALESCE(COUNT(DISTINCT api_key_id), 0)
                 FROM usage_records
                 WHERE key_owner_id = ?1 AND user_id != key_owner_id
                   AND date(created_at) >= date(?2)
                   AND date(created_at) <= date(?3)",
                [&key_owner_id, &from, &to],
                |r| r.get(0),
            )?;

            // Total keys
            let total_keys: i64 = conn.query_row(
                "SELECT COUNT(*) FROM endpoint_api_keys WHERE created_by = ?1",
                [&key_owner_id],
                |r| r.get(0),
            )?;

            // Active users in period
            let active_users: i64 = conn.query_row(
                "SELECT COALESCE(COUNT(DISTINCT user_id), 0)
                 FROM usage_records
                 WHERE key_owner_id = ?1 AND user_id != key_owner_id
                   AND date(created_at) >= date(?2)
                   AND date(created_at) <= date(?3)",
                [&key_owner_id, &from, &to],
                |r| r.get(0),
            )?;

            Ok(SharedUsageSummary {
                today_tokens: today,
                yesterday_tokens: prev_tokens,
                trend_pct,
                active_keys,
                total_keys,
                active_users,
            })
        })
        .await
    }
}

/// Helper: compute approximate days between two date strings
fn days_between(from: &str, to: &str) -> i64 {
    // Simple approximation: parse YYYY-MM-DD
    let f_parts: Vec<i64> = from.split('-').filter_map(|s| s.parse().ok()).collect();
    let t_parts: Vec<i64> = to.split('-').filter_map(|s| s.parse().ok()).collect();
    if f_parts.len() != 3 || t_parts.len() != 3 {
        return 7; // default fallback
    }
    let f_days = f_parts[0] * 365 + f_parts[1] * 30 + f_parts[2];
    let t_days = t_parts[0] * 365 + t_parts[1] * 30 + t_parts[2];
    (t_days - f_days + 1).max(1)
}
