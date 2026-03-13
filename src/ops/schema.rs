use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

use crate::DbHandle;

fn exec_ignore(conn: &mut rusqlite::Connection, sql: &str) {
    let _ = conn.execute_batch(sql);
}

const SCHEMA_VERSION: i64 = 2;
const POST_MIGRATIONS_VERSION: i64 = 3;

fn has_migration_run(conn: &rusqlite::Connection, name: &str) -> rusqlite::Result<bool> {
    let row: Option<i32> = conn
        .query_row(
            "SELECT 1 FROM migrations WHERE name = ?1 LIMIT 1",
            [name],
            |r| r.get(0),
        )
        .optional()?;
    Ok(row.is_some())
}

fn mark_migration_complete(conn: &rusqlite::Connection, name: &str) -> rusqlite::Result<()> {
    let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
    conn.execute(
        &format!(
            "INSERT OR REPLACE INTO migrations (name, executed_at) VALUES (?1, {now})",
            now = now_expr
        ),
        [name],
    )?;
    Ok(())
}

fn set_schema_meta(conn: &rusqlite::Connection, key: &str, value: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO schema_metadata (key, value) VALUES (?1, ?2)",
        params![key, value.to_string()],
    )?;
    Ok(())
}

fn get_schema_meta_i64(conn: &rusqlite::Connection, key: &str) -> rusqlite::Result<Option<i64>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_metadata WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(raw.and_then(|value| value.parse::<i64>().ok()))
}

fn parse_json_array(raw: &str) -> Vec<Value> {
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Array(items)) => items,
        _ => Vec::new(),
    }
}

#[napi]
impl DbHandle {
    #[napi]
    pub fn ensure_schema(&self) -> Result<bool> {
        self.with_connection(|conn| {
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS chat_threads (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    model TEXT,
                    is_generating BOOLEAN DEFAULT FALSE,
                    reasoning_effort TEXT DEFAULT 'medium',
                    metadata TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS chat_messages (
                    id TEXT PRIMARY KEY,
                    thread_id TEXT NOT NULL,
                    parent_id TEXT,
                    slot_id TEXT,
                    depth INTEGER NOT NULL DEFAULT 0,
                    parent_tool_call_id TEXT,
                    message TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    metadata TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY (thread_id) REFERENCES chat_threads(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS app_settings (
                    id TEXT PRIMARY KEY DEFAULT 'default',
                    settings_data TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS providers (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    type TEXT NOT NULL,
                    api_key TEXT NOT NULL,
                    models TEXT NOT NULL,
                    base_url TEXT,
                    api_version TEXT,
                    enabled BOOLEAN NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS model_capabilities_cache (
                    id TEXT PRIMARY KEY,
                    capabilities TEXT NOT NULL,
                    fetched_at TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS provider_models_cache (
                    id TEXT PRIMARY KEY,
                    provider_id TEXT NOT NULL,
                    models TEXT NOT NULL,
                    fetched_at TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS thread_diff_stats_cache (
                    id TEXT PRIMARY KEY,
                    thread_updated_at TEXT NOT NULL,
                    additions INTEGER NOT NULL DEFAULT 0,
                    deletions INTEGER NOT NULL DEFAULT 0,
                    files_changed INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS prompt_apps (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
                    icon TEXT,
                    prompt_template TEXT NOT NULL,
                    placeholders TEXT NOT NULL DEFAULT '[]',
                    model TEXT,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    shortcut TEXT,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS prompt_app_executions (
                    id TEXT PRIMARY KEY,
                    prompt_app_id TEXT NOT NULL,
                    thread_id TEXT,
                    input_values TEXT NOT NULL,
                    generated_prompt TEXT NOT NULL,
                    attachment_count INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (prompt_app_id) REFERENCES prompt_apps(id) ON DELETE CASCADE,
                    FOREIGN KEY (thread_id) REFERENCES chat_threads(id) ON DELETE SET NULL
                );

                CREATE TABLE IF NOT EXISTS gallery_images (
                    id TEXT PRIMARY KEY,
                    message_id TEXT NOT NULL,
                    thread_id TEXT NOT NULL,
                    thread_title TEXT NOT NULL,
                    part_index INTEGER NOT NULL,
                    media_type TEXT NOT NULL,
                    filename TEXT,
                    width INTEGER,
                    height INTEGER,
                    aspect_ratio REAL,
                    file_path TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (message_id) REFERENCES chat_messages(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS mcp_servers (
                    id TEXT PRIMARY KEY,
                    registry_id TEXT,
                    name TEXT NOT NULL,
                    description TEXT,
                    config TEXT NOT NULL,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    status TEXT NOT NULL DEFAULT 'disconnected',
                    last_error TEXT,
                    installed_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS mcp_oauth_tokens (
                    id TEXT PRIMARY KEY,
                    server_id TEXT NOT NULL,
                    authorization_server_url TEXT,
                    resource_url TEXT,
                    client_id TEXT,
                    client_secret TEXT,
                    client_id_issued_at INTEGER,
                    client_secret_expires_at INTEGER,
                    access_token TEXT,
                    refresh_token TEXT,
                    token_type TEXT,
                    expires_at INTEGER,
                    scope TEXT,
                    code_verifier TEXT,
                    last_refresh_at TEXT,
                    last_error_at TEXT,
                    last_error TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS workspaces (
                    id TEXT PRIMARY KEY,
                    path TEXT NOT NULL,
                    name TEXT NOT NULL,
                    is_temporary INTEGER NOT NULL DEFAULT 0,
                    show_in_list INTEGER NOT NULL DEFAULT 1,
                    is_worktree INTEGER NOT NULL DEFAULT 0,
                    parent_workspace_id TEXT,
                    worktree_branch TEXT,
                    auto_worktree INTEGER NOT NULL DEFAULT 0,
                    pr_number INTEGER,
                    pr_url TEXT,
                    pr_state TEXT,
                    pr_base_branch TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS prompts (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    content TEXT NOT NULL,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS skills (
                    id TEXT PRIMARY KEY,
                    path TEXT NOT NULL,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS plugins (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    version TEXT NOT NULL,
                    description TEXT NOT NULL,
                    author TEXT NOT NULL,
                    icon TEXT,
                    source TEXT NOT NULL,
                    source_path TEXT NOT NULL,
                    install_url TEXT,
                    manifest TEXT NOT NULL,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    settings TEXT NOT NULL DEFAULT '{}',
                    installed_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS plugin_permissions (
                    id TEXT PRIMARY KEY,
                    plugin_id TEXT NOT NULL REFERENCES plugins(id) ON DELETE CASCADE,
                    permission TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    granted_at TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(plugin_id, permission)
                );

                CREATE TABLE IF NOT EXISTS custom_themes (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    display_name TEXT NOT NULL,
                    type TEXT NOT NULL CHECK(type IN ('dark', 'light')),
                    base_30 TEXT NOT NULL,
                    base_16 TEXT NOT NULL,
                    based_on TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS thread_labels (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    color TEXT NOT NULL,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS channel_mappings (
                    id TEXT PRIMARY KEY,
                    platform TEXT NOT NULL,
                    external_chat_id TEXT NOT NULL,
                    external_user_id TEXT NOT NULL,
                    thread_id TEXT NOT NULL REFERENCES chat_threads(id) ON DELETE CASCADE,
                    is_active INTEGER DEFAULT 1,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS migrations (
                    name TEXT PRIMARY KEY,
                    executed_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS schema_metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS usage_records (
                    id TEXT PRIMARY KEY,
                    message_id TEXT NOT NULL,
                    thread_id TEXT NOT NULL,
                    model TEXT,
                    provider_id TEXT,
                    date TEXT NOT NULL,
                    input_tokens INTEGER DEFAULT 0,
                    output_tokens INTEGER DEFAULT 0,
                    cached_input_tokens INTEGER DEFAULT 0,
                    cache_write_input_tokens INTEGER DEFAULT 0,
                    reasoning_tokens INTEGER DEFAULT 0,
                    total_tokens INTEGER DEFAULT 0,
                    timestamp TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (message_id) REFERENCES chat_messages(id) ON DELETE CASCADE,
                    FOREIGN KEY (thread_id) REFERENCES chat_threads(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS usage_migration_status (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    status TEXT NOT NULL DEFAULT 'pending',
                    total_count INTEGER DEFAULT 0,
                    migrated_count INTEGER DEFAULT 0,
                    last_migrated_id TEXT,
                    started_at TEXT,
                    completed_at TEXT,
                    error_message TEXT
                );

                CREATE TABLE IF NOT EXISTS preview_servers (
                    id TEXT PRIMARY KEY,
                    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
                    port INTEGER NOT NULL,
                    project_type TEXT NOT NULL,
                    command TEXT NOT NULL,
                    pid INTEGER,
                    status TEXT NOT NULL DEFAULT 'stopped',
                    last_error TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                "#,
            )?;

            let indexes = [
                "CREATE INDEX IF NOT EXISTS idx_messages_thread_id ON chat_messages(thread_id)",
                "CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON chat_messages(timestamp)",
                "CREATE INDEX IF NOT EXISTS idx_messages_version_info ON chat_messages(thread_id, timestamp, id, slot_id, created_at)",
                "CREATE INDEX IF NOT EXISTS idx_messages_parent_id ON chat_messages(parent_id)",
                "CREATE INDEX IF NOT EXISTS idx_messages_slot_id ON chat_messages(slot_id)",
                "CREATE INDEX IF NOT EXISTS idx_messages_depth ON chat_messages(depth)",
                "CREATE INDEX IF NOT EXISTS idx_threads_updated_at ON chat_threads(updated_at)",
                "CREATE INDEX IF NOT EXISTS idx_threads_prompt_app_id ON chat_threads(prompt_app_id)",
                "CREATE INDEX IF NOT EXISTS idx_threads_workspace_id ON chat_threads(workspace_id)",
                "CREATE INDEX IF NOT EXISTS idx_threads_artifact_workspace_id ON chat_threads(artifact_workspace_id)",
                "CREATE INDEX IF NOT EXISTS idx_providers_type ON providers(type)",
                "CREATE INDEX IF NOT EXISTS idx_providers_enabled ON providers(enabled)",
                "CREATE INDEX IF NOT EXISTS idx_provider_models_cache_provider_id ON provider_models_cache(provider_id)",
                "CREATE INDEX IF NOT EXISTS idx_prompt_apps_enabled ON prompt_apps(enabled)",
                "CREATE INDEX IF NOT EXISTS idx_prompt_apps_sort_order ON prompt_apps(sort_order)",
                "CREATE INDEX IF NOT EXISTS idx_prompt_app_executions_prompt_app_id ON prompt_app_executions(prompt_app_id)",
                "CREATE INDEX IF NOT EXISTS idx_prompt_app_executions_thread_id ON prompt_app_executions(thread_id)",
                "CREATE INDEX IF NOT EXISTS idx_prompt_app_executions_created_at ON prompt_app_executions(created_at)",
                "CREATE INDEX IF NOT EXISTS idx_gallery_created_at ON gallery_images(created_at DESC)",
                "CREATE INDEX IF NOT EXISTS idx_gallery_message_id ON gallery_images(message_id)",
                "CREATE INDEX IF NOT EXISTS idx_mcp_servers_enabled ON mcp_servers(enabled)",
                "CREATE INDEX IF NOT EXISTS idx_mcp_servers_registry_id ON mcp_servers(registry_id)",
                "CREATE INDEX IF NOT EXISTS idx_mcp_oauth_tokens_server_id ON mcp_oauth_tokens(server_id)",
                "CREATE INDEX IF NOT EXISTS idx_workspaces_is_temporary ON workspaces(is_temporary)",
                "CREATE INDEX IF NOT EXISTS idx_workspaces_show_in_list ON workspaces(show_in_list)",
                "CREATE INDEX IF NOT EXISTS idx_prompts_sort_order ON prompts(sort_order)",
                "CREATE INDEX IF NOT EXISTS idx_prompts_name ON prompts(name)",
                "CREATE INDEX IF NOT EXISTS idx_skills_enabled ON skills(enabled)",
                "CREATE INDEX IF NOT EXISTS idx_skills_path ON skills(path)",
                "CREATE INDEX IF NOT EXISTS idx_plugins_enabled ON plugins(enabled)",
                "CREATE INDEX IF NOT EXISTS idx_plugins_source ON plugins(source)",
                "CREATE INDEX IF NOT EXISTS idx_plugin_permissions_plugin_id ON plugin_permissions(plugin_id)",
                "CREATE INDEX IF NOT EXISTS idx_custom_themes_name ON custom_themes(name)",
                "CREATE INDEX IF NOT EXISTS idx_custom_themes_type ON custom_themes(type)",
                "CREATE INDEX IF NOT EXISTS idx_usage_records_date ON usage_records(date)",
                "CREATE INDEX IF NOT EXISTS idx_usage_records_model_date ON usage_records(model, date)",
                "CREATE INDEX IF NOT EXISTS idx_usage_records_provider_date ON usage_records(provider_id, date)",
                "CREATE INDEX IF NOT EXISTS idx_usage_records_message_id ON usage_records(message_id)",
                "CREATE INDEX IF NOT EXISTS idx_usage_records_thread_id ON usage_records(thread_id)",
                "CREATE INDEX IF NOT EXISTS idx_preview_servers_workspace_id ON preview_servers(workspace_id)",
            ];
            for sql in indexes {
                exec_ignore(conn, sql);
            }

            exec_ignore(conn, "UPDATE chat_messages SET metadata = '{}' WHERE metadata IS NULL");
            exec_ignore(
                conn,
                "UPDATE chat_messages SET updated_at = created_at WHERE updated_at IS NULL",
            );

            let alter_statements = [
                "ALTER TABLE gallery_images ADD COLUMN thread_title TEXT NOT NULL DEFAULT ''",
                "ALTER TABLE gallery_images ADD COLUMN width INTEGER",
                "ALTER TABLE gallery_images ADD COLUMN height INTEGER",
                "ALTER TABLE gallery_images ADD COLUMN aspect_ratio REAL",
                "ALTER TABLE gallery_images ADD COLUMN file_path TEXT",
                "ALTER TABLE chat_messages ADD COLUMN parent_id TEXT",
                "ALTER TABLE chat_messages ADD COLUMN slot_id TEXT",
                "ALTER TABLE chat_messages ADD COLUMN depth INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE chat_messages ADD COLUMN parent_tool_call_id TEXT",
                "ALTER TABLE chat_threads ADD COLUMN reasoning_effort TEXT DEFAULT 'medium'",
                "ALTER TABLE chat_threads ADD COLUMN prompt_app_id TEXT REFERENCES prompt_apps(id) ON DELETE SET NULL",
                "ALTER TABLE prompt_apps ADD COLUMN tools TEXT",
                "ALTER TABLE prompt_apps ADD COLUMN reasoning_effort TEXT",
                "ALTER TABLE prompt_apps ADD COLUMN expects_image_result INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE prompt_apps ADD COLUMN is_incognito INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE prompt_apps ADD COLUMN shortcut TEXT",
                "ALTER TABLE prompt_apps ADD COLUMN window_width INTEGER",
                "ALTER TABLE prompt_apps ADD COLUMN window_height INTEGER",
                "ALTER TABLE prompt_apps ADD COLUMN font_size INTEGER",
                "ALTER TABLE chat_threads ADD COLUMN tools TEXT",
                "ALTER TABLE chat_threads ADD COLUMN is_favorited INTEGER DEFAULT 0",
                "ALTER TABLE chat_threads ADD COLUMN is_incognito INTEGER DEFAULT 0",
                "ALTER TABLE chat_threads ADD COLUMN is_favorite_pinned INTEGER DEFAULT 0",
                "ALTER TABLE chat_threads ADD COLUMN favorite_pinned_order INTEGER",
                "ALTER TABLE chat_threads ADD COLUMN enable_artifacts INTEGER DEFAULT 0",
                "ALTER TABLE chat_threads ADD COLUMN workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL",
                "ALTER TABLE chat_threads ADD COLUMN artifact_workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL",
                "ALTER TABLE chat_threads ADD COLUMN skill_ids TEXT",
                "ALTER TABLE chat_threads ADD COLUMN tools_compact_view INTEGER",
                "ALTER TABLE chat_threads ADD COLUMN parent_thread_id TEXT",
                "ALTER TABLE workspaces ADD COLUMN is_worktree INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE workspaces ADD COLUMN parent_workspace_id TEXT",
                "ALTER TABLE workspaces ADD COLUMN worktree_branch TEXT",
                "ALTER TABLE workspaces ADD COLUMN auto_worktree INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE workspaces ADD COLUMN pr_number INTEGER",
                "ALTER TABLE workspaces ADD COLUMN pr_url TEXT",
                "ALTER TABLE workspaces ADD COLUMN pr_state TEXT",
                "ALTER TABLE workspaces ADD COLUMN pr_base_branch TEXT",
                "ALTER TABLE providers ADD COLUMN api_version TEXT",
                "ALTER TABLE providers ADD COLUMN is_response_api INTEGER DEFAULT 0",
                "ALTER TABLE providers ADD COLUMN acp_command TEXT",
                "ALTER TABLE providers ADD COLUMN acp_args TEXT",
                "ALTER TABLE providers ADD COLUMN acp_mcp_server_ids TEXT",
                "ALTER TABLE providers ADD COLUMN acp_auth_method_id TEXT",
                "ALTER TABLE providers ADD COLUMN acp_api_provider_id TEXT",
                "ALTER TABLE providers ADD COLUMN acp_model_mapping TEXT",
                "ALTER TABLE providers ADD COLUMN use_max_completion_tokens INTEGER DEFAULT 0",
                "ALTER TABLE providers ADD COLUMN api_format TEXT",
                "ALTER TABLE providers ADD COLUMN copilot_account_id TEXT",
                "ALTER TABLE providers ADD COLUMN available_models TEXT NOT NULL DEFAULT '[]'",
                "ALTER TABLE plugins ADD COLUMN install_url TEXT",
                "ALTER TABLE usage_records ADD COLUMN cache_write_input_tokens INTEGER DEFAULT 0",
            ];
            for sql in alter_statements {
                exec_ignore(conn, sql);
            }

            exec_ignore(
                conn,
                "UPDATE providers SET api_format = CASE WHEN is_response_api = 1 THEN 'openai-responses' ELSE 'openai-chat' END WHERE type = 'custom' AND api_format IS NULL",
            );

            set_schema_meta(conn, "schema_version", SCHEMA_VERSION)?;

            Ok(true)
        })
    }

    #[napi]
    pub fn run_post_schema_migrations(&self) -> Result<bool> {
        self.with_connection(|conn| {
            let mut providers_type_constraint_migrated = false;
            let probe_insert = conn.execute(
                "INSERT INTO providers (id, name, type, api_key, models, enabled, created_at, updated_at) VALUES ('__type_constraint_test__', 'test', '__test_type__', '', '[]', 0, '', '')",
                [],
            );
            match probe_insert {
                Ok(_) => {
                    let _ = conn.execute(
                        "DELETE FROM providers WHERE id = '__type_constraint_test__'",
                        [],
                    );
                }
                Err(err) => {
                    if err.to_string().contains("CHECK constraint failed") {
                        conn.execute_batch("BEGIN TRANSACTION")?;
                        let migration_result = (|| -> rusqlite::Result<()> {
                            conn.execute_batch(
                                "
                                CREATE TABLE providers_new (
                                    id TEXT PRIMARY KEY,
                                    name TEXT NOT NULL,
                                    type TEXT NOT NULL,
                                    api_key TEXT NOT NULL,
                                    models TEXT NOT NULL,
                                    base_url TEXT,
                                    enabled BOOLEAN NOT NULL DEFAULT 1,
                                    created_at TEXT NOT NULL,
                                    updated_at TEXT NOT NULL
                                )",
                            )?;
                            conn.execute_batch(
                                "
                                INSERT INTO providers_new (id, name, type, api_key, models, base_url, enabled, created_at, updated_at)
                                SELECT id, name, type, api_key, models, base_url, enabled, created_at, updated_at FROM providers",
                            )?;
                            conn.execute_batch("DROP TABLE providers")?;
                            conn.execute_batch("ALTER TABLE providers_new RENAME TO providers")?;
                            conn.execute_batch(
                                "CREATE INDEX IF NOT EXISTS idx_providers_type ON providers(type)",
                            )?;
                            conn.execute_batch(
                                "CREATE INDEX IF NOT EXISTS idx_providers_enabled ON providers(enabled)",
                            )?;
                            Ok(())
                        })();

                        if migration_result.is_ok() {
                            conn.execute_batch("COMMIT")?;
                            providers_type_constraint_migrated = true;
                        } else {
                            conn.execute_batch("ROLLBACK")?;
                            migration_result?;
                        }
                    } else {
                        return Err(err);
                    }
                }
            }

            if !has_migration_run(conn, "mcp_oauth_tokens_remove_fk_v1")? {
                let table_sql: Option<String> = conn
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE type='table' AND name='mcp_oauth_tokens'",
                        [],
                        |row| row.get(0),
                    )
                    .optional()?;

                if table_sql
                    .as_deref()
                    .map(|sql| sql.contains("REFERENCES"))
                    .unwrap_or(false)
                {
                    conn.execute_batch("BEGIN TRANSACTION")?;
                    let migration_result = (|| -> rusqlite::Result<()> {
                        conn.execute_batch(
                            "
                            CREATE TABLE mcp_oauth_tokens_new (
                                id TEXT PRIMARY KEY,
                                server_id TEXT NOT NULL,
                                authorization_server_url TEXT,
                                resource_url TEXT,
                                client_id TEXT,
                                client_secret TEXT,
                                client_id_issued_at INTEGER,
                                client_secret_expires_at INTEGER,
                                access_token TEXT,
                                refresh_token TEXT,
                                token_type TEXT,
                                expires_at INTEGER,
                                scope TEXT,
                                code_verifier TEXT,
                                last_refresh_at TEXT,
                                last_error_at TEXT,
                                last_error TEXT,
                                created_at TEXT NOT NULL,
                                updated_at TEXT NOT NULL
                            )",
                        )?;
                        conn.execute_batch(
                            "INSERT INTO mcp_oauth_tokens_new SELECT * FROM mcp_oauth_tokens",
                        )?;
                        conn.execute_batch("DROP TABLE mcp_oauth_tokens")?;
                        conn.execute_batch(
                            "ALTER TABLE mcp_oauth_tokens_new RENAME TO mcp_oauth_tokens",
                        )?;
                        conn.execute_batch(
                            "CREATE INDEX IF NOT EXISTS idx_mcp_oauth_tokens_server_id ON mcp_oauth_tokens(server_id)",
                        )?;
                        Ok(())
                    })();

                    if migration_result.is_ok() {
                        conn.execute_batch("COMMIT")?;
                    } else {
                        conn.execute_batch("ROLLBACK")?;
                        migration_result?;
                    }
                }
                mark_migration_complete(conn, "mcp_oauth_tokens_remove_fk_v1")?;
            }

            if !has_migration_run(conn, "duplicate_predefined_providers_v1")? {
                let canonical = ["openai", "anthropic", "gemini", "openrouter"];
                for stable_id in canonical {
                    let provider_type = if stable_id == "gemini" { "google" } else { stable_id };
                    let mut stmt = conn.prepare(
                        "SELECT id, api_key, base_url, models, enabled FROM providers WHERE type = ?1 ORDER BY created_at ASC",
                    )?;
                    let rows = stmt
                        .query_map([provider_type], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, Option<String>>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, i64>(4)?,
                            ))
                        })?
                        .collect::<rusqlite::Result<Vec<_>>>()?;
                    if rows.len() <= 1 {
                        continue;
                    }

                    let canonical_row = rows.iter().find(|(id, _, _, _, _)| id == stable_id);
                    if let Some((_, canonical_api_key, canonical_base_url, canonical_models, canonical_enabled)) = canonical_row
                    {
                        for (id, api_key, base_url, models, enabled) in &rows {
                            if id == stable_id {
                                continue;
                            }
                            if !api_key.is_empty() && canonical_api_key.is_empty() {
                                let merged_enabled = if *canonical_enabled == 1 || *enabled == 1 {
                                    1
                                } else {
                                    0
                                };
                                conn.execute(
                                    "UPDATE providers SET api_key = ?2, base_url = ?3, models = ?4, enabled = ?5 WHERE id = ?1",
                                    params![stable_id, api_key, base_url, models, merged_enabled],
                                )?;
                            } else {
                                let _ = (canonical_base_url, canonical_models);
                            }
                            conn.execute("DELETE FROM providers WHERE id = ?1", [id])?;
                        }
                    } else {
                        let preferred = rows
                            .iter()
                            .find(|(_, api_key, _, _, _)| !api_key.is_empty())
                            .unwrap_or(&rows[0]);

                        conn.execute(
                            "UPDATE providers SET id = ?2 WHERE id = ?1",
                            params![preferred.0, stable_id],
                        )?;

                        for (id, api_key, base_url, models, enabled) in &rows {
                            if id == &preferred.0 {
                                continue;
                            }
                            if !api_key.is_empty() {
                                let canonical_state: Option<(String, i64)> = conn
                                    .query_row(
                                        "SELECT api_key, enabled FROM providers WHERE id = ?1",
                                        [stable_id],
                                        |row| Ok((row.get(0)?, row.get(1)?)),
                                    )
                                    .optional()?;
                                if let Some((existing_api_key, existing_enabled)) = canonical_state {
                                    if existing_api_key.is_empty() {
                                        let merged_enabled =
                                            if existing_enabled == 1 || *enabled == 1 { 1 } else { 0 };
                                        conn.execute(
                                            "UPDATE providers SET api_key = ?2, base_url = ?3, models = ?4, enabled = ?5 WHERE id = ?1",
                                            params![stable_id, api_key, base_url, models, merged_enabled],
                                        )?;
                                    }
                                }
                            }
                            conn.execute("DELETE FROM providers WHERE id = ?1", [id])?;
                        }
                    }
                }
                mark_migration_complete(conn, "duplicate_predefined_providers_v1")?;
            }

            if !has_migration_run(conn, "provider_available_models_v1")? {
                exec_ignore(
                    conn,
                    "ALTER TABLE providers ADD COLUMN available_models TEXT NOT NULL DEFAULT '[]'",
                );

                let mut stmt = conn.prepare("SELECT id, models, available_models FROM providers")?;
                let providers = stmt
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                for (provider_id, models_raw, available_raw) in providers {
                    let enabled_models = parse_json_array(&models_raw);
                    let mut available = parse_json_array(&available_raw);

                    let mut available_by_id: HashSet<String> = available
                        .iter()
                        .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
                        .collect();

                    if available.is_empty() {
                        let cache_raw: Option<String> = conn
                            .query_row(
                                "SELECT models FROM provider_models_cache WHERE provider_id = ?1 ORDER BY fetched_at DESC LIMIT 1",
                                [provider_id.as_str()],
                                |row| row.get(0),
                            )
                            .optional()?;
                        if let Some(raw) = cache_raw {
                            available = parse_json_array(&raw);
                            available_by_id = available
                                .iter()
                                .filter_map(|item| {
                                    item.get("id").and_then(Value::as_str).map(str::to_string)
                                })
                                .collect();
                        }
                    }

                    let mut changed = false;
                    for model in enabled_models {
                        let (model_id, model_name, capability_overrides) = if let Some(id) = model.as_str() {
                            (id.to_string(), id.to_string(), None)
                        } else if let Some(obj) = model.as_object() {
                            let id = obj
                                .get("id")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string();
                            if id.is_empty() {
                                continue;
                            }
                            let name = obj
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or(id.as_str())
                                .to_string();
                            let caps = obj
                                .get("capabilityOverrides")
                                .cloned()
                                .or_else(|| obj.get("capabilities").cloned());
                            (id, name, caps)
                        } else {
                            continue;
                        };

                        if available_by_id.contains(&model_id) {
                            continue;
                        }

                        let mut inserted = Map::new();
                        inserted.insert("id".to_string(), Value::String(model_id.clone()));
                        inserted.insert("name".to_string(), Value::String(model_name));
                        if let Some(caps) = capability_overrides {
                            inserted.insert("capabilityOverrides".to_string(), caps);
                        }
                        inserted.insert("isManual".to_string(), Value::Bool(true));
                        available.push(Value::Object(inserted));
                        available_by_id.insert(model_id);
                        changed = true;
                    }

                    if changed || !available.is_empty() {
                        conn.execute(
                            "UPDATE providers SET available_models = ?2 WHERE id = ?1",
                            params![provider_id, Value::Array(available).to_string()],
                        )?;
                    }
                }

                mark_migration_complete(conn, "provider_available_models_v1")?;
            }

            if !has_migration_run(conn, "provider_models_to_ids_v1")? {
                let mut stmt = conn.prepare("SELECT id, models, available_models FROM providers")?;
                let providers = stmt
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                for (provider_id, models_raw, available_raw) in providers {
                    let mut available = parse_json_array(&available_raw);
                    let mut available_by_id: HashSet<String> = available
                        .iter()
                        .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
                        .collect();

                    if available.is_empty() {
                        let cache_raw: Option<String> = conn
                            .query_row(
                                "SELECT models FROM provider_models_cache WHERE provider_id = ?1 ORDER BY fetched_at DESC LIMIT 1",
                                [provider_id.as_str()],
                                |row| row.get(0),
                            )
                            .optional()?;
                        if let Some(raw) = cache_raw {
                            available = parse_json_array(&raw);
                            available_by_id = available
                                .iter()
                                .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
                                .collect();
                        }
                    }

                    let enabled_models = parse_json_array(&models_raw);
                    let mut changed_available = false;
                    for model in enabled_models {
                        let model_id = if let Some(id) = model.as_str() {
                            id.to_string()
                        } else if let Some(obj) = model.as_object() {
                            obj.get("id")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string()
                        } else {
                            String::new()
                        };
                        if model_id.is_empty() || available_by_id.contains(&model_id) {
                            continue;
                        }

                        let mut inserted = Map::new();
                        inserted.insert("id".to_string(), Value::String(model_id.clone()));
                        inserted.insert("name".to_string(), Value::String(model_id.clone()));
                        if let Some(obj) = model.as_object() {
                            if let Some(name) = obj.get("name").and_then(Value::as_str) {
                                inserted.insert("name".to_string(), Value::String(name.to_string()));
                            }
                            if let Some(capabilities) = obj.get("capabilities") {
                                inserted.insert(
                                    "capabilityOverrides".to_string(),
                                    capabilities.clone(),
                                );
                            }
                            if let Some(provider_options) = obj.get("providerOptions") {
                                inserted.insert("providerOptions".to_string(), provider_options.clone());
                            }
                        }
                        inserted.insert("isManual".to_string(), Value::Bool(true));
                        available.push(Value::Object(inserted));
                        available_by_id.insert(model_id);
                        changed_available = true;
                    }

                    if changed_available {
                        conn.execute(
                            "UPDATE providers SET available_models = ?2 WHERE id = ?1",
                            params![provider_id, Value::Array(available).to_string()],
                        )?;
                    }
                }

                let mut stmt = conn.prepare("SELECT id, models, available_models FROM providers")?;
                let providers = stmt
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                for (provider_id, models_raw, available_raw) in providers {
                    let models = parse_json_array(&models_raw);
                    let first_is_string = models
                        .first()
                        .map(|v| v.is_string())
                        .unwrap_or(true);
                    if models.is_empty() || first_is_string {
                        continue;
                    }

                    let mut model_ids: Vec<String> = Vec::new();
                    let mut available = parse_json_array(&available_raw);
                    let mut changed_available = false;
                    let mut available_by_id: HashSet<String> = available
                        .iter()
                        .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
                        .collect();

                    for model in models {
                        if let Some(id) = model.as_str() {
                            model_ids.push(id.to_string());
                            continue;
                        }
                        let Some(model_obj) = model.as_object() else {
                            continue;
                        };
                        let Some(id) = model_obj.get("id").and_then(Value::as_str) else {
                            continue;
                        };
                        model_ids.push(id.to_string());

                        if !available_by_id.contains(id) {
                            let mut inserted = Map::new();
                            inserted.insert("id".to_string(), Value::String(id.to_string()));
                            inserted.insert(
                                "name".to_string(),
                                Value::String(
                                    model_obj
                                        .get("name")
                                        .and_then(Value::as_str)
                                        .unwrap_or(id)
                                        .to_string(),
                                ),
                            );
                            if let Some(capabilities) = model_obj.get("capabilities") {
                                inserted.insert(
                                    "capabilityOverrides".to_string(),
                                    capabilities.clone(),
                                );
                            }
                            if let Some(provider_options) = model_obj.get("providerOptions") {
                                inserted.insert("providerOptions".to_string(), provider_options.clone());
                            }
                            inserted.insert("isManual".to_string(), Value::Bool(true));
                            available.push(Value::Object(inserted));
                            available_by_id.insert(id.to_string());
                            changed_available = true;
                        }
                    }

                    if changed_available {
                        conn.execute(
                            "UPDATE providers SET models = ?2, available_models = ?3 WHERE id = ?1",
                            params![
                                provider_id,
                                Value::Array(
                                    model_ids.iter().map(|id| Value::String(id.clone())).collect()
                                )
                                .to_string(),
                                Value::Array(available).to_string(),
                            ],
                        )?;
                    } else {
                        conn.execute(
                            "UPDATE providers SET models = ?2 WHERE id = ?1",
                            params![
                                provider_id,
                                Value::Array(
                                    model_ids.iter().map(|id| Value::String(id.clone())).collect()
                                )
                                .to_string(),
                            ],
                        )?;
                    }
                }

                mark_migration_complete(conn, "provider_models_to_ids_v1")?;
            }

            if !has_migration_run(conn, "capabilities_to_capability_overrides_v1")? {
                let mut stmt = conn.prepare("SELECT id, available_models FROM providers")?;
                let providers = stmt
                    .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                for (provider_id, available_raw) in providers {
                    let mut available = parse_json_array(&available_raw);
                    let mut changed = false;
                    for item in &mut available {
                        let Some(obj) = item.as_object_mut() else {
                            continue;
                        };
                        if let Some(capabilities) = obj.remove("capabilities") {
                            obj.insert("capabilityOverrides".to_string(), capabilities);
                            changed = true;
                        }
                    }
                    if changed {
                        conn.execute(
                            "UPDATE providers SET available_models = ?2 WHERE id = ?1",
                            params![provider_id, Value::Array(available).to_string()],
                        )?;
                    }
                }

                mark_migration_complete(conn, "capabilities_to_capability_overrides_v1")?;
            }

            if providers_type_constraint_migrated {
                set_schema_meta(conn, "providers_type_constraint_removed", 1)?;
            }
            set_schema_meta(conn, "post_migrations_version", POST_MIGRATIONS_VERSION)?;

            Ok(true)
        })
    }

    #[napi]
    pub fn get_schema_migration_status(&self) -> Result<String> {
        self.with_connection(|conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            )?;
            let schema_version = get_schema_meta_i64(conn, "schema_version")?.unwrap_or(0);
            let post_version = get_schema_meta_i64(conn, "post_migrations_version")?.unwrap_or(0);
            let providers_type_removed =
                get_schema_meta_i64(conn, "providers_type_constraint_removed")?.unwrap_or(0) == 1;

            let payload = json!({
                "schemaVersion": {
                    "current": schema_version,
                    "target": SCHEMA_VERSION,
                },
                "postMigrationsVersion": {
                    "current": post_version,
                    "target": POST_MIGRATIONS_VERSION,
                },
                "migrations": {
                    "mcp_oauth_tokens_remove_fk_v1": has_migration_run(conn, "mcp_oauth_tokens_remove_fk_v1")?,
                    "duplicate_predefined_providers_v1": has_migration_run(conn, "duplicate_predefined_providers_v1")?,
                    "provider_available_models_v1": has_migration_run(conn, "provider_available_models_v1")?,
                    "provider_models_to_ids_v1": has_migration_run(conn, "provider_models_to_ids_v1")?,
                    "capabilities_to_capability_overrides_v1": has_migration_run(conn, "capabilities_to_capability_overrides_v1")?,
                },
                "providersTypeConstraintRemoved": providers_type_removed,
            });

            Ok(payload.to_string())
        })
    }
}
