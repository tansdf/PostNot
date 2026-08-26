use tauri::State;

use crate::{
    app_state::AppState,
    domain::history::{HistoryEntryDetail, HistoryEntrySummary},
    domain::storage::HistoryRetentionResult,
    error::AppResult,
    services::history_service,
};

#[tauri::command]
pub async fn list_history(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> AppResult<Vec<HistoryEntrySummary>> {
    history_service::list_history(state.db(), limit).await
}

#[tauri::command]
pub async fn get_history_entry(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<HistoryEntryDetail> {
    history_service::get_history_entry(state.db(), state.response_bodies(), &id).await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> AppResult<()> {
    history_service::clear_history(state.db(), state.response_bodies()).await
}

#[tauri::command]
pub async fn apply_history_retention(
    state: State<'_, AppState>,
) -> AppResult<HistoryRetentionResult> {
    history_service::apply_history_retention(state.db(), state.response_bodies()).await
}
