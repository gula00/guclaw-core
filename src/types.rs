use napi_derive::napi;
use serde::Serialize;

#[napi(object)]
pub struct PingResult {
    pub ok: bool,
    pub sqlite_version: String,
}

#[derive(Clone, Serialize)]
#[napi(object)]
pub struct ThreadRecord {
    pub id: String,
    pub title: String,
    pub model: Option<String>,
    pub prompt_app_id: Option<String>,
    pub tools: Option<String>,
    pub is_favorited: Option<bool>,
    pub is_incognito: Option<bool>,
    pub workspace_id: Option<String>,
    pub artifact_workspace_id: Option<String>,
    pub enable_artifacts: Option<bool>,
    pub skill_ids: Option<String>,
    pub tools_compact_view: Option<bool>,
    pub parent_thread_id: Option<String>,
    pub is_favorite_pinned: Option<bool>,
    pub favorite_pinned_order: Option<i32>,
    pub is_generating: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct CreateThreadInput {
    pub id: String,
    pub title: String,
    pub model: Option<String>,
    pub prompt_app_id: Option<String>,
    pub tools: Option<String>,
    pub skill_ids: Option<String>,
    pub tools_compact_view: Option<bool>,
    pub workspace_id: Option<String>,
    pub artifact_workspace_id: Option<String>,
    pub enable_artifacts: Option<bool>,
    pub parent_thread_id: Option<String>,
    pub is_generating: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub metadata: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Serialize)]
#[napi(object)]
pub struct MessageRecord {
    pub id: String,
    pub thread_id: String,
    pub parent_id: Option<String>,
    pub slot_id: Option<String>,
    pub depth: Option<i32>,
    pub parent_tool_call_id: Option<String>,
    pub message: String,
    pub timestamp: String,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize)]
#[napi(object)]
pub struct MessageVersionInfoRecord {
    pub id: String,
    pub slot_id: Option<String>,
    pub created_at: String,
}

#[napi(object)]
pub struct MessageMetadataRecord {
    pub id: String,
    pub metadata: String,
}

#[napi(object)]
pub struct AddMessageInput {
    pub thread_id: String,
    pub message_id: String,
    pub message: String,
    pub metadata: Option<String>,
    pub parent_id: Option<String>,
    pub slot_id: Option<String>,
    pub depth: Option<i32>,
    pub parent_tool_call_id: Option<String>,
    pub timestamp: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateMessageContentInput {
    pub id: String,
    pub message: String,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateMessageMetadataInput {
    pub id: String,
    pub metadata: String,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateThreadCoreInput {
    pub id: String,
    pub metadata: Option<String>,
    pub is_generating: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateThreadFieldsInput {
    pub id: String,
    pub set_title: Option<bool>,
    pub title: Option<String>,
    pub set_model: Option<bool>,
    pub model: Option<String>,
    pub set_workspace_id: Option<bool>,
    pub workspace_id: Option<String>,
    pub set_artifact_workspace_id: Option<bool>,
    pub artifact_workspace_id: Option<String>,
    pub set_enable_artifacts: Option<bool>,
    pub enable_artifacts: Option<bool>,
    pub set_parent_thread_id: Option<bool>,
    pub parent_thread_id: Option<String>,
    pub set_metadata: Option<bool>,
    pub metadata: Option<String>,
    pub set_is_generating: Option<bool>,
    pub is_generating: Option<bool>,
    pub set_reasoning_effort: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub set_updated_at: Option<bool>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateFtsForMessageInput {
    pub message_id: String,
    pub thread_id: String,
    pub content: String,
}

#[napi(object)]
pub struct FtsEntryInput {
    pub message_id: String,
    pub thread_id: String,
    pub content: String,
}

#[napi(object)]
pub struct SearchFtsInput {
    pub match_query: String,
    pub limit: Option<i32>,
}

#[napi(object)]
pub struct SearchThreadsInput {
    pub match_query: String,
    pub title_query: String,
    pub limit: Option<i32>,
    pub context_size: Option<i32>,
    pub max_messages_per_thread: Option<i32>,
}

#[napi(object)]
pub struct SyncGalleryMessageInput {
    pub id: String,
    pub thread_id: String,
    pub thread_title: Option<String>,
    pub message_json: String,
    pub created_at: String,
    pub cache_dir: Option<String>,
}

#[napi(object)]
pub struct FtsHitRecord {
    pub thread_id: String,
    pub message_id: String,
}

#[napi(object)]
pub struct McpServerRecord {
    pub id: String,
    pub registry_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub config: String,
    pub enabled: Option<bool>,
    pub status: String,
    pub last_error: Option<String>,
    pub installed_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct CreateMcpServerInput {
    pub id: String,
    pub registry_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub config: String,
    pub enabled: Option<bool>,
    pub status: Option<String>,
    pub last_error: Option<String>,
    pub installed_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateMcpServerFieldsInput {
    pub id: String,
    pub set_registry_id: Option<bool>,
    pub registry_id: Option<String>,
    pub set_name: Option<bool>,
    pub name: Option<String>,
    pub set_description: Option<bool>,
    pub description: Option<String>,
    pub set_config: Option<bool>,
    pub config: Option<String>,
    pub set_enabled: Option<bool>,
    pub enabled: Option<bool>,
    pub set_status: Option<bool>,
    pub status: Option<String>,
    pub set_last_error: Option<bool>,
    pub last_error: Option<String>,
    pub set_installed_at: Option<bool>,
    pub installed_at: Option<String>,
    pub set_updated_at: Option<bool>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct McpOauthTokenRecord {
    pub id: String,
    pub server_id: String,
    pub authorization_server_url: Option<String>,
    pub resource_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub client_id_issued_at: Option<i64>,
    pub client_secret_expires_at: Option<i64>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_at: Option<i64>,
    pub scope: Option<String>,
    pub code_verifier: Option<String>,
    pub last_refresh_at: Option<String>,
    pub last_error_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct WorkspaceRecord {
    pub id: String,
    pub path: String,
    pub name: String,
    pub is_temporary: Option<bool>,
    pub show_in_list: Option<bool>,
    pub is_worktree: Option<bool>,
    pub parent_workspace_id: Option<String>,
    pub worktree_branch: Option<String>,
    pub auto_worktree: Option<bool>,
    pub pr_number: Option<i32>,
    pub pr_url: Option<String>,
    pub pr_state: Option<String>,
    pub pr_base_branch: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct GetAllWorkspacesInput {
    pub include_worktrees: Option<bool>,
    pub include_hidden_temporary: Option<bool>,
}

#[napi(object)]
pub struct CreateWorkspaceInput {
    pub id: String,
    pub path: String,
    pub name: String,
    pub is_temporary: Option<bool>,
    pub show_in_list: Option<bool>,
    pub is_worktree: Option<bool>,
    pub parent_workspace_id: Option<String>,
    pub worktree_branch: Option<String>,
    pub auto_worktree: Option<bool>,
    pub pr_number: Option<i32>,
    pub pr_url: Option<String>,
    pub pr_state: Option<String>,
    pub pr_base_branch: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateWorkspaceFieldsInput {
    pub id: String,
    pub set_path: Option<bool>,
    pub path: Option<String>,
    pub set_name: Option<bool>,
    pub name: Option<String>,
    pub set_is_temporary: Option<bool>,
    pub is_temporary: Option<bool>,
    pub set_show_in_list: Option<bool>,
    pub show_in_list: Option<bool>,
    pub set_is_worktree: Option<bool>,
    pub is_worktree: Option<bool>,
    pub set_parent_workspace_id: Option<bool>,
    pub parent_workspace_id: Option<String>,
    pub set_worktree_branch: Option<bool>,
    pub worktree_branch: Option<String>,
    pub set_auto_worktree: Option<bool>,
    pub auto_worktree: Option<bool>,
    pub set_pr_number: Option<bool>,
    pub pr_number: Option<i32>,
    pub set_pr_url: Option<bool>,
    pub pr_url: Option<String>,
    pub set_pr_state: Option<bool>,
    pub pr_state: Option<String>,
    pub set_pr_base_branch: Option<bool>,
    pub pr_base_branch: Option<String>,
    pub set_updated_at: Option<bool>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct PromptRecord {
    pub id: String,
    pub name: String,
    pub content: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct CreatePromptInput {
    pub id: String,
    pub name: String,
    pub content: String,
    pub sort_order: Option<i32>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdatePromptFieldsInput {
    pub id: String,
    pub set_name: Option<bool>,
    pub name: Option<String>,
    pub set_content: Option<bool>,
    pub content: Option<String>,
    pub set_sort_order: Option<bool>,
    pub sort_order: Option<i32>,
    pub set_updated_at: Option<bool>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct ThreadLabelRecord {
    pub id: String,
    pub name: String,
    pub color: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct CreateThreadLabelInput {
    pub id: String,
    pub name: String,
    pub color: String,
    pub sort_order: Option<i32>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateThreadLabelFieldsInput {
    pub id: String,
    pub set_name: Option<bool>,
    pub name: Option<String>,
    pub set_color: Option<bool>,
    pub color: Option<String>,
    pub set_sort_order: Option<bool>,
    pub sort_order: Option<i32>,
    pub set_updated_at: Option<bool>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct SkillStateRecord {
    pub id: String,
    pub path: String,
    pub enabled: Option<bool>,
    pub sort_order: i32,
    pub updated_at: String,
}

#[napi(object)]
pub struct PluginStateRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub icon: Option<String>,
    pub source: String,
    pub source_path: String,
    pub install_url: Option<String>,
    pub manifest: String,
    pub enabled: Option<bool>,
    pub settings: String,
    pub installed_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct UpsertPluginStateInput {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub icon: Option<String>,
    pub source: String,
    pub source_path: String,
    pub install_url: Option<String>,
    pub manifest: String,
    pub enabled: Option<bool>,
    pub settings: Option<String>,
    pub installed_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct PluginPermissionRecord {
    pub id: String,
    pub plugin_id: String,
    pub permission: String,
    pub status: String,
    pub granted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct UpsertPluginPermissionInput {
    pub id: Option<String>,
    pub plugin_id: String,
    pub permission: String,
    pub status: String,
    pub granted_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UsageRecord {
    pub id: String,
    pub message_id: String,
    pub thread_id: String,
    pub model: Option<String>,
    pub provider_id: Option<String>,
    pub date: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cached_input_tokens: Option<i32>,
    pub cache_write_input_tokens: Option<i32>,
    pub reasoning_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub timestamp: String,
    pub created_at: String,
}

#[napi(object)]
pub struct SaveUsageRecordInput {
    pub message_id: String,
    pub thread_id: String,
    pub model: Option<String>,
    pub timestamp: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cached_input_tokens: Option<i32>,
    pub cache_write_input_tokens: Option<i32>,
    pub reasoning_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
}

#[napi(object)]
pub struct GetUsageStatsInput {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
}

#[napi(object)]
pub struct UsageStatByModelAndDate {
    pub model: String,
    pub date: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cached_tokens: i32,
    pub cache_write_tokens: i32,
    pub reasoning_tokens: i32,
    pub total_tokens: i32,
    pub message_count: i32,
}

#[napi(object)]
pub struct UsageActivityByDate {
    pub date: String,
    pub total_tokens: i32,
}

#[napi(object)]
pub struct UsageStatsResult {
    pub by_model_and_date: Vec<UsageStatByModelAndDate>,
    pub activity_by_date: Vec<UsageActivityByDate>,
}

#[napi(object)]
pub struct UsageMigrationStatusRecord {
    pub id: i32,
    pub status: String,
    pub total_count: Option<i32>,
    pub migrated_count: Option<i32>,
    pub last_migrated_id: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

#[napi(object)]
pub struct UpdateUsageMigrationStatusInput {
    pub status: Option<String>,
    pub total_count: Option<i32>,
    pub migrated_count: Option<i32>,
    pub last_migrated_id: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

#[napi(object)]
pub struct UsageMigrationBatchItem {
    pub message_id: String,
    pub thread_id: String,
    pub model: Option<String>,
    pub timestamp: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cached_input_tokens: Option<i32>,
    pub cache_write_input_tokens: Option<i32>,
    pub reasoning_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
}

#[napi(object)]
pub struct CustomThemeRecord {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub r#type: String,
    pub base_30: String,
    pub base_16: String,
    pub based_on: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct CreateCustomThemeInput {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub r#type: String,
    pub base_30: String,
    pub base_16: String,
    pub based_on: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateCustomThemeInput {
    pub id: String,
    pub display_name: Option<String>,
    pub r#type: Option<String>,
    pub base_30: Option<String>,
    pub base_16: Option<String>,
    pub based_on: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpsertSkillStateInput {
    pub id: String,
    pub path: String,
    pub enabled: Option<bool>,
    pub sort_order: Option<i32>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateSkillStateFieldsInput {
    pub id: String,
    pub set_path: Option<bool>,
    pub path: Option<String>,
    pub set_enabled: Option<bool>,
    pub enabled: Option<bool>,
    pub set_sort_order: Option<bool>,
    pub sort_order: Option<i32>,
    pub set_updated_at: Option<bool>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct ProviderRecord {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub api_key: String,
    pub models: String,
    pub base_url: Option<String>,
    pub api_version: Option<String>,
    pub enabled: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
    pub is_response_api: Option<bool>,
    pub acp_command: Option<String>,
    pub acp_args: Option<String>,
    pub acp_mcp_server_ids: Option<String>,
    pub acp_auth_method_id: Option<String>,
    pub acp_api_provider_id: Option<String>,
    pub acp_model_mapping: Option<String>,
    pub use_max_completion_tokens: Option<bool>,
    pub api_format: Option<String>,
    pub available_models: String,
    pub copilot_account_id: Option<String>,
}

#[napi(object)]
pub struct AppSettingsRecord {
    pub id: String,
    pub settings_data: String,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct ChannelMappingRecord {
    pub id: String,
    pub platform: String,
    pub external_chat_id: String,
    pub external_user_id: String,
    pub thread_id: String,
    pub is_active: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct CreateChannelMappingInput {
    pub id: String,
    pub platform: String,
    pub external_chat_id: String,
    pub external_user_id: String,
    pub thread_id: String,
    pub is_active: Option<bool>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct SetActiveChannelMappingInput {
    pub platform: String,
    pub external_chat_id: String,
    pub external_user_id: String,
    pub thread_id: String,
    pub new_mapping_id: Option<String>,
}

#[napi(object)]
pub struct DistinctChannelRecord {
    pub external_chat_id: String,
    pub label: String,
    pub last_active: String,
}

#[napi(object)]
pub struct PromptAppRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub prompt_template: String,
    pub placeholders: String,
    pub model: Option<String>,
    pub enabled: Option<bool>,
    pub shortcut: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub tools: Option<String>,
    pub reasoning_effort: Option<String>,
    pub expects_image_result: Option<bool>,
    pub is_incognito: Option<bool>,
    pub window_width: Option<i32>,
    pub window_height: Option<i32>,
    pub font_size: Option<i32>,
}

#[napi(object)]
pub struct PromptAppExecutionRecord {
    pub id: String,
    pub prompt_app_id: String,
    pub thread_id: Option<String>,
    pub input_values: String,
    pub generated_prompt: String,
    pub attachment_count: i32,
    pub created_at: String,
}

#[napi(object)]
pub struct CreatePromptAppInput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub prompt_template: String,
    pub placeholders: Option<String>,
    pub model: Option<String>,
    pub tools: Option<String>,
    pub reasoning_effort: Option<String>,
    pub expects_image_result: Option<bool>,
    pub is_incognito: Option<bool>,
    pub enabled: Option<bool>,
    pub shortcut: Option<String>,
    pub sort_order: Option<i32>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdatePromptAppFieldsInput {
    pub id: String,
    pub set_name: Option<bool>,
    pub name: Option<String>,
    pub set_description: Option<bool>,
    pub description: Option<String>,
    pub set_icon: Option<bool>,
    pub icon: Option<String>,
    pub set_prompt_template: Option<bool>,
    pub prompt_template: Option<String>,
    pub set_placeholders: Option<bool>,
    pub placeholders: Option<String>,
    pub set_model: Option<bool>,
    pub model: Option<String>,
    pub set_tools: Option<bool>,
    pub tools: Option<String>,
    pub set_reasoning_effort: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub set_expects_image_result: Option<bool>,
    pub expects_image_result: Option<bool>,
    pub set_is_incognito: Option<bool>,
    pub is_incognito: Option<bool>,
    pub set_enabled: Option<bool>,
    pub enabled: Option<bool>,
    pub set_shortcut: Option<bool>,
    pub shortcut: Option<String>,
    pub set_window_width: Option<bool>,
    pub window_width: Option<i32>,
    pub set_window_height: Option<bool>,
    pub window_height: Option<i32>,
    pub set_font_size: Option<bool>,
    pub font_size: Option<i32>,
    pub set_updated_at: Option<bool>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct CreatePromptAppExecutionInput {
    pub id: String,
    pub prompt_app_id: String,
    pub thread_id: Option<String>,
    pub input_values: String,
    pub generated_prompt: String,
    pub attachment_count: Option<i32>,
    pub created_at: Option<String>,
}

#[napi(object)]
pub struct ModelCapabilitiesCacheRecord {
    pub id: String,
    pub capabilities: String,
    pub fetched_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct BulkModelCapabilitiesEntryInput {
    pub model_id: String,
    pub capabilities: String,
}

#[napi(object)]
pub struct ProviderModelsCacheRecord {
    pub id: String,
    pub provider_id: String,
    pub models: String,
    pub fetched_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct ThreadDiffStatsCacheRecord {
    pub id: String,
    pub thread_updated_at: String,
    pub additions: i32,
    pub deletions: i32,
    pub files_changed: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
pub struct SetModelCapabilitiesCacheInput {
    pub id: String,
    pub capabilities: String,
    pub fetched_at: String,
}

#[napi(object)]
pub struct SetProviderModelsCacheInput {
    pub id: String,
    pub provider_id: String,
    pub models: String,
    pub fetched_at: String,
}

#[napi(object)]
pub struct SetThreadDiffStatsCacheInput {
    pub id: String,
    pub thread_updated_at: String,
    pub additions: i32,
    pub deletions: i32,
    pub files_changed: i32,
}

#[napi(object)]
pub struct CreateProviderInput {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub api_key: String,
    pub models: String,
    pub base_url: Option<String>,
    pub enabled: Option<bool>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[napi(object)]
pub struct UpdateProviderFieldsInput {
    pub id: String,
    pub set_name: Option<bool>,
    pub name: Option<String>,
    pub set_type: Option<bool>,
    pub r#type: Option<String>,
    pub set_api_key: Option<bool>,
    pub api_key: Option<String>,
    pub set_models: Option<bool>,
    pub models: Option<String>,
    pub set_base_url: Option<bool>,
    pub base_url: Option<String>,
    pub set_api_version: Option<bool>,
    pub api_version: Option<String>,
    pub set_enabled: Option<bool>,
    pub enabled: Option<bool>,
    pub set_is_response_api: Option<bool>,
    pub is_response_api: Option<bool>,
    pub set_acp_command: Option<bool>,
    pub acp_command: Option<String>,
    pub set_acp_args: Option<bool>,
    pub acp_args: Option<String>,
    pub set_acp_mcp_server_ids: Option<bool>,
    pub acp_mcp_server_ids: Option<String>,
    pub set_acp_auth_method_id: Option<bool>,
    pub acp_auth_method_id: Option<String>,
    pub set_acp_api_provider_id: Option<bool>,
    pub acp_api_provider_id: Option<String>,
    pub set_acp_model_mapping: Option<bool>,
    pub acp_model_mapping: Option<String>,
    pub set_use_max_completion_tokens: Option<bool>,
    pub use_max_completion_tokens: Option<bool>,
    pub set_api_format: Option<bool>,
    pub api_format: Option<String>,
    pub set_available_models: Option<bool>,
    pub available_models: Option<String>,
    pub set_copilot_account_id: Option<bool>,
    pub copilot_account_id: Option<String>,
    pub set_updated_at: Option<bool>,
    pub updated_at: Option<String>,
}
