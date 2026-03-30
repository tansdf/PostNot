use tauri::{AppHandle, State};

use crate::{
    app_state::AppState,
    domain::requests::{ResponsePayload, SendRequestPayload},
    error::AppResult,
    services::{environments_service, history_service, http_client, settings_service},
};

#[tauri::command]
pub async fn send_request(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: SendRequestPayload,
) -> AppResult<ResponsePayload> {
    let (request_id, cancel_rx) = state.start_request()?;
    let settings = settings_service::get_settings(state.db()).await?;
    let active_environment = environments_service::get_active_environment(state.db()).await?;
    let resolved_payload =
        environments_service::resolve_request(&payload, active_environment.as_ref());

    let request_result = http_client::send_request(&resolved_payload, &settings, cancel_rx).await;

    let result = match request_result {
        Ok(response) => {
            match history_service::record_success(state.db(), &resolved_payload, &response, &app)
                .await
            {
                Ok(()) => Ok(response),
                Err(error) => Err(error),
            }
        }
        Err(error) => match error.is_cancelled() {
            true => Err(error),
            false => {
                let history_result = history_service::record_failure(
                    state.db(),
                    &resolved_payload,
                    &error.to_string(),
                )
                .await;

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

#[tauri::command]
pub async fn pick_multipart_files() -> AppResult<Vec<String>> {
    let files = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Select files for multipart upload")
            .pick_files()
    })
    .await
    .map_err(|error| crate::error::AppError::Message(error.to_string()))?;

    Ok(files
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect())
}
