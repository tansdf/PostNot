use tauri::State;

use crate::{
    app_state::AppState,
    domain::{
        collections::{CollectionSummary, CreateCollectionInput, SavedRequestDetail, SavedRequestSummary},
        requests::SendRequestPayload,
    },
    error::AppResult,
    services::collections_service,
};

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> AppResult<Vec<CollectionSummary>> {
    collections_service::list_collections(state.db()).await
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, AppState>,
    input: CreateCollectionInput,
) -> AppResult<CollectionSummary> {
    collections_service::create_collection(state.db(), &input).await
}

#[tauri::command]
pub async fn update_collection(
    state: State<'_, AppState>,
    collection_id: String,
    input: CreateCollectionInput,
) -> AppResult<CollectionSummary> {
    collections_service::update_collection(state.db(), &collection_id, &input).await
}

#[tauri::command]
pub async fn delete_collection(state: State<'_, AppState>, collection_id: String) -> AppResult<()> {
    collections_service::delete_collection(state.db(), &collection_id).await
}

#[tauri::command]
pub async fn list_saved_requests(
    state: State<'_, AppState>,
    collection_id: String,
) -> AppResult<Vec<SavedRequestSummary>> {
    collections_service::list_saved_requests(state.db(), &collection_id).await
}

#[tauri::command]
pub async fn save_request_to_collection(
    state: State<'_, AppState>,
    collection_id: String,
    request: SendRequestPayload,
) -> AppResult<SavedRequestSummary> {
    collections_service::save_request(state.db(), &collection_id, &request).await
}

#[tauri::command]
pub async fn update_saved_request(
    state: State<'_, AppState>,
    item_id: String,
    request: SendRequestPayload,
) -> AppResult<SavedRequestSummary> {
    collections_service::update_saved_request(state.db(), &item_id, &request).await
}

#[tauri::command]
pub async fn get_saved_request(
    state: State<'_, AppState>,
    item_id: String,
) -> AppResult<SavedRequestDetail> {
    collections_service::get_saved_request(state.db(), &item_id).await
}

#[tauri::command]
pub async fn delete_saved_request(state: State<'_, AppState>, item_id: String) -> AppResult<()> {
    collections_service::delete_saved_request(state.db(), &item_id).await
}
