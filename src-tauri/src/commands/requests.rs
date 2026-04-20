use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{
    app_state::AppState,
    domain::requests::{SendRequestPayload, SendRequestResult},
    error::AppResult,
    services::{environments_service, history_service, http_client, settings_service},
};

const HISTORY_PERSISTENCE_EVENT: &str = "history-persistence-error";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryPersistenceEvent {
    message: String,
}

fn emit_history_persistence_error(app: &AppHandle, message: String) {
    let _ = app.emit(
        HISTORY_PERSISTENCE_EVENT,
        HistoryPersistenceEvent { message },
    );
}

#[tauri::command]
pub async fn send_request(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: SendRequestPayload,
    persist_history: Option<bool>,
) -> AppResult<SendRequestResult> {
    let should_persist_history = persist_history.unwrap_or(true);
    let (request_id, cancel_rx) = state.start_request()?;
    let settings = settings_service::get_settings(state.db()).await?;
    let active_environment =
        environments_service::get_active_environment(state.db(), state.secret_store()).await?;
    let resolved_request =
        environments_service::resolve_request(&payload, active_environment.as_ref());
    let history_payload = environments_service::redact_secret_history_payload(
        &payload,
        &resolved_request.payload,
        &resolved_request.secret_usage,
    );

    let request_result =
        http_client::send_request(&resolved_request.payload, &settings, cancel_rx).await;

    let result = match request_result {
        Ok(response) => {
            let history_persistence_error = if should_persist_history {
                match history_service::record_success(state.db(), &history_payload, &response, &app)
                    .await
                {
                    Ok(()) => None,
                    Err(error) => Some(error.to_string()),
                }
            } else {
                None
            };
            Ok(SendRequestResult {
                response,
                history_persistence_error,
            })
        }
        Err(error) => match error.is_cancelled() {
            true => Err(error),
            false => {
                if should_persist_history {
                    if let Err(history_error) = history_service::record_failure(
                        state.db(),
                        &history_payload,
                        &error.to_string(),
                    )
                    .await
                    {
                        emit_history_persistence_error(&app, history_error.to_string());
                    }
                }
                Err(error)
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
    .await?;

    Ok(files
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect())
}
