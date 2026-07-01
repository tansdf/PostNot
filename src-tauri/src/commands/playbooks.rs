use tauri::State;

use crate::{
    app_state::AppState,
    domain::playbooks::{
        AddPlaybookStepInput, CreatePlaybookRunInput, FinishPlaybookRunInput, PlaybookDetail,
        PlaybookExecutionContext, PlaybookInput, PlaybookRunDetail, PlaybookRunStep,
        PlaybookRunSummary, PlaybookStep, PlaybookSummary, RecordPlaybookRunStepInput,
        ReorderPlaybookStepsInput, UpdatePlaybookStepInput,
    },
    error::AppResult,
    services::playbooks_service,
};

#[tauri::command]
pub async fn list_playbooks(state: State<'_, AppState>) -> AppResult<Vec<PlaybookSummary>> {
    playbooks_service::list_playbooks(state.db()).await
}

#[tauri::command]
pub async fn create_playbook(
    state: State<'_, AppState>,
    input: PlaybookInput,
) -> AppResult<PlaybookDetail> {
    playbooks_service::create_playbook(state.db(), &input).await
}

#[tauri::command]
pub async fn get_playbook(
    state: State<'_, AppState>,
    playbook_id: String,
) -> AppResult<PlaybookDetail> {
    playbooks_service::get_playbook(state.db(), &playbook_id).await
}

#[tauri::command]
pub async fn update_playbook(
    state: State<'_, AppState>,
    playbook_id: String,
    input: PlaybookInput,
) -> AppResult<PlaybookDetail> {
    playbooks_service::update_playbook(state.db(), &playbook_id, &input).await
}

#[tauri::command]
pub async fn duplicate_playbook(
    state: State<'_, AppState>,
    playbook_id: String,
) -> AppResult<PlaybookDetail> {
    playbooks_service::duplicate_playbook(state.db(), &playbook_id).await
}

#[tauri::command]
pub async fn delete_playbook(state: State<'_, AppState>, playbook_id: String) -> AppResult<()> {
    playbooks_service::delete_playbook(state.db(), &playbook_id).await
}

#[tauri::command]
pub async fn add_playbook_step(
    state: State<'_, AppState>,
    playbook_id: String,
    input: AddPlaybookStepInput,
) -> AppResult<PlaybookStep> {
    playbooks_service::add_playbook_step(state.db(), &playbook_id, &input).await
}

#[tauri::command]
pub async fn update_playbook_step(
    state: State<'_, AppState>,
    step_id: String,
    input: UpdatePlaybookStepInput,
) -> AppResult<PlaybookStep> {
    playbooks_service::update_playbook_step(state.db(), &step_id, &input).await
}

#[tauri::command]
pub async fn reorder_playbook_steps(
    state: State<'_, AppState>,
    playbook_id: String,
    input: ReorderPlaybookStepsInput,
) -> AppResult<Vec<PlaybookStep>> {
    playbooks_service::reorder_playbook_steps(state.db(), &playbook_id, &input).await
}

#[tauri::command]
pub async fn delete_playbook_step(state: State<'_, AppState>, step_id: String) -> AppResult<()> {
    playbooks_service::delete_playbook_step(state.db(), &step_id).await
}

#[tauri::command]
pub async fn get_playbook_execution_context(
    state: State<'_, AppState>,
    step_id: String,
) -> AppResult<PlaybookExecutionContext> {
    playbooks_service::get_playbook_execution_context(state.db(), &step_id).await
}

#[tauri::command]
pub async fn create_playbook_run(
    state: State<'_, AppState>,
    input: CreatePlaybookRunInput,
) -> AppResult<PlaybookRunSummary> {
    playbooks_service::create_playbook_run(state.db(), &input).await
}

#[tauri::command]
pub async fn finish_playbook_run(
    state: State<'_, AppState>,
    run_id: String,
    input: FinishPlaybookRunInput,
) -> AppResult<PlaybookRunSummary> {
    playbooks_service::finish_playbook_run(state.db(), &run_id, &input).await
}

#[tauri::command]
pub async fn record_playbook_run_step(
    state: State<'_, AppState>,
    run_id: String,
    input: RecordPlaybookRunStepInput,
) -> AppResult<PlaybookRunStep> {
    playbooks_service::record_playbook_run_step(state.db(), &run_id, &input).await
}

#[tauri::command]
pub async fn list_playbook_runs(
    state: State<'_, AppState>,
    playbook_id: String,
    limit: Option<i64>,
) -> AppResult<Vec<PlaybookRunSummary>> {
    playbooks_service::list_playbook_runs(state.db(), &playbook_id, limit).await
}

#[tauri::command]
pub async fn get_playbook_run(
    state: State<'_, AppState>,
    run_id: String,
) -> AppResult<PlaybookRunDetail> {
    playbooks_service::get_playbook_run(state.db(), &run_id).await
}
