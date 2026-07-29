use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionExportResult {
    pub file_path: String,
    pub format: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub omitted_realtime_request_count: usize,
}
