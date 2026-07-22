use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSetupInfo {
    pub executable_path: String,
    pub arguments: Vec<String>,
    pub generic_config_json: String,
    pub codex_config_toml: String,
    pub claude_config_json: String,
    pub cursor_config_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub ui_scale: f64,
    pub request_timeout_ms: u64,
    pub follow_redirects: bool,
    pub validate_tls: bool,
    pub history_limit: u32,
    pub is_history_collapsed: bool,
    pub environment_autosave: bool,
    pub notification_timeout_ms: u64,
    pub last_update_checked_at: Option<String>,
}
