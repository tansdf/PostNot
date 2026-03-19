use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntrySummary {
    pub id: String,
    pub request_name: String,
    pub method: String,
    pub url: String,
    pub status_code: Option<i64>,
    pub duration_ms: i64,
    pub response_body_preview: String,
    pub error_text: String,
    pub executed_at: String,
}
