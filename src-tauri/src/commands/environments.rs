use tauri::State;

use crate::{
    app_state::AppState,
    domain::environments::{
        EnvironmentDetail, EnvironmentInput, EnvironmentSummary, ImportEnvironmentInput, ImportEnvironmentResult,
    },
    error::AppResult,
    services::{environments_service, imports_service},
};

#[tauri::command]
pub async fn list_environments(state: State<'_, AppState>) -> AppResult<Vec<EnvironmentSummary>> {
    environments_service::list_environments(state.db()).await
}

#[tauri::command]
pub async fn create_environment(state: State<'_, AppState>) -> AppResult<EnvironmentDetail> {
    environments_service::create_environment(state.db()).await
}

#[tauri::command]
pub async fn get_environment(state: State<'_, AppState>, environment_id: String) -> AppResult<EnvironmentDetail> {
    environments_service::get_environment(state.db(), &environment_id).await
}

#[tauri::command]
pub async fn update_environment(
    state: State<'_, AppState>,
    environment_id: String,
    input: EnvironmentInput,
) -> AppResult<EnvironmentDetail> {
    environments_service::update_environment(state.db(), &environment_id, &input).await
}

#[tauri::command]
pub async fn delete_environment(state: State<'_, AppState>, environment_id: String) -> AppResult<()> {
    environments_service::delete_environment(state.db(), &environment_id).await
}

#[tauri::command]
pub async fn set_active_environment(
    state: State<'_, AppState>,
    environment_id: Option<String>,
) -> AppResult<()> {
    environments_service::set_active_environment(state.db(), environment_id.as_deref()).await
}

#[tauri::command]
pub async fn import_postman_environment(
    state: State<'_, AppState>,
    input: ImportEnvironmentInput,
) -> AppResult<ImportEnvironmentResult> {
    imports_service::import_postman_environment(state.db(), &input).await
}
