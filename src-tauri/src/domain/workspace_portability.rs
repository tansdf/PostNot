use serde::{Deserialize, Serialize};

use crate::domain::{
    realtime::{
        RealtimeConnectionDraft, RealtimeMessageDraft, VersionedRealtimeConnection,
        VersionedRealtimeMessage,
    },
    requests::SendRequestPayload,
};

pub const POSTNOT_WORKSPACE_SCHEMA: &str = "https://post-not.com/schemas/workspace.json";
pub const POSTNOT_WORKSPACE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableWorkspaceDocument {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub version: u32,
    pub exported_at: String,
    pub exported_by: PortableWorkspaceProducer,
    pub collections: Vec<PortableCollection>,
    pub realtime_connections: Vec<PortableRealtimeConnection>,
    pub environments: Vec<PortableEnvironment>,
    pub playbooks: Vec<PortablePlaybook>,
    #[serde(default)]
    pub drafts: PortableWorkspaceDrafts,
    #[serde(default)]
    pub redactions: Vec<WorkspaceRedaction>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableWorkspaceProducer {
    pub application: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableCollection {
    pub export_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub pre_request_script: String,
    #[serde(default)]
    pub test_script: String,
    #[serde(default)]
    pub items: Vec<PortableCollectionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PortableCollectionItem {
    Folder {
        export_id: String,
        parent_export_id: Option<String>,
        sort_order: i64,
        name: String,
        #[serde(default)]
        pre_request_script: String,
        #[serde(default)]
        test_script: String,
    },
    Http {
        export_id: String,
        parent_export_id: Option<String>,
        sort_order: i64,
        request: Box<SendRequestPayload>,
    },
    Message {
        export_id: String,
        parent_export_id: Option<String>,
        sort_order: i64,
        message: VersionedRealtimeMessage,
    },
}

impl PortableCollectionItem {
    pub fn export_id(&self) -> &str {
        match self {
            Self::Folder { export_id, .. }
            | Self::Http { export_id, .. }
            | Self::Message { export_id, .. } => export_id,
        }
    }

    pub fn parent_export_id(&self) -> Option<&str> {
        match self {
            Self::Folder {
                parent_export_id, ..
            }
            | Self::Http {
                parent_export_id, ..
            }
            | Self::Message {
                parent_export_id, ..
            } => parent_export_id.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableRealtimeConnection {
    pub export_id: String,
    pub connection: VersionedRealtimeConnection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableEnvironment {
    pub export_id: String,
    pub name: String,
    #[serde(default)]
    pub variables: Vec<PortableEnvironmentVariable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableEnvironmentVariable {
    pub export_id: String,
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortablePlaybook {
    pub export_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default_delay_ms: i64,
    #[serde(default = "default_true")]
    pub stop_on_failure: bool,
    #[serde(default = "default_true")]
    pub fail_on_http_error: bool,
    #[serde(default)]
    pub steps: Vec<PortablePlaybookStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortablePlaybookStep {
    pub export_id: String,
    pub saved_request_export_id: Option<String>,
    #[serde(default)]
    pub saved_request_name: String,
    #[serde(default)]
    pub name_override: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub sort_order: i64,
    pub delay_after_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableWorkspaceDrafts {
    #[serde(default)]
    pub requests: Vec<PortableRequestDraft>,
    #[serde(default)]
    pub realtime: Vec<PortableRealtimeDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableRequestDraft {
    pub saved_request_export_id: Option<String>,
    pub request: SendRequestPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableRealtimeDraft {
    pub selected_profile_export_id: Option<String>,
    pub selected_message_export_id: Option<String>,
    pub connection: RealtimeConnectionDraft,
    pub message: RealtimeMessageDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRedaction {
    pub resource_kind: String,
    pub resource_export_id: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPortableWorkspaceInput {
    #[serde(default)]
    pub include_open_drafts: bool,
    #[serde(default)]
    pub drafts: PortableWorkspaceDrafts,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableWorkspaceCounts {
    pub collections: usize,
    pub folders: usize,
    pub http_requests: usize,
    pub realtime_messages: usize,
    pub realtime_connections: usize,
    pub environments: usize,
    pub environment_variables: usize,
    pub playbooks: usize,
    pub playbook_steps: usize,
    pub request_drafts: usize,
    pub realtime_drafts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableWorkspaceExportResult {
    pub file_path: String,
    pub counts: PortableWorkspaceCounts,
    pub redaction_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableWorkspaceImportPreview {
    pub version: u32,
    pub exported_at: String,
    pub exported_by_version: String,
    pub counts: PortableWorkspaceCounts,
    pub redaction_count: usize,
    pub credential_fields_requiring_input: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPortableWorkspaceInput {
    pub source: String,
    #[serde(default = "default_true")]
    pub include_open_drafts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedPortableRequestDraft {
    pub saved_request_id: Option<String>,
    pub collection_id: Option<String>,
    pub parent_id: Option<String>,
    pub request: SendRequestPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedPortableRealtimeDraft {
    pub selected_profile_id: Option<String>,
    pub selected_message_id: Option<String>,
    pub collection_id: Option<String>,
    pub parent_id: Option<String>,
    pub connection: RealtimeConnectionDraft,
    pub message: RealtimeMessageDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableWorkspaceImportResult {
    pub counts: PortableWorkspaceCounts,
    pub reused_realtime_connection_count: usize,
    pub credential_fields_requiring_input: Vec<WorkspaceRedaction>,
    pub request_drafts: Vec<ImportedPortableRequestDraft>,
    pub realtime_drafts: Vec<ImportedPortableRealtimeDraft>,
    pub warnings: Vec<String>,
}

fn default_true() -> bool {
    true
}
