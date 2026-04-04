use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub current_version: String,
    pub version: String,
    pub date: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub configured: bool,
    pub update: Option<AvailableUpdate>,
}
