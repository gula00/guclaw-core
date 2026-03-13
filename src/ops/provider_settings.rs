use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, OptionalExtension};

use crate::mappers::{
    map_provider_row, map_settings_row, table_exists, PROVIDER_SELECT_SQL, SETTINGS_SELECT_SQL,
};
use crate::{
    AppSettingsRecord, CreateProviderInput, DbHandle, ProviderRecord, UpdateProviderFieldsInput,
};

#[napi]
impl DbHandle {
    #[napi]
    pub fn get_all_providers(&self) -> Result<Vec<ProviderRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} ORDER BY created_at DESC", PROVIDER_SELECT_SQL);
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_provider_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_provider_by_id(&self, id: String) -> Result<Option<ProviderRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", PROVIDER_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_provider_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_enabled_providers(&self) -> Result<Vec<ProviderRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} WHERE enabled = 1 ORDER BY created_at DESC",
                PROVIDER_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_provider_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn create_provider(&self, input: CreateProviderInput) -> Result<ProviderRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            conn.execute(
                &format!(
                    "INSERT INTO providers \
                     (id, name, type, api_key, models, base_url, enabled, created_at, updated_at, available_models) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE(?7, 1), COALESCE(?8, {now}), COALESCE(?9, {now}), '[]')",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.name,
                    input.r#type,
                    input.api_key,
                    input.models,
                    input.base_url,
                    input.enabled,
                    input.created_at,
                    input.updated_at
                ],
            )?;
            let sql = format!("{} WHERE id = ?1", PROVIDER_SELECT_SQL);
            conn.query_row(&sql, [input.id], |row| map_provider_row(row))
        })
    }

    #[napi]
    pub fn update_provider_fields(
        &self,
        input: UpdateProviderFieldsInput,
    ) -> Result<Option<ProviderRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM providers WHERE id = ?1",
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
                    "UPDATE providers SET name = ?2 WHERE id = ?1",
                    params![input.id, input.name],
                )?;
            }
            if input.set_type.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET type = ?2 WHERE id = ?1",
                    params![input.id, input.r#type],
                )?;
            }
            if input.set_api_key.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET api_key = ?2 WHERE id = ?1",
                    params![input.id, input.api_key],
                )?;
            }
            if input.set_models.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET models = ?2 WHERE id = ?1",
                    params![input.id, input.models],
                )?;
            }
            if input.set_base_url.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET base_url = ?2 WHERE id = ?1",
                    params![input.id, input.base_url],
                )?;
            }
            if input.set_api_version.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET api_version = ?2 WHERE id = ?1",
                    params![input.id, input.api_version],
                )?;
            }
            if input.set_enabled.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET enabled = ?2 WHERE id = ?1",
                    params![input.id, input.enabled],
                )?;
            }
            if input.set_is_response_api.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET is_response_api = ?2 WHERE id = ?1",
                    params![input.id, input.is_response_api],
                )?;
            }
            if input.set_acp_command.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET acp_command = ?2 WHERE id = ?1",
                    params![input.id, input.acp_command],
                )?;
            }
            if input.set_acp_args.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET acp_args = ?2 WHERE id = ?1",
                    params![input.id, input.acp_args],
                )?;
            }
            if input.set_acp_mcp_server_ids.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET acp_mcp_server_ids = ?2 WHERE id = ?1",
                    params![input.id, input.acp_mcp_server_ids],
                )?;
            }
            if input.set_acp_auth_method_id.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET acp_auth_method_id = ?2 WHERE id = ?1",
                    params![input.id, input.acp_auth_method_id],
                )?;
            }
            if input.set_acp_api_provider_id.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET acp_api_provider_id = ?2 WHERE id = ?1",
                    params![input.id, input.acp_api_provider_id],
                )?;
            }
            if input.set_acp_model_mapping.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET acp_model_mapping = ?2 WHERE id = ?1",
                    params![input.id, input.acp_model_mapping],
                )?;
            }
            if input.set_use_max_completion_tokens.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET use_max_completion_tokens = ?2 WHERE id = ?1",
                    params![input.id, input.use_max_completion_tokens],
                )?;
            }
            if input.set_api_format.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET api_format = ?2 WHERE id = ?1",
                    params![input.id, input.api_format],
                )?;
            }
            if input.set_available_models.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET available_models = ?2 WHERE id = ?1",
                    params![input.id, input.available_models],
                )?;
            }
            if input.set_copilot_account_id.unwrap_or(false) {
                tx.execute(
                    "UPDATE providers SET copilot_account_id = ?2 WHERE id = ?1",
                    params![input.id, input.copilot_account_id],
                )?;
            }

            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            if input.set_updated_at.unwrap_or(false) {
                tx.execute(
                    &format!(
                        "UPDATE providers SET updated_at = COALESCE(?2, {now}) WHERE id = ?1",
                        now = now_expr
                    ),
                    params![input.id, input.updated_at],
                )?;
            }

            let sql = format!("{} WHERE id = ?1", PROVIDER_SELECT_SQL);
            let row = tx
                .query_row(&sql, [input.id], |row| map_provider_row(row))
                .optional()?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_provider(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM providers WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn update_provider_models(
        &self,
        id: String,
        models: String,
    ) -> Result<Option<ProviderRecord>> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE providers SET models = ?2 WHERE id = ?1",
                params![id, models],
            )?;
            if affected == 0 {
                return Ok(None);
            }
            let sql = format!("{} WHERE id = ?1", PROVIDER_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_provider_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn update_provider_available_models(
        &self,
        id: String,
        available_models: String,
    ) -> Result<Option<ProviderRecord>> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "UPDATE providers SET available_models = ?2 WHERE id = ?1",
                params![id, available_models],
            )?;
            if affected == 0 {
                return Ok(None);
            }
            let sql = format!("{} WHERE id = ?1", PROVIDER_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_provider_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_settings(&self) -> Result<Option<AppSettingsRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = 'default'", SETTINGS_SELECT_SQL);
            conn.query_row(&sql, [], |row| map_settings_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn save_settings(&self, settings_data: String) -> Result<AppSettingsRecord> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            tx.execute(
                &format!(
                    "INSERT INTO app_settings (id, settings_data, created_at, updated_at) \
                     VALUES ('default', ?1, {now}, {now}) \
                     ON CONFLICT(id) DO UPDATE SET settings_data = excluded.settings_data, updated_at = {now}",
                    now = now_expr
                ),
                [settings_data],
            )?;
            let sql = format!("{} WHERE id = 'default'", SETTINGS_SELECT_SQL);
            let row = tx.query_row(&sql, [], |row| map_settings_row(row))?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn reset_settings(&self) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM app_settings WHERE id = 'default'", [])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn clear_all_data(&self) -> Result<bool> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let tables = [
                "chat_messages",
                "chat_threads",
                "app_settings",
                "providers",
                "model_capabilities_cache",
                "provider_models_cache",
            ];

            for table in tables {
                if table_exists(&tx, table)? {
                    tx.execute(&format!("DELETE FROM {table}"), [])?;
                }
            }

            tx.commit()?;
            Ok(true)
        })
    }
}
