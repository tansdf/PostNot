use tauri::State;

use crate::{
    app_state::AppState,
    domain::{settings::AppSettings, workspace::RequestWorkspaceState},
    error::AppResult,
    services::settings_service,
};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    settings_service::get_settings(state.db()).await
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
