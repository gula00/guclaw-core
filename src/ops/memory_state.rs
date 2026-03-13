use napi::bindgen_prelude::Result;
use napi_derive::napi;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::DbHandle;

#[napi]
impl DbHandle {
    #[napi]
    pub fn ensure_memory_schema(&self) -> Result<bool> {
        self.with_connection(|conn| {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS memories (
                    id TEXT PRIMARY KEY,
                    content TEXT NOT NULL,
                    metadata TEXT NOT NULL,
                    thread_id TEXT REFERENCES chat_threads(id) ON DELETE SET NULL,
                    message_id TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS memory_metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                ",
            )?;

            let _ = conn.execute_batch("ALTER TABLE memories ADD COLUMN user_id TEXT");

            let indexes = [
                "CREATE INDEX IF NOT EXISTS idx_memories_thread_id ON memories(thread_id)",
                "CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at)",
                "CREATE INDEX IF NOT EXISTS idx_memories_updated_at ON memories(updated_at)",
                "CREATE INDEX IF NOT EXISTS idx_memories_user_id ON memories(user_id)",
            ];
            for sql in indexes {
                let _ = conn.execute_batch(sql);
            }

            Ok(true)
        })
    }

    #[napi]
    pub fn get_memory_metadata(&self, key: String) -> Result<Option<String>> {
        self.with_connection(|conn| {
            match conn.query_row(
                "SELECT value FROM memory_metadata WHERE key = ?1",
                [key],
                |row| row.get(0),
            ) {
                Ok(value) => Ok(Some(value)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(err) => Err(err),
            }
        })
    }

    #[napi]
    pub fn set_memory_metadata(&self, key: String, value: String) -> Result<bool> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO memory_metadata (key, value) VALUES (?1, ?2)",
                [key, value],
            )?;
            Ok(true)
        })
    }

    #[napi]
    pub fn get_memory_stats(&self) -> Result<String> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT metadata, thread_id FROM memories")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let mut by_source: HashMap<String, i64> = HashMap::new();
            let mut by_thread: HashMap<String, i64> = HashMap::new();

            for (metadata_raw, thread_id) in &rows {
                let source = serde_json::from_str::<Value>(metadata_raw)
                    .ok()
                    .and_then(|v| v.get("source").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_else(|| "unknown".to_string());
                *by_source.entry(source).or_insert(0) += 1;

                if let Some(thread_id) = thread_id {
                    *by_thread.entry(thread_id.clone()).or_insert(0) += 1;
                }
            }

            Ok(json!({
                "total": rows.len(),
                "bySource": by_source,
                "byThread": by_thread,
            })
            .to_string())
        })
    }

    #[napi]
    pub fn get_all_memories(
        &self,
        thread_id: Option<String>,
        limit: Option<i32>,
    ) -> Result<String> {
        self.with_connection(|conn| {
            let limit = limit.unwrap_or(100).clamp(1, 1000);
            let rows: Vec<Value> = if let Some(thread_id) = thread_id {
                let mut stmt = conn.prepare(
                    "SELECT id, content, metadata, thread_id, message_id, user_id, created_at, updated_at \
                     FROM memories WHERE thread_id = ?1 ORDER BY updated_at DESC LIMIT ?2",
                )?;
                let mapped = stmt.query_map([thread_id, limit.to_string()], |row| {
                    let metadata_raw: String = row.get(2)?;
                    let metadata = serde_json::from_str::<Value>(&metadata_raw)
                        .ok()
                        .unwrap_or_else(|| json!({}));
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "content": row.get::<_, String>(1)?,
                        "metadata": metadata,
                        "threadId": row.get::<_, Option<String>>(3)?,
                        "messageId": row.get::<_, Option<String>>(4)?,
                        "userId": row.get::<_, Option<String>>(5)?,
                        "createdAt": row.get::<_, String>(6)?,
                        "updatedAt": row.get::<_, String>(7)?,
                    }))
                })?;
                mapped.collect::<rusqlite::Result<Vec<_>>>()?
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, content, metadata, thread_id, message_id, user_id, created_at, updated_at \
                     FROM memories ORDER BY updated_at DESC LIMIT ?1",
                )?;
                let mapped = stmt.query_map([limit.to_string()], |row| {
                    let metadata_raw: String = row.get(2)?;
                    let metadata = serde_json::from_str::<Value>(&metadata_raw)
                        .ok()
                        .unwrap_or_else(|| json!({}));
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "content": row.get::<_, String>(1)?,
                        "metadata": metadata,
                        "threadId": row.get::<_, Option<String>>(3)?,
                        "messageId": row.get::<_, Option<String>>(4)?,
                        "userId": row.get::<_, Option<String>>(5)?,
                        "createdAt": row.get::<_, String>(6)?,
                        "updatedAt": row.get::<_, String>(7)?,
                    }))
                })?;
                mapped.collect::<rusqlite::Result<Vec<_>>>()?
            };

            Ok(Value::Array(rows).to_string())
        })
    }

    #[napi]
    pub fn get_memory_by_id(&self, id: String) -> Result<Option<String>> {
        self.with_connection(|conn| {
            let payload = match conn.query_row(
                    "SELECT id, content, metadata, thread_id, message_id, user_id, created_at, updated_at FROM memories WHERE id = ?1",
                    [id],
                    |row| {
                        let metadata_raw: String = row.get(2)?;
                        let metadata = serde_json::from_str::<Value>(&metadata_raw)
                            .ok()
                            .unwrap_or_else(|| json!({}));
                        Ok(json!({
                            "id": row.get::<_, String>(0)?,
                            "content": row.get::<_, String>(1)?,
                            "metadata": metadata,
                            "threadId": row.get::<_, Option<String>>(3)?,
                            "messageId": row.get::<_, Option<String>>(4)?,
                            "userId": row.get::<_, Option<String>>(5)?,
                            "createdAt": row.get::<_, String>(6)?,
                            "updatedAt": row.get::<_, String>(7)?,
                        })
                        .to_string())
                    },
                ) {
                Ok(value) => Some(value),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(err) => return Err(err.into()),
            };
            Ok(payload)
        })
    }

    #[napi]
    pub fn add_memory_with_embedding(
        &self,
        id: String,
        content: String,
        metadata_json: String,
        thread_id: Option<String>,
        message_id: Option<String>,
        user_id: Option<String>,
        created_at: String,
        updated_at: String,
        embedding_json: String,
    ) -> Result<Option<String>> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO memories (id, content, metadata, thread_id, message_id, user_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    id,
                    content,
                    metadata_json,
                    thread_id,
                    message_id,
                    user_id,
                    created_at,
                    updated_at,
                ],
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO memory_embeddings (memory_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![id, embedding_json],
            )?;

            let payload = conn.query_row(
                "SELECT id, content, metadata, thread_id, message_id, user_id, created_at, updated_at FROM memories WHERE id = ?1",
                [id],
                |row| {
                    let metadata_raw: String = row.get(2)?;
                    let metadata = serde_json::from_str::<Value>(&metadata_raw)
                        .ok()
                        .unwrap_or_else(|| json!({}));
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "content": row.get::<_, String>(1)?,
                        "metadata": metadata,
                        "threadId": row.get::<_, Option<String>>(3)?,
                        "messageId": row.get::<_, Option<String>>(4)?,
                        "userId": row.get::<_, Option<String>>(5)?,
                        "createdAt": row.get::<_, String>(6)?,
                        "updatedAt": row.get::<_, String>(7)?,
                    })
                    .to_string())
                },
            )?;
            Ok(Some(payload))
        })
    }

    #[napi]
    pub fn update_memory_with_embedding(
        &self,
        id: String,
        content: String,
        metadata_json: String,
        updated_at: String,
        embedding_json: String,
    ) -> Result<Option<String>> {
        self.with_connection(|conn| {
            let changed = conn.execute(
                "UPDATE memories SET content = ?2, metadata = ?3, updated_at = ?4 WHERE id = ?1",
                rusqlite::params![id, content, metadata_json, updated_at],
            )?;
            if changed == 0 {
                return Ok(None);
            }
            conn.execute(
                "INSERT OR REPLACE INTO memory_embeddings (memory_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![id, embedding_json],
            )?;

            let payload = conn.query_row(
                "SELECT id, content, metadata, thread_id, message_id, user_id, created_at, updated_at FROM memories WHERE id = ?1",
                [id],
                |row| {
                    let metadata_raw: String = row.get(2)?;
                    let metadata = serde_json::from_str::<Value>(&metadata_raw)
                        .ok()
                        .unwrap_or_else(|| json!({}));
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "content": row.get::<_, String>(1)?,
                        "metadata": metadata,
                        "threadId": row.get::<_, Option<String>>(3)?,
                        "messageId": row.get::<_, Option<String>>(4)?,
                        "userId": row.get::<_, Option<String>>(5)?,
                        "createdAt": row.get::<_, String>(6)?,
                        "updatedAt": row.get::<_, String>(7)?,
                    })
                    .to_string())
                },
            )?;
            Ok(Some(payload))
        })
    }

    #[napi]
    pub fn delete_memory_with_embedding(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            conn.execute(
                "DELETE FROM memory_embeddings WHERE memory_id = ?1",
                [id.as_str()],
            )?;
            let changed = conn.execute("DELETE FROM memories WHERE id = ?1", [id])?;
            Ok(changed > 0)
        })
    }

    #[napi]
    pub fn delete_memories_by_thread_id_with_embeddings(&self, thread_id: String) -> Result<i32> {
        self.with_connection(|conn| {
            conn.execute(
                "DELETE FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE thread_id = ?1)",
                [thread_id.as_str()],
            )?;
            let changed = conn.execute("DELETE FROM memories WHERE thread_id = ?1", [thread_id])?;
            Ok(changed as i32)
        })
    }

    #[napi]
    pub fn clear_all_memories_with_embeddings(&self) -> Result<bool> {
        self.with_connection(|conn| {
            conn.execute("DELETE FROM memory_embeddings", [])?;
            conn.execute("DELETE FROM memories", [])?;
            Ok(true)
        })
    }
}
