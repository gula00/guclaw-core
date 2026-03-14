#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use serde_json::Value;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_db_path() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let pid = std::process::id();
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("gula-db-core-test-{pid}-{nanos}-{seq}.db"))
            .to_string_lossy()
            .to_string()
    }

    fn setup_minimal_schema(db_path: &str) {
        let conn = Connection::open(db_path).expect("failed to open sqlite for schema setup");
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS chat_threads (
              id TEXT PRIMARY KEY,
              title TEXT NOT NULL,
              model TEXT,
              prompt_app_id TEXT,
              tools TEXT,
              is_favorited INTEGER DEFAULT 0,
              is_incognito INTEGER DEFAULT 0,
              workspace_id TEXT,
              artifact_workspace_id TEXT,
              enable_artifacts BOOLEAN DEFAULT FALSE,
              skill_ids TEXT,
              tools_compact_view INTEGER,
              parent_thread_id TEXT,
              is_favorite_pinned INTEGER DEFAULT 0,
              favorite_pinned_order INTEGER,
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
              metadata TEXT NOT NULL,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              FOREIGN KEY (thread_id) REFERENCES chat_threads(id) ON DELETE CASCADE
            );
            ",
        )
        .expect("failed to create minimal schema");
    }

    fn setup_mcp_schema(db_path: &str) {
        let conn = Connection::open(db_path).expect("failed to open sqlite for mcp schema setup");
        conn.execute_batch(
            "
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
            ",
        )
        .expect("failed to create mcp schema");
    }

    fn setup_workspace_schema(db_path: &str) {
        let conn =
            Connection::open(db_path).expect("failed to open sqlite for workspace schema setup");
        conn.execute_batch(
            "
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
            ",
        )
        .expect("failed to create workspace schema");
    }

    fn setup_prompt_skill_schema(db_path: &str) {
        let conn =
            Connection::open(db_path).expect("failed to open sqlite for prompt/skill schema setup");
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS prompts (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              content TEXT NOT NULL,
              sort_order INTEGER NOT NULL DEFAULT 0,
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

            CREATE TABLE IF NOT EXISTS skills (
              id TEXT PRIMARY KEY,
              path TEXT NOT NULL,
              enabled INTEGER NOT NULL DEFAULT 1,
              sort_order INTEGER NOT NULL DEFAULT 0,
              updated_at TEXT NOT NULL
            );
            ",
        )
        .expect("failed to create prompt/skill schema");
    }

    #[derive(Debug)]
    struct ComposedMessageMeta {
        id: String,
        slot_id: String,
        depth: i32,
        version_index: usize,
        version_count: usize,
    }

    fn parse_active_path(metadata: &str) -> Vec<String> {
        let parsed: Value = match serde_json::from_str(metadata) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        match parsed.get("activePath") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => vec![],
        }
    }

    fn compose_thread_with_messages_like_js(
        handle: &DbHandle,
        thread_id: &str,
    ) -> (Vec<String>, Vec<ComposedMessageMeta>) {
        let thread = handle
            .get_thread(thread_id.to_string())
            .expect("get_thread should succeed")
            .expect("thread should exist");

        let messages = handle
            .get_messages_by_thread_id(thread_id.to_string())
            .expect("get_messages_by_thread_id should succeed");

        let mut active_path = parse_active_path(&thread.metadata);
        if active_path.is_empty() {
            active_path = messages.iter().map(|m| m.id.clone()).collect();
        }

        let version_info = handle
            .get_message_version_info_by_thread_id(thread_id.to_string())
            .expect("get_message_version_info_by_thread_id should succeed");

        let mut versions_by_slot: std::collections::HashMap<String, Vec<(String, String)>> =
            std::collections::HashMap::new();
        for row in version_info {
            let slot = row.slot_id.clone().unwrap_or_else(|| row.id.clone());
            versions_by_slot
                .entry(slot)
                .or_default()
                .push((row.id, row.created_at));
        }
        for versions in versions_by_slot.values_mut() {
            versions.sort_by(|a, b| a.1.cmp(&b.1));
        }

        let message_map: std::collections::HashMap<String, MessageRecord> =
            messages.into_iter().map(|m| (m.id.clone(), m)).collect();

        let selected_ids: Vec<String> = active_path
            .iter()
            .filter(|id| message_map.contains_key(*id))
            .cloned()
            .collect();

        let composed = selected_ids
            .iter()
            .enumerate()
            .map(|(idx, id)| {
                let m = message_map
                    .get(id)
                    .expect("selected id should exist in message map");
                let slot = m.slot_id.clone().unwrap_or_else(|| m.id.clone());
                let versions = versions_by_slot
                    .get(&slot)
                    .cloned()
                    .unwrap_or_else(|| vec![(m.id.clone(), m.created_at.clone())]);
                let version_index = versions.iter().position(|(id, _)| id == &m.id).unwrap_or(0);
                ComposedMessageMeta {
                    id: m.id.clone(),
                    slot_id: slot,
                    depth: m.depth.unwrap_or(idx as i32),
                    version_index,
                    version_count: versions.len(),
                }
            })
            .collect();

        (active_path, composed)
    }

    #[test]
    fn get_thread_returns_existing_row() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seeding");
            conn.execute(
                "INSERT INTO chat_threads (id, title, model, workspace_id, artifact_workspace_id, enable_artifacts, parent_thread_id, is_generating, reasoning_effort, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    "thread-1",
                    "Hello Thread",
                    "openai:gpt-4o",
                    "ws-1",
                    "aws-1",
                    true,
                    "parent-1",
                    true,
                    "high",
                    "{}",
                    "2026-01-01T00:00:00.000Z",
                    "2026-01-01T00:00:00.000Z"
                ],
            )
            .expect("failed to seed thread");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let row = handle
            .get_thread("thread-1".to_string())
            .expect("get_thread should succeed")
            .expect("thread should exist");

        assert_eq!(row.id, "thread-1");
        assert_eq!(row.title, "Hello Thread");
        assert_eq!(row.model.as_deref(), Some("openai:gpt-4o"));
        assert_eq!(row.workspace_id.as_deref(), Some("ws-1"));
        assert_eq!(row.artifact_workspace_id.as_deref(), Some("aws-1"));
        assert_eq!(row.enable_artifacts, Some(true));
        assert_eq!(row.parent_thread_id.as_deref(), Some("parent-1"));
        assert_eq!(row.is_generating, Some(true));
        assert_eq!(row.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(row.metadata, "{}");

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn get_thread_returns_none_for_missing_id() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let row = handle
            .get_thread("missing-thread".to_string())
            .expect("get_thread should succeed");

        assert!(row.is_none());

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn get_messages_by_thread_id_preserves_order_and_fields() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seeding");
            conn.execute(
                "INSERT INTO chat_threads (id, title, model, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "thread-order",
                    "Order Test",
                    "openai:gpt-4o",
                    "{}",
                    "2026-01-01T00:00:00.000Z",
                    "2026-01-01T00:00:00.000Z"
                ],
            )
            .expect("failed to seed thread");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    "thread-order--m2",
                    "thread-order",
                    Option::<String>::None,
                    "slot-a",
                    1,
                    "tool-1",
                    "{\"id\":\"m2\"}",
                    "2026-01-01T00:00:02.000Z",
                    "{\"k\":2}",
                    "2026-01-01T00:00:02.000Z",
                    "2026-01-01T00:00:02.000Z"
                ],
            )
            .expect("failed to seed message m2");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    "thread-order--m1",
                    "thread-order",
                    Option::<String>::None,
                    "slot-a",
                    0,
                    Option::<String>::None,
                    "{\"id\":\"m1\"}",
                    "2026-01-01T00:00:01.000Z",
                    "{\"k\":1}",
                    "2026-01-01T00:00:01.000Z",
                    "2026-01-01T00:00:01.000Z"
                ],
            )
            .expect("failed to seed message m1");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let rows = handle
            .get_messages_by_thread_id("thread-order".to_string())
            .expect("get_messages_by_thread_id should succeed");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "thread-order--m1");
        assert_eq!(rows[0].slot_id.as_deref(), Some("slot-a"));
        assert_eq!(rows[0].depth, Some(0));
        assert_eq!(rows[0].parent_tool_call_id, None);

        assert_eq!(rows[1].id, "thread-order--m2");
        assert_eq!(rows[1].slot_id.as_deref(), Some("slot-a"));
        assert_eq!(rows[1].depth, Some(1));
        assert_eq!(rows[1].parent_tool_call_id.as_deref(), Some("tool-1"));

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn create_thread_and_add_message_roundtrip() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let created = handle
            .create_thread(CreateThreadInput {
                id: "thread-2".to_string(),
                title: "Created By Rust".to_string(),
                model: Some("google:gemini-2.5-pro".to_string()),
                prompt_app_id: None,
                tools: None,
                skill_ids: None,
                tools_compact_view: None,
                workspace_id: None,
                artifact_workspace_id: None,
                enable_artifacts: None,
                parent_thread_id: None,
                is_generating: None,
                reasoning_effort: Some("medium".to_string()),
                metadata: Some("{}".to_string()),
                created_at: Some("2026-01-02T00:00:00.000Z".to_string()),
                updated_at: Some("2026-01-02T00:00:00.000Z".to_string()),
            })
            .expect("create_thread should succeed");

        assert_eq!(created.id, "thread-2");

        let inserted_message = handle
            .add_message(AddMessageInput {
                thread_id: "thread-2".to_string(),
                message_id: "msg-1".to_string(),
                message: "{\"id\":\"msg-1\",\"role\":\"user\",\"parts\":[{\"type\":\"text\",\"text\":\"hello\"}]}".to_string(),
                metadata: Some("{}".to_string()),
                parent_id: None,
                slot_id: None,
                depth: None,
                parent_tool_call_id: None,
                timestamp: Some("2026-01-02T00:00:01.000Z".to_string()),
                created_at: Some("2026-01-02T00:00:01.000Z".to_string()),
                updated_at: Some("2026-01-02T00:00:01.000Z".to_string()),
            })
            .expect("add_message should succeed");

        assert_eq!(inserted_message.id, "thread-2--msg-1");

        let messages = handle
            .get_messages_by_thread_id("thread-2".to_string())
            .expect("get_messages_by_thread_id should succeed");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "thread-2--msg-1");

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn delete_message_removes_fts_and_updates_thread() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seed");
            conn.execute_batch(
                "
                CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                  message_id UNINDEXED,
                  thread_id UNINDEXED,
                  content
                );

                INSERT INTO chat_threads (id, title, model, metadata, created_at, updated_at)
                VALUES ('thread-fts', 'FTS Thread', NULL, '{}', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');

                INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at)
                VALUES ('thread-fts--msg-1', 'thread-fts', NULL, 'thread-fts--msg-1', 0, NULL, '{\"id\":\"msg-1\"}', '2026-01-01T00:00:01.000Z', '{}', '2026-01-01T00:00:01.000Z', '2026-01-01T00:00:01.000Z');

                INSERT INTO messages_fts (message_id, thread_id, content)
                VALUES ('thread-fts--msg-1', 'thread-fts', 'hello world');
                ",
            )
            .expect("failed to seed data for delete_message test");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let deleted = handle
            .delete_message("thread-fts--msg-1".to_string())
            .expect("delete_message should succeed");
        assert!(deleted);

        let conn = Connection::open(&db_path).expect("failed to reopen sqlite for verification");
        let msg_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chat_messages WHERE id = 'thread-fts--msg-1'",
                [],
                |row| row.get(0),
            )
            .expect("count query should succeed");
        let fts_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages_fts WHERE message_id = 'thread-fts--msg-1'",
                [],
                |row| row.get(0),
            )
            .expect("fts count query should succeed");
        assert_eq!(msg_count, 0);
        assert_eq!(fts_count, 0);

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn delete_thread_removes_thread_messages_and_fts() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seed");
            conn.execute_batch(
                "
                CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                  message_id UNINDEXED,
                  thread_id UNINDEXED,
                  content
                );

                INSERT INTO chat_threads (id, title, model, metadata, created_at, updated_at)
                VALUES ('thread-del', 'Delete Thread', NULL, '{}', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');

                INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at)
                VALUES ('thread-del--msg-1', 'thread-del', NULL, 'thread-del--msg-1', 0, NULL, '{\"id\":\"msg-1\"}', '2026-01-01T00:00:01.000Z', '{}', '2026-01-01T00:00:01.000Z', '2026-01-01T00:00:01.000Z');

                INSERT INTO messages_fts (message_id, thread_id, content)
                VALUES ('thread-del--msg-1', 'thread-del', 'foo bar');
                ",
            )
            .expect("failed to seed data for delete_thread test");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let deleted = handle
            .delete_thread("thread-del".to_string())
            .expect("delete_thread should succeed");
        assert!(deleted);

        let conn = Connection::open(&db_path).expect("failed to reopen sqlite for verification");
        let thread_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chat_threads WHERE id = 'thread-del'",
                [],
                |row| row.get(0),
            )
            .expect("thread count query should succeed");
        let msg_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chat_messages WHERE thread_id = 'thread-del'",
                [],
                |row| row.get(0),
            )
            .expect("message count query should succeed");
        let fts_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages_fts WHERE thread_id = 'thread-del'",
                [],
                |row| row.get(0),
            )
            .expect("fts count query should succeed");
        assert_eq!(thread_count, 0);
        assert_eq!(msg_count, 0);
        assert_eq!(fts_count, 0);

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn add_message_with_existing_id_updates_instead_of_unique_error() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        handle
            .create_thread(CreateThreadInput {
                id: "thread-dup".to_string(),
                title: "Duplicate Message".to_string(),
                model: None,
                prompt_app_id: None,
                tools: None,
                skill_ids: None,
                tools_compact_view: None,
                workspace_id: None,
                artifact_workspace_id: None,
                enable_artifacts: None,
                parent_thread_id: None,
                is_generating: None,
                reasoning_effort: None,
                metadata: Some("{}".to_string()),
                created_at: Some("2026-01-02T00:00:00.000Z".to_string()),
                updated_at: Some("2026-01-02T00:00:00.000Z".to_string()),
            })
            .expect("create_thread should succeed");

        let first = handle
            .add_message(AddMessageInput {
                thread_id: "thread-dup".to_string(),
                message_id: "msg-1".to_string(),
                message: "{\"id\":\"msg-1\",\"role\":\"assistant\",\"parts\":[{\"type\":\"text\",\"text\":\"hello\"}]}".to_string(),
                metadata: Some("{}".to_string()),
                parent_id: None,
                slot_id: None,
                depth: None,
                parent_tool_call_id: None,
                timestamp: Some("2026-01-02T00:00:01.000Z".to_string()),
                created_at: Some("2026-01-02T00:00:01.000Z".to_string()),
                updated_at: Some("2026-01-02T00:00:01.000Z".to_string()),
            })
            .expect("first add_message should succeed");

        let second = handle
            .add_message(AddMessageInput {
                thread_id: "thread-dup".to_string(),
                message_id: "msg-1".to_string(),
                message: "{\"id\":\"msg-1\",\"role\":\"assistant\",\"parts\":[{\"type\":\"text\",\"text\":\"hello updated\"}]}".to_string(),
                metadata: Some("{}".to_string()),
                parent_id: None,
                slot_id: None,
                depth: None,
                parent_tool_call_id: None,
                timestamp: Some("2026-01-02T00:00:02.000Z".to_string()),
                created_at: Some("2026-01-02T00:00:02.000Z".to_string()),
                updated_at: Some("2026-01-02T00:00:02.000Z".to_string()),
            })
            .expect("second add_message should update existing row");

        assert_eq!(first.id, second.id);
        assert!(second.message.contains("hello updated"));

        let messages = handle
            .get_messages_by_thread_id("thread-dup".to_string())
            .expect("get_messages_by_thread_id should succeed");
        assert_eq!(messages.len(), 1);

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn update_thread_core_updates_metadata_and_flags() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seed");
            conn.execute(
                "INSERT INTO chat_threads (id, title, model, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "thread-core-1",
                    "Core Update",
                    "openai:gpt-4o",
                    "{}",
                    "2026-01-01T00:00:00.000Z",
                    "2026-01-01T00:00:00.000Z"
                ],
            )
            .expect("failed to seed thread for update_thread_core test");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let updated = handle
            .update_thread_core(UpdateThreadCoreInput {
                id: "thread-core-1".to_string(),
                metadata: Some("{\"activePath\":[\"m1\"]}".to_string()),
                is_generating: Some(true),
                reasoning_effort: Some("high".to_string()),
                updated_at: Some("2026-01-01T00:00:10.000Z".to_string()),
            })
            .expect("update_thread_core should succeed")
            .expect("thread should exist");

        assert_eq!(updated.id, "thread-core-1");
        assert_eq!(updated.metadata, "{\"activePath\":[\"m1\"]}");
        assert_eq!(updated.is_generating, Some(true));
        assert_eq!(updated.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(updated.updated_at, "2026-01-01T00:00:10.000Z");

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn fts_update_search_and_rebuild_work() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seed");
            conn.execute_batch(
                "
                CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                  message_id UNINDEXED,
                  thread_id UNINDEXED,
                  content
                );
                ",
            )
            .expect("failed to create fts table");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");

        handle
            .update_fts_for_message(UpdateFtsForMessageInput {
                message_id: "m1".to_string(),
                thread_id: "t1".to_string(),
                content: "hello world".to_string(),
            })
            .expect("update_fts_for_message should succeed");

        let hits = handle
            .search_fts(SearchFtsInput {
                match_query: "hello".to_string(),
                limit: Some(10),
            })
            .expect("search_fts should succeed");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].thread_id, "t1");
        assert_eq!(hits[0].message_id, "m1");

        let rebuilt = handle
            .rebuild_fts_index(vec![FtsEntryInput {
                message_id: "m2".to_string(),
                thread_id: "t2".to_string(),
                content: "rust fts".to_string(),
            }])
            .expect("rebuild_fts_index should succeed");
        assert_eq!(rebuilt, 1);

        let hits_after = handle
            .search_fts(SearchFtsInput {
                match_query: "rust".to_string(),
                limit: Some(10),
            })
            .expect("search_fts after rebuild should succeed");
        assert_eq!(hits_after.len(), 1);
        assert_eq!(hits_after[0].message_id, "m2");

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn search_threads_returns_thread_results_with_context_messages() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seed");
            conn.execute_batch(
                "
                CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                  message_id UNINDEXED,
                  thread_id UNINDEXED,
                  content
                );
                ",
            )
            .expect("failed to create fts table");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        handle
            .create_thread(CreateThreadInput {
                id: "thread-search".to_string(),
                title: "Rust Search Thread".to_string(),
                model: Some("openai:gpt-4o".to_string()),
                prompt_app_id: None,
                tools: None,
                skill_ids: None,
                tools_compact_view: None,
                workspace_id: None,
                artifact_workspace_id: None,
                enable_artifacts: Some(false),
                parent_thread_id: None,
                is_generating: Some(false),
                reasoning_effort: Some("medium".to_string()),
                metadata: Some("{}".to_string()),
                created_at: Some("2026-01-04T00:00:00.000Z".to_string()),
                updated_at: Some("2026-01-04T00:00:00.000Z".to_string()),
            })
            .expect("create_thread should succeed");

        let messages = [
            (
                "m1",
                "2026-01-04T00:00:01.000Z",
                "{\"id\":\"m1\",\"role\":\"user\",\"parts\":[{\"type\":\"text\",\"text\":\"before context\"}]}",
            ),
            (
                "m2",
                "2026-01-04T00:00:02.000Z",
                "{\"id\":\"m2\",\"role\":\"assistant\",\"parts\":[{\"type\":\"text\",\"text\":\"rust validation keyword\"}]}",
            ),
            (
                "m3",
                "2026-01-04T00:00:03.000Z",
                "{\"id\":\"m3\",\"role\":\"assistant\",\"parts\":[{\"type\":\"text\",\"text\":\"after context\"}]}",
            ),
        ];

        for (message_id, timestamp, message) in messages {
            handle
                .add_message(AddMessageInput {
                    thread_id: "thread-search".to_string(),
                    message_id: message_id.to_string(),
                    message: message.to_string(),
                    metadata: Some("{}".to_string()),
                    parent_id: None,
                    slot_id: None,
                    depth: None,
                    parent_tool_call_id: None,
                    timestamp: Some(timestamp.to_string()),
                    created_at: Some(timestamp.to_string()),
                    updated_at: Some(timestamp.to_string()),
                })
                .expect("add_message should succeed");
        }

        handle
            .rebuild_fts_index(vec![FtsEntryInput {
                message_id: "thread-search--m2".to_string(),
                thread_id: "thread-search".to_string(),
                content: "rust validation keyword".to_string(),
            }])
            .expect("rebuild_fts_index should succeed");

        let raw = handle
            .search_threads(SearchThreadsInput {
                match_query: "\"rust\" \"validation\"".to_string(),
                title_query: "validation".to_string(),
                limit: Some(10),
                context_size: Some(1),
                max_messages_per_thread: Some(10),
            })
            .expect("search_threads should succeed");

        let parsed: Value = serde_json::from_str(&raw).expect("search_threads should return json");
        let results = parsed
            .as_array()
            .expect("search_threads result should be an array");
        assert_eq!(results.len(), 1);

        let first = &results[0];
        assert_eq!(first["id"], "thread-search");
        assert_eq!(first["title"], "Rust Search Thread");
        assert_eq!(first["matchCount"], 1);

        let returned_messages = first["messages"]
            .as_array()
            .expect("messages should be an array");
        assert_eq!(returned_messages.len(), 3);
        assert_eq!(returned_messages[0]["id"], "thread-search--m1");
        assert_eq!(returned_messages[1]["id"], "thread-search--m2");
        assert_eq!(returned_messages[2]["id"], "thread-search--m3");

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn message_version_and_metadata_queries_return_rows() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        handle
            .create_thread(CreateThreadInput {
                id: "thread-vm".to_string(),
                title: "Version Meta".to_string(),
                model: None,
                prompt_app_id: None,
                tools: None,
                skill_ids: None,
                tools_compact_view: None,
                workspace_id: None,
                artifact_workspace_id: None,
                enable_artifacts: None,
                parent_thread_id: None,
                is_generating: None,
                reasoning_effort: None,
                metadata: Some("{}".to_string()),
                created_at: Some("2026-01-03T00:00:00.000Z".to_string()),
                updated_at: Some("2026-01-03T00:00:00.000Z".to_string()),
            })
            .expect("create_thread should succeed");

        handle
            .add_message(AddMessageInput {
                thread_id: "thread-vm".to_string(),
                message_id: "msg-1".to_string(),
                message: "{\"id\":\"msg-1\",\"role\":\"user\",\"parts\":[{\"type\":\"text\",\"text\":\"hello\"}]}".to_string(),
                metadata: Some("{\"k\":\"v\"}".to_string()),
                parent_id: None,
                slot_id: None,
                depth: None,
                parent_tool_call_id: None,
                timestamp: Some("2026-01-03T00:00:01.000Z".to_string()),
                created_at: Some("2026-01-03T00:00:01.000Z".to_string()),
                updated_at: Some("2026-01-03T00:00:01.000Z".to_string()),
            })
            .expect("add_message should succeed");

        let versions = handle
            .get_message_version_info_by_thread_id("thread-vm".to_string())
            .expect("version query should succeed");
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].id, "thread-vm--msg-1");

        let metas = handle
            .get_message_metadata_by_thread_id("thread-vm".to_string())
            .expect("metadata query should succeed");
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].id, "thread-vm--msg-1");
        assert_eq!(metas[0].metadata, "{\"k\":\"v\"}");

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn version_and_subagent_queries_preserve_filters_and_order() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seeding");
            conn.execute(
                "INSERT INTO chat_threads (id, title, model, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "thread-q",
                    "Query Thread",
                    "openai:gpt-4o",
                    "{}",
                    "2026-01-01T00:00:00.000Z",
                    "2026-01-01T00:00:00.000Z"
                ],
            )
            .expect("failed to seed thread");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    "thread-q--m2",
                    "thread-q",
                    "slot-a",
                    1,
                    "tool-x",
                    "{\"id\":\"m2\"}",
                    "2026-01-01T00:00:02.000Z",
                    "{\"m\":2}",
                    "2026-01-01T00:00:02.000Z",
                    "2026-01-01T00:00:02.000Z"
                ],
            )
            .expect("failed to seed m2");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    "thread-q--m1",
                    "thread-q",
                    "slot-a",
                    0,
                    Option::<String>::None,
                    "{\"id\":\"m1\"}",
                    "2026-01-01T00:00:01.000Z",
                    "{\"m\":1}",
                    "2026-01-01T00:00:01.000Z",
                    "2026-01-01T00:00:01.000Z"
                ],
            )
            .expect("failed to seed m1");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    "thread-q--m3",
                    "thread-q",
                    "slot-b",
                    2,
                    "tool-x",
                    "{\"id\":\"m3\"}",
                    "2026-01-01T00:00:03.000Z",
                    "{\"m\":3}",
                    "2026-01-01T00:00:03.000Z",
                    "2026-01-01T00:00:03.000Z"
                ],
            )
            .expect("failed to seed m3");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");

        let versions = handle
            .get_message_version_info_by_thread_id("thread-q".to_string())
            .expect("version info query should succeed");
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].id, "thread-q--m1");
        assert_eq!(versions[1].id, "thread-q--m2");
        assert_eq!(versions[2].id, "thread-q--m3");
        assert_eq!(versions[0].slot_id.as_deref(), Some("slot-a"));

        let metas = handle
            .get_message_metadata_by_thread_id("thread-q".to_string())
            .expect("metadata query should succeed");
        assert_eq!(metas.len(), 3);
        assert_eq!(metas[0].metadata, "{\"m\":1}");

        let tool_rows = handle
            .get_messages_by_tool_call_id("tool-x".to_string())
            .expect("tool call query should succeed");
        assert_eq!(tool_rows.len(), 2);
        assert_eq!(tool_rows[0].id, "thread-q--m2");
        assert_eq!(tool_rows[1].id, "thread-q--m3");

        let subagent_rows = handle
            .get_subagent_messages_by_thread_id("thread-q".to_string())
            .expect("subagent query should succeed");
        assert_eq!(subagent_rows.len(), 2);
        assert!(subagent_rows
            .iter()
            .all(|m| m.parent_tool_call_id.is_some()));

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn thread_with_messages_respects_active_path_and_versions() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seeding");
            conn.execute(
                "INSERT INTO chat_threads (id, title, model, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "thread-active",
                    "Active Path Thread",
                    "openai:gpt-4o",
                    "{\"activePath\":[\"thread-active--m2\",\"thread-active--m3\"]}",
                    "2026-01-01T00:00:00.000Z",
                    "2026-01-01T00:00:00.000Z"
                ],
            )
            .expect("failed to seed thread");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9)",
                params![
                    "thread-active--m1",
                    "thread-active",
                    "slot-a",
                    0,
                    "{\"id\":\"m1\"}",
                    "2026-01-01T00:00:01.000Z",
                    "{}",
                    "2026-01-01T00:00:01.000Z",
                    "2026-01-01T00:00:01.000Z"
                ],
            )
            .expect("failed to seed m1");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9)",
                params![
                    "thread-active--m2",
                    "thread-active",
                    "slot-a",
                    1,
                    "{\"id\":\"m2\"}",
                    "2026-01-01T00:00:02.000Z",
                    "{}",
                    "2026-01-01T00:00:02.000Z",
                    "2026-01-01T00:00:02.000Z"
                ],
            )
            .expect("failed to seed m2");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9)",
                params![
                    "thread-active--m3",
                    "thread-active",
                    "slot-a",
                    2,
                    "{\"id\":\"m3\"}",
                    "2026-01-01T00:00:03.000Z",
                    "{}",
                    "2026-01-01T00:00:03.000Z",
                    "2026-01-01T00:00:03.000Z"
                ],
            )
            .expect("failed to seed m3");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let (active_path, composed) =
            compose_thread_with_messages_like_js(&handle, "thread-active");

        assert_eq!(active_path, vec!["thread-active--m2", "thread-active--m3"]);
        assert_eq!(composed.len(), 2);
        assert_eq!(composed[0].id, "thread-active--m2");
        assert_eq!(composed[0].slot_id, "slot-a");
        assert_eq!(composed[0].depth, 1);
        assert_eq!(composed[0].version_index, 1);
        assert_eq!(composed[0].version_count, 3);
        assert_eq!(composed[1].id, "thread-active--m3");
        assert_eq!(composed[1].version_index, 2);
        assert_eq!(composed[1].version_count, 3);

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn thread_with_messages_falls_back_to_all_messages_when_active_path_missing() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seeding");
            conn.execute(
                "INSERT INTO chat_threads (id, title, model, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "thread-fallback",
                    "Fallback Thread",
                    "openai:gpt-4o",
                    "{}",
                    "2026-01-01T00:00:00.000Z",
                    "2026-01-01T00:00:00.000Z"
                ],
            )
            .expect("failed to seed thread");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9)",
                params![
                    "thread-fallback--m2",
                    "thread-fallback",
                    "slot-b",
                    1,
                    "{\"id\":\"m2\"}",
                    "2026-01-01T00:00:02.000Z",
                    "{}",
                    "2026-01-01T00:00:02.000Z",
                    "2026-01-01T00:00:02.000Z"
                ],
            )
            .expect("failed to seed m2");

            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9)",
                params![
                    "thread-fallback--m1",
                    "thread-fallback",
                    "slot-a",
                    0,
                    "{\"id\":\"m1\"}",
                    "2026-01-01T00:00:01.000Z",
                    "{}",
                    "2026-01-01T00:00:01.000Z",
                    "2026-01-01T00:00:01.000Z"
                ],
            )
            .expect("failed to seed m1");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let (active_path, composed) =
            compose_thread_with_messages_like_js(&handle, "thread-fallback");

        assert_eq!(
            active_path,
            vec!["thread-fallback--m1", "thread-fallback--m2"]
        );
        assert_eq!(composed.len(), 2);
        assert_eq!(composed[0].id, "thread-fallback--m1");
        assert_eq!(composed[1].id, "thread-fallback--m2");

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn create_thread_defaults_match_js_expectations() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let created = handle
            .create_thread(CreateThreadInput {
                id: "thread-defaults".to_string(),
                title: "Defaults".to_string(),
                model: Some("openai:gpt-4o".to_string()),
                prompt_app_id: None,
                tools: None,
                skill_ids: None,
                tools_compact_view: None,
                workspace_id: None,
                artifact_workspace_id: None,
                enable_artifacts: None,
                parent_thread_id: None,
                is_generating: None,
                reasoning_effort: None,
                metadata: None,
                created_at: Some("2026-01-01T00:00:00.000Z".to_string()),
                updated_at: Some("2026-01-01T00:00:00.000Z".to_string()),
            })
            .expect("create_thread should succeed");

        assert_eq!(created.id, "thread-defaults");
        assert_eq!(created.model.as_deref(), Some("openai:gpt-4o"));
        assert_eq!(created.metadata, "{}");
        assert_eq!(created.reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(created.enable_artifacts, Some(false));
        assert_eq!(created.is_generating, Some(false));

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn delete_thread_works_without_fts_table() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seed");
            conn.execute(
                "INSERT INTO chat_threads (id, title, model, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "thread-no-fts",
                    "No FTS",
                    "openai:gpt-4o",
                    "{}",
                    "2026-01-01T00:00:00.000Z",
                    "2026-01-01T00:00:00.000Z"
                ],
            )
            .expect("failed to seed thread");
            conn.execute(
                "INSERT INTO chat_messages (id, thread_id, parent_id, slot_id, depth, parent_tool_call_id, message, timestamp, metadata, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9)",
                params![
                    "thread-no-fts--m1",
                    "thread-no-fts",
                    "thread-no-fts--m1",
                    0,
                    "{\"id\":\"m1\"}",
                    "2026-01-01T00:00:01.000Z",
                    "{}",
                    "2026-01-01T00:00:01.000Z",
                    "2026-01-01T00:00:01.000Z"
                ],
            )
            .expect("failed to seed message");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let deleted = handle
            .delete_thread("thread-no-fts".to_string())
            .expect("delete_thread should succeed");
        assert!(deleted);

        let conn = Connection::open(&db_path).expect("failed to reopen sqlite for verify");
        let remaining_threads: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chat_threads WHERE id = 'thread-no-fts'",
                [],
                |row| row.get(0),
            )
            .expect("thread count query should succeed");
        let remaining_messages: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chat_messages WHERE thread_id = 'thread-no-fts'",
                [],
                |row| row.get(0),
            )
            .expect("message count query should succeed");

        assert_eq!(remaining_threads, 0);
        assert_eq!(remaining_messages, 0);

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn update_thread_fields_updates_non_core_prompt_related_fields() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seed");
            conn.execute(
                "INSERT INTO chat_threads (id, title, model, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "thread-fields-1",
                    "Before",
                    "openai:gpt-4o",
                    "{}",
                    "2026-01-01T00:00:00.000Z",
                    "2026-01-01T00:00:00.000Z"
                ],
            )
            .expect("failed to seed thread");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let updated = handle
            .update_thread_fields(UpdateThreadFieldsInput {
                id: "thread-fields-1".to_string(),
                set_title: Some(true),
                title: Some("After".to_string()),
                set_model: Some(true),
                model: Some("google:gemini-2.5-pro".to_string()),
                set_workspace_id: Some(true),
                workspace_id: Some("ws-99".to_string()),
                set_artifact_workspace_id: Some(true),
                artifact_workspace_id: Some("aws-99".to_string()),
                set_enable_artifacts: Some(true),
                enable_artifacts: Some(true),
                set_parent_thread_id: Some(true),
                parent_thread_id: Some("parent-99".to_string()),
                set_metadata: Some(true),
                metadata: Some("{\"activePath\":[\"m1\"]}".to_string()),
                set_is_generating: Some(true),
                is_generating: Some(true),
                set_reasoning_effort: Some(true),
                reasoning_effort: Some("high".to_string()),
                set_updated_at: Some(true),
                updated_at: Some("2026-01-01T00:00:10.000Z".to_string()),
            })
            .expect("update_thread_fields should succeed")
            .expect("thread should exist");

        assert_eq!(updated.id, "thread-fields-1");
        assert_eq!(updated.title, "After");
        assert_eq!(updated.model.as_deref(), Some("google:gemini-2.5-pro"));
        assert_eq!(updated.workspace_id.as_deref(), Some("ws-99"));
        assert_eq!(updated.artifact_workspace_id.as_deref(), Some("aws-99"));
        assert_eq!(updated.enable_artifacts, Some(true));
        assert_eq!(updated.parent_thread_id.as_deref(), Some("parent-99"));
        assert_eq!(updated.is_generating, Some(true));
        assert_eq!(updated.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(updated.metadata, "{\"activePath\":[\"m1\"]}");
        assert_eq!(updated.updated_at, "2026-01-01T00:00:10.000Z");

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn update_thread_fields_can_clear_nullable_fields_with_set_flags() {
        let db_path = temp_db_path();
        setup_minimal_schema(&db_path);

        {
            let conn = Connection::open(&db_path).expect("failed to open sqlite for seed");
            conn.execute(
                "INSERT INTO chat_threads (id, title, model, workspace_id, artifact_workspace_id, parent_thread_id, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    "thread-clear-1",
                    "Before",
                    "openai:gpt-4o",
                    "ws-1",
                    "aws-1",
                    "parent-1",
                    "{}",
                    "2026-01-01T00:00:00.000Z",
                    "2026-01-01T00:00:00.000Z"
                ],
            )
            .expect("failed to seed thread");
        }

        let handle = open_db(db_path.clone()).expect("open_db should succeed");
        let updated = handle
            .update_thread_fields(UpdateThreadFieldsInput {
                id: "thread-clear-1".to_string(),
                set_title: Some(false),
                title: None,
                set_model: Some(false),
                model: None,
                set_workspace_id: Some(true),
                workspace_id: None,
                set_artifact_workspace_id: Some(true),
                artifact_workspace_id: None,
                set_enable_artifacts: Some(false),
                enable_artifacts: None,
                set_parent_thread_id: Some(true),
                parent_thread_id: None,
                set_metadata: Some(false),
                metadata: None,
                set_is_generating: Some(false),
                is_generating: None,
                set_reasoning_effort: Some(false),
                reasoning_effort: None,
                set_updated_at: Some(true),
                updated_at: Some("2026-01-01T00:00:11.000Z".to_string()),
            })
            .expect("update_thread_fields should succeed")
            .expect("thread should exist");

        assert_eq!(updated.workspace_id, None);
        assert_eq!(updated.artifact_workspace_id, None);
        assert_eq!(updated.parent_thread_id, None);
        assert_eq!(updated.updated_at, "2026-01-01T00:00:11.000Z");

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn mcp_server_crud_roundtrip() {
        let db_path = temp_db_path();
        setup_mcp_schema(&db_path);

        let handle = open_db(db_path.clone()).expect("open_db should succeed");

        let created = handle
            .create_mcp_server(CreateMcpServerInput {
                id: "mcp-1".to_string(),
                registry_id: Some("registry-1".to_string()),
                name: "Filesystem".to_string(),
                description: Some("Local FS MCP".to_string()),
                config: "{\"command\":\"node\"}".to_string(),
                enabled: Some(true),
                status: Some("disconnected".to_string()),
                last_error: None,
                installed_at: Some("2026-01-10T00:00:00.000Z".to_string()),
                updated_at: Some("2026-01-10T00:00:00.000Z".to_string()),
            })
            .expect("create_mcp_server should succeed");

        assert_eq!(created.id, "mcp-1");
        assert_eq!(created.name, "Filesystem");

        let enabled = handle
            .get_enabled_mcp_servers()
            .expect("get_enabled_mcp_servers should succeed");
        assert_eq!(enabled.len(), 1);

        let updated = handle
            .update_mcp_server_fields(UpdateMcpServerFieldsInput {
                id: "mcp-1".to_string(),
                set_registry_id: Some(false),
                registry_id: None,
                set_name: Some(false),
                name: None,
                set_description: Some(false),
                description: None,
                set_config: Some(false),
                config: None,
                set_enabled: Some(true),
                enabled: Some(false),
                set_status: Some(true),
                status: Some("error".to_string()),
                set_last_error: Some(true),
                last_error: Some("connection failed".to_string()),
                set_installed_at: Some(false),
                installed_at: None,
                set_updated_at: Some(true),
                updated_at: Some("2026-01-10T00:00:01.000Z".to_string()),
            })
            .expect("update_mcp_server_fields should succeed")
            .expect("mcp server should exist");

        assert_eq!(updated.enabled, Some(false));
        assert_eq!(updated.status, "error");
        assert_eq!(updated.last_error.as_deref(), Some("connection failed"));

        let deleted = handle
            .delete_mcp_server("mcp-1".to_string())
            .expect("delete_mcp_server should succeed");
        assert!(deleted);

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn mcp_oauth_save_update_migrate_roundtrip() {
        let db_path = temp_db_path();
        setup_mcp_schema(&db_path);

        let handle = open_db(db_path.clone()).expect("open_db should succeed");

        let saved = handle
            .save_mcp_oauth_token(
                "server-a".to_string(),
                "{\"accessToken\":\"token-a\",\"refreshToken\":\"refresh-a\",\"clientId\":\"cid-a\"}".to_string(),
                "oauth-a".to_string(),
            )
            .expect("save_mcp_oauth_token should succeed");
        assert_eq!(saved.server_id, "server-a");
        assert_eq!(saved.access_token.as_deref(), Some("token-a"));

        let updated = handle
            .update_mcp_oauth_token(
                "server-a".to_string(),
                "{\"accessToken\":\"token-a2\",\"scope\":\"read\"}".to_string(),
            )
            .expect("update_mcp_oauth_token should succeed")
            .expect("oauth row should exist");
        assert_eq!(updated.access_token.as_deref(), Some("token-a2"));
        assert_eq!(updated.scope.as_deref(), Some("read"));

        let _ = handle
            .save_mcp_oauth_token(
                "server-b".to_string(),
                "{\"accessToken\":\"token-b\"}".to_string(),
                "oauth-b".to_string(),
            )
            .expect("save_mcp_oauth_token for server-b should succeed");

        let migrated = handle
            .migrate_mcp_oauth_token("server-a".to_string(), "server-b".to_string())
            .expect("migrate_mcp_oauth_token should succeed");
        assert!(migrated);

        let source = handle
            .get_mcp_oauth_token_by_server_id("server-a".to_string())
            .expect("get_mcp_oauth_token_by_server_id for source should succeed");
        assert!(source.is_none());

        let target = handle
            .get_mcp_oauth_token_by_server_id("server-b".to_string())
            .expect("get_mcp_oauth_token_by_server_id for target should succeed")
            .expect("target oauth row should exist");
        assert_eq!(target.access_token.as_deref(), Some("token-a2"));

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn workspace_crud_and_filters_roundtrip() {
        let db_path = temp_db_path();
        setup_workspace_schema(&db_path);

        let handle = open_db(db_path.clone()).expect("open_db should succeed");

        let created = handle
            .create_workspace(CreateWorkspaceInput {
                id: "ws-1".to_string(),
                path: "/tmp/ws-1".to_string(),
                name: "Workspace 1".to_string(),
                is_temporary: Some(false),
                show_in_list: Some(true),
                is_worktree: Some(false),
                parent_workspace_id: None,
                worktree_branch: None,
                auto_worktree: Some(false),
                pr_number: None,
                pr_url: None,
                pr_state: None,
                pr_base_branch: None,
                created_at: Some("2026-02-01T00:00:00.000Z".to_string()),
                updated_at: Some("2026-02-01T00:00:00.000Z".to_string()),
            })
            .expect("create_workspace should succeed");
        assert_eq!(created.id, "ws-1");

        let _ = handle
            .create_workspace(CreateWorkspaceInput {
                id: "ws-2".to_string(),
                path: "/tmp/ws-2".to_string(),
                name: "Workspace 2".to_string(),
                is_temporary: Some(true),
                show_in_list: Some(false),
                is_worktree: Some(true),
                parent_workspace_id: Some("ws-1".to_string()),
                worktree_branch: Some("feature/x".to_string()),
                auto_worktree: Some(true),
                pr_number: Some(12),
                pr_url: Some("https://example.com/pr/12".to_string()),
                pr_state: Some("open".to_string()),
                pr_base_branch: Some("main".to_string()),
                created_at: Some("2026-02-01T00:00:01.000Z".to_string()),
                updated_at: Some("2026-02-01T00:00:01.000Z".to_string()),
            })
            .expect("create_workspace ws-2 should succeed");

        let filtered = handle
            .get_all_workspaces(Some(GetAllWorkspacesInput {
                include_worktrees: Some(false),
                include_hidden_temporary: Some(false),
            }))
            .expect("get_all_workspaces filtered should succeed");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "ws-1");

        let full = handle
            .get_all_workspaces(Some(GetAllWorkspacesInput {
                include_worktrees: Some(true),
                include_hidden_temporary: Some(true),
            }))
            .expect("get_all_workspaces full should succeed");
        assert_eq!(full.len(), 2);

        let updated = handle
            .update_workspace_fields(UpdateWorkspaceFieldsInput {
                id: "ws-1".to_string(),
                set_path: Some(false),
                path: None,
                set_name: Some(true),
                name: Some("Workspace One".to_string()),
                set_is_temporary: Some(false),
                is_temporary: None,
                set_show_in_list: Some(false),
                show_in_list: None,
                set_is_worktree: Some(false),
                is_worktree: None,
                set_parent_workspace_id: Some(false),
                parent_workspace_id: None,
                set_worktree_branch: Some(false),
                worktree_branch: None,
                set_auto_worktree: Some(false),
                auto_worktree: None,
                set_pr_number: Some(false),
                pr_number: None,
                set_pr_url: Some(false),
                pr_url: None,
                set_pr_state: Some(false),
                pr_state: None,
                set_pr_base_branch: Some(false),
                pr_base_branch: None,
                set_updated_at: Some(true),
                updated_at: Some("2026-02-01T00:00:02.000Z".to_string()),
            })
            .expect("update_workspace_fields should succeed")
            .expect("workspace should exist");
        assert_eq!(updated.name, "Workspace One");

        let by_path = handle
            .get_workspace_by_path("/tmp/ws-1".to_string())
            .expect("get_workspace_by_path should succeed")
            .expect("workspace by path should exist");
        assert_eq!(by_path.id, "ws-1");

        let deleted = handle
            .delete_workspace("ws-2".to_string())
            .expect("delete_workspace should succeed");
        assert!(deleted);

        let _ = fs::remove_file(&db_path);
    }

    #[test]
    fn prompt_threadlabel_skillstate_roundtrip() {
        let db_path = temp_db_path();
        setup_prompt_skill_schema(&db_path);

        let handle = open_db(db_path.clone()).expect("open_db should succeed");

        let prompt = handle
            .create_prompt(CreatePromptInput {
                id: "p-1".to_string(),
                name: "Prompt A".to_string(),
                content: "Hello".to_string(),
                sort_order: Some(0),
                created_at: Some("2026-03-01T00:00:00.000Z".to_string()),
                updated_at: Some("2026-03-01T00:00:00.000Z".to_string()),
            })
            .expect("create_prompt should succeed");
        assert_eq!(prompt.name, "Prompt A");

        let updated_prompt = handle
            .update_prompt_fields(UpdatePromptFieldsInput {
                id: "p-1".to_string(),
                set_name: Some(true),
                name: Some("Prompt A+".to_string()),
                set_content: Some(true),
                content: Some("Hello world".to_string()),
                set_sort_order: Some(false),
                sort_order: None,
                set_updated_at: Some(true),
                updated_at: Some("2026-03-01T00:00:01.000Z".to_string()),
            })
            .expect("update_prompt_fields should succeed")
            .expect("prompt should exist");
        assert_eq!(updated_prompt.name, "Prompt A+");

        let label = handle
            .create_thread_label(CreateThreadLabelInput {
                id: "l-1".to_string(),
                name: "Bug".to_string(),
                color: "#ff0000".to_string(),
                sort_order: Some(0),
                created_at: Some("2026-03-01T00:00:00.000Z".to_string()),
                updated_at: Some("2026-03-01T00:00:00.000Z".to_string()),
            })
            .expect("create_thread_label should succeed");
        assert_eq!(label.name, "Bug");

        let updated_label = handle
            .update_thread_label_fields(UpdateThreadLabelFieldsInput {
                id: "l-1".to_string(),
                set_name: Some(false),
                name: None,
                set_color: Some(true),
                color: Some("#00ff00".to_string()),
                set_sort_order: Some(false),
                sort_order: None,
                set_updated_at: Some(true),
                updated_at: Some("2026-03-01T00:00:01.000Z".to_string()),
            })
            .expect("update_thread_label_fields should succeed")
            .expect("label should exist");
        assert_eq!(updated_label.color, "#00ff00");

        let skill = handle
            .upsert_skill_state(UpsertSkillStateInput {
                id: "s-1".to_string(),
                path: "skills/a".to_string(),
                enabled: Some(true),
                sort_order: Some(0),
                updated_at: Some("2026-03-01T00:00:00.000Z".to_string()),
            })
            .expect("upsert_skill_state insert should succeed");
        assert_eq!(skill.path, "skills/a");

        let skill2 = handle
            .update_skill_state_fields(UpdateSkillStateFieldsInput {
                id: "s-1".to_string(),
                set_path: Some(false),
                path: None,
                set_enabled: Some(true),
                enabled: Some(false),
                set_sort_order: Some(true),
                sort_order: Some(1),
                set_updated_at: Some(true),
                updated_at: Some("2026-03-01T00:00:01.000Z".to_string()),
            })
            .expect("update_skill_state_fields should succeed")
            .expect("skill should exist");
        assert_eq!(skill2.enabled, Some(false));
        assert_eq!(skill2.sort_order, 1);

        handle
            .reorder_prompts(vec!["p-1".to_string()], None)
            .expect("reorder_prompts should succeed");
        handle
            .reorder_thread_labels(vec!["l-1".to_string()], None)
            .expect("reorder_thread_labels should succeed");
        handle
            .reorder_skills(vec!["s-1".to_string()], None)
            .expect("reorder_skills should succeed");

        assert!(
            handle
                .delete_prompt("p-1".to_string())
                .expect("delete_prompt should succeed")
        );
        assert!(
            handle
                .delete_thread_label("l-1".to_string())
                .expect("delete_thread_label should succeed")
        );
        assert!(
            handle
                .delete_skill_state("s-1".to_string())
                .expect("delete_skill_state should succeed")
        );

        let _ = fs::remove_file(&db_path);
    }
}
