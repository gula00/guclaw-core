use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, OptionalExtension};
use serde_json::Value as JsonValue;

use crate::mappers::{
    map_mcp_oauth_token_row, map_mcp_server_row, MCP_OAUTH_TOKEN_SELECT_SQL, MCP_SERVER_SELECT_SQL,
};
use crate::{
    CreateMcpServerInput, DbHandle, McpOauthTokenRecord, McpServerRecord,
    UpdateMcpServerFieldsInput,
};

fn oauth_key_to_column(key: &str) -> Option<&'static str> {
    match key {
        "authorizationServerUrl" => Some("authorization_server_url"),
        "resourceUrl" => Some("resource_url"),
        "clientId" => Some("client_id"),
        "clientSecret" => Some("client_secret"),
        "clientIdIssuedAt" => Some("client_id_issued_at"),
        "clientSecretExpiresAt" => Some("client_secret_expires_at"),
        "accessToken" => Some("access_token"),
        "refreshToken" => Some("refresh_token"),
        "tokenType" => Some("token_type"),
        "expiresAt" => Some("expires_at"),
        "scope" => Some("scope"),
        "codeVerifier" => Some("code_verifier"),
        "lastRefreshAt" => Some("last_refresh_at"),
        "lastErrorAt" => Some("last_error_at"),
        "lastError" => Some("last_error"),
        _ => None,
    }
}

fn json_to_sql_value(v: &JsonValue) -> rusqlite::Result<SqlValue> {
    match v {
        JsonValue::Null => Ok(SqlValue::Null),
        JsonValue::String(s) => Ok(SqlValue::Text(s.clone())),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(SqlValue::Integer(i))
            } else {
                Err(rusqlite::Error::InvalidParameterName(
                    "numeric oauth payload value must be i64".to_string(),
                ))
            }
        }
        JsonValue::Bool(b) => Ok(SqlValue::Integer(if *b { 1 } else { 0 })),
        _ => Err(rusqlite::Error::InvalidParameterName(
            "oauth payload value must be string/number/bool/null".to_string(),
        )),
    }
}

fn parse_oauth_payload(payload_json: &str) -> rusqlite::Result<serde_json::Map<String, JsonValue>> {
    match serde_json::from_str::<JsonValue>(payload_json) {
        Ok(JsonValue::Object(m)) => Ok(m),
        Ok(_) => Err(rusqlite::Error::InvalidParameterName(
            "oauth payload must be a JSON object".to_string(),
        )),
        Err(err) => Err(rusqlite::Error::InvalidParameterName(format!(
            "invalid oauth payload JSON: {err}"
        ))),
    }
}

#[napi]
impl DbHandle {
    #[napi]
    pub fn get_all_mcp_servers(&self) -> Result<Vec<McpServerRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} ORDER BY installed_at DESC", MCP_SERVER_SELECT_SQL);
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_mcp_server_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_enabled_mcp_servers(&self) -> Result<Vec<McpServerRecord>> {
        self.with_connection(|conn| {
            let sql = format!(
                "{} WHERE enabled = 1 ORDER BY installed_at DESC",
                MCP_SERVER_SELECT_SQL
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_mcp_server_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_mcp_server_by_id(&self, id: String) -> Result<Option<McpServerRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", MCP_SERVER_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_mcp_server_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_mcp_server_by_registry_id(
        &self,
        registry_id: String,
    ) -> Result<Option<McpServerRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE registry_id = ?1", MCP_SERVER_SELECT_SQL);
            conn.query_row(&sql, [registry_id], |row| map_mcp_server_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_mcp_server_by_name(&self, name: String) -> Result<Option<McpServerRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE name = ?1", MCP_SERVER_SELECT_SQL);
            conn.query_row(&sql, [name], |row| map_mcp_server_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn create_mcp_server(&self, input: CreateMcpServerInput) -> Result<McpServerRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            conn.execute(
                &format!(
                    "INSERT INTO mcp_servers \
                     (id, registry_id, name, description, config, enabled, status, last_error, installed_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, 1), COALESCE(?7, 'disconnected'), ?8, COALESCE(?9, {now}), COALESCE(?10, {now}))",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.registry_id,
                    input.name,
                    input.description,
                    input.config,
                    input.enabled,
                    input.status,
                    input.last_error,
                    input.installed_at,
                    input.updated_at,
                ],
            )?;

            let sql = format!("{} WHERE id = ?1", MCP_SERVER_SELECT_SQL);
            conn.query_row(&sql, [input.id], |row| map_mcp_server_row(row))
        })
    }

    #[napi]
    pub fn update_mcp_server_fields(
        &self,
        input: UpdateMcpServerFieldsInput,
    ) -> Result<Option<McpServerRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;

            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM mcp_servers WHERE id = ?1",
                    [&input.id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                tx.rollback()?;
                return Ok(None);
            }

            if input.set_registry_id.unwrap_or(false) {
                tx.execute(
                    "UPDATE mcp_servers SET registry_id = ?2 WHERE id = ?1",
                    params![input.id, input.registry_id],
                )?;
            }
            if input.set_name.unwrap_or(false) {
                tx.execute(
                    "UPDATE mcp_servers SET name = ?2 WHERE id = ?1",
                    params![input.id, input.name],
                )?;
            }
            if input.set_description.unwrap_or(false) {
                tx.execute(
                    "UPDATE mcp_servers SET description = ?2 WHERE id = ?1",
                    params![input.id, input.description],
                )?;
            }
            if input.set_config.unwrap_or(false) {
                tx.execute(
                    "UPDATE mcp_servers SET config = ?2 WHERE id = ?1",
                    params![input.id, input.config],
                )?;
            }
            if input.set_enabled.unwrap_or(false) {
                tx.execute(
                    "UPDATE mcp_servers SET enabled = ?2 WHERE id = ?1",
                    params![input.id, input.enabled],
                )?;
            }
            if input.set_status.unwrap_or(false) {
                tx.execute(
                    "UPDATE mcp_servers SET status = ?2 WHERE id = ?1",
                    params![input.id, input.status],
                )?;
            }
            if input.set_last_error.unwrap_or(false) {
                tx.execute(
                    "UPDATE mcp_servers SET last_error = ?2 WHERE id = ?1",
                    params![input.id, input.last_error],
                )?;
            }
            if input.set_installed_at.unwrap_or(false) {
                tx.execute(
                    "UPDATE mcp_servers SET installed_at = ?2 WHERE id = ?1",
                    params![input.id, input.installed_at],
                )?;
            }

            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            if input.set_updated_at.unwrap_or(false) {
                tx.execute(
                    &format!(
                        "UPDATE mcp_servers SET updated_at = COALESCE(?2, {now}) WHERE id = ?1",
                        now = now_expr
                    ),
                    params![input.id, input.updated_at],
                )?;
            }

            let sql = format!("{} WHERE id = ?1", MCP_SERVER_SELECT_SQL);
            let row = tx
                .query_row(&sql, [input.id], |row| map_mcp_server_row(row))
                .optional()?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_mcp_server(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM mcp_servers WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn get_mcp_oauth_token_by_server_id(
        &self,
        server_id: String,
    ) -> Result<Option<McpOauthTokenRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE server_id = ?1", MCP_OAUTH_TOKEN_SELECT_SQL);
            conn.query_row(&sql, [server_id], |row| map_mcp_oauth_token_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_all_mcp_oauth_tokens(&self) -> Result<Vec<McpOauthTokenRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(MCP_OAUTH_TOKEN_SELECT_SQL)?;
            let mapped = stmt.query_map([], |row| map_mcp_oauth_token_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn save_mcp_oauth_token(
        &self,
        server_id: String,
        payload_json: String,
        new_id: String,
    ) -> Result<McpOauthTokenRecord> {
        self.with_connection(|conn| {
            let payload = parse_oauth_payload(&payload_json)?;
            let tx = conn.transaction()?;
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

            let existing: Option<String> = tx
                .query_row(
                    "SELECT id FROM mcp_oauth_tokens WHERE server_id = ?1",
                    [&server_id],
                    |row| row.get(0),
                )
                .optional()?;

            if existing.is_some() {
                let mut set_clauses = vec![];
                let mut values: Vec<SqlValue> = vec![];

                for (key, value) in payload.iter() {
                    if let Some(column) = oauth_key_to_column(key.as_str()) {
                        set_clauses.push(format!("{column} = ?"));
                        values.push(json_to_sql_value(value)?);
                    }
                }

                set_clauses.push("updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')".to_string());
                values.push(SqlValue::Text(server_id.clone()));

                tx.execute(
                    &format!(
                        "UPDATE mcp_oauth_tokens SET {} WHERE server_id = ?",
                        set_clauses.join(", ")
                    ),
                    params_from_iter(values.iter()),
                )?;
            } else {
                let mut columns = vec!["id", "server_id", "created_at", "updated_at"];
                let mut placeholders = vec!["?", "?", now_expr, now_expr];
                let mut values: Vec<SqlValue> =
                    vec![SqlValue::Text(new_id), SqlValue::Text(server_id.clone())];

                for (key, value) in payload.iter() {
                    if let Some(column) = oauth_key_to_column(key.as_str()) {
                        columns.push(column);
                        placeholders.push("?");
                        values.push(json_to_sql_value(value)?);
                    }
                }

                tx.execute(
                    &format!(
                        "INSERT INTO mcp_oauth_tokens ({}) VALUES ({})",
                        columns.join(", "),
                        placeholders.join(", ")
                    ),
                    params_from_iter(values.iter()),
                )?;
            }

            let sql = format!("{} WHERE server_id = ?1", MCP_OAUTH_TOKEN_SELECT_SQL);
            let row = tx.query_row(&sql, [server_id], |row| map_mcp_oauth_token_row(row))?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn update_mcp_oauth_token(
        &self,
        server_id: String,
        payload_json: String,
    ) -> Result<Option<McpOauthTokenRecord>> {
        self.with_connection(|conn| {
            let payload = parse_oauth_payload(&payload_json)?;

            let mut set_clauses = vec![];
            let mut values: Vec<SqlValue> = vec![];
            for (key, value) in payload.iter() {
                if let Some(column) = oauth_key_to_column(key.as_str()) {
                    set_clauses.push(format!("{column} = ?"));
                    values.push(json_to_sql_value(value)?);
                }
            }
            set_clauses.push("updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')".to_string());
            values.push(SqlValue::Text(server_id.clone()));

            let affected = conn.execute(
                &format!(
                    "UPDATE mcp_oauth_tokens SET {} WHERE server_id = ?",
                    set_clauses.join(", ")
                ),
                params_from_iter(values.iter()),
            )?;
            if affected == 0 {
                return Ok(None);
            }

            let sql = format!("{} WHERE server_id = ?1", MCP_OAUTH_TOKEN_SELECT_SQL);
            conn.query_row(&sql, [server_id], |row| map_mcp_oauth_token_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn delete_mcp_oauth_token(&self, server_id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute(
                "DELETE FROM mcp_oauth_tokens WHERE server_id = ?1",
                [server_id],
            )?;
            Ok(affected > 0)
        })
    }

    #[napi]
    pub fn migrate_mcp_oauth_token(
        &self,
        from_server_id: String,
        to_server_id: String,
    ) -> Result<bool> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;

            let source_sql = format!("{} WHERE server_id = ?1", MCP_OAUTH_TOKEN_SELECT_SQL);
            let source = tx
                .query_row(&source_sql, [from_server_id.clone()], |row| {
                    map_mcp_oauth_token_row(row)
                })
                .optional()?;

            let Some(source) = source else {
                tx.rollback()?;
                return Ok(false);
            };

            let target = tx
                .query_row(&source_sql, [to_server_id.clone()], |row| {
                    map_mcp_oauth_token_row(row)
                })
                .optional()?;

            if target.is_some() {
                tx.execute(
                    "UPDATE mcp_oauth_tokens SET \
                     access_token = COALESCE(?2, access_token), \
                     refresh_token = COALESCE(?3, refresh_token), \
                     token_type = COALESCE(?4, token_type), \
                     expires_at = COALESCE(?5, expires_at), \
                     scope = COALESCE(?6, scope), \
                     client_id = COALESCE(?7, client_id), \
                     client_secret = COALESCE(?8, client_secret), \
                     last_error = NULL, \
                     last_error_at = NULL, \
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                     WHERE server_id = ?1",
                    params![
                        to_server_id,
                        source.access_token,
                        source.refresh_token,
                        source.token_type,
                        source.expires_at,
                        source.scope,
                        source.client_id,
                        source.client_secret,
                    ],
                )?;
                tx.execute(
                    "DELETE FROM mcp_oauth_tokens WHERE server_id = ?1",
                    [from_server_id],
                )?;
                tx.commit()?;
                return Ok(true);
            }

            let affected = tx.execute(
                "UPDATE mcp_oauth_tokens \
                 SET server_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE server_id = ?1",
                params![from_server_id, to_server_id],
            )?;
            tx.commit()?;
            Ok(affected > 0)
        })
    }
}
