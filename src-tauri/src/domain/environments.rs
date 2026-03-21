use serde::{Deserialize, Serialize};

use crate::domain::requests::KeyValueRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSummary {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub variable_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentDetail {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub variables: Vec<KeyValueRow>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentInput {
    pub name: String,
    pub variables: Vec<KeyValueRow>,
}
