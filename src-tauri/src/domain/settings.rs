use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub request_timeout_ms: u64,
    pub follow_redirects: bool,
    pub validate_tls: bool,
    pub history_limit: u32,
}
