use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, OptionalExtension};

use crate::mappers::{
    map_channel_mapping_row, map_distinct_channel_row, map_model_caps_cache_row,
    map_provider_models_cache_row, map_thread_diff_stats_cache_row, CHANNEL_MAPPING_SELECT_SQL,
    DISTINCT_CHANNELS_SQL, MODEL_CAPS_CACHE_SELECT_SQL, PROVIDER_MODELS_CACHE_SELECT_SQL,
    THREAD_DIFF_STATS_CACHE_SELECT_SQL,
};
use crate::{
    BulkModelCapabilitiesEntryInput, ChannelMappingRecord, CreateChannelMappingInput, DbHandle,
    DistinctChannelRecord, ModelCapabilitiesCacheRecord, ProviderModelsCacheRecord,
    SetActiveChannelMappingInput, SetModelCapabilitiesCacheInput, SetProviderModelsCacheInput,
    SetThreadDiffStatsCacheInput, ThreadDiffStatsCacheRecord,
};

#[napi]
impl DbHandle {
    #[napi]
    pub fn get_active_channel_mapping(
        &self,
        platform: String,
        external_chat_id: String,
        external_user_id: String,
    ) -> Result<Option<ChannelMappingRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} WHERE platform = ?1 AND external_chat_id = ?2 AND external_user_id = ?3 AND is_active = 1",
                CHANNEL_MAPPING_SELECT_SQL
            );
            conn.query_row(
                &sql,
                params![platform, external_chat_id, external_user_id],
                |row| map_channel_mapping_row(row),
            )
            .optional()
        })
    }

    #[napi]
    pub fn create_channel_mapping(
        &self,
        input: CreateChannelMappingInput,
    ) -> Result<ChannelMappingRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            conn.execute(
                &format!(
                    "INSERT INTO channel_mappings (id, platform, external_chat_id, external_user_id, thread_id, is_active, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, 1), COALESCE(?7, {now}), COALESCE(?8, {now}))",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.platform,
                    input.external_chat_id,
                    input.external_user_id,
                    input.thread_id,
                    input.is_active,
                    input.created_at,
                    input.updated_at
                ],
            )?;
            let sql = format!("{} WHERE id = ?1", CHANNEL_MAPPING_SELECT_SQL);
            conn.query_row(&sql, [input.id], |row| map_channel_mapping_row(row))
        })
    }

    #[napi]
    pub fn set_active_channel_mapping(&self, input: SetActiveChannelMappingInput) -> Result<bool> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            tx.execute(
                &format!(
                    "UPDATE channel_mappings SET is_active = 0, updated_at = {now} WHERE platform = ?1 AND external_chat_id = ?2 AND external_user_id = ?3",
                    now = now_expr
                ),
                params![
                    input.platform,
                    input.external_chat_id,
                    input.external_user_id
                ],
            )?;

            let affected = tx.execute(
                &format!(
                    "UPDATE channel_mappings SET is_active = 1, updated_at = {now} WHERE platform = ?1 AND external_chat_id = ?2 AND external_user_id = ?3 AND thread_id = ?4",
                    now = now_expr
                ),
                params![
                    input.platform,
                    input.external_chat_id,
                    input.external_user_id,
                    input.thread_id
                ],
            )?;

            if affected == 0 {
                let id = input.new_mapping_id.ok_or_else(|| {
                    rusqlite::Error::InvalidParameterName("new_mapping_id".to_string())
                })?;
                tx.execute(
                    &format!(
                        "INSERT INTO channel_mappings (id, platform, external_chat_id, external_user_id, thread_id, is_active, created_at, updated_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, 1, {now}, {now})",
                        now = now_expr
                    ),
                    params![
                        id,
                        input.platform,
                        input.external_chat_id,
                        input.external_user_id,
                        input.thread_id
                    ],
                )?;
            }

            tx.commit()?;
            Ok(true)
        })
    }

    #[napi]
    pub fn get_distinct_channels(&self, platform: String) -> Result<Vec<DistinctChannelRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(DISTINCT_CHANNELS_SQL)?;
            let mapped = stmt.query_map([platform], |row| map_distinct_channel_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_channel_mappings_by_thread_id(
        &self,
        thread_id: String,
    ) -> Result<Vec<ChannelMappingRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE thread_id = ?1", CHANNEL_MAPPING_SELECT_SQL);
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([thread_id], |row| map_channel_mapping_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_channel_mappings_for_user(
        &self,
        platform: String,
        external_user_id: String,
    ) -> Result<Vec<ChannelMappingRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} WHERE platform = ?1 AND external_user_id = ?2 ORDER BY updated_at DESC",
                CHANNEL_MAPPING_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([platform, external_user_id], |row| {
                map_channel_mapping_row(row)
            })?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_all_model_capabilities_cache(&self) -> Result<Vec<ModelCapabilitiesCacheRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(MODEL_CAPS_CACHE_SELECT_SQL)?;
            let mapped = stmt.query_map([], |row| map_model_caps_cache_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_model_capabilities_from_cache(
        &self,
        id: String,
    ) -> Result<Option<ModelCapabilitiesCacheRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", MODEL_CAPS_CACHE_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_model_caps_cache_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn set_model_capabilities_cache(
        &self,
        input: SetModelCapabilitiesCacheInput,
    ) -> Result<ModelCapabilitiesCacheRecord> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            tx.execute(
                &format!(
                    "INSERT INTO model_capabilities_cache (id, capabilities, fetched_at, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, {now}, {now}) \
                     ON CONFLICT(id) DO UPDATE SET capabilities = excluded.capabilities, fetched_at = excluded.fetched_at, updated_at = {now}",
                    now = now_expr
                ),
                params![input.id, input.capabilities, input.fetched_at],
            )?;
            let sql = format!("{} WHERE id = ?1", MODEL_CAPS_CACHE_SELECT_SQL);
            let row = tx.query_row(&sql, [input.id], |row| map_model_caps_cache_row(row))?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn bulk_set_model_capabilities_cache(
        &self,
        entries: Vec<BulkModelCapabilitiesEntryInput>,
        fetched_at: String,
    ) -> Result<bool> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            let mut stmt = tx.prepare(&format!(
                "INSERT INTO model_capabilities_cache (id, capabilities, fetched_at, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, {now}, {now}) \
                 ON CONFLICT(id) DO UPDATE SET capabilities = excluded.capabilities, fetched_at = excluded.fetched_at, updated_at = {now}",
                now = now_expr
            ))?;
            for entry in entries {
                stmt.execute(params![entry.model_id, entry.capabilities, fetched_at.clone()])?;
            }
            drop(stmt);
            tx.commit()?;
            Ok(true)
        })
    }

    #[napi]
    pub fn clear_model_capabilities_cache(&self) -> Result<bool> {
        self.with_connection(|conn| {
            conn.execute("DELETE FROM model_capabilities_cache", [])?;
            Ok(true)
        })
    }

    #[napi]
    pub fn get_provider_models_cache_by_id(
        &self,
        id: String,
    ) -> Result<Option<ProviderModelsCacheRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", PROVIDER_MODELS_CACHE_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_provider_models_cache_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_provider_models_cache_by_provider_id(
        &self,
        provider_id: String,
    ) -> Result<Option<ProviderModelsCacheRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} WHERE provider_id = ?1",
                PROVIDER_MODELS_CACHE_SELECT_SQL
            );
            conn.query_row(&sql, [provider_id], |row| {
                map_provider_models_cache_row(row)
            })
            .optional()
        })
    }

    #[napi]
    pub fn set_provider_models_cache(
        &self,
        input: SetProviderModelsCacheInput,
    ) -> Result<ProviderModelsCacheRecord> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            tx.execute(
                &format!(
                    "INSERT INTO provider_models_cache (id, provider_id, models, fetched_at, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, {now}, {now}) \
                     ON CONFLICT(id) DO UPDATE SET provider_id = excluded.provider_id, models = excluded.models, fetched_at = excluded.fetched_at, updated_at = {now}",
                    now = now_expr
                ),
                params![input.id, input.provider_id, input.models, input.fetched_at],
            )?;
            let sql = format!("{} WHERE id = ?1", PROVIDER_MODELS_CACHE_SELECT_SQL);
            let row = tx.query_row(&sql, [input.id], |row| map_provider_models_cache_row(row))?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_provider_models_cache(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM provider_models_cache WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn delete_provider_models_cache_by_provider_id(&self, provider_id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "DELETE FROM provider_models_cache WHERE provider_id = ?1",
                [provider_id],
            )?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn clear_provider_models_cache(&self) -> Result<bool> {
        self.with_connection(|conn| {
            conn.execute("DELETE FROM provider_models_cache", [])?;
            Ok(true)
        })
    }

    #[napi]
    pub fn get_thread_diff_stats_cache(
        &self,
        id: String,
    ) -> Result<Option<ThreadDiffStatsCacheRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", THREAD_DIFF_STATS_CACHE_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_thread_diff_stats_cache_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn set_thread_diff_stats_cache(
        &self,
        input: SetThreadDiffStatsCacheInput,
    ) -> Result<ThreadDiffStatsCacheRecord> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            tx.execute(
                &format!(
                    "INSERT INTO thread_diff_stats_cache (id, thread_updated_at, additions, deletions, files_changed, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, {now}, {now}) \
                     ON CONFLICT(id) DO UPDATE SET thread_updated_at = excluded.thread_updated_at, additions = excluded.additions, deletions = excluded.deletions, files_changed = excluded.files_changed, updated_at = {now}",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.thread_updated_at,
                    input.additions,
                    input.deletions,
                    input.files_changed
                ],
            )?;
            let sql = format!("{} WHERE id = ?1", THREAD_DIFF_STATS_CACHE_SELECT_SQL);
            let row = tx.query_row(&sql, [input.id], |row| map_thread_diff_stats_cache_row(row))?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_thread_diff_stats_cache(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected =
                conn.execute("DELETE FROM thread_diff_stats_cache WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }
}
