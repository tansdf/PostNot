use tauri::State;

use crate::{
    app_state::AppState,
    domain::imports::{
        CurlImportInput, ImportRequestInput, ImportResult, ImportedRequestDraft,
        OpenApiDraftImportInput,
    },
    error::AppResult,
    services::imports_service,
};

#[tauri::command]
pub async fn import_requests(
    state: State<'_, AppState>,
    input: ImportRequestInput,
) -> AppResult<ImportResult> {
    imports_service::import_requests(state.db(), &input).await
}

#[tauri::command]
pub fn import_curl_request_to_draft(input: CurlImportInput) -> AppResult<ImportedRequestDraft> {
    imports_service::import_curl_to_draft(&input.source)
}

#[tauri::command]
pub fn import_openapi_request_to_draft(
    input: OpenApiDraftImportInput,
) -> AppResult<ImportedRequestDraft> {
    imports_service::import_openapi_to_draft(&input.source)
}
