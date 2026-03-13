use rusqlite::{Connection, OptionalExtension};

use crate::{
    AppSettingsRecord, ChannelMappingRecord, DistinctChannelRecord, McpOauthTokenRecord,
    McpServerRecord, MessageRecord, ModelCapabilitiesCacheRecord, PromptAppExecutionRecord,
    PromptAppRecord, PromptRecord, ProviderModelsCacheRecord, ProviderRecord, SkillStateRecord,
    ThreadDiffStatsCacheRecord, ThreadLabelRecord, ThreadRecord, WorkspaceRecord,
};

pub(crate) fn map_message_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageRecord> {
    Ok(MessageRecord {
        id: row.get(0)?,
        thread_id: row.get(1)?,
        parent_id: row.get(2)?,
        slot_id: row.get(3)?,
        depth: row.get(4)?,
        parent_tool_call_id: row.get(5)?,
        message: row.get(6)?,
        timestamp: row.get(7)?,
        metadata: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub(crate) fn map_thread_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ThreadRecord> {
    Ok(ThreadRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        model: row.get(2)?,
        prompt_app_id: row.get(3)?,
        tools: row.get(4)?,
        is_favorited: row.get(5)?,
        is_incognito: row.get(6)?,
        workspace_id: row.get(7)?,
        artifact_workspace_id: row.get(8)?,
        enable_artifacts: row.get(9)?,
        skill_ids: row.get(10)?,
        tools_compact_view: row.get(11)?,
        parent_thread_id: row.get(12)?,
        is_favorite_pinned: row.get(13)?,
        favorite_pinned_order: row.get(14)?,
        is_generating: row.get(15)?,
        reasoning_effort: row.get(16)?,
        metadata: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}

pub(crate) const THREAD_SELECT_SQL: &str = "SELECT id, title, model, prompt_app_id, tools, is_favorited, is_incognito, workspace_id, artifact_workspace_id, enable_artifacts, skill_ids, tools_compact_view, parent_thread_id, is_favorite_pinned, favorite_pinned_order, is_generating, reasoning_effort, metadata, created_at, updated_at FROM chat_threads";
pub(crate) const PROVIDER_SELECT_SQL: &str = "SELECT id, name, type, api_key, models, base_url, api_version, enabled, created_at, updated_at, is_response_api, acp_command, acp_args, acp_mcp_server_ids, acp_auth_method_id, acp_api_provider_id, acp_model_mapping, use_max_completion_tokens, api_format, available_models, copilot_account_id FROM providers";
pub(crate) const SETTINGS_SELECT_SQL: &str =
    "SELECT id, settings_data, created_at, updated_at FROM app_settings";
pub(crate) const CHANNEL_MAPPING_SELECT_SQL: &str = "SELECT id, platform, external_chat_id, external_user_id, thread_id, is_active, created_at, updated_at FROM channel_mappings";
pub(crate) const DISTINCT_CHANNELS_SQL: &str = "SELECT cm.external_chat_id, COALESCE(ct.title, cm.external_chat_id) AS label, cm.updated_at FROM channel_mappings cm LEFT JOIN chat_threads ct ON cm.thread_id = ct.id WHERE cm.platform = ?1 GROUP BY cm.external_chat_id ORDER BY cm.updated_at DESC";
pub(crate) const PROMPT_APP_SELECT_SQL: &str = "SELECT id, name, description, icon, prompt_template, placeholders, model, enabled, shortcut, sort_order, created_at, updated_at, tools, reasoning_effort, expects_image_result, is_incognito, window_width, window_height, font_size FROM prompt_apps";
pub(crate) const PROMPT_APP_EXEC_SELECT_SQL: &str = "SELECT id, prompt_app_id, thread_id, input_values, generated_prompt, attachment_count, created_at FROM prompt_app_executions";
pub(crate) const MODEL_CAPS_CACHE_SELECT_SQL: &str =
    "SELECT id, capabilities, fetched_at, created_at, updated_at FROM model_capabilities_cache";
pub(crate) const PROVIDER_MODELS_CACHE_SELECT_SQL: &str =
    "SELECT id, provider_id, models, fetched_at, created_at, updated_at FROM provider_models_cache";
pub(crate) const THREAD_DIFF_STATS_CACHE_SELECT_SQL: &str = "SELECT id, thread_updated_at, additions, deletions, files_changed, created_at, updated_at FROM thread_diff_stats_cache";
pub(crate) const MCP_SERVER_SELECT_SQL: &str = "SELECT id, registry_id, name, description, config, enabled, status, last_error, installed_at, updated_at FROM mcp_servers";
pub(crate) const MCP_OAUTH_TOKEN_SELECT_SQL: &str = "SELECT id, server_id, authorization_server_url, resource_url, client_id, client_secret, client_id_issued_at, client_secret_expires_at, access_token, refresh_token, token_type, expires_at, scope, code_verifier, last_refresh_at, last_error_at, last_error, created_at, updated_at FROM mcp_oauth_tokens";
pub(crate) const WORKSPACE_SELECT_SQL: &str = "SELECT id, path, name, is_temporary, show_in_list, is_worktree, parent_workspace_id, worktree_branch, auto_worktree, pr_number, pr_url, pr_state, pr_base_branch, created_at, updated_at FROM workspaces";
pub(crate) const PROMPT_SELECT_SQL: &str =
    "SELECT id, name, content, sort_order, created_at, updated_at FROM prompts";
pub(crate) const THREAD_LABEL_SELECT_SQL: &str =
    "SELECT id, name, color, sort_order, created_at, updated_at FROM thread_labels";
pub(crate) const SKILL_STATE_SELECT_SQL: &str =
    "SELECT id, path, enabled, sort_order, updated_at FROM skills";

pub(crate) fn map_provider_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderRecord> {
    Ok(ProviderRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        r#type: row.get(2)?,
        api_key: row.get(3)?,
        models: row.get(4)?,
        base_url: row.get(5)?,
        api_version: row.get(6)?,
        enabled: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        is_response_api: row.get(10)?,
        acp_command: row.get(11)?,
        acp_args: row.get(12)?,
        acp_mcp_server_ids: row.get(13)?,
        acp_auth_method_id: row.get(14)?,
        acp_api_provider_id: row.get(15)?,
        acp_model_mapping: row.get(16)?,
        use_max_completion_tokens: row.get(17)?,
        api_format: row.get(18)?,
        available_models: row.get(19)?,
        copilot_account_id: row.get(20)?,
    })
}

pub(crate) fn map_settings_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AppSettingsRecord> {
    Ok(AppSettingsRecord {
        id: row.get(0)?,
        settings_data: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
    })
}

pub(crate) fn map_channel_mapping_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ChannelMappingRecord> {
    Ok(ChannelMappingRecord {
        id: row.get(0)?,
        platform: row.get(1)?,
        external_chat_id: row.get(2)?,
        external_user_id: row.get(3)?,
        thread_id: row.get(4)?,
        is_active: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub(crate) fn map_distinct_channel_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<DistinctChannelRecord> {
    Ok(DistinctChannelRecord {
        external_chat_id: row.get(0)?,
        label: row.get(1)?,
        last_active: row.get(2)?,
    })
}

pub(crate) fn map_prompt_app_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PromptAppRecord> {
    Ok(PromptAppRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        icon: row.get(3)?,
        prompt_template: row.get(4)?,
        placeholders: row.get(5)?,
        model: row.get(6)?,
        enabled: row.get(7)?,
        shortcut: row.get(8)?,
        sort_order: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        tools: row.get(12)?,
        reasoning_effort: row.get(13)?,
        expects_image_result: row.get(14)?,
        is_incognito: row.get(15)?,
        window_width: row.get(16)?,
        window_height: row.get(17)?,
        font_size: row.get(18)?,
    })
}

pub(crate) fn map_prompt_app_exec_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<PromptAppExecutionRecord> {
    Ok(PromptAppExecutionRecord {
        id: row.get(0)?,
        prompt_app_id: row.get(1)?,
        thread_id: row.get(2)?,
        input_values: row.get(3)?,
        generated_prompt: row.get(4)?,
        attachment_count: row.get(5)?,
        created_at: row.get(6)?,
    })
}

pub(crate) fn map_model_caps_cache_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ModelCapabilitiesCacheRecord> {
    Ok(ModelCapabilitiesCacheRecord {
        id: row.get(0)?,
        capabilities: row.get(1)?,
        fetched_at: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

pub(crate) fn map_provider_models_cache_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ProviderModelsCacheRecord> {
    Ok(ProviderModelsCacheRecord {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        models: row.get(2)?,
        fetched_at: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub(crate) fn map_thread_diff_stats_cache_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ThreadDiffStatsCacheRecord> {
    Ok(ThreadDiffStatsCacheRecord {
        id: row.get(0)?,
        thread_updated_at: row.get(1)?,
        additions: row.get(2)?,
        deletions: row.get(3)?,
        files_changed: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

pub(crate) fn map_mcp_server_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<McpServerRecord> {
    Ok(McpServerRecord {
        id: row.get(0)?,
        registry_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        config: row.get(4)?,
        enabled: row.get(5)?,
        status: row.get(6)?,
        last_error: row.get(7)?,
        installed_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub(crate) fn map_mcp_oauth_token_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<McpOauthTokenRecord> {
    Ok(McpOauthTokenRecord {
        id: row.get(0)?,
        server_id: row.get(1)?,
        authorization_server_url: row.get(2)?,
        resource_url: row.get(3)?,
        client_id: row.get(4)?,
        client_secret: row.get(5)?,
        client_id_issued_at: row.get(6)?,
        client_secret_expires_at: row.get(7)?,
        access_token: row.get(8)?,
        refresh_token: row.get(9)?,
        token_type: row.get(10)?,
        expires_at: row.get(11)?,
        scope: row.get(12)?,
        code_verifier: row.get(13)?,
        last_refresh_at: row.get(14)?,
        last_error_at: row.get(15)?,
        last_error: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

pub(crate) fn map_workspace_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceRecord> {
    Ok(WorkspaceRecord {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        is_temporary: row.get(3)?,
        show_in_list: row.get(4)?,
        is_worktree: row.get(5)?,
        parent_workspace_id: row.get(6)?,
        worktree_branch: row.get(7)?,
        auto_worktree: row.get(8)?,
        pr_number: row.get(9)?,
        pr_url: row.get(10)?,
        pr_state: row.get(11)?,
        pr_base_branch: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

pub(crate) fn map_prompt_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PromptRecord> {
    Ok(PromptRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        content: row.get(2)?,
        sort_order: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub(crate) fn map_thread_label_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ThreadLabelRecord> {
    Ok(ThreadLabelRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        sort_order: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub(crate) fn map_skill_state_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SkillStateRecord> {
    Ok(SkillStateRecord {
        id: row.get(0)?,
        path: row.get(1)?,
        enabled: row.get(2)?,
        sort_order: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

pub(crate) fn table_exists(conn: &Connection, table_name: &str) -> rusqlite::Result<bool> {
    let exists: Option<i32> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            [table_name],
            |row| row.get(0),
        )
        .optional()?;
    Ok(exists.is_some())
}
