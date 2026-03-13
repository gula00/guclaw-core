use napi::bindgen_prelude::Result;
use napi_derive::napi;
use rusqlite::{params, OptionalExtension};

use crate::mappers::{map_workspace_row, WORKSPACE_SELECT_SQL};
use crate::{
    CreateWorkspaceInput, DbHandle, GetAllWorkspacesInput, UpdateWorkspaceFieldsInput,
    WorkspaceRecord,
};

#[napi]
impl DbHandle {
    #[napi]
    pub fn get_all_workspaces(
        &self,
        input: Option<GetAllWorkspacesInput>,
    ) -> Result<Vec<WorkspaceRecord>> {
        self.with_connection(|conn| {
            let include_worktrees = input
                .as_ref()
                .and_then(|i| i.include_worktrees)
                .unwrap_or(false);
            let include_hidden_temporary = input
                .as_ref()
                .and_then(|i| i.include_hidden_temporary)
                .unwrap_or(false);

            let mut conditions = vec![];
            if !include_worktrees {
                conditions.push("(is_worktree = 0 OR is_worktree IS NULL)");
            }
            if !include_hidden_temporary {
                conditions.push("show_in_list = 1");
            }

            let mut sql = WORKSPACE_SELECT_SQL.to_string();
            if !conditions.is_empty() {
                sql.push_str(" WHERE ");
                sql.push_str(&conditions.join(" AND "));
            }
            sql.push_str(" ORDER BY created_at DESC");

            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| map_workspace_row(row))?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    #[napi]
    pub fn get_workspace_by_id(&self, id: String) -> Result<Option<WorkspaceRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE id = ?1", WORKSPACE_SELECT_SQL);
            conn.query_row(&sql, [id], |row| map_workspace_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn get_workspace_by_path(&self, path: String) -> Result<Option<WorkspaceRecord>> {
        self.with_connection(|conn| {
            let sql = format!("{} WHERE path = ?1", WORKSPACE_SELECT_SQL);
            conn.query_row(&sql, [path], |row| map_workspace_row(row))
                .optional()
        })
    }

    #[napi]
    pub fn create_workspace(&self, input: CreateWorkspaceInput) -> Result<WorkspaceRecord> {
        self.with_connection(|conn| {
            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            conn.execute(
                &format!(
                    "INSERT INTO workspaces \
                     (id, path, name, is_temporary, show_in_list, is_worktree, parent_workspace_id, worktree_branch, auto_worktree, pr_number, pr_url, pr_state, pr_base_branch, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, COALESCE(?4, 0), COALESCE(?5, 1), COALESCE(?6, 0), ?7, ?8, COALESCE(?9, 0), ?10, ?11, ?12, ?13, COALESCE(?14, {now}), COALESCE(?15, {now}))",
                    now = now_expr
                ),
                params![
                    input.id,
                    input.path,
                    input.name,
                    input.is_temporary,
                    input.show_in_list,
                    input.is_worktree,
                    input.parent_workspace_id,
                    input.worktree_branch,
                    input.auto_worktree,
                    input.pr_number,
                    input.pr_url,
                    input.pr_state,
                    input.pr_base_branch,
                    input.created_at,
                    input.updated_at,
                ],
            )?;

            let sql = format!("{} WHERE id = ?1", WORKSPACE_SELECT_SQL);
            conn.query_row(&sql, [input.id], |row| map_workspace_row(row))
        })
    }

    #[napi]
    pub fn update_workspace_fields(
        &self,
        input: UpdateWorkspaceFieldsInput,
    ) -> Result<Option<WorkspaceRecord>> {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;

            let exists: Option<String> = tx
                .query_row(
                    "SELECT id FROM workspaces WHERE id = ?1",
                    [&input.id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                tx.rollback()?;
                return Ok(None);
            }

            if input.set_path.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET path = ?2 WHERE id = ?1",
                    params![input.id, input.path],
                )?;
            }
            if input.set_name.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET name = ?2 WHERE id = ?1",
                    params![input.id, input.name],
                )?;
            }
            if input.set_is_temporary.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET is_temporary = ?2 WHERE id = ?1",
                    params![input.id, input.is_temporary],
                )?;
            }
            if input.set_show_in_list.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET show_in_list = ?2 WHERE id = ?1",
                    params![input.id, input.show_in_list],
                )?;
            }
            if input.set_is_worktree.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET is_worktree = ?2 WHERE id = ?1",
                    params![input.id, input.is_worktree],
                )?;
            }
            if input.set_parent_workspace_id.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET parent_workspace_id = ?2 WHERE id = ?1",
                    params![input.id, input.parent_workspace_id],
                )?;
            }
            if input.set_worktree_branch.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET worktree_branch = ?2 WHERE id = ?1",
                    params![input.id, input.worktree_branch],
                )?;
            }
            if input.set_auto_worktree.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET auto_worktree = ?2 WHERE id = ?1",
                    params![input.id, input.auto_worktree],
                )?;
            }
            if input.set_pr_number.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET pr_number = ?2 WHERE id = ?1",
                    params![input.id, input.pr_number],
                )?;
            }
            if input.set_pr_url.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET pr_url = ?2 WHERE id = ?1",
                    params![input.id, input.pr_url],
                )?;
            }
            if input.set_pr_state.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET pr_state = ?2 WHERE id = ?1",
                    params![input.id, input.pr_state],
                )?;
            }
            if input.set_pr_base_branch.unwrap_or(false) {
                tx.execute(
                    "UPDATE workspaces SET pr_base_branch = ?2 WHERE id = ?1",
                    params![input.id, input.pr_base_branch],
                )?;
            }

            let now_expr = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
            if input.set_updated_at.unwrap_or(false) {
                tx.execute(
                    &format!(
                        "UPDATE workspaces SET updated_at = COALESCE(?2, {now}) WHERE id = ?1",
                        now = now_expr
                    ),
                    params![input.id, input.updated_at],
                )?;
            }

            let sql = format!("{} WHERE id = ?1", WORKSPACE_SELECT_SQL);
            let row = tx
                .query_row(&sql, [input.id], |row| map_workspace_row(row))
                .optional()?;
            tx.commit()?;
            Ok(row)
        })
    }

    #[napi]
    pub fn delete_workspace(&self, id: String) -> Result<bool> {
        self.with_connection(|conn| {
            let affected = conn.execute("DELETE FROM workspaces WHERE id = ?1", [id])?;
            Ok(affected > 0)
        })
    }
}
