use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, OptionalExtension};

use crate::mappers::{
    map_prompt_row, map_skill_state_row, map_thread_label_row, PROMPT_SELECT_SQL,
    SKILL_STATE_SELECT_SQL, THREAD_LABEL_SELECT_SQL,
};
use crate::{
    CreatePromptInput, CreateThreadLabelInput, DbHandle, PromptRecord, SkillStateRecord,
    ThreadLabelRecord, UpdatePromptFieldsInput, UpdateSkillStateFieldsInput,
    UpdateThreadLabelFieldsInput, UpsertSkillStateInput,
};

#[napi]
impl DbHandle {
    #[napi]
    pub fn get_all_prompts(&self) -> Result<Vec<PromptRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} ORDER BY sort_order ASC, created_at DESC",
                PROMPT_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_prompt_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_prompt_by_id(&self, id: String) -> Result<Option<PromptRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", PROMPT_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_prompt_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_prompt_by_name(&self, name: String) -> Result<Option<PromptRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE name = ?1", PROMPT_SELECT_SQL);
            conn.query_row(&sql, [name], |row| map_prompt_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn create_prompt(&self, input: CreatePromptInput) -> Result<PromptRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            let max_sort: Option<i32> = conn
                .query_row("SELECT MAX(sort_order) FROM prompts", [], |row| row.get(0))
                .optional()?
                .flatten();
            let sort_order = input.sort_order.unwrap_or(max_sort.unwrap_or(-1) + 1);

            conn.execute(
                &format!(
                    "INSERT INTO prompts (id, name, content, sort_order, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, COALESCE(?5, {now}), COALESCE(?6, {now}))",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.name,
                    input.content,
                    sort_order,
                    input.created_at,
                    input.updated_at,
                ],
            )?;

            let sql = format!("{} WHERE id = ?1", PROMPT_SELECT_SQL);
            conn.query_row(&sql, [input.id], |row| map_prompt_row(row))
        })
    }

    #[napi]
    pub fn update_prompt_fields(
        &self,
        input: UpdatePromptFieldsInput,
    ) -> Result<Option<PromptRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let exists: Option<String> = tx
                .query_row("SELECT id FROM prompts WHERE id = ?1", [&input.id], |row| row.get(0))
                .optional()?;
            if exists.is_none() {
                tx.rollback()?;
                return Ok(None);
            }

            if input.set_name.unwrap_or(false) {
                tx.execute("UPDATE prompts SET name = ?2 WHERE id = ?1", params![input.id, input.name])?;
            }
            if input.set_content.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompts SET content = ?2 WHERE id = ?1",
                    params![input.id, input.content],
                )?;
            }
            if input.set_sort_order.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompts SET sort_order = ?2 WHERE id = ?1",
                    params![input.id, input.sort_order],
                )?;
            }
            if input.set_updated_at.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompts SET updated_at = COALESCE(?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1",
                    params![input.id, input.updated_at],
                )?;
            }

            let sql = format!("{} WHERE id = ?1", PROMPT_SELECT_SQL);
            let row = tx
                .query_row(&sql, [input.id], |row| map_prompt_row(row))
                .optional()?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_prompt(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM prompts WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn reorder_prompts(
        &self,
        ordered_ids: Vec<String>,
        updated_at: Option<String>,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let ts = updated_at;
            for (idx, id) in ordered_ids.into_iter().enumerate() {
                tx.execute(
                    "UPDATE prompts SET sort_order = ?2, updated_at = COALESCE(?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1",
                    params![id, idx as i32, ts],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    #[napi]
    pub fn get_all_thread_labels(&self) -> Result<Vec<ThreadLabelRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} ORDER BY sort_order ASC, created_at DESC",
                THREAD_LABEL_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_thread_label_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_thread_label_by_id(&self, id: String) -> Result<Option<ThreadLabelRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", THREAD_LABEL_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_thread_label_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn create_thread_label(&self, input: CreateThreadLabelInput) -> Result<ThreadLabelRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            let max_sort: Option<i32> = conn
                .query_row("SELECT MAX(sort_order) FROM thread_labels", [], |row| row.get(0))
                .optional()?
                .flatten();
            let sort_order = input.sort_order.unwrap_or(max_sort.unwrap_or(-1) + 1);

            conn.execute(
                &format!(
                    "INSERT INTO thread_labels (id, name, color, sort_order, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, COALESCE(?5, {now}), COALESCE(?6, {now}))",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.name,
                    input.color,
                    sort_order,
                    input.created_at,
                    input.updated_at,
                ],
            )?;

            let sql = format!("{} WHERE id = ?1", THREAD_LABEL_SELECT_SQL);
            conn.query_row(&sql, [input.id], |row| map_thread_label_row(row))
        })
    }

    #[napi]
    pub fn update_thread_label_fields(
        &self,
        input: UpdateThreadLabelFieldsInput,
    ) -> Result<Option<ThreadLabelRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM thread_labels WHERE id = ?1",
                    [&input.id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                tx.rollback()?;
                return Ok(None);
            }

            if input.set_name.unwrap_or(false) {
                tx.execute(
                    "UPDATE thread_labels SET name = ?2 WHERE id = ?1",
                    params![input.id, input.name],
                )?;
            }
            if input.set_color.unwrap_or(false) {
                tx.execute(
                    "UPDATE thread_labels SET color = ?2 WHERE id = ?1",
                    params![input.id, input.color],
                )?;
            }
            if input.set_sort_order.unwrap_or(false) {
                tx.execute(
                    "UPDATE thread_labels SET sort_order = ?2 WHERE id = ?1",
                    params![input.id, input.sort_order],
                )?;
            }
            if input.set_updated_at.unwrap_or(false) {
                tx.execute(
                    "UPDATE thread_labels SET updated_at = COALESCE(?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1",
                    params![input.id, input.updated_at],
                )?;
            }

            let sql = format!("{} WHERE id = ?1", THREAD_LABEL_SELECT_SQL);
            let row = tx
                .query_row(&sql, [input.id], |row| map_thread_label_row(row))
                .optional()?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_thread_label(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM thread_labels WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn reorder_thread_labels(
        &self,
        ordered_ids: Vec<String>,
        updated_at: Option<String>,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let ts = updated_at;
            for (idx, id) in ordered_ids.into_iter().enumerate() {
                tx.execute(
                    "UPDATE thread_labels SET sort_order = ?2, updated_at = COALESCE(?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1",
                    params![id, idx as i32, ts],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    #[napi]
    pub fn get_all_skill_states(&self) -> Result<Vec<SkillStateRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} ORDER BY sort_order ASC", SKILL_STATE_SELECT_SQL);
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_skill_state_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_skill_state_by_id(&self, id: String) -> Result<Option<SkillStateRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", SKILL_STATE_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_skill_state_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_skill_state_by_path(&self, path: String) -> Result<Option<SkillStateRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE path = ?1", SKILL_STATE_SELECT_SQL);
            conn.query_row(&sql, [path], |row| map_skill_state_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn upsert_skill_state(&self, input: UpsertSkillStateInput) -> Result<SkillStateRecord> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let existing = tx
                .query_row(
                    "SELECT id, path, enabled, sort_order, updated_at FROM skills WHERE id = ?1",
                    [&input.id],
                    |row| map_skill_state_row(row),
                )
                .optional()?;

            let ts = input.updated_at;
            if let Some(old) = existing {
                tx.execute(
                    "UPDATE skills SET path = ?2, enabled = ?3, sort_order = ?4, updated_at = COALESCE(?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1",
                    params![
                        input.id,
                        input.path,
                        input.enabled.unwrap_or(old.enabled.unwrap_or(true)),
                        input.sort_order.unwrap_or(old.sort_order),
                        ts,
                    ],
                )?;
            } else {
                tx.execute(
                    "INSERT INTO skills (id, path, enabled, sort_order, updated_at) VALUES (?1, ?2, COALESCE(?3, 1), COALESCE(?4, 0), COALESCE(?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')))",
                    params![input.id, input.path, input.enabled, input.sort_order, ts],
                )?;
            }

            let sql = format!("{} WHERE id = ?1", SKILL_STATE_SELECT_SQL);
            let row = tx.query_row(&sql, [input.id], |row| map_skill_state_row(row))?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn update_skill_state_fields(
        &self,
        input: UpdateSkillStateFieldsInput,
    ) -> Result<Option<SkillStateRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let exists: Option<String> = tx
                .query_row("SELECT id FROM skills WHERE id = ?1", [&input.id], |row| row.get(0))
                .optional()?;
            if exists.is_none() {
                tx.rollback()?;
                return Ok(None);
            }

            if input.set_path.unwrap_or(false) {
                tx.execute(
                    "UPDATE skills SET path = ?2 WHERE id = ?1",
                    params![input.id, input.path],
                )?;
            }
            if input.set_enabled.unwrap_or(false) {
                tx.execute(
                    "UPDATE skills SET enabled = ?2 WHERE id = ?1",
                    params![input.id, input.enabled],
                )?;
            }
            if input.set_sort_order.unwrap_or(false) {
                tx.execute(
                    "UPDATE skills SET sort_order = ?2 WHERE id = ?1",
                    params![input.id, input.sort_order],
                )?;
            }
            if input.set_updated_at.unwrap_or(false) {
                tx.execute(
                    "UPDATE skills SET updated_at = COALESCE(?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1",
                    params![input.id, input.updated_at],
                )?;
            }

            let sql = format!("{} WHERE id = ?1", SKILL_STATE_SELECT_SQL);
            let row = tx
                .query_row(&sql, [input.id], |row| map_skill_state_row(row))
                .optional()?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_skill_state(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM skills WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn reorder_skills(
        &self,
        ordered_ids: Vec<String>,
        updated_at: Option<String>,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let ts = updated_at;
            for (idx, id) in ordered_ids.into_iter().enumerate() {
                tx.execute(
                    "UPDATE skills SET sort_order = ?2, updated_at = COALESCE(?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1",
                    params![id, idx as i32, ts],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }
}
