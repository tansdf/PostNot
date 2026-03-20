use tauri::State;

use crate::{
    app_state::AppState,
    domain::history::{HistoryEntryDetail, HistoryEntrySummary},
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
    history_service::get_history_entry(state.db(), &id).await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> AppResult<()> {
    history_service::clear_history(state.db()).await
}
