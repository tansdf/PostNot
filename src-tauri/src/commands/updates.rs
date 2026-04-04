use tauri::{AppHandle, State};

use crate::{app_state::AppState, domain::updates::UpdateCheckResult, error::AppResult, services::updates_service};

#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<UpdateCheckResult> {
    updates_service::check_for_updates(&app, &state).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    updates_service::install_update(&app, &state).await
}
