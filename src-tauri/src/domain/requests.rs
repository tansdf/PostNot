use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::services::response_body_service::{ResponsePresentation, StoredResponseBody};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeyValueRow {
    #[serde(default)]
    pub id: String,
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestBody {
    #[serde(default = "default_body_mode")]
    pub mode: String,
    #[serde(default)]
    pub raw: String,
    #[serde(default)]
    pub form: Vec<KeyValueRow>,
    #[serde(default)]
    pub files: Vec<FileRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestAuth {
    #[serde(rename = "type")]
    #[serde(default = "default_auth_type")]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendRequestPayload {
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub query_params: Vec<KeyValueRow>,
    #[serde(default)]
    pub headers: Vec<KeyValueRow>,
    #[serde(default)]
    pub body: RequestBody,
    #[serde(default)]
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
    pub size_bytes: u64,
    pub headers: Vec<KeyValueRow>,
    pub body: ResponseBody,
    pub error_text: String,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum ResponseBody {
    Inline {
        text: String,
        #[serde(rename = "sizeBytes")]
        size_bytes: u64,
        #[serde(rename = "contentType")]
        content_type: Option<String>,
        charset: Option<String>,
        presentation: ResponsePresentation,
    },
    File {
        #[serde(rename = "handleId")]
        handle_id: String,
        #[serde(rename = "previewText")]
        preview_text: String,
        #[serde(rename = "sizeBytes")]
        size_bytes: u64,
        #[serde(rename = "contentType")]
        content_type: Option<String>,
        charset: Option<String>,
        presentation: ResponsePresentation,
    },
}

impl ResponseBody {
    pub fn inline_text(&self) -> Option<&str> {
        match self {
            Self::Inline { text, .. } => Some(text),
            Self::File { .. } => None,
        }
    }

    pub fn handle_id(&self) -> Option<&str> {
        match self {
            Self::File { handle_id, .. } => Some(handle_id),
            Self::Inline { .. } => None,
        }
    }

    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::Inline { size_bytes, .. } | Self::File { size_bytes, .. } => *size_bytes,
        }
    }
}

impl From<StoredResponseBody> for ResponseBody {
    fn from(value: StoredResponseBody) -> Self {
        Self::File {
            handle_id: value.handle_id,
            preview_text: value.preview_text,
            size_bytes: value.size_bytes,
            content_type: value.content_type,
            charset: value.charset,
            presentation: value.presentation,
        }
    }
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

fn default_body_mode() -> String {
    "none".to_string()
}

fn default_auth_type() -> String {
    "none".to_string()
}

impl Default for RequestBody {
    fn default() -> Self {
        Self {
            mode: default_body_mode(),
            raw: String::new(),
            form: Vec::new(),
            files: Vec::new(),
        }
    }
}

impl Default for RequestAuth {
    fn default() -> Self {
        Self {
            auth_type: default_auth_type(),
            basic_username: String::new(),
            basic_password: String::new(),
            bearer_token: String::new(),
            api_key_name: String::new(),
            api_key_value: String::new(),
            api_key_in: default_api_key_in(),
            oauth2_access_token: String::new(),
            oauth2_token_url: String::new(),
            oauth2_client_id: String::new(),
            oauth2_client_secret: String::new(),
            oauth2_scope: String::new(),
        }
    }
}
