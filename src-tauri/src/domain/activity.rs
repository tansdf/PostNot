use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentActivityEntry {
    pub id: i64,
    pub batch_id: String,
    pub occurred_at: String,
    pub actor_name: String,
    pub actor_version: String,
    pub session_id: String,
    pub operation: String,
    pub outcome: String,
    pub target_kind: String,
    pub target_id: Option<String>,
    pub target_name: String,
    pub collection_id: Option<String>,
    pub changed_fields: Vec<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentActivityPage {
    pub entries: Vec<AgentActivityEntry>,
    pub latest_id: i64,
}

#[derive(Debug, Clone)]
pub struct AgentActor {
    pub name: String,
    pub version: String,
    pub session_id: String,
}

#[derive(Debug, Clone)]
pub struct NewAgentActivity<'a> {
    pub batch_id: &'a str,
    pub actor: &'a AgentActor,
    pub operation: &'a str,
    pub outcome: &'a str,
    pub target_kind: &'a str,
    pub target_id: Option<&'a str>,
    pub target_name: &'a str,
    pub collection_id: Option<&'a str>,
    pub changed_fields: &'a [&'a str],
    pub error_code: Option<&'a str>,
    pub error_message: Option<&'a str>,
}
