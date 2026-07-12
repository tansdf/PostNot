use serde::{Deserialize, Serialize};

use crate::domain::requests::{KeyValueRow, ResponseBody, SendRequestPayload};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryDetail {
    pub id: String,
    pub request_name: String,
    pub method: String,
    pub url: String,
    pub status_code: Option<i64>,
    pub duration_ms: i64,
    pub request_snapshot: SendRequestPayload,
    pub response_headers: Vec<KeyValueRow>,
    pub response_body: ResponseBody,
    pub error_text: String,
    pub executed_at: String,
}
