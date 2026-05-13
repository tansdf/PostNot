use serde::{Deserialize, Serialize};

use crate::domain::collections::SavedRequestDetail;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_delay_ms: i64,
    pub stop_on_failure: bool,
    pub fail_on_http_error: bool,
    pub step_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_delay_ms: i64,
    pub stop_on_failure: bool,
    pub fail_on_http_error: bool,
    pub steps: Vec<PlaybookStep>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookInput {
    pub name: String,
    pub description: String,
    pub default_delay_ms: i64,
    pub stop_on_failure: bool,
    pub fail_on_http_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPlaybookStepInput {
    pub saved_request_id: String,
    #[serde(default)]
    pub name_override: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub delay_after_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlaybookStepInput {
    #[serde(default)]
    pub name_override: String,
    #[serde(default)]
    pub notes: String,
    pub enabled: bool,
    pub delay_after_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderPlaybookStepsInput {
    pub step_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookStep {
    pub id: String,
    pub playbook_id: String,
    pub saved_request_id: Option<String>,
    pub saved_request_name: String,
    pub collection_name: Option<String>,
    pub method: Option<String>,
    pub url: Option<String>,
    pub name_override: String,
    pub notes: String,
    pub enabled: bool,
    pub sort_order: i64,
    pub delay_after_ms: Option<i64>,
    pub missing_saved_request: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookExecutionContext {
    pub step_id: String,
    pub saved_request: SavedRequestDetail,
    pub inherited_scripts: PlaybookInheritedScripts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookInheritedScripts {
    pub pre_request_script: String,
    pub test_script: String,
    pub folder_scripts: Vec<PlaybookFolderScripts>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookFolderScripts {
    pub name: String,
    pub pre_request_script: String,
    pub test_script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookRunSummary {
    pub id: String,
    pub playbook_id: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub total_steps: i64,
    pub passed_steps: i64,
    pub failed_steps: i64,
    pub skipped_steps: i64,
    pub total_duration_ms: i64,
    pub stopped_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookRunDetail {
    pub id: String,
    pub playbook_id: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub total_steps: i64,
    pub passed_steps: i64,
    pub failed_steps: i64,
    pub skipped_steps: i64,
    pub total_duration_ms: i64,
    pub stopped_reason: String,
    pub steps: Vec<PlaybookRunStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlaybookRunInput {
    pub playbook_id: String,
    pub total_steps: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishPlaybookRunInput {
    pub status: String,
    pub stopped_reason: String,
    pub total_duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordPlaybookRunStepInput {
    pub step_id: Option<String>,
    pub saved_request_id: Option<String>,
    pub saved_request_name: String,
    pub method: String,
    pub url: String,
    pub status: String,
    pub status_code: Option<i64>,
    pub duration_ms: i64,
    pub response_size_bytes: i64,
    pub test_passed_count: i64,
    pub test_failed_count: i64,
    pub test_error_text: String,
    pub error_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookRunStep {
    pub id: String,
    pub run_id: String,
    pub step_id: Option<String>,
    pub saved_request_id: Option<String>,
    pub saved_request_name: String,
    pub method: String,
    pub url: String,
    pub status: String,
    pub status_code: Option<i64>,
    pub duration_ms: i64,
    pub response_size_bytes: i64,
    pub test_passed_count: i64,
    pub test_failed_count: i64,
    pub test_error_text: String,
    pub error_text: String,
    pub executed_at: String,
}

fn default_true() -> bool {
    true
}
