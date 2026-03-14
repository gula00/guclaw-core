use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, params_from_iter, OptionalExtension};
use serde_json::Value;

use crate::mappers::{
    map_usage_activity_row, map_usage_migration_status_row, map_usage_record_row,
    map_usage_stat_row, USAGE_MIGRATION_STATUS_SELECT_SQL, USAGE_RECORD_SELECT_SQL,
};
use crate::{
    DbHandle, GetUsageStatsInput, SaveUsageRecordInput, UpdateUsageMigrationStatusInput,
    UsageMigrationBatchItem, UsageMigrationStatusRecord, UsageRecord, UsageStatsResult,
};

fn json_usage_i32(root: &Value, key: &str) -> Option<i32> {
    root.get("usage")
        .and_then(|usage| usage.get(key))
        .and_then(Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
}

#[napi]
impl DbHandle {
    #[napi]
    pub fn save_usage_record(&self, input: SaveUsageRecordInput) -> Result<Option<UsageRecord>> {
        self.with_connection(|conn| {
            let provider_id = input
                .model
                .as_ref()
                .and_then(|model| model.split_once(':').map(|(provider_id, _)| provider_id))
                .map(str::to_string);
            let date = input.timestamp.split('T').next().unwrap_or("").to_string();
            let total_tokens = input.total_tokens.unwrap_or(
                input.input_tokens.unwrap_or(0) + input.output_tokens.unwrap_or(0),
            );
            let record_id = format!("usage_{}", input.message_id);

            conn.execute(
                "INSERT INTO usage_records (
                    id, message_id, thread_id, model, provider_id, date,
                    input_tokens, output_tokens, cached_input_tokens, cache_write_input_tokens,
                    reasoning_tokens, total_tokens, timestamp, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ON CONFLICT(id) DO UPDATE SET
                    input_tokens = excluded.input_tokens,
                    output_tokens = excluded.output_tokens,
                    cached_input_tokens = excluded.cached_input_tokens,
                    cache_write_input_tokens = excluded.cache_write_input_tokens,
                    reasoning_tokens = excluded.reasoning_tokens,
                    total_tokens = excluded.total_tokens",
                params![
                    record_id,
                    input.message_id,
                    input.thread_id,
                    input.model,
                    provider_id,
                    date,
                    input.input_tokens.unwrap_or(0),
                    input.output_tokens.unwrap_or(0),
                    input.cached_input_tokens.unwrap_or(0),
                    input.cache_write_input_tokens.unwrap_or(0),
                    input.reasoning_tokens.unwrap_or(0),
                    total_tokens,
                    input.timestamp,
                ],
            )?;

            let sql = format!("{} WHERE id = ?1", USAGE_RECORD_SELECT_SQL);
            conn.query_row(&sql, [record_id], |row| map_usage_record_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_usage_stats(&self, input: Option<GetUsageStatsInput>) -> Result<UsageStatsResult> {
        self.with_connection(|conn| {
            let mut conditions: Vec<&str> = Vec::new();
            let mut args: Vec<String> = Vec::new();

            if let Some(input) = input.as_ref() {
                if let Some(from_date) = input.from_date.as_ref() {
                    conditions.push("timestamp >= ?");
                    args.push(from_date.clone());
                }
                if let Some(to_date) = input.to_date.as_ref() {
                    conditions.push("timestamp <= ?");
                    args.push(to_date.clone());
                }
                if let Some(provider_id) = input.provider_id.as_ref() {
                    conditions.push("provider_id = ?");
                    args.push(provider_id.clone());
                }
                if let Some(model_id) = input.model_id.as_ref() {
                    conditions.push("(model LIKE '%:' || ? OR model = ?)");
                    args.push(model_id.clone());
                    args.push(model_id.clone());
                }
            }

            let mut by_model_sql = "
                SELECT
                    COALESCE(model, 'unknown') as model,
                    date,
                    SUM(COALESCE(input_tokens, 0)) as inputTokens,
                    SUM(COALESCE(output_tokens, 0)) as outputTokens,
                    SUM(COALESCE(cached_input_tokens, 0)) as cachedTokens,
                    SUM(COALESCE(cache_write_input_tokens, 0)) as cacheWriteTokens,
                    SUM(COALESCE(reasoning_tokens, 0)) as reasoningTokens,
                    SUM(COALESCE(total_tokens, 0)) as totalTokens,
                    COUNT(*) as messageCount
                FROM usage_records
            "
            .to_string();
            if !conditions.is_empty() {
                by_model_sql.push_str(" WHERE ");
                by_model_sql.push_str(&conditions.join(" AND "));
            }
            by_model_sql.push_str(" GROUP BY model, date ORDER BY date ASC, model ASC");

            let mut stmt = conn.prepare(&by_model_sql)?;
            let by_model_and_date = stmt
                .query_map(params_from_iter(args.iter()), |row| map_usage_stat_row(row))?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let mut one_year_ago_stmt = conn.prepare(
                "
                SELECT
                    date,
                    SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)) as totalTokens
                FROM usage_records
                WHERE date >= date('now', '-1 year')
                GROUP BY date
                ORDER BY date ASC
                ",
            )?;
            let activity_by_date = one_year_ago_stmt
                .query_map([], |row| map_usage_activity_row(row))?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            Ok(UsageStatsResult {
                by_model_and_date,
                activity_by_date,
            })
        })
    }

    #[napi]
    pub fn get_usage_migration_status(&self) -> Result<Option<UsageMigrationStatusRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                &format!("{} WHERE id = 1", USAGE_MIGRATION_STATUS_SELECT_SQL),
                [],
                |row| map_usage_migration_status_row(row),
            )
            .optional()
        })
    }

    #[napi]
    pub fn init_usage_migration_status(&self) -> Result<bool> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO usage_migration_status (id, status, total_count, migrated_count) VALUES (1, 'pending', 0, 0)",
                [],
            )?;
            Ok(true)
        })
    }

    #[napi]
    pub fn update_usage_migration_status(
        &self,
        input: UpdateUsageMigrationStatusInput,
    ) -> Result<Option<UsageMigrationStatusRecord>> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO usage_migration_status (id, status, total_count, migrated_count) VALUES (1, 'pending', 0, 0)",
                [],
            )?;

            if let Some(status) = input.status {
                conn.execute(
                    "UPDATE usage_migration_status SET status = ?1 WHERE id = 1",
                    [status],
                )?;
            }
            if let Some(total_count) = input.total_count {
                conn.execute(
                    "UPDATE usage_migration_status SET total_count = ?1 WHERE id = 1",
                    [total_count],
                )?;
            }
            if let Some(migrated_count) = input.migrated_count {
                conn.execute(
                    "UPDATE usage_migration_status SET migrated_count = ?1 WHERE id = 1",
                    [migrated_count],
                )?;
            }
            if let Some(last_migrated_id) = input.last_migrated_id {
                conn.execute(
                    "UPDATE usage_migration_status SET last_migrated_id = ?1 WHERE id = 1",
                    [last_migrated_id],
                )?;
            }
            if let Some(started_at) = input.started_at {
                conn.execute(
                    "UPDATE usage_migration_status SET started_at = ?1 WHERE id = 1",
                    [started_at],
                )?;
            }
            if let Some(completed_at) = input.completed_at {
                conn.execute(
                    "UPDATE usage_migration_status SET completed_at = ?1 WHERE id = 1",
                    [completed_at],
                )?;
            }
            if input.error_message.is_some() {
                conn.execute(
                    "UPDATE usage_migration_status SET error_message = ?1 WHERE id = 1",
                    [input.error_message],
                )?;
            }

            conn.query_row(
                &format!("{} WHERE id = 1", USAGE_MIGRATION_STATUS_SELECT_SQL),
                [],
                |row| map_usage_migration_status_row(row),
            )
            .optional()
        })
    }

    #[napi]
    pub fn get_messages_with_usage_count(&self) -> Result<i32> {
        self.with_connection(|conn| {
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM chat_messages WHERE json_extract(metadata, '$.usage') IS NOT NULL",
                [],
                |row| row.get(0),
            )?;
            Ok(count)
        })
    }

    #[napi]
    pub fn get_next_batch_to_migrate(
        &self,
        last_message_id: Option<String>,
        limit: i32,
    ) -> Result<Vec<UsageMigrationBatchItem>> {
        self.with_connection(|conn| {
            let sql = if last_message_id.is_some() {
                "
                SELECT m.id, m.thread_id, t.model, m.timestamp, m.metadata
                FROM chat_messages m
                JOIN chat_threads t ON m.thread_id = t.id
                WHERE json_extract(m.metadata, '$.usage') IS NOT NULL
                  AND m.id > ?1
                ORDER BY m.id ASC
                LIMIT ?2
                "
            } else {
                "
                SELECT m.id, m.thread_id, t.model, m.timestamp, m.metadata
                FROM chat_messages m
                JOIN chat_threads t ON m.thread_id = t.id
                WHERE json_extract(m.metadata, '$.usage') IS NOT NULL
                ORDER BY m.id ASC
                LIMIT ?1
                "
            };

            let mut stmt = conn.prepare(sql)?;
            let mapped = if let Some(last_message_id) = last_message_id {
                stmt.query_map(params![last_message_id, limit], |row| {
                    let metadata = row.get::<_, String>(4)?;
                    let parsed = serde_json::from_str::<Value>(&metadata).unwrap_or(Value::Null);
                    let input_tokens = json_usage_i32(&parsed, "inputTokens");
                    let output_tokens = json_usage_i32(&parsed, "outputTokens");
                    let cached_input_tokens = json_usage_i32(&parsed, "cachedInputTokens");
                    let cache_write_input_tokens = json_usage_i32(&parsed, "cacheWriteInputTokens");
                    let reasoning_tokens = json_usage_i32(&parsed, "reasoningTokens");
                    let total_tokens = json_usage_i32(&parsed, "totalTokens")
                        .or_else(|| Some(input_tokens.unwrap_or(0) + output_tokens.unwrap_or(0)));
                    Ok(UsageMigrationBatchItem {
                        message_id: row.get(0)?,
                        thread_id: row.get(1)?,
                        model: row.get(2)?,
                        timestamp: row.get(3)?,
                        input_tokens,
                        output_tokens,
                        cached_input_tokens,
                        cache_write_input_tokens,
                        reasoning_tokens,
                        total_tokens,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
            } else {
                stmt.query_map([limit], |row| {
                    let metadata = row.get::<_, String>(4)?;
                    let parsed = serde_json::from_str::<Value>(&metadata).unwrap_or(Value::Null);
                    let input_tokens = json_usage_i32(&parsed, "inputTokens");
                    let output_tokens = json_usage_i32(&parsed, "outputTokens");
                    let cached_input_tokens = json_usage_i32(&parsed, "cachedInputTokens");
                    let cache_write_input_tokens = json_usage_i32(&parsed, "cacheWriteInputTokens");
                    let reasoning_tokens = json_usage_i32(&parsed, "reasoningTokens");
                    let total_tokens = json_usage_i32(&parsed, "totalTokens")
                        .or_else(|| Some(input_tokens.unwrap_or(0) + output_tokens.unwrap_or(0)));
                    Ok(UsageMigrationBatchItem {
                        message_id: row.get(0)?,
                        thread_id: row.get(1)?,
                        model: row.get(2)?,
                        timestamp: row.get(3)?,
                        input_tokens,
                        output_tokens,
                        cached_input_tokens,
                        cache_write_input_tokens,
                        reasoning_tokens,
                        total_tokens,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
            };
            Ok(mapped)
        })
    }

    #[napi]
    pub fn has_usage_record(&self, message_id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let exists: Option<i32> = conn
                .query_row(
                    "SELECT 1 FROM usage_records WHERE message_id = ?1 LIMIT 1",
                    [message_id],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(exists.is_some())
        })
    }

    #[napi]
    pub fn insert_usage_records_batch(&self, items: Vec<UsageMigrationBatchItem>) -> Result<bool> {
        self.with_connection(|conn| {
            if items.is_empty() {
                return Ok(true);
            }
            let tx = conn.transaction()?;
            let now = tx
                .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
                    row.get::<_, String>(0)
                })?;
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO usage_records (
                    id, message_id, thread_id, model, provider_id, date,
                    input_tokens, output_tokens, cached_input_tokens, cache_write_input_tokens,
                    reasoning_tokens, total_tokens, timestamp, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?;
            for item in items {
                let provider_id = item
                    .model
                    .as_ref()
                    .and_then(|model| model.split_once(':').map(|(provider_id, _)| provider_id))
                    .map(str::to_string);
                let date = item.timestamp.split('T').next().unwrap_or("").to_string();
                let total_tokens = item
                    .total_tokens
                    .unwrap_or(item.input_tokens.unwrap_or(0) + item.output_tokens.unwrap_or(0));
                stmt.execute(params![
                    format!("usage_{}", item.message_id),
                    item.message_id,
                    item.thread_id,
                    item.model,
                    provider_id,
                    date,
                    item.input_tokens.unwrap_or(0),
                    item.output_tokens.unwrap_or(0),
                    item.cached_input_tokens.unwrap_or(0),
                    item.cache_write_input_tokens.unwrap_or(0),
                    item.reasoning_tokens.unwrap_or(0),
                    total_tokens,
                    item.timestamp,
                    now,
                ])?;
            }
            drop(stmt);
            tx.commit()?;
            Ok(true)
        })
    }

    #[napi]
    pub fn get_usage_records_count(&self) -> Result<i32> {
        self.with_connection(|conn| {
            let count: i32 =
                conn.query_row("SELECT COUNT(*) FROM usage_records", [], |row| row.get(0))?;
            Ok(count)
        })
    }
}
