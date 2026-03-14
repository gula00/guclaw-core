use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, OptionalExtension};

use crate::mappers::{
    map_plugin_permission_row, map_plugin_state_row, PLUGIN_PERMISSION_SELECT_SQL,
    PLUGIN_STATE_SELECT_SQL,
};
use crate::{
    DbHandle, PluginPermissionRecord, PluginStateRecord, UpsertPluginPermissionInput,
    UpsertPluginStateInput,
};

#[napi]
impl DbHandle {
    #[napi]
    pub fn get_all_plugin_states(&self) -> Result<Vec<PluginStateRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} ORDER BY installed_at DESC", PLUGIN_STATE_SELECT_SQL);
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_plugin_state_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_plugin_state_by_id(&self, id: String) -> Result<Option<PluginStateRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", PLUGIN_STATE_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_plugin_state_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn upsert_plugin_state(&self, input: UpsertPluginStateInput) -> Result<PluginStateRecord> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let exists: Option<String> = tx
                .query_row("SELECT id FROM plugins WHERE id = ?1", [&input.id], |row| row.get(0))
                .optional()?;

            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            if exists.is_some() {
                tx.execute(
                    &format!(
                        "UPDATE plugins SET \
                         name = ?2, version = ?3, description = ?4, author = ?5, icon = ?6, \
                         source = ?7, source_path = ?8, install_url = ?9, manifest = ?10, \
                         enabled = COALESCE(?11, enabled), settings = COALESCE(?12, settings), \
                         updated_at = COALESCE(?13, {now}) WHERE id = ?1",
                        now = now_expr
                    ),
                    params![
                        input.id,
                        input.name,
                        input.version,
                        input.description,
                        input.author,
                        input.icon,
                        input.source,
                        input.source_path,
                        input.install_url,
                        input.manifest,
                        input.enabled,
                        input.settings,
                        input.updated_at,
                    ],
                )?;
            } else {
                tx.execute(
                    &format!(
                        "INSERT INTO plugins \
                         (id, name, version, description, author, icon, source, source_path, install_url, manifest, enabled, settings, installed_at, updated_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, COALESCE(?11, 1), COALESCE(?12, '{{}}'), COALESCE(?13, {now}), COALESCE(?14, {now}))",
                        now = now_expr
                    ),
                    params![
                        input.id,
                        input.name,
                        input.version,
                        input.description,
                        input.author,
                        input.icon,
                        input.source,
                        input.source_path,
                        input.install_url,
                        input.manifest,
                        input.enabled,
                        input.settings,
                        input.installed_at,
                        input.updated_at,
                    ],
                )?;
            }

            let sql = format!("{} WHERE id = ?1", PLUGIN_STATE_SELECT_SQL);
            let row = tx.query_row(&sql, [input.id], |row| map_plugin_state_row(row))?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_plugin_state(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM plugins WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn get_plugin_permissions(&self, plugin_id: String) -> Result<Vec<PluginPermissionRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} WHERE plugin_id = ?1 ORDER BY created_at ASC",
                PLUGIN_PERMISSION_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([plugin_id], |row| map_plugin_permission_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn upsert_plugin_permission(
        &self,
        input: UpsertPluginPermissionInput,
    ) -> Result<PluginPermissionRecord> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let existing = tx
                .query_row(
                    &format!(
                        "{} WHERE plugin_id = ?1 AND permission = ?2",
                        PLUGIN_PERMISSION_SELECT_SQL
                    ),
                    params![input.plugin_id, input.permission],
                    |row| map_plugin_permission_row(row),
                )
                .optional()?;

            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            let record_id = if let Some(existing) = existing {
                tx.execute(
                    &format!(
                        "UPDATE plugin_permissions SET \
                         status = ?2, granted_at = ?3, updated_at = COALESCE(?4, {now}) \
                         WHERE id = ?1",
                        now = now_expr
                    ),
                    params![
                        existing.id,
                        input.status,
                        input.granted_at,
                        input.updated_at,
                    ],
                )?;
                existing.id
            } else {
                let record_id = input.id.ok_or_else(|| {
                    rusqlite::Error::InvalidParameterName(
                        "plugin permission id is required for insert".to_string(),
                    )
                })?;
                tx.execute(
                    &format!(
                        "INSERT INTO plugin_permissions \
                         (id, plugin_id, permission, status, granted_at, created_at, updated_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, {now}), COALESCE(?7, {now}))",
                        now = now_expr
                    ),
                    params![
                        record_id,
                        input.plugin_id,
                        input.permission,
                        input.status,
                        input.granted_at,
                        input.created_at,
                        input.updated_at,
                    ],
                )?;
                record_id
            };

            let sql = format!("{} WHERE id = ?1", PLUGIN_PERMISSION_SELECT_SQL);
            let row = tx.query_row(&sql, [record_id], |row| map_plugin_permission_row(row))?;
            tx.commit()?;
            Ok(row)
        })
    }
}
