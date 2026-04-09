use serde::{Deserialize, Serialize};

use crate::domain::requests::SendRequestPayload;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub request_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionInput {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionFolderInput {
    pub name: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionItemSummary {
    pub id: String,
    pub collection_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub method: Option<String>,
    pub url: Option<String>,
    pub updated_at: String,
    pub children: Vec<CollectionItemSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSidebarState {
    pub expanded_collection_ids: Vec<String>,
    pub expanded_folder_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedRequestSummary {
    pub id: String,
    pub collection_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub method: String,
    pub url: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedRequestDetail {
    pub id: String,
    pub collection_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub updated_at: String,
    pub request: SendRequestPayload,
}
