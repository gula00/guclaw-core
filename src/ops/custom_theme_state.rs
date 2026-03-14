use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, OptionalExtension};

use crate::mappers::{map_custom_theme_row, CUSTOM_THEME_SELECT_SQL};
use crate::{CreateCustomThemeInput, CustomThemeRecord, DbHandle, UpdateCustomThemeInput};

#[napi]
impl DbHandle {
    #[napi]
    pub fn get_all_custom_themes(&self) -> Result<Vec<CustomThemeRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} ORDER BY created_at DESC", CUSTOM_THEME_SELECT_SQL);
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_custom_theme_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_custom_theme_by_id(&self, id: String) -> Result<Option<CustomThemeRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", CUSTOM_THEME_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_custom_theme_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_custom_theme_by_name(&self, name: String) -> Result<Option<CustomThemeRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE name = ?1", CUSTOM_THEME_SELECT_SQL);
            conn.query_row(&sql, [name], |row| map_custom_theme_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn create_custom_theme(&self, input: CreateCustomThemeInput) -> Result<CustomThemeRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            conn.execute(
                &format!(
                    "INSERT INTO custom_themes (id, name, display_name, type, base_30, base_16, based_on, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(?8, {now}), COALESCE(?9, {now}))",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.name,
                    input.display_name,
                    input.r#type,
                    input.base_30,
                    input.base_16,
                    input.based_on,
                    input.created_at,
                    input.updated_at,
                ],
            )?;

            let sql = format!("{} WHERE id = ?1", CUSTOM_THEME_SELECT_SQL);
            conn.query_row(&sql, [input.id], |row| map_custom_theme_row(row))
        })
    }

    #[napi]
    pub fn update_custom_theme(
        &self,
        input: UpdateCustomThemeInput,
    ) -> Result<Option<CustomThemeRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM custom_themes WHERE id = ?1",
                    [&input.id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                tx.rollback()?;
                return Ok(None);
            }

            if let Some(display_name) = input.display_name {
                tx.execute(
                    "UPDATE custom_themes SET display_name = ?2 WHERE id = ?1",
                    params![input.id, display_name],
                )?;
            }
            if let Some(theme_type) = input.r#type {
                tx.execute(
                    "UPDATE custom_themes SET type = ?2 WHERE id = ?1",
                    params![input.id, theme_type],
                )?;
            }
            if let Some(base_30) = input.base_30 {
                tx.execute(
                    "UPDATE custom_themes SET base_30 = ?2 WHERE id = ?1",
                    params![input.id, base_30],
                )?;
            }
            if let Some(base_16) = input.base_16 {
                tx.execute(
                    "UPDATE custom_themes SET base_16 = ?2 WHERE id = ?1",
                    params![input.id, base_16],
                )?;
            }
            if input.based_on.is_some() {
                tx.execute(
                    "UPDATE custom_themes SET based_on = ?2 WHERE id = ?1",
                    params![input.id, input.based_on],
                )?;
            }

            tx.execute(
                "UPDATE custom_themes SET updated_at = COALESCE(?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1",
                params![input.id, input.updated_at],
            )?;

            let sql = format!("{} WHERE id = ?1", CUSTOM_THEME_SELECT_SQL);
            let row = tx
                .query_row(&sql, [input.id], |row| map_custom_theme_row(row))
                .optional()?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_custom_theme(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM custom_themes WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }
}
