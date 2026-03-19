use tauri::State;

use crate::{
    app_state::AppState,
    domain::history::HistoryEntrySummary,
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
