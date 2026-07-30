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
    #[serde(default = "default_realtime_connect_timeout_ms")]
    pub realtime_connect_timeout_ms: u64,
    #[serde(default = "default_realtime_max_concurrent_sessions")]
    pub realtime_max_concurrent_sessions: u32,
    #[serde(default = "default_realtime_max_message_bytes")]
    pub realtime_max_message_bytes: u64,
    #[serde(default = "default_realtime_transcript_max_entries")]
    pub realtime_transcript_max_entries: u32,
    #[serde(default = "default_realtime_transcript_max_bytes")]
    pub realtime_transcript_max_bytes: u64,
    pub last_update_checked_at: Option<String>,
}

pub fn default_realtime_connect_timeout_ms() -> u64 {
    30_000
}

pub fn default_realtime_max_concurrent_sessions() -> u32 {
    20
}

pub fn default_realtime_max_message_bytes() -> u64 {
    64 * 1024 * 1024
}

pub fn default_realtime_transcript_max_entries() -> u32 {
    2_000
}

pub fn default_realtime_transcript_max_bytes() -> u64 {
    64 * 1024 * 1024
}
