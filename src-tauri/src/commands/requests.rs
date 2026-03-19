use tauri::State;

use crate::{
    app_state::AppState,
    domain::requests::{ResponsePayload, SendRequestPayload},
    error::AppResult,
    services::{history_service, http_client, settings_service},
};

#[tauri::command]
pub async fn send_request(
    state: State<'_, AppState>,
    payload: SendRequestPayload,
) -> AppResult<ResponsePayload> {
    let settings = settings_service::get_settings(state.db()).await?;

    match http_client::send_request(&payload, &settings).await {
        Ok(response) => {
            history_service::record_success(state.db(), &payload, &response).await?;
            Ok(response)
        }
        Err(error) => {
            history_service::record_failure(state.db(), &payload, &error.to_string()).await?;
            Err(error)
        }
    }
}
