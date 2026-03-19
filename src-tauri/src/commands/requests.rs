use crate::domain::requests::{ResponsePayload, SendRequestPayload};
use crate::error::AppResult;
use crate::services::http_client;

#[tauri::command]
pub async fn send_request(payload: SendRequestPayload) -> AppResult<ResponsePayload> {
    http_client::send_request(&payload).await
}
