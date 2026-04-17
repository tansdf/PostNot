use serde::{Deserialize, Serialize};

use crate::domain::requests::{ResponsePayload, SendRequestPayload};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTestResult {
    pub id: String,
    pub name: String,
    pub status: String,
    pub error_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestScriptExecution {
    #[serde(default)]
    pub pre_request_error_text: String,
    #[serde(default)]
    pub test_script_error_text: String,
    #[serde(default)]
    pub tests: Vec<ScriptTestResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestWorkspaceTab {
    pub id: String,
    pub source: String,
    pub saved_request_id: Option<String>,
    pub collection_id: Option<String>,
    pub parent_id: Option<String>,
    pub request: SendRequestPayload,
    pub baseline_request: Option<SendRequestPayload>,
    pub response: Option<ResponsePayload>,
    pub script_execution: RequestScriptExecution,
    #[serde(default)]
    pub error_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestWorkspaceState {
    pub tabs: Vec<RequestWorkspaceTab>,
    pub active_tab_id: String,
}
