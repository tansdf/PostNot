use tauri::State;

use crate::{
    app_state::AppState,
    domain::{
        collections::{
            CollectionItemSummary, CollectionSearchResult, CollectionSidebarState,
            CollectionSummary, CreateCollectionFolderInput, CreateCollectionInput,
            MoveCollectionItemInput, SavedRequestDetail, SavedRequestSummary,
            UpdateCollectionFolderInput,
        },
        exports::ExportResult,
        requests::SendRequestPayload,
    },
    error::AppResult,
    services::{collections_service, exports_service},
};

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> AppResult<Vec<CollectionSummary>> {
    collections_service::list_collections(state.db()).await
}

#[tauri::command]
pub async fn search_collection_entities(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> AppResult<Vec<CollectionSearchResult>> {
    collections_service::search_collection_entities(state.db(), &query, limit).await
}

#[tauri::command]
pub async fn get_collection_sidebar_state(
    state: State<'_, AppState>,
) -> AppResult<CollectionSidebarState> {
    crate::services::settings_service::get_collection_sidebar_state(state.db()).await
}

#[tauri::command]
pub async fn save_collection_sidebar_state(
    state: State<'_, AppState>,
    sidebar_state: CollectionSidebarState,
) -> AppResult<()> {
    crate::services::settings_service::save_collection_sidebar_state(state.db(), &sidebar_state)
        .await
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, AppState>,
    input: CreateCollectionInput,
) -> AppResult<CollectionSummary> {
    collections_service::create_collection(state.db(), &input).await
}

#[tauri::command]
pub async fn list_collection_items(
    state: State<'_, AppState>,
    collection_id: String,
) -> AppResult<Vec<CollectionItemSummary>> {
    collections_service::list_collection_items(state.db(), &collection_id).await
}

#[tauri::command]
pub async fn create_collection_folder(
    state: State<'_, AppState>,
    collection_id: String,
    input: CreateCollectionFolderInput,
) -> AppResult<CollectionItemSummary> {
    collections_service::create_collection_folder(state.db(), &collection_id, &input).await
}

#[tauri::command]
pub async fn update_collection_folder(
    state: State<'_, AppState>,
    item_id: String,
    input: UpdateCollectionFolderInput,
) -> AppResult<CollectionItemSummary> {
    collections_service::update_collection_folder(state.db(), &item_id, &input).await
}

#[tauri::command]
pub async fn move_collection_item(
    state: State<'_, AppState>,
    item_id: String,
    input: MoveCollectionItemInput,
) -> AppResult<CollectionItemSummary> {
    collections_service::move_collection_item(state.db(), &item_id, &input).await
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
    parent_id: Option<String>,
    request: SendRequestPayload,
) -> AppResult<SavedRequestSummary> {
    collections_service::save_request(state.db(), &collection_id, parent_id.as_deref(), &request)
        .await
}

#[tauri::command]
pub async fn update_saved_request(
    state: State<'_, AppState>,
    item_id: String,
    request: SendRequestPayload,
    expected_updated_at: Option<String>,
) -> AppResult<SavedRequestSummary> {
    collections_service::update_saved_request_with_revision(
        state.db(),
        &item_id,
        &request,
        expected_updated_at.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn get_saved_request(
    state: State<'_, AppState>,
    item_id: String,
) -> AppResult<SavedRequestDetail> {
    collections_service::get_saved_request(state.db(), &item_id).await
}

#[tauri::command]
pub async fn delete_collection_item(state: State<'_, AppState>, item_id: String) -> AppResult<()> {
    collections_service::delete_collection_item(state.db(), &item_id).await
}

#[tauri::command]
pub async fn delete_saved_request(state: State<'_, AppState>, item_id: String) -> AppResult<()> {
    collections_service::delete_saved_request(state.db(), &item_id).await
}

#[tauri::command]
pub async fn export_collection(
    state: State<'_, AppState>,
    collection_id: String,
) -> AppResult<Option<ExportResult>> {
    exports_service::export_collection(state.db(), &collection_id).await
}
