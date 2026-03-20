use tauri::{AppHandle, State};

use crate::{
    app_state::AppState,
    domain::requests::{ResponsePayload, SendRequestPayload},
    error::AppResult,
    services::{history_service, http_client, settings_service},
};

#[tauri::command]
pub async fn send_request(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: SendRequestPayload,
) -> AppResult<ResponsePayload> {
    let (request_id, cancel_rx) = state.start_request()?;
    let settings = settings_service::get_settings(state.db()).await?;

    let request_result = http_client::send_request(&payload, &settings, cancel_rx).await;

    let result = match request_result {
        Ok(response) => match history_service::record_success(state.db(), &payload, &response, &app).await {
            Ok(()) => Ok(response),
            Err(error) => Err(error),
        },
        Err(error) => match error.is_cancelled() {
            true => Err(error),
            false => {
                let history_result =
                    history_service::record_failure(state.db(), &payload, &error.to_string()).await;

                match history_result {
                    Ok(()) => Err(error),
                    Err(history_error) => Err(history_error),
                }
            }
        },
    };

    state.finish_request(&request_id);
    result
}

#[tauri::command]
pub fn cancel_active_request(state: State<'_, AppState>) -> AppResult<bool> {
    state.cancel_active_request()
}
