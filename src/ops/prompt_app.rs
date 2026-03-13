use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, OptionalExtension};

use crate::mappers::{
    map_prompt_app_exec_row, map_prompt_app_row, map_thread_row, PROMPT_APP_EXEC_SELECT_SQL,
    PROMPT_APP_SELECT_SQL, THREAD_SELECT_SQL,
};
use crate::{
    CreatePromptAppExecutionInput, CreatePromptAppInput, DbHandle, PromptAppExecutionRecord,
    PromptAppRecord, ThreadRecord, UpdatePromptAppFieldsInput,
};

#[napi]
impl DbHandle {
    #[napi]
    pub fn get_all_prompt_apps(&self) -> Result<Vec<PromptAppRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} ORDER BY sort_order ASC, created_at DESC",
                PROMPT_APP_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_prompt_app_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_enabled_prompt_apps(&self) -> Result<Vec<PromptAppRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} WHERE enabled = 1 ORDER BY sort_order ASC, created_at DESC",
                PROMPT_APP_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_prompt_app_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_prompt_app_by_id(&self, id: String) -> Result<Option<PromptAppRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", PROMPT_APP_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_prompt_app_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn create_prompt_app(&self, input: CreatePromptAppInput) -> Result<PromptAppRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            let sort_order = if let Some(v) = input.sort_order {
                v
            } else {
                conn.query_row(
                    "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM prompt_apps",
                    [],
                    |row| row.get::<_, i32>(0),
                )?
            };

            conn.execute(
                &format!(
                    "INSERT INTO prompt_apps \
                     (id, name, description, icon, prompt_template, placeholders, model, tools, reasoning_effort, expects_image_result, is_incognito, enabled, shortcut, sort_order, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, '[]'), ?7, ?8, ?9, COALESCE(?10, 0), COALESCE(?11, 0), COALESCE(?12, 1), ?13, ?14, COALESCE(?15, {now}), COALESCE(?16, {now}))",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.name,
                    input.description,
                    input.icon,
                    input.prompt_template,
                    input.placeholders,
                    input.model,
                    input.tools,
                    input.reasoning_effort,
                    input.expects_image_result,
                    input.is_incognito,
                    input.enabled,
                    input.shortcut,
                    sort_order,
                    input.created_at,
                    input.updated_at
                ],
            )?;
            let sql = format!("{} WHERE id = ?1", PROMPT_APP_SELECT_SQL);
            conn.query_row(&sql, [input.id], |row| map_prompt_app_row(row))
        })
    }

    #[napi]
    pub fn update_prompt_app_fields(
        &self,
        input: UpdatePromptAppFieldsInput,
    ) -> Result<Option<PromptAppRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM prompt_apps WHERE id = ?1",
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
                    "UPDATE prompt_apps SET name = ?2 WHERE id = ?1",
                    params![input.id, input.name],
                )?;
            }
            if input.set_description.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET description = ?2 WHERE id = ?1",
                    params![input.id, input.description],
                )?;
            }
            if input.set_icon.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET icon = ?2 WHERE id = ?1",
                    params![input.id, input.icon],
                )?;
            }
            if input.set_prompt_template.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET prompt_template = ?2 WHERE id = ?1",
                    params![input.id, input.prompt_template],
                )?;
            }
            if input.set_placeholders.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET placeholders = ?2 WHERE id = ?1",
                    params![input.id, input.placeholders],
                )?;
            }
            if input.set_model.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET model = ?2 WHERE id = ?1",
                    params![input.id, input.model],
                )?;
            }
            if input.set_tools.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET tools = ?2 WHERE id = ?1",
                    params![input.id, input.tools],
                )?;
            }
            if input.set_reasoning_effort.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET reasoning_effort = ?2 WHERE id = ?1",
                    params![input.id, input.reasoning_effort],
                )?;
            }
            if input.set_expects_image_result.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET expects_image_result = ?2 WHERE id = ?1",
                    params![input.id, input.expects_image_result],
                )?;
            }
            if input.set_is_incognito.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET is_incognito = ?2 WHERE id = ?1",
                    params![input.id, input.is_incognito],
                )?;
            }
            if input.set_enabled.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET enabled = ?2 WHERE id = ?1",
                    params![input.id, input.enabled],
                )?;
            }
            if input.set_shortcut.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET shortcut = ?2 WHERE id = ?1",
                    params![input.id, input.shortcut],
                )?;
            }
            if input.set_window_width.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET window_width = ?2 WHERE id = ?1",
                    params![input.id, input.window_width],
                )?;
            }
            if input.set_window_height.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET window_height = ?2 WHERE id = ?1",
                    params![input.id, input.window_height],
                )?;
            }
            if input.set_font_size.unwrap_or(false) {
                tx.execute(
                    "UPDATE prompt_apps SET font_size = ?2 WHERE id = ?1",
                    params![input.id, input.font_size],
                )?;
            }

            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            if input.set_updated_at.unwrap_or(false) {
                tx.execute(
                    &format!(
                        "UPDATE prompt_apps SET updated_at = COALESCE(?2, {now}) WHERE id = ?1",
                        now = now_expr
                    ),
                    params![input.id, input.updated_at],
                )?;
            }

            let sql = format!("{} WHERE id = ?1", PROMPT_APP_SELECT_SQL);
            let row = tx
                .query_row(&sql, [input.id], |row| map_prompt_app_row(row))
                .optional()?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_prompt_app(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM prompt_apps WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn reorder_prompt_apps(&self, ids: Vec<String>) -> Result<bool> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            let mut stmt = tx.prepare(&format!(
                "UPDATE prompt_apps SET sort_order = ?1, updated_at = {now} WHERE id = ?2",
                now = now_expr
            ))?;
            for (idx, id) in ids.iter().enumerate() {
                stmt.execute(params![idx as i32, id])?;
            }
            drop(stmt);
            tx.commit()?;
            Ok(true)
        })
    }

    #[napi]
    pub fn get_prompt_app_executions(
        &self,
        prompt_app_id: String,
        limit: Option<i32>,
    ) -> Result<Vec<PromptAppExecutionRecord>> {
        self.with_connection(|conn| {
            let mut sql = format!(
                "{} WHERE prompt_app_id = ?1 ORDER BY created_at DESC",
                PROMPT_APP_EXEC_SELECT_SQL
            );
            if limit.unwrap_or(0) > 0 {
                sql.push_str(" LIMIT ?2");
                let mut stmt = conn.prepare(&sql)?;
                let mapped = stmt.query_map(params![prompt_app_id, limit], |row| {
                    map_prompt_app_exec_row(row)
                })?;
                return mapped.collect::<rusqlite::Result<Vec<_>>>();
            }
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([prompt_app_id], |row| map_prompt_app_exec_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_prompt_app_execution_by_id(
        &self,
        id: String,
    ) -> Result<Option<PromptAppExecutionRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", PROMPT_APP_EXEC_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_prompt_app_exec_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn create_prompt_app_execution(
        &self,
        input: CreatePromptAppExecutionInput,
    ) -> Result<PromptAppExecutionRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            conn.execute(
                &format!(
                    "INSERT INTO prompt_app_executions (id, prompt_app_id, thread_id, input_values, generated_prompt, attachment_count, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, 0), COALESCE(?7, {now}))",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.prompt_app_id,
                    input.thread_id,
                    input.input_values,
                    input.generated_prompt,
                    input.attachment_count,
                    input.created_at
                ],
            )?;
            let sql = format!("{} WHERE id = ?1", PROMPT_APP_EXEC_SELECT_SQL);
            conn.query_row(&sql, [input.id], |row| map_prompt_app_exec_row(row))
        })
    }

    #[napi]
    pub fn delete_prompt_app_execution(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM prompt_app_executions WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn get_threads_by_prompt_app_id(&self, prompt_app_id: String) -> Result<Vec<ThreadRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} WHERE prompt_app_id = ?1 ORDER BY updated_at DESC",
                THREAD_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([prompt_app_id], |row| map_thread_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }
}
