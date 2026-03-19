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
    pub basic_username: String,
    pub basic_password: String,
    pub bearer_token: String,
    pub api_key_name: String,
    pub api_key_value: String,
    pub api_key_in: String,
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
