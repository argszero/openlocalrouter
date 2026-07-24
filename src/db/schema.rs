use rusqlite::Connection;

use crate::error::AppError;

/// Current schema version — bump when adding new migrations below
const SCHEMA_VERSION: i64 = 2;

pub fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    log::info!("检查数据库迁移…");

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_version (
            version     INTEGER NOT NULL DEFAULT 1
        );
        ",
    )?;

    // Insert initial version if table was just created (empty)
    let current_version: i64 = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .unwrap_or(0);

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS users (
            id              TEXT PRIMARY KEY,
            username        TEXT NOT NULL UNIQUE,
            password_hash   TEXT NOT NULL,
            is_admin        INTEGER NOT NULL DEFAULT 0,
            enabled         INTEGER NOT NULL DEFAULT 1,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS sessions (
            token           TEXT PRIMARY KEY,
            user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS endpoints (
            id          TEXT PRIMARY KEY,
            user_id     TEXT NOT NULL REFERENCES users(id),
            name        TEXT NOT NULL,
            listen_path TEXT NOT NULL UNIQUE,
            protocol    TEXT NOT NULL CHECK(protocol IN ('openai_chat', 'openai_responses', 'anthropic_messages')),
            enabled     INTEGER NOT NULL DEFAULT 1,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS providers (
            id           TEXT PRIMARY KEY,
            user_id      TEXT NOT NULL REFERENCES users(id),
            name         TEXT NOT NULL,
            base_url     TEXT NOT NULL,
            api_key      TEXT NOT NULL DEFAULT '',
            api_type     TEXT NOT NULL DEFAULT 'openai_chat',
            enabled      INTEGER NOT NULL DEFAULT 1,
            extra_config TEXT NOT NULL DEFAULT '{}',
            created_at   TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS models (
            id              TEXT PRIMARY KEY,
            provider_id     TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
            slug            TEXT NOT NULL,
            display_name    TEXT NOT NULL,
            context_window  INTEGER NOT NULL DEFAULT 128000,
            extra_config    TEXT NOT NULL DEFAULT '{}',
            UNIQUE(provider_id, slug)
        );

        CREATE TABLE IF NOT EXISTS model_endpoint_visibility (
            model_id    TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
            endpoint_id TEXT NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
            PRIMARY KEY (model_id, endpoint_id)
        );

        CREATE TABLE IF NOT EXISTS endpoint_api_keys (
            id              TEXT PRIMARY KEY,
            endpoint_id     TEXT NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
            user_id         TEXT NOT NULL REFERENCES users(id),
            created_by      TEXT NOT NULL DEFAULT '',
            assigned_to     TEXT NOT NULL DEFAULT '',
            name            TEXT NOT NULL DEFAULT '',
            key_value       TEXT NOT NULL DEFAULT '',
            key_hash        TEXT NOT NULL,
            key_prefix      TEXT NOT NULL,
            enabled         INTEGER NOT NULL DEFAULT 1,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            last_used_at    TEXT
        );

        CREATE TABLE IF NOT EXISTS usage_records (
            id              INTEGER PRIMARY KEY,
            api_key_id      TEXT NOT NULL REFERENCES endpoint_api_keys(id),
            key_owner_id    TEXT NOT NULL DEFAULT '',
            endpoint_id     TEXT NOT NULL REFERENCES endpoints(id),
            user_id         TEXT NOT NULL REFERENCES users(id),
            provider_id     TEXT NOT NULL DEFAULT '',
            provider_name   TEXT NOT NULL DEFAULT '',
            model           TEXT NOT NULL,
            input_tokens    INTEGER NOT NULL DEFAULT 0,
            output_tokens   INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_models_provider ON models(provider_id);
        CREATE INDEX IF NOT EXISTS idx_models_slug ON models(slug);
        CREATE INDEX IF NOT EXISTS idx_visibility_endpoint ON model_endpoint_visibility(endpoint_id);
        CREATE INDEX IF NOT EXISTS idx_visibility_model ON model_endpoint_visibility(model_id);
        CREATE INDEX IF NOT EXISTS idx_endpoints_user ON endpoints(user_id);
        CREATE INDEX IF NOT EXISTS idx_providers_user ON providers(user_id);
        CREATE INDEX IF NOT EXISTS idx_api_keys_endpoint ON endpoint_api_keys(endpoint_id);
        CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON endpoint_api_keys(key_hash);
        CREATE INDEX IF NOT EXISTS idx_usage_user ON usage_records(user_id);
        CREATE INDEX IF NOT EXISTS idx_usage_api_key ON usage_records(api_key_id);
        CREATE INDEX IF NOT EXISTS idx_usage_created ON usage_records(created_at);
        CREATE INDEX IF NOT EXISTS idx_usage_endpoint ON usage_records(endpoint_id);
        ",
    )?;

    // ── Migrations ────────────────────────────────────

    // 1. Add key_value column for existing databases
    let need_migrate: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('endpoint_api_keys') WHERE name='key_value'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .is_ok_and(|c| c == 0);

    if need_migrate {
        conn.execute_batch(
            "ALTER TABLE endpoint_api_keys ADD COLUMN key_value TEXT NOT NULL DEFAULT '';",
        )?;
    }

    // 2. Add created_by / assigned_to to endpoint_api_keys
    for col in &["created_by", "assigned_to"] {
        let exists: bool = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM pragma_table_info('endpoint_api_keys') WHERE name='{col}'"
                ),
                [],
                |row| row.get::<_, i64>(0),
            )
            .is_ok_and(|c| c > 0);
        if !exists {
            conn.execute_batch(&format!(
                "ALTER TABLE endpoint_api_keys ADD COLUMN {col} TEXT NOT NULL DEFAULT '';"
            ))?;
        }
    }

    // Backfill: copy user_id to created_by and assigned_to for existing keys
    let need_backfill: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM endpoint_api_keys WHERE created_by = '' AND user_id != ''",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if need_backfill > 0 {
        log::info!("迁移: 回填 {need_backfill} 条 endpoint_api_keys 的 created_by/assigned_to");
        conn.execute_batch(
            "UPDATE endpoint_api_keys SET
                created_by = user_id,
                assigned_to = user_id
             WHERE created_by = '';",
        )?;
    }

    // 3. Add provider_id, provider_name, key_owner_id to usage_records
    for col in &["provider_id", "provider_name", "key_owner_id"] {
        let exists: bool = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM pragma_table_info('usage_records') WHERE name='{col}'"
                ),
                [],
                |row| row.get::<_, i64>(0),
            )
            .is_ok_and(|c| c > 0);
        if !exists {
            conn.execute_batch(&format!(
                "ALTER TABLE usage_records ADD COLUMN {col} TEXT NOT NULL DEFAULT '';"
            ))?;
        }
    }

    // Backfill key_owner_id for existing usage records (via JOIN)
    let need_owner_backfill: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_records WHERE key_owner_id = ''",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if need_owner_backfill > 0 {
        log::info!("迁移: 回填 {need_owner_backfill} 条 usage_records 的 key_owner_id");
        conn.execute(
            "UPDATE usage_records SET key_owner_id = (
                SELECT COALESCE(k.created_by, k.user_id)
                FROM endpoint_api_keys k WHERE k.id = usage_records.api_key_id
            ) WHERE key_owner_id = ''",
            [],
        )?;
    }

    // 4. Fix empty created_at in usage_records (bug: INSERT included column)
    let empty_created: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM usage_records WHERE created_at = '' OR created_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if empty_created > 0 {
        log::info!("迁移: 修复 {empty_created} 条 usage_records 的 created_at 为空");
        conn.execute(
            "UPDATE usage_records SET created_at = datetime('now') WHERE created_at = '' OR created_at IS NULL",
            [],
        )?;
    }

    // 5. usage_records: migrate id from TEXT (UUID) to INTEGER PRIMARY KEY
    //    INTEGER PRIMARY KEY = alias for rowid, sequential inserts avoid
    //    random B-tree page splits that corrupt on hard-kill.
    let id_is_text: bool = conn
        .query_row(
            "SELECT type FROM pragma_table_info('usage_records') WHERE name='id'",
            [],
            |row| {
                let t: String = row.get(0)?;
                Ok(t.to_uppercase() == "TEXT")
            },
        )
        .unwrap_or(false);

    if id_is_text {
        log::info!("迁移: usage_records.id TEXT → INTEGER PRIMARY KEY");
        conn.execute_batch(
            "CREATE TABLE usage_records_new (
                id              INTEGER PRIMARY KEY,
                api_key_id      TEXT NOT NULL REFERENCES endpoint_api_keys(id),
                key_owner_id    TEXT NOT NULL DEFAULT '',
                endpoint_id     TEXT NOT NULL REFERENCES endpoints(id),
                user_id         TEXT NOT NULL REFERENCES users(id),
                provider_id     TEXT NOT NULL DEFAULT '',
                provider_name   TEXT NOT NULL DEFAULT '',
                model           TEXT NOT NULL,
                input_tokens    INTEGER NOT NULL DEFAULT 0,
                output_tokens   INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                created_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );
            INSERT INTO usage_records_new (api_key_id, key_owner_id, endpoint_id, user_id,
                provider_id, provider_name, model, input_tokens, output_tokens,
                cache_read_tokens, created_at)
            SELECT api_key_id, key_owner_id, endpoint_id, user_id,
                provider_id, provider_name, model, input_tokens, output_tokens,
                cache_read_tokens, created_at
            FROM usage_records;
            DROP TABLE usage_records;
            ALTER TABLE usage_records_new RENAME TO usage_records;
            CREATE INDEX IF NOT EXISTS idx_usage_user ON usage_records(user_id);
            CREATE INDEX IF NOT EXISTS idx_usage_api_key ON usage_records(api_key_id);
            CREATE INDEX IF NOT EXISTS idx_usage_created ON usage_records(created_at);
            CREATE INDEX IF NOT EXISTS idx_usage_endpoint ON usage_records(endpoint_id);",
        )?;
    }

    // ── Schema version tracking ─────────────────────────
    if current_version < SCHEMA_VERSION {
        conn.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [SCHEMA_VERSION],
        )?;
        log::info!(
            "数据库迁移完成: v{} → v{}",
            if current_version == 0 {
                "0".to_string()
            } else {
                current_version.to_string()
            },
            SCHEMA_VERSION
        );
    } else {
        log::info!("数据库已是最新版本 (v{SCHEMA_VERSION})");
    }

    Ok(())
}
