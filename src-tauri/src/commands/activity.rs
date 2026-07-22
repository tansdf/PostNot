use tauri::State;

use crate::{
    app_state::AppState, domain::activity::AgentActivityPage, error::AppResult,
    services::activity_service,
};

#[tauri::command]
pub async fn list_agent_activity(
    state: State<'_, AppState>,
    after_id: Option<i64>,
    limit: Option<usize>,
) -> AppResult<AgentActivityPage> {
    activity_service::list(state.db(), after_id, limit).await
}
