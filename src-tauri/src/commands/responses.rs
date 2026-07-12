use std::path::PathBuf;

use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::{
    app_state::AppState,
    domain::requests::ResponseBody,
    error::AppResult,
    services::response_body_service::{
        ResponseBodyJobProgressSink, ResponseBodyWindow, ResponseSearchProgressSink,
        ResponseSearchResult,
    },
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResponseBodyWindowInput {
    pub handle_id: String,
    pub start_row: u64,
    pub row_count: u64,
    pub max_bytes: usize,
    #[serde(default)]
    pub representation: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponseBodyInput {
    pub handle_id: String,
    pub query: String,
    pub case_sensitive: bool,
    pub search_id: String,
    #[serde(default)]
    pub representation: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindResponseMatchInput {
    pub handle_id: String,
    pub query: String,
    pub case_sensitive: bool,
    pub from_offset: u64,
    pub direction: String,
    pub wrap: bool,
    #[serde(default)]
    pub representation: String,
}

#[tauri::command]
pub async fn read_response_body_window(
    state: State<'_, AppState>,
    input: ReadResponseBodyWindowInput,
) -> AppResult<ResponseBodyWindow> {
    if input.representation == "hex" {
        state
            .response_bodies()
            .read_hex_window(&input.handle_id, input.start_row, input.row_count.min(500))
            .await
    } else {
        state
            .response_bodies()
            .read_window(
                &input.handle_id,
                input.start_row,
                input.row_count.min(500),
                input.max_bytes.min(4 * 1024 * 1024),
            )
            .await
    }
}

#[tauri::command]
pub async fn search_response_body(
    app: AppHandle,
    state: State<'_, AppState>,
    input: SearchResponseBodyInput,
) -> AppResult<ResponseSearchResult> {
    let hex = input.representation == "hex";
    let progress: ResponseSearchProgressSink = Arc::new(move |mut event| {
        if hex {
            if let Some(first) = event.first_match.as_mut() {
                first.row_index = first.byte_offset / 16;
            }
        }
        let _ = app.emit("response-search-progress", event);
    });
    let mut result = state
        .response_bodies()
        .search_with_progress(
            &input.search_id,
            &input.handle_id,
            &input.query,
            input.case_sensitive,
            progress,
        )
        .await?;
    if hex {
        for item in &mut result.matches {
            item.row_index = item.byte_offset / 16;
        }
    }
    Ok(result)
}

#[tauri::command]
pub fn cancel_response_search(state: State<'_, AppState>, search_id: String) {
    state.response_bodies().cancel_search(&search_id);
}

#[tauri::command]
pub async fn find_response_match(
    state: State<'_, AppState>,
    input: FindResponseMatchInput,
) -> AppResult<Option<crate::services::response_body_service::ResponseSearchMatch>> {
    let mut found = state
        .response_bodies()
        .find_directional_match(
            &input.handle_id,
            &input.query,
            input.case_sensitive,
            input.from_offset,
            input.direction != "previous",
            input.wrap,
        )
        .await?;
    if input.representation == "hex" {
        if let Some(item) = found.as_mut() {
            item.row_index = item.byte_offset / 16;
        }
    }
    Ok(found)
}

#[tauri::command]
pub async fn read_response_body_text(
    state: State<'_, AppState>,
    handle_id: String,
) -> AppResult<String> {
    state.response_bodies().read_all_text(&handle_id).await
}

#[tauri::command]
pub fn retain_response_body(state: State<'_, AppState>, handle_id: String) -> AppResult<()> {
    state.response_bodies().retain(&handle_id)
}

#[tauri::command]
pub fn release_response_body(state: State<'_, AppState>, handle_id: String) -> AppResult<()> {
    state.response_bodies().release(&handle_id)
}

#[tauri::command]
pub fn get_response_body_path(state: State<'_, AppState>, handle_id: String) -> AppResult<String> {
    Ok(state
        .response_bodies()
        .path_for(&handle_id)?
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub async fn save_response_body(
    state: State<'_, AppState>,
    handle_id: String,
    suggested_name: Option<String>,
) -> AppResult<Option<String>> {
    let suggested_name = suggested_name.unwrap_or_else(|| "response-body.txt".to_string());
    let destination = tauri::async_runtime::spawn_blocking(move || {
        rfd::FileDialog::new()
            .set_title("Save response body")
            .set_file_name(suggested_name)
            .save_file()
    })
    .await?;
    let Some(destination) = destination else {
        return Ok(None);
    };
    state
        .response_bodies()
        .copy_to(&handle_id, &PathBuf::from(&destination))
        .await?;
    Ok(Some(destination.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn format_response_body(
    app: AppHandle,
    state: State<'_, AppState>,
    handle_id: String,
    job_id: String,
) -> AppResult<ResponseBody> {
    let progress: ResponseBodyJobProgressSink = Arc::new(move |event| {
        let _ = app.emit("response-body-job-progress", event);
    });
    Ok(state
        .response_bodies()
        .format_json_with_id(&job_id, &handle_id, Some(progress))
        .await?
        .into())
}

#[tauri::command]
pub fn cancel_response_body_job(state: State<'_, AppState>, job_id: String) {
    state.response_bodies().cancel_job(&job_id);
}
