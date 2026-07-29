use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::requests::{KeyValueRow, RequestAuth};

pub const REALTIME_REQUEST_SCHEMA_VERSION: u32 = 1;

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
pub struct RealtimeRequestCommon {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SocketIoTransport {
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
#[serde(tag = "requestType", rename_all = "lowercase")]
pub enum RealtimeRequestDraft {
    Websocket {
        #[serde(flatten)]
        common: RealtimeRequestCommon,
        #[serde(default)]
        subprotocols: Vec<String>,
        #[serde(default)]
        composer: RawWebSocketComposer,
    },
    Socketio {
        #[serde(flatten)]
        common: RealtimeRequestCommon,
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

impl RealtimeRequestDraft {
    pub const fn request_type(&self) -> RequestType {
        match self {
            Self::Websocket { .. } => RequestType::Websocket,
            Self::Socketio { .. } => RequestType::Socketio,
        }
    }

    pub const fn common(&self) -> &RealtimeRequestCommon {
        match self {
            Self::Websocket { common, .. } | Self::Socketio { common, .. } => common,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionedRealtimeRequest {
    pub version: u32,
    #[serde(flatten)]
    pub request: RealtimeRequestDraft,
}

impl VersionedRealtimeRequest {
    pub const fn new(request: RealtimeRequestDraft) -> Self {
        Self {
            version: REALTIME_REQUEST_SCHEMA_VERSION,
            request,
        }
    }
}

impl Default for SocketIoTransport {
    fn default() -> Self {
        Self::Auto
    }
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
    fn realtime_draft_round_trips_with_version_and_discriminator() {
        let request = RealtimeRequestDraft::Websocket {
            common: RealtimeRequestCommon {
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

        let json = serde_json::to_value(VersionedRealtimeRequest::new(request.clone()))
            .expect("serialize");
        assert_eq!(json["version"], REALTIME_REQUEST_SCHEMA_VERSION);
        assert_eq!(json["requestType"], "websocket");

        let restored: VersionedRealtimeRequest = serde_json::from_value(json).expect("deserialize");
        assert_eq!(
            serde_json::to_value(restored.request).expect("serialize restored"),
            serde_json::to_value(request).expect("serialize original")
        );
    }
}
