use serde::{Deserialize, Serialize};

use crate::domain::{
    realtime::{VersionedLegacyRealtimeRequest, VersionedRealtimeMessage},
    requests::SendRequestPayload,
};

pub const POSTNOT_COLLECTION_SCHEMA: &str = "https://post-not.com/schemas/collection.json";
pub const POSTNOT_COLLECTION_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostNotCollectionDocument {
    pub schema: String,
    pub version: u32,
    pub collection: PostNotCollectionMetadata,
    pub items: Vec<PostNotCollectionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostNotCollectionMetadata {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub pre_request_script: String,
    #[serde(default)]
    pub test_script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PostNotCollectionItem {
    Folder {
        name: String,
        #[serde(default)]
        pre_request_script: String,
        #[serde(default)]
        test_script: String,
        #[serde(default)]
        items: Vec<PostNotCollectionItem>,
    },
    Http {
        request: SendRequestPayload,
    },
    Message {
        message: VersionedRealtimeMessage,
    },
    /// PostNot collection v1 compatibility.
    Realtime {
        #[serde(flatten)]
        request: VersionedLegacyRealtimeRequest,
    },
}
