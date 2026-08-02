use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::requests::{KeyValueRow, RequestAuth};

pub const REALTIME_CONNECTION_SCHEMA_VERSION: u32 = 1;
pub const REALTIME_MESSAGE_SCHEMA_VERSION: u32 = 1;
pub const LEGACY_REALTIME_REQUEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RequestType {
    Http,
    Websocket,
    Socketio,
}

impl RequestType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Websocket => "websocket",
            Self::Socketio => "socketio",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReconnectPolicy {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_reconnect_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_reconnect_initial_delay_ms")]
    pub initial_delay_ms: u64,
    #[serde(default = "default_reconnect_max_delay_ms")]
    pub max_delay_ms: u64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            max_attempts: default_reconnect_attempts(),
            initial_delay_ms: default_reconnect_initial_delay_ms(),
            max_delay_ms: default_reconnect_max_delay_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeConnectionCommon {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub query_params: Vec<KeyValueRow>,
    #[serde(default)]
    pub headers: Vec<KeyValueRow>,
    #[serde(default)]
    pub auth: RequestAuth,
    #[serde(default)]
    pub reconnect: ReconnectPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RawMessageMode {
    Text,
    Json,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum BinaryPayloadSource {
    File { path: String },
    Hex { value: String },
    Base64 { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RawWebSocketComposer {
    #[serde(default = "default_raw_message_mode")]
    pub mode: RawMessageMode,
    #[serde(default)]
    pub content: String,
    pub binary: Option<BinaryPayloadSource>,
}

impl Default for RawWebSocketComposer {
    fn default() -> Self {
        Self {
            mode: default_raw_message_mode(),
            content: String::new(),
            binary: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SocketIoTransport {
    #[default]
    Auto,
    WebsocketOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SocketIoComposer {
    #[serde(default)]
    pub event: String,
    #[serde(default = "default_socketio_arguments")]
    pub arguments: serde_json::Value,
    pub binary: Option<BinaryPayloadSource>,
    #[serde(default)]
    pub wait_for_ack: bool,
    #[serde(default = "default_ack_timeout_ms")]
    pub ack_timeout_ms: u64,
}

impl Default for SocketIoComposer {
    fn default() -> Self {
        Self {
            event: String::new(),
            arguments: default_socketio_arguments(),
            binary: None,
            wait_for_ack: false,
            ack_timeout_ms: default_ack_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum RealtimeConnectionDraft {
    Websocket {
        #[serde(flatten)]
        common: RealtimeConnectionCommon,
        #[serde(default)]
        subprotocols: Vec<String>,
    },
    Socketio {
        #[serde(flatten)]
        common: RealtimeConnectionCommon,
        #[serde(default = "default_socketio_path")]
        path: String,
        #[serde(default = "default_socketio_namespace")]
        namespace: String,
        #[serde(default = "default_socketio_auth_payload")]
        auth_payload: serde_json::Value,
        #[serde(default)]
        transport: SocketIoTransport,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeConnectionProfileSummary {
    pub id: String,
    pub name: String,
    pub protocol: RequestType,
    pub url: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeConnectionProfileDetail {
    pub id: String,
    pub name: String,
    pub protocol: RequestType,
    pub url: String,
    pub updated_at: String,
    pub connection: RealtimeConnectionDraft,
}

impl RealtimeConnectionDraft {
    pub const fn protocol(&self) -> RequestType {
        match self {
            Self::Websocket { .. } => RequestType::Websocket,
            Self::Socketio { .. } => RequestType::Socketio,
        }
    }

    pub const fn common(&self) -> &RealtimeConnectionCommon {
        match self {
            Self::Websocket { common, .. } | Self::Socketio { common, .. } => common,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum RealtimeMessageDraft {
    Websocket {
        name: String,
        #[serde(default)]
        composer: RawWebSocketComposer,
    },
    Socketio {
        name: String,
        #[serde(default)]
        composer: SocketIoComposer,
    },
}

impl RealtimeMessageDraft {
    pub const fn protocol(&self) -> RequestType {
        match self {
            Self::Websocket { .. } => RequestType::Websocket,
            Self::Socketio { .. } => RequestType::Socketio,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Websocket { name, .. } | Self::Socketio { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionedRealtimeConnection {
    pub version: u32,
    pub connection: RealtimeConnectionDraft,
}

impl VersionedRealtimeConnection {
    pub const fn new(connection: RealtimeConnectionDraft) -> Self {
        Self {
            version: REALTIME_CONNECTION_SCHEMA_VERSION,
            connection,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionedRealtimeMessage {
    pub version: u32,
    pub message: RealtimeMessageDraft,
}

impl VersionedRealtimeMessage {
    pub const fn new(message: RealtimeMessageDraft) -> Self {
        Self {
            version: REALTIME_MESSAGE_SCHEMA_VERSION,
            message,
        }
    }
}

/// Version 1 persisted a connection and one composer as one collection item.
/// It is retained only for database and PostNot v1 import compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "requestType", rename_all = "lowercase")]
pub enum LegacyRealtimeRequestDraft {
    Websocket {
        #[serde(flatten)]
        common: RealtimeConnectionCommon,
        #[serde(default)]
        subprotocols: Vec<String>,
        #[serde(default)]
        composer: RawWebSocketComposer,
    },
    Socketio {
        #[serde(flatten)]
        common: RealtimeConnectionCommon,
        #[serde(default = "default_socketio_path")]
        path: String,
        #[serde(default = "default_socketio_namespace")]
        namespace: String,
        #[serde(default = "default_socketio_auth_payload")]
        auth_payload: serde_json::Value,
        #[serde(default)]
        transport: SocketIoTransport,
        #[serde(default)]
        composer: SocketIoComposer,
    },
}

impl LegacyRealtimeRequestDraft {
    pub fn split(self) -> (RealtimeConnectionDraft, RealtimeMessageDraft) {
        match self {
            Self::Websocket {
                common,
                subprotocols,
                composer,
            } => {
                let message_name = format!("{} message", common.name.trim());
                (
                    RealtimeConnectionDraft::Websocket {
                        common,
                        subprotocols,
                    },
                    RealtimeMessageDraft::Websocket {
                        name: message_name,
                        composer,
                    },
                )
            }
            Self::Socketio {
                common,
                path,
                namespace,
                auth_payload,
                transport,
                composer,
            } => {
                let message_name = if composer.event.trim().is_empty() {
                    format!("{} message", common.name.trim())
                } else {
                    composer.event.trim().to_string()
                };
                (
                    RealtimeConnectionDraft::Socketio {
                        common,
                        path,
                        namespace,
                        auth_payload,
                        transport,
                    },
                    RealtimeMessageDraft::Socketio {
                        name: message_name,
                        composer,
                    },
                )
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionedLegacyRealtimeRequest {
    pub version: u32,
    #[serde(flatten)]
    pub request: LegacyRealtimeRequestDraft,
}

fn default_reconnect_attempts() -> u32 {
    5
}

fn default_reconnect_initial_delay_ms() -> u64 {
    500
}

fn default_reconnect_max_delay_ms() -> u64 {
    10_000
}

fn default_raw_message_mode() -> RawMessageMode {
    RawMessageMode::Text
}

fn default_socketio_arguments() -> serde_json::Value {
    serde_json::Value::Array(Vec::new())
}

fn default_socketio_auth_payload() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn default_ack_timeout_ms() -> u64 {
    5_000
}

fn default_socketio_path() -> String {
    "/socket.io/".to_string()
}

fn default_socketio_namespace() -> String {
    "/".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_request_splits_into_connection_and_message() {
        let request = LegacyRealtimeRequestDraft::Websocket {
            common: RealtimeConnectionCommon {
                name: "Events".to_string(),
                url: "wss://example.test/events".to_string(),
                query_params: Vec::new(),
                headers: Vec::new(),
                auth: RequestAuth::default(),
                reconnect: ReconnectPolicy::default(),
            },
            subprotocols: vec!["graphql-transport-ws".to_string()],
            composer: RawWebSocketComposer::default(),
        };

        let (connection, message) = request.split();
        assert_eq!(connection.protocol(), RequestType::Websocket);
        assert_eq!(message.name(), "Events message");
    }
}
