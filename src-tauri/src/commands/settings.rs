use tauri::State;

use crate::{
    app_state::AppState,
    domain::{
        settings::{AppSettings, McpSetupInfo},
        workspace::RequestWorkspaceState,
    },
    error::AppResult,
    services::settings_service,
};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    settings_service::get_settings(state.db()).await
}

#[tauri::command]
pub fn get_mcp_setup_info() -> AppResult<McpSetupInfo> {
    let executable = std::env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .ok_or_else(|| {
            crate::error::AppError::Message(
                "Could not determine the PostNot executable path.".to_string(),
            )
        })?;
    let executable_path = executable.to_string_lossy().to_string();
    let command_json = serde_json::to_string(&executable_path)?;
    let generic = serde_json::to_string_pretty(&serde_json::json!({
        "command": executable_path,
        "args": ["--mcp"]
    }))?;
    let desktop = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": { "postnot": { "command": executable_path, "args": ["--mcp"] } }
    }))?;

    Ok(McpSetupInfo {
        executable_path,
        arguments: vec!["--mcp".to_string()],
        generic_config_json: generic,
        codex_config_toml: format!(
            "[mcp_servers.postnot]\ncommand = {command_json}\nargs = [\"--mcp\"]"
        ),
        claude_config_json: desktop.clone(),
        cursor_config_json: desktop,
    })
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> AppResult<AppSettings> {
    settings_service::save_settings(state.db(), &settings).await?;
    settings_service::get_settings(state.db()).await
}

#[tauri::command]
pub async fn get_request_workspace_state(
    app_state: State<'_, AppState>,
) -> AppResult<Option<RequestWorkspaceState>> {
    settings_service::get_request_workspace_state(app_state.db()).await
}

#[tauri::command]
pub async fn save_request_workspace_state(
    app_state: State<'_, AppState>,
    state: RequestWorkspaceState,
) -> AppResult<()> {
    settings_service::save_request_workspace_state(app_state.db(), &state).await
}
