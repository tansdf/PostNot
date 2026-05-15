use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValueRow {
    pub id: String,
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestBody {
    pub mode: String,
    pub raw: String,
    pub form: Vec<KeyValueRow>,
    pub files: Vec<FileRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAuth {
    #[serde(rename = "type")]
    pub auth_type: String,
    #[serde(default)]
    pub basic_username: String,
    #[serde(default)]
    pub basic_password: String,
    #[serde(default)]
    pub bearer_token: String,
    #[serde(default)]
    pub api_key_name: String,
    #[serde(default)]
    pub api_key_value: String,
    #[serde(default = "default_api_key_in")]
    pub api_key_in: String,
    #[serde(default)]
    pub oauth2_access_token: String,
    #[serde(default)]
    pub oauth2_token_url: String,
    #[serde(default)]
    pub oauth2_client_id: String,
    #[serde(default)]
    pub oauth2_client_secret: String,
    #[serde(default)]
    pub oauth2_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRequestPayload {
    pub name: String,
    pub method: String,
    pub url: String,
    pub query_params: Vec<KeyValueRow>,
    pub headers: Vec<KeyValueRow>,
    pub body: RequestBody,
    pub auth: RequestAuth,
    #[serde(default)]
    pub pre_request_script: String,
    #[serde(default)]
    pub test_script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponsePayload {
    pub status_code: Option<u16>,
    pub status_text: String,
    pub duration_ms: u128,
    pub size_bytes: usize,
    pub headers: Vec<KeyValueRow>,
    pub body_text: String,
    pub error_text: String,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRequestResult {
    pub response: ResponsePayload,
    pub history_persistence_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPreviewSettings {
    pub request_timeout_ms: u64,
    pub follow_redirects: bool,
    pub validate_tls: bool,
    pub active_environment_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPreview {
    pub name: String,
    pub method: String,
    pub final_url: String,
    pub query_params: Vec<KeyValueRow>,
    pub headers: Vec<KeyValueRow>,
    pub body: RequestBody,
    pub auth: RequestAuth,
    pub settings: RequestPreviewSettings,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_api_key_in() -> String {
    "header".to_string()
}
