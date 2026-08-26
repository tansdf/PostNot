use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSummary {
    pub data_directory: String,
    pub database_size_bytes: u64,
    pub history_entry_count: u64,
    pub history_response_body_bytes: u64,
    pub realtime_temporary_bytes: u64,
    pub collection_count: u64,
    pub collection_item_count: u64,
    pub realtime_connection_count: u64,
    pub environment_count: u64,
    pub playbook_count: u64,
    pub playbook_run_count: u64,
    pub agent_activity_count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRetentionResult {
    pub removed_entry_count: u64,
    pub released_response_body_bytes: u64,
}
