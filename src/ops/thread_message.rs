use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, params_from_iter, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

use crate::mappers::{map_message_row, map_thread_row, table_exists, THREAD_SELECT_SQL};
use crate::{
    AddMessageInput, CreateThreadInput, DbHandle, FtsEntryInput, FtsHitRecord,
    MessageMetadataRecord, MessageRecord, MessageVersionInfoRecord, SearchFtsInput,
    SearchThreadsInput, ThreadRecord, UpdateFtsForMessageInput, UpdateMessageContentInput,
    UpdateMessageMetadataInput, UpdateThreadCoreInput, UpdateThreadFieldsInput,
};

fn parse_metadata_object(raw: &str) -> Map<String, Value> {
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

fn enrich_messages_with_version_metadata(
    messages: &mut [MessageRecord],
    version_info: &[MessageVersionInfoRecord],
) {
    let mut versions_by_slot: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for item in version_info {
        let slot_key = item.slot_id.clone().unwrap_or_else(|| item.id.clone());
        versions_by_slot
            .entry(slot_key)
            .or_default()
            .push((item.id.clone(), item.created_at.clone()));
    }

    for entries in versions_by_slot.values_mut() {
        entries.sort_by(|a, b| a.1.cmp(&b.1));
    }

    for (index, message) in messages.iter_mut().enumerate() {
        let slot_key = message
            .slot_id
            .clone()
            .unwrap_or_else(|| message.id.clone());
        let fallback = vec![(message.id.clone(), message.created_at.clone())];
        let versions = versions_by_slot.get(&slot_key).unwrap_or(&fallback);
        let version_index = versions
            .iter()
            .position(|(id, _)| id == &message.id)
            .unwrap_or(0);

        let mut metadata = parse_metadata_object(&message.metadata);
        metadata.insert("slotId".to_string(), Value::String(slot_key));
        metadata.insert(
            "depth".to_string(),
            Value::from(message.depth.unwrap_or(index as i32)),
        );
        metadata.insert(
            "versionIndex".to_string(),
            Value::from(version_index as i64),
        );
        metadata.insert(
            "versionCount".to_string(),
            Value::from(versions.len() as i64),
        );

        message.metadata = Value::Object(metadata).to_string();
    }
}

#[napi]
impl DbHandle {
    #[napi]
    pub fn get_thread(&self, id: String) -> Result<Option<ThreadRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", THREAD_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_thread_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_all_threads(&self) -> Result<Vec<ThreadRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} WHERE prompt_app_id IS NULL ORDER BY is_favorited DESC, is_favorite_pinned DESC, favorite_pinned_order DESC, updated_at DESC",
                THREAD_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_thread_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_threads_by_ids(&self, ids: Vec<String>) -> Result<Vec<ThreadRecord>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        self.with_connection(|conn| {
            let placeholders = std::iter::repeat("?")
                .take(ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!("{} WHERE id IN ({})", THREAD_SELECT_SQL, placeholders);
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map(params_from_iter(ids.iter()), |row| map_thread_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn update_thread_core(&self, input: UpdateThreadCoreInput) -> Result<Option<ThreadRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;

            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM chat_threads WHERE id = ?1",
                    [&input.id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                tx.rollback()?;
                return Ok(None);
            }

            if let Some(metadata) = input.metadata {
                tx.execute(
                    "UPDATE chat_threads SET metadata = ?2 WHERE id = ?1",
                    params![input.id, metadata],
                )?;
            }

            if let Some(is_generating) = input.is_generating {
                tx.execute(
                    "UPDATE chat_threads SET is_generating = ?2 WHERE id = ?1",
                    params![input.id, is_generating],
                )?;
            }

            if let Some(reasoning_effort) = input.reasoning_effort {
                tx.execute(
                    "UPDATE chat_threads SET reasoning_effort = ?2 WHERE id = ?1",
                    params![input.id, reasoning_effort],
                )?;
            }

            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            tx.execute(
                &format!(
                    "UPDATE chat_threads SET updated_at = COALESCE(?2, {now}) WHERE id = ?1",
                    now = now_expr
                ),
                params![input.id, input.updated_at],
            )?;

            let row = tx
                .query_row(
                    &format!("{} WHERE id = ?1", THREAD_SELECT_SQL),
                    [input.id],
                    |row| map_thread_row(row),
                )
                .optional()?;

            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn update_thread_fields(
        &self,
        input: UpdateThreadFieldsInput,
    ) -> Result<Option<ThreadRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;

            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM chat_threads WHERE id = ?1",
                    [&input.id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                tx.rollback()?;
                return Ok(None);
            }

            if input.set_title.unwrap_or(false) {
                tx.execute(
                    "UPDATE chat_threads SET title = ?2 WHERE id = ?1",
                    params![input.id, input.title],
                )?;
            }

            if input.set_model.unwrap_or(false) {
                tx.execute(
                    "UPDATE chat_threads SET model = ?2 WHERE id = ?1",
                    params![input.id, input.model],
                )?;
            }

            if input.set_workspace_id.unwrap_or(false) {
                tx.execute(
                    "UPDATE chat_threads SET workspace_id = ?2 WHERE id = ?1",
                    params![input.id, input.workspace_id],
                )?;
            }

            if input.set_artifact_workspace_id.unwrap_or(false) {
                tx.execute(
                    "UPDATE chat_threads SET artifact_workspace_id = ?2 WHERE id = ?1",
                    params![input.id, input.artifact_workspace_id],
                )?;
            }

            if input.set_enable_artifacts.unwrap_or(false) {
                tx.execute(
                    "UPDATE chat_threads SET enable_artifacts = ?2 WHERE id = ?1",
                    params![input.id, input.enable_artifacts],
                )?;
            }

            if input.set_parent_thread_id.unwrap_or(false) {
                tx.execute(
                    "UPDATE chat_threads SET parent_thread_id = ?2 WHERE id = ?1",
                    params![input.id, input.parent_thread_id],
                )?;
            }

            if input.set_metadata.unwrap_or(false) {
                tx.execute(
                    "UPDATE chat_threads SET metadata = ?2 WHERE id = ?1",
                    params![input.id, input.metadata],
                )?;
            }

            if input.set_is_generating.unwrap_or(false) {
                tx.execute(
                    "UPDATE chat_threads SET is_generating = ?2 WHERE id = ?1",
                    params![input.id, input.is_generating],
                )?;
            }

            if input.set_reasoning_effort.unwrap_or(false) {
                tx.execute(
                    "UPDATE chat_threads SET reasoning_effort = ?2 WHERE id = ?1",
                    params![input.id, input.reasoning_effort],
                )?;
            }

            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            if input.set_updated_at.unwrap_or(false) {
                tx.execute(
                    &format!(
                        "UPDATE chat_threads SET updated_at = COALESCE(?2, {now}) WHERE id = ?1",
                        now = now_expr
                    ),
                    params![input.id, input.updated_at],
                )?;
            }

            let row = tx
                .query_row(
                    &format!("{} WHERE id = ?1", THREAD_SELECT_SQL),
                    [input.id],
                    |row| map_thread_row(row),
                )
                .optional()?;

            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn get_messages_by_thread_id(&self, thread_id: String) -> Result<Vec<MessageRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                 FROM chat_messages WHERE thread_id = ?1 ORDER BY timestamp ASC",
            )?;

            let mapped = stmt.query_map([thread_id], |row| map_message_row(row))?;

            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_messages_by_tool_call_id(&self, tool_call_id: String) -> Result<Vec<MessageRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                 FROM chat_messages WHERE parent_tool_call_id = ?1 ORDER BY timestamp ASC",
            )?;

            let mapped = stmt.query_map([tool_call_id], |row| map_message_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_subagent_messages_by_thread_id(
        &self,
        thread_id: String,
    ) -> Result<Vec<MessageRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                 FROM chat_messages WHERE thread_id = ?1 AND parent_tool_call_id IS NOT NULL ORDER BY timestamp ASC",
            )?;

            let mapped = stmt.query_map([thread_id], |row| map_message_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_message_version_info_by_thread_id(
        &self,
        thread_id: String,
    ) -> Result<Vec<MessageVersionInfoRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, slot_id, created_at FROM chat_messages WHERE thread_id = ?1 ORDER BY timestamp ASC",
            )?;

            let mapped = stmt.query_map([thread_id], |row| {
                Ok(MessageVersionInfoRecord {
                    id: row.get(0)?,
                    slot_id: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })?;

            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_message_metadata_by_thread_id(
        &self,
        thread_id: String,
    ) -> Result<Vec<MessageMetadataRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, metadata FROM chat_messages WHERE thread_id = ?1 ORDER BY timestamp ASC",
            )?;

            let mapped = stmt.query_map([thread_id], |row| {
                Ok(MessageMetadataRecord {
                    id: row.get(0)?,
                    metadata: row.get(1)?,
                })
            })?;

            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_message_by_id(&self, id: String) -> Result<Option<MessageRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                 FROM chat_messages WHERE id = ?1",
                [id],
                |row| map_message_row(row),
            )
            .optional()
        })
    }

    #[napi]
    pub fn get_thread_with_messages(&self, thread_id: String) -> Result<Option<String>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let thread_sql = format!("{} WHERE id = ?1", THREAD_SELECT_SQL);

            let mut thread = tx
                .query_row(&thread_sql, [&thread_id], |row| map_thread_row(row))
                .optional()?;
            let Some(mut thread_record) = thread.take() else {
                tx.rollback()?;
                return Ok(None);
            };

            let mut stmt = tx.prepare(
                "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                 FROM chat_messages WHERE thread_id = ?1 ORDER BY timestamp ASC",
            )?;
            let all_messages = stmt
                .query_map([&thread_id], |row| map_message_row(row))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(stmt);

            let mut thread_metadata = parse_metadata_object(&thread_record.metadata);
            let mut active_path = thread_metadata
                .get("activePath")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(|id| id.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            if active_path.is_empty() {
                active_path = all_messages.iter().map(|message| message.id.clone()).collect();
                thread_metadata.insert(
                    "activePath".to_string(),
                    Value::Array(
                        active_path
                            .iter()
                            .map(|id| Value::String(id.clone()))
                            .collect(),
                    ),
                );

                let updated_metadata = Value::Object(thread_metadata).to_string();
                let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
                tx.execute(
                    &format!(
                        "UPDATE chat_threads SET metadata = ?2, updated_at = {now} WHERE id = ?1",
                        now = now_expr
                    ),
                    params![&thread_id, updated_metadata],
                )?;
                thread_record = tx.query_row(&thread_sql, [&thread_id], |row| map_thread_row(row))?;
            }

            let mut version_stmt = tx.prepare(
                "SELECT id, slot_id, created_at FROM chat_messages WHERE thread_id = ?1 ORDER BY timestamp ASC",
            )?;
            let version_info = version_stmt
                .query_map([&thread_id], |row| {
                    Ok(MessageVersionInfoRecord {
                        id: row.get(0)?,
                        slot_id: row.get(1)?,
                        created_at: row.get(2)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(version_stmt);

            let messages_by_id: HashMap<String, MessageRecord> = all_messages
                .into_iter()
                .map(|message| (message.id.clone(), message))
                .collect();

            let mut selected_messages = active_path
                .iter()
                .filter_map(|message_id| messages_by_id.get(message_id).cloned())
                .collect::<Vec<_>>();
            enrich_messages_with_version_metadata(&mut selected_messages, &version_info);

            tx.commit()?;
            let payload = json!({
                "thread": thread_record,
                "messages": selected_messages,
            })
            .to_string();

            Ok(Some(payload))
        })
    }

    #[napi]
    pub fn create_thread(&self, input: CreateThreadInput) -> Result<ThreadRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            conn.execute(
                &format!(
                    "INSERT INTO chat_threads \
                     (id, title, model, prompt_app_id, tools, skill_ids, tools_compact_view, workspace_id, artifact_workspace_id, enable_artifacts, parent_thread_id, is_generating, reasoning_effort, metadata, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, COALESCE(?10, 0), ?11, COALESCE(?12, 0), ?13, ?14, COALESCE(?15, {now}), COALESCE(?16, {now}))",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.title,
                    input.model,
                    input.prompt_app_id,
                    input.tools,
                    input.skill_ids,
                    input.tools_compact_view,
                    input.workspace_id,
                    input.artifact_workspace_id,
                    input.enable_artifacts,
                    input.parent_thread_id,
                    input.is_generating,
                    input.reasoning_effort.unwrap_or_else(|| "medium".to_string()),
                    input.metadata.unwrap_or_else(|| "{}".to_string()),
                    input.created_at,
                    input.updated_at,
                ],
            )?;

            conn.query_row(
                &format!("{} WHERE id = ?1", THREAD_SELECT_SQL),
                [input.id],
                |row| map_thread_row(row),
            )
        })
    }

    #[napi]
    pub fn add_message(&self, input: AddMessageInput) -> Result<MessageRecord> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let message_row_id = format!("{}--{}", input.thread_id, input.message_id);
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

            let existing: Option<MessageRecord> = tx
                .query_row(
                    "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                     FROM chat_messages WHERE id = ?1",
                    [&message_row_id],
                    |row| map_message_row(row),
                )
                .optional()?;

            if existing.is_some() {
                tx.execute(
                    "UPDATE chat_messages SET message = ?2 WHERE id = ?1",
                    params![message_row_id, input.message],
                )?;

                let updated = tx.query_row(
                    "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                     FROM chat_messages WHERE id = ?1",
                    [&message_row_id],
                    |row| map_message_row(row),
                )?;

                tx.commit()?;
                return Ok(updated);
            }

            tx.execute(
                &format!(
                    "INSERT INTO chat_messages \
                     (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, COALESCE(?4, ?1), COALESCE(?5, 0), ?6, ?7, COALESCE(?8, {now}), ?9, COALESCE(?10, {now}), COALESCE(?11, {now}))",
                    now = now_expr
                ),
                params![
                    message_row_id,
                    input.thread_id,
                    input.parent_id,
                    input.slot_id,
                    input.depth,
                    input.parent_tool_call_id,
                    input.message,
                    input.timestamp,
                    input.metadata.unwrap_or_else(|| "{}".to_string()),
                    input.created_at,
                    input.updated_at,
                ],
            )?;

            tx.execute(
                &format!(
                    "UPDATE chat_threads SET updated_at = COALESCE(?2, {now}) WHERE id = ?1",
                    now = now_expr
                ),
                params![input.thread_id, input.updated_at],
            )?;

            let record = tx.query_row(
                "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                 FROM chat_messages WHERE id = ?1",
                [message_row_id],
                |row| map_message_row(row),
            )?;

            tx.commit()?;
            Ok(record)
        })
    }

    #[napi]
    pub fn update_message_content(
        &self,
        input: UpdateMessageContentInput,
    ) -> Result<Option<MessageRecord>> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            conn.execute(
                &format!(
                    "UPDATE chat_messages SET message = ?2, updated_at = COALESCE(?3, {now}) WHERE id = ?1",
                    now = now_expr
                ),
                params![input.id, input.message, input.updated_at],
            )?;

            conn.query_row(
                "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                 FROM chat_messages WHERE id = ?1",
                [input.id],
                |row| map_message_row(row),
            )
            .optional()
        })
    }

    #[napi]
    pub fn update_message_metadata(
        &self,
        input: UpdateMessageMetadataInput,
    ) -> Result<Option<MessageRecord>> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            conn.execute(
                &format!(
                    "UPDATE chat_messages SET metadata = ?2, updated_at = COALESCE(?3, {now}) WHERE id = ?1",
                    now = now_expr
                ),
                params![input.id, input.metadata, input.updated_at],
            )?;

            conn.query_row(
                "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                 FROM chat_messages WHERE id = ?1",
                [input.id],
                |row| map_message_row(row),
            )
            .optional()
        })
    }

    #[napi]
    pub fn delete_thread(&self, thread_id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            if table_exists(&tx, "messages_fts")? {
                tx.execute(
                    "DELETE FROM messages_fts WHERE thread_id = ?1",
                    [&thread_id],
                )?;
            }
            tx.execute(
                "DELETE FROM chat_messages WHERE thread_id = ?1",
                [&thread_id],
            )?;
            let affected = tx.execute("DELETE FROM chat_threads WHERE id = ?1", [&thread_id])?;
            tx.commit()?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn delete_message(&self, message_id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let thread_id: Option<String> = tx
                .query_row(
                    "SELECT thread_id FROM chat_messages WHERE id = ?1",
                    [&message_id],
                    |row| row.get(0),
                )
                .optional()?;

            let Some(thread_id) = thread_id else {
                tx.rollback()?;
                return Ok(false);
            };

            let deleted = tx.execute("DELETE FROM chat_messages WHERE id = ?1", [&message_id])?;
            if deleted == 0 {
                tx.rollback()?;
                return Ok(false);
            }

            if table_exists(&tx, "messages_fts")? {
                tx.execute(
                    "DELETE FROM messages_fts WHERE message_id = ?1",
                    [&message_id],
                )?;
            }

            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            tx.execute(
                &format!(
                    "UPDATE chat_threads SET updated_at = {now} WHERE id = ?1",
                    now = now_expr
                ),
                [&thread_id],
            )?;

            tx.commit()?;
            Ok(true)
        })
    }

    #[napi]
    pub fn update_fts_for_message(&self, input: UpdateFtsForMessageInput) -> Result<()> {
        self.with_connection(|conn| {
            if !table_exists(conn, "messages_fts")? {
                return Ok(());
            }

            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM messages_fts WHERE message_id = ?1",
                [&input.message_id],
            )?;
            if !input.content.trim().is_empty() {
                tx.execute(
                    "INSERT INTO messages_fts(message_id, thread_id, content) VALUES (?1, ?2, ?3)",
                    params![input.message_id, input.thread_id, input.content],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    #[napi]
    pub fn delete_fts_for_message(&self, message_id: String) -> Result<bool> {
        self.with_connection(|conn| {
            if !table_exists(conn, "messages_fts")? {
                return Ok(false);
            }
            let affected = conn.execute(
                "DELETE FROM messages_fts WHERE message_id = ?1",
                [message_id],
            )?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn delete_fts_for_thread(&self, thread_id: String) -> Result<bool> {
        self.with_connection(|conn| {
            if !table_exists(conn, "messages_fts")? {
                return Ok(false);
            }
            let affected =
                conn.execute("DELETE FROM messages_fts WHERE thread_id = ?1", [thread_id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn ensure_fts_schema(&self) -> Result<bool> {
        self.with_connection(|conn| {
            let fts_exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='messages_fts')",
                    [],
                    |row| row.get(0),
                )?;

            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS fts_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            )?;

            let current_version: i32 = conn
                .query_row(
                    "SELECT value FROM fts_metadata WHERE key = 'version'",
                    [],
                    |row| {
                        let raw: String = row.get(0)?;
                        Ok(raw.parse::<i32>().unwrap_or(0))
                    },
                )
                .optional()?
                .unwrap_or(0);

            let mut dropped_old_table = false;
            if fts_exists && current_version < 6 {
                conn.execute_batch("DROP TABLE IF EXISTS messages_fts")?;
                dropped_old_table = true;
            }

            conn.execute_batch(
                "CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(message_id UNINDEXED, thread_id UNINDEXED, content)",
            )?;

            let is_new_table = !fts_exists || dropped_old_table;
            let message_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM chat_messages",
                [],
                |row| row.get(0),
            )?;
            let rebuild_needed =
                (is_new_table || current_version < 6) && (message_count > 0 || current_version < 6);

            Ok(rebuild_needed)
        })
    }

    #[napi]
    pub fn set_fts_version(&self, version: i32) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS fts_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO fts_metadata (key, value) VALUES ('version', ?1)",
                [version.to_string()],
            )?;
            Ok(())
        })
    }

    #[napi]
    pub fn rebuild_fts_index(&self, entries: Vec<FtsEntryInput>) -> Result<i32> {
        self.with_connection(|conn| {
            if !table_exists(conn, "messages_fts")? {
                return Ok(0);
            }

            let tx = conn.transaction()?;
            tx.execute("DELETE FROM messages_fts", [])?;
            let mut inserted = 0;
            let mut stmt = tx.prepare(
                "INSERT INTO messages_fts(message_id, thread_id, content) VALUES (?1, ?2, ?3)",
            )?;

            for entry in entries {
                if entry.content.trim().is_empty() {
                    continue;
                }
                stmt.execute(params![entry.message_id, entry.thread_id, entry.content])?;
                inserted += 1;
            }

            drop(stmt);
            tx.commit()?;
            Ok(inserted)
        })
    }

    #[napi]
    pub fn search_fts(&self, input: SearchFtsInput) -> Result<Vec<FtsHitRecord>> {
        self.with_connection(|conn| {
            if !table_exists(conn, "messages_fts")? {
                return Ok(vec![]);
            }

            let limit = input.limit.unwrap_or(1000).clamp(1, 2000);
            let mut stmt = conn.prepare(
                "SELECT thread_id, message_id FROM messages_fts WHERE messages_fts MATCH ?1 ORDER BY rank LIMIT ?2",
            )?;
            let mapped = stmt.query_map(params![input.match_query, limit], |row| {
                Ok(FtsHitRecord {
                    thread_id: row.get(0)?,
                    message_id: row.get(1)?,
                })
            })?;

            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn search_threads(&self, input: SearchThreadsInput) -> Result<String> {
        self.with_connection(|conn| {
            let limit = input.limit.unwrap_or(20).clamp(1, 50) as usize;
            let context_size = input.context_size.unwrap_or(3).clamp(0, 10) as usize;
            let window_size = context_size * 2 + 1;
            let max_messages_per_thread = std::cmp::max(
                window_size,
                input.max_messages_per_thread.unwrap_or(50).clamp(1, 200) as usize,
            );

            let mut matches_by_thread: HashMap<String, HashSet<String>> = HashMap::new();

            if table_exists(conn, "messages_fts")? {
                let mut fts_stmt = conn.prepare(
                    "SELECT thread_id, message_id FROM messages_fts WHERE messages_fts MATCH ?1 ORDER BY rank LIMIT 1000",
                )?;
                let fts_hits = fts_stmt.query_map([&input.match_query], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;

                for hit in fts_hits {
                    let (thread_id, message_id) = hit?;
                    matches_by_thread
                        .entry(thread_id)
                        .or_default()
                        .insert(message_id);
                }
            }

            let title_like = format!("%{}%", input.title_query.to_lowercase());
            let mut title_stmt = conn.prepare(
                "SELECT id FROM chat_threads WHERE LOWER(title) LIKE ?1 LIMIT 100",
            )?;
            let title_hits = title_stmt.query_map([title_like], |row| row.get::<_, String>(0))?;
            for hit in title_hits {
                matches_by_thread.entry(hit?).or_default();
            }

            if matches_by_thread.is_empty() {
                return Ok("[]".to_string());
            }

            let mut results: Vec<Value> = Vec::new();
            let thread_sql = format!("{} WHERE id = ?1", THREAD_SELECT_SQL);
            for (thread_id, matched_message_ids) in matches_by_thread {
                let Some(thread) = conn
                    .query_row(&thread_sql, [&thread_id], |row| map_thread_row(row))
                    .optional()?
                else {
                    continue;
                };

                let mut msg_stmt = conn.prepare(
                    "SELECT id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at \
                     FROM chat_messages WHERE thread_id = ?1 ORDER BY timestamp ASC",
                )?;
                let messages = msg_stmt
                    .query_map([&thread_id], |row| map_message_row(row))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                if messages.is_empty() && matched_message_ids.is_empty() {
                    continue;
                }

                let mut matched_indexes: Vec<usize> = Vec::new();
                for (index, message) in messages.iter().enumerate() {
                    if matched_message_ids.contains(&message.id) {
                        matched_indexes.push(index);
                    }
                }

                let mut selected_indexes: HashSet<usize> = HashSet::new();
                for hit_index in matched_indexes {
                    let start = hit_index.saturating_sub(context_size);
                    let end = std::cmp::min(messages.len().saturating_sub(1), hit_index + context_size);
                    for index in start..=end {
                        selected_indexes.insert(index);
                        if selected_indexes.len() >= max_messages_per_thread {
                            break;
                        }
                    }
                    if selected_indexes.len() >= max_messages_per_thread {
                        break;
                    }
                }

                if selected_indexes.is_empty() && !messages.is_empty() {
                    for index in 0..std::cmp::min(3, messages.len()) {
                        selected_indexes.insert(index);
                    }
                }

                let mut sorted_indexes: Vec<usize> = selected_indexes.into_iter().collect();
                sorted_indexes.sort_unstable();
                let selected_messages: Vec<MessageRecord> = sorted_indexes
                    .iter()
                    .filter_map(|index| messages.get(*index).cloned())
                    .collect();

                results.push(json!({
                    "id": thread.id,
                    "title": thread.title,
                    "model": thread.model,
                    "isGenerating": thread.is_generating.unwrap_or(false),
                    "createdAt": thread.created_at,
                    "updatedAt": thread.updated_at,
                    "matchCount": if matched_message_ids.is_empty() { 1 } else { matched_message_ids.len() },
                    "messages": selected_messages,
                }));
            }

            results.sort_by(|left, right| {
                let left_match = left
                    .get("matchCount")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let right_match = right
                    .get("matchCount")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                if right_match != left_match {
                    return right_match.cmp(&left_match);
                }

                let left_updated = left
                    .get("updatedAt")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let right_updated = right
                    .get("updatedAt")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                right_updated.cmp(left_updated)
            });

            if results.len() > limit {
                results.truncate(limit);
            }

            Ok(Value::Array(results).to_string())
        })
    }
}
