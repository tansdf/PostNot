use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{
        client::IntoClientRequest,
        http::{
            header::{AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL},
            HeaderName, HeaderValue,
        },
        protocol::{frame::coding::CloseCode, CloseFrame, WebSocketConfig},
        Message,
    },
    Connector,
};
use url::Url;
use uuid::Uuid;

use crate::{
    domain::{
        realtime::{
            BinaryPayloadSource, RawMessageMode, RawWebSocketComposer, RealtimeConnectionDraft,
            RealtimeMessageDraft, ReconnectPolicy, RequestType,
        },
        requests::RequestAuth,
    },
    error::{AppError, AppResult},
    services::{
        realtime_payload_service::{RealtimePayload, RealtimePayloadStore},
        realtime_resolution_service::sanitize_error,
    },
};

const FORBIDDEN_WEBSOCKET_HEADERS: [&str; 7] = [
    "host",
    "connection",
    "upgrade",
    "sec-websocket-key",
    "sec-websocket-version",
    "sec-websocket-extensions",
    "sec-websocket-protocol",
];

#[derive(Debug, Clone, Copy)]
pub struct RealtimeRuntimeLimits {
    pub connect_timeout: Duration,
    pub max_concurrent_sessions: usize,
    pub max_message_bytes: usize,
    pub transcript_max_entries: usize,
    pub transcript_max_bytes: u64,
    pub validate_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeConnectInput {
    pub session_id: String,
    pub connection: RealtimeConnectionDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RealtimeConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Disconnecting,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RealtimeTranscriptDirection {
    Sent,
    Received,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RealtimeTranscriptKind {
    Lifecycle,
    Text,
    Json,
    Binary,
    Ping,
    Pong,
    Event,
    Ack,
    Error,
    Trimmed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeTranscriptEntry {
    pub id: String,
    pub session_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub occurred_at: String,
    pub direction: RealtimeTranscriptDirection,
    pub kind: RealtimeTranscriptKind,
    pub label: String,
    pub event_name: Option<String>,
    pub payload: Option<RealtimePayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RealtimeRuntimeEvent {
    Status {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        sequence: u64,
        status: RealtimeConnectionStatus,
        message: String,
    },
    Transcript {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        sequence: u64,
        entry: RealtimeTranscriptEntry,
    },
    TranscriptReset {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        sequence: u64,
        entries: Vec<RealtimeTranscriptEntry>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeSessionSnapshot {
    pub session_id: String,
    pub generation: u64,
    pub last_sequence: u64,
    pub status: RealtimeConnectionStatus,
    pub status_message: String,
    pub transcript: Vec<RealtimeTranscriptEntry>,
    pub transcript_size_bytes: u64,
}

#[derive(Clone)]
pub struct RealtimeConnectionManager {
    inner: Arc<ManagerInner>,
}

struct ManagerInner {
    sessions: Mutex<HashMap<String, Arc<RuntimeSession>>>,
    payloads: RealtimePayloadStore,
}

pub(crate) struct RuntimeSession {
    session_id: String,
    protocol: RequestType,
    generation: u64,
    event_channel: Channel<RealtimeRuntimeEvent>,
    state: Mutex<SessionState>,
    command_tx: mpsc::Sender<SessionCommand>,
    payloads: RealtimePayloadStore,
    limits: RealtimeRuntimeLimits,
    secret_values: Vec<String>,
}

struct SessionState {
    sequence: u64,
    status: RealtimeConnectionStatus,
    status_message: String,
    transcript: VecDeque<RealtimeTranscriptEntry>,
    transcript_size_bytes: u64,
    has_trim_marker: bool,
}

pub(crate) enum SessionCommand {
    Send {
        message: RealtimeMessageDraft,
        secret_values: Vec<String>,
        used_secret: bool,
    },
    Ping(Option<String>),
    Close {
        code: u16,
        reason: String,
    },
    Disconnect,
}

impl RealtimeConnectionManager {
    pub fn new(payloads: RealtimePayloadStore) -> Self {
        Self {
            inner: Arc::new(ManagerInner {
                sessions: Mutex::new(HashMap::new()),
                payloads,
            }),
        }
    }

    pub fn payloads(&self) -> &RealtimePayloadStore {
        &self.inner.payloads
    }

    pub async fn connect(
        &self,
        input: RealtimeConnectInput,
        connection: RealtimeConnectionDraft,
        secret_values: Vec<String>,
        limits: RealtimeRuntimeLimits,
        event_channel: Channel<RealtimeRuntimeEvent>,
    ) -> AppResult<RealtimeSessionSnapshot> {
        if input.session_id.trim().is_empty() {
            return Err(AppError::Message(
                "Realtime session ID is required.".to_string(),
            ));
        }

        let (generation, previous) = {
            let sessions = self.inner.sessions.lock().map_err(|_| manager_error())?;
            let active = sessions
                .values()
                .filter(|session| session.is_live())
                .count();
            let previous = sessions.get(&input.session_id).cloned();
            if previous.as_ref().is_none_or(|session| !session.is_live())
                && active >= limits.max_concurrent_sessions
            {
                return Err(AppError::Message(format!(
                    "The maximum of {} live realtime sessions has been reached.",
                    limits.max_concurrent_sessions
                )));
            }
            (
                previous
                    .as_ref()
                    .map(|session| session.generation.saturating_add(1))
                    .unwrap_or(1),
                previous,
            )
        };

        if let Some(previous) = previous {
            let _ = previous.command_tx.send(SessionCommand::Disconnect).await;
        }

        let (command_tx, command_rx) = mpsc::channel(32);
        let protocol = connection.protocol();
        let session = Arc::new(RuntimeSession {
            session_id: input.session_id.clone(),
            protocol,
            generation,
            event_channel,
            state: Mutex::new(SessionState {
                sequence: 0,
                status: RealtimeConnectionStatus::Connecting,
                status_message: "Connecting".to_string(),
                transcript: VecDeque::new(),
                transcript_size_bytes: 0,
                has_trim_marker: false,
            }),
            command_tx,
            payloads: self.inner.payloads.clone(),
            limits,
            secret_values,
        });

        self.inner
            .sessions
            .lock()
            .map_err(|_| manager_error())?
            .insert(input.session_id, Arc::clone(&session));

        session.emit_status(RealtimeConnectionStatus::Connecting, "Connecting");
        let session_for_task = Arc::clone(&session);
        tauri::async_runtime::spawn(async move {
            match connection {
                RealtimeConnectionDraft::Websocket { .. } => {
                    run_raw_websocket(session_for_task, connection, command_rx).await
                }
                RealtimeConnectionDraft::Socketio { .. } => {
                    crate::services::realtime_socketio_service::run_socketio(
                        session_for_task,
                        connection,
                        command_rx,
                    )
                    .await
                }
            }
        });

        session.snapshot()
    }

    pub async fn disconnect(&self, session_id: &str) -> AppResult<()> {
        self.session(session_id)?
            .command_tx
            .send(SessionCommand::Disconnect)
            .await
            .map_err(|_| AppError::Message("Realtime connection is no longer active.".to_string()))
    }

    pub async fn send(
        &self,
        session_id: &str,
        message: RealtimeMessageDraft,
        secret_values: Vec<String>,
        used_secret: bool,
    ) -> AppResult<()> {
        let session = self.session(session_id)?;
        if session.status() != RealtimeConnectionStatus::Connected {
            return Err(AppError::Message(
                "Messages can only be sent while the connection is connected.".to_string(),
            ));
        }
        if session.protocol != message.protocol() {
            return Err(AppError::Message(
                "The selected message protocol does not match the live connection.".to_string(),
            ));
        }
        session
            .command_tx
            .send(SessionCommand::Send {
                message,
                secret_values,
                used_secret,
            })
            .await
            .map_err(|_| AppError::Message("Realtime connection is no longer active.".to_string()))
    }

    pub async fn ping(&self, session_id: &str, payload: Option<String>) -> AppResult<()> {
        let session = self.session(session_id)?;
        if session.status() != RealtimeConnectionStatus::Connected {
            return Err(AppError::Message(
                "Ping is only available while the connection is connected.".to_string(),
            ));
        }
        session
            .command_tx
            .send(SessionCommand::Ping(payload))
            .await
            .map_err(|_| AppError::Message("Realtime connection is no longer active.".to_string()))
    }

    pub async fn close(&self, session_id: &str, code: u16, reason: String) -> AppResult<()> {
        validate_close(code, &reason)?;
        self.session(session_id)?
            .command_tx
            .send(SessionCommand::Close { code, reason })
            .await
            .map_err(|_| AppError::Message("Realtime connection is no longer active.".to_string()))
    }

    pub fn snapshot(&self, session_id: &str) -> AppResult<RealtimeSessionSnapshot> {
        self.session(session_id)?.snapshot()
    }

    pub fn snapshot_for_export(
        &self,
        session_id: &str,
    ) -> AppResult<(RealtimeSessionSnapshot, Vec<String>)> {
        self.session(session_id)?.snapshot_for_export()
    }

    pub async fn clear_transcript(&self, session_id: &str) -> AppResult<()> {
        self.session(session_id)?.clear_transcript().await
    }

    pub async fn release(&self, session_id: &str) -> AppResult<()> {
        let session = self
            .inner
            .sessions
            .lock()
            .map_err(|_| manager_error())?
            .remove(session_id);
        if let Some(session) = session {
            let _ = session.command_tx.send(SessionCommand::Disconnect).await;
            session.clear_transcript().await?;
        }
        Ok(())
    }

    fn session(&self, session_id: &str) -> AppResult<Arc<RuntimeSession>> {
        self.inner
            .sessions
            .lock()
            .map_err(|_| manager_error())?
            .get(session_id)
            .cloned()
            .ok_or_else(|| AppError::Message("Realtime connection not found.".to_string()))
    }
}

impl RuntimeSession {
    pub(crate) fn limits(&self) -> RealtimeRuntimeLimits {
        self.limits
    }

    pub(crate) fn payloads(&self) -> &RealtimePayloadStore {
        &self.payloads
    }

    pub(crate) fn secret_values(&self) -> &[String] {
        &self.secret_values
    }

    fn status(&self) -> RealtimeConnectionStatus {
        self.state
            .lock()
            .map(|state| state.status)
            .unwrap_or(RealtimeConnectionStatus::Failed)
    }

    fn is_live(&self) -> bool {
        matches!(
            self.status(),
            RealtimeConnectionStatus::Connecting
                | RealtimeConnectionStatus::Connected
                | RealtimeConnectionStatus::Reconnecting
                | RealtimeConnectionStatus::Disconnecting
        )
    }

    pub(crate) fn emit_status(&self, status: RealtimeConnectionStatus, message: impl Into<String>) {
        let message = sanitize_error(&message.into(), &self.secret_values);
        let event = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            state.sequence = state.sequence.saturating_add(1);
            state.status = status;
            state.status_message.clone_from(&message);
            RealtimeRuntimeEvent::Status {
                session_id: self.session_id.clone(),
                generation: self.generation,
                sequence: state.sequence,
                status,
                message,
            }
        };
        let _ = self.event_channel.send(event);
    }

    pub(crate) async fn record(
        &self,
        direction: RealtimeTranscriptDirection,
        kind: RealtimeTranscriptKind,
        label: impl Into<String>,
        event_name: Option<String>,
        payload: Option<RealtimePayload>,
    ) {
        let label = sanitize_error(&label.into(), &self.secret_values);
        let event_name = event_name.map(|name| sanitize_error(&name, &self.secret_values));
        let mut released_handles = Vec::new();
        let payload = match payload {
            Some(payload) if payload.size_bytes() > self.limits.transcript_max_bytes => {
                if let Some(handle) = payload.handle_id() {
                    released_handles.push(handle.to_string());
                }
                Some(RealtimePayload::Inline {
                    text: format!(
                        "Payload omitted because it exceeded the {} byte transcript limit.",
                        self.limits.transcript_max_bytes
                    ),
                    size_bytes: 0,
                    encoding:
                        crate::services::realtime_payload_service::RealtimePayloadEncoding::Utf8,
                    truncated: true,
                })
            }
            payload => payload,
        };
        let event = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            state.sequence = state.sequence.saturating_add(1);
            let entry = RealtimeTranscriptEntry {
                id: Uuid::new_v4().to_string(),
                session_id: self.session_id.clone(),
                generation: self.generation,
                sequence: state.sequence,
                occurred_at: Utc::now().to_rfc3339(),
                direction,
                kind,
                label,
                event_name,
                payload,
            };
            state.transcript_size_bytes = state.transcript_size_bytes.saturating_add(
                entry
                    .payload
                    .as_ref()
                    .map_or(0, RealtimePayload::size_bytes),
            );
            state.transcript.push_back(entry.clone());
            let mut trimmed = false;
            while state.transcript.len() > 1
                && (state.transcript.len() > self.limits.transcript_max_entries
                    || state.transcript_size_bytes > self.limits.transcript_max_bytes)
            {
                if let Some(evicted) = state.transcript.pop_front() {
                    state.transcript_size_bytes = state.transcript_size_bytes.saturating_sub(
                        evicted
                            .payload
                            .as_ref()
                            .map_or(0, RealtimePayload::size_bytes),
                    );
                    if let Some(handle) = evicted
                        .payload
                        .as_ref()
                        .and_then(RealtimePayload::handle_id)
                    {
                        released_handles.push(handle.to_string());
                    }
                    trimmed = true;
                }
            }
            if trimmed {
                if !state.has_trim_marker && self.limits.transcript_max_entries > 1 {
                    state.sequence = state.sequence.saturating_add(1);
                    let marker = RealtimeTranscriptEntry {
                        id: Uuid::new_v4().to_string(),
                        session_id: self.session_id.clone(),
                        generation: self.generation,
                        sequence: state.sequence,
                        occurred_at: Utc::now().to_rfc3339(),
                        direction: RealtimeTranscriptDirection::System,
                        kind: RealtimeTranscriptKind::Trimmed,
                        label: "Older messages removed".to_string(),
                        event_name: None,
                        payload: None,
                    };
                    state.transcript.push_front(marker);
                    state.has_trim_marker = true;
                }
                while state.transcript.len() > self.limits.transcript_max_entries {
                    let remove_index = usize::from(state.has_trim_marker);
                    if let Some(evicted) = state.transcript.remove(remove_index) {
                        state.transcript_size_bytes = state.transcript_size_bytes.saturating_sub(
                            evicted
                                .payload
                                .as_ref()
                                .map_or(0, RealtimePayload::size_bytes),
                        );
                        if let Some(handle) = evicted
                            .payload
                            .as_ref()
                            .and_then(RealtimePayload::handle_id)
                        {
                            released_handles.push(handle.to_string());
                        }
                    }
                }
                state.sequence = state.sequence.saturating_add(1);
                RealtimeRuntimeEvent::TranscriptReset {
                    session_id: self.session_id.clone(),
                    generation: self.generation,
                    sequence: state.sequence,
                    entries: state.transcript.iter().cloned().collect(),
                }
            } else {
                RealtimeRuntimeEvent::Transcript {
                    session_id: self.session_id.clone(),
                    generation: self.generation,
                    sequence: state.sequence,
                    entry,
                }
            }
        };

        let _ = self.event_channel.send(event);
        for handle in released_handles {
            let _ = self.payloads.release(&handle).await;
        }
    }

    fn snapshot(&self) -> AppResult<RealtimeSessionSnapshot> {
        let state = self.state.lock().map_err(|_| session_error())?;
        Ok(RealtimeSessionSnapshot {
            session_id: self.session_id.clone(),
            generation: self.generation,
            last_sequence: state.sequence,
            status: state.status,
            status_message: state.status_message.clone(),
            transcript: state.transcript.iter().cloned().collect(),
            transcript_size_bytes: state.transcript_size_bytes,
        })
    }

    fn snapshot_for_export(&self) -> AppResult<(RealtimeSessionSnapshot, Vec<String>)> {
        let state = self.state.lock().map_err(|_| session_error())?;
        let handles = state
            .transcript
            .iter()
            .filter_map(|entry| entry.payload.as_ref()?.handle_id().map(str::to_string))
            .collect::<Vec<_>>();
        for handle in &handles {
            self.payloads.retain(handle)?;
        }
        Ok((
            RealtimeSessionSnapshot {
                session_id: self.session_id.clone(),
                generation: self.generation,
                last_sequence: state.sequence,
                status: state.status,
                status_message: state.status_message.clone(),
                transcript: state.transcript.iter().cloned().collect(),
                transcript_size_bytes: state.transcript_size_bytes,
            },
            handles,
        ))
    }

    async fn clear_transcript(&self) -> AppResult<()> {
        let (handles, event) = {
            let mut state = self.state.lock().map_err(|_| session_error())?;
            let handles = state
                .transcript
                .iter()
                .filter_map(|entry| entry.payload.as_ref()?.handle_id().map(str::to_string))
                .collect::<Vec<_>>();
            state.transcript.clear();
            state.transcript_size_bytes = 0;
            state.has_trim_marker = false;
            state.sequence = state.sequence.saturating_add(1);
            (
                handles,
                RealtimeRuntimeEvent::TranscriptReset {
                    session_id: self.session_id.clone(),
                    generation: self.generation,
                    sequence: state.sequence,
                    entries: Vec::new(),
                },
            )
        };
        for handle in handles {
            self.payloads.release(&handle).await?;
        }
        let _ = self.event_channel.send(event);
        Ok(())
    }
}

async fn run_raw_websocket(
    session: Arc<RuntimeSession>,
    request: RealtimeConnectionDraft,
    mut commands: mpsc::Receiver<SessionCommand>,
) {
    let RealtimeConnectionDraft::Websocket {
        common,
        subprotocols,
        ..
    } = request
    else {
        return;
    };
    let reconnect = common.reconnect.clone();
    let mut attempt = 0_u32;

    loop {
        if attempt > 0 {
            session.emit_status(
                RealtimeConnectionStatus::Reconnecting,
                format!("Reconnecting ({attempt}/{})", reconnect.max_attempts),
            );
        }

        let connect_result = tokio::time::timeout(
            session.limits.connect_timeout,
            connect_raw_socket(
                &common.url,
                &common.query_params,
                &common.headers,
                &common.auth,
                &subprotocols,
                session.limits,
            ),
        )
        .await;

        let mut socket = match connect_result {
            Ok(Ok(socket)) => socket,
            Ok(Err(error)) => {
                if schedule_reconnect(&session, &reconnect, &mut attempt, &mut commands).await {
                    continue;
                }
                session.emit_status(
                    RealtimeConnectionStatus::Failed,
                    sanitize_error(&error.to_string(), &session.secret_values),
                );
                return;
            }
            Err(_) => {
                if schedule_reconnect(&session, &reconnect, &mut attempt, &mut commands).await {
                    continue;
                }
                session.emit_status(
                    RealtimeConnectionStatus::Failed,
                    "Realtime connection timed out.",
                );
                return;
            }
        };

        attempt = 0;
        session.emit_status(RealtimeConnectionStatus::Connected, "Connected");
        session
            .record(
                RealtimeTranscriptDirection::System,
                RealtimeTranscriptKind::Lifecycle,
                "Connected",
                None,
                None,
            )
            .await;

        loop {
            tokio::select! {
                command = commands.recv() => {
                    let Some(command) = command else {
                        let _ = socket.close(None).await;
                        session.record(
                            RealtimeTranscriptDirection::System,
                            RealtimeTranscriptKind::Lifecycle,
                            "Connection command channel closed",
                            None,
                            None,
                        ).await;
                        session.emit_status(RealtimeConnectionStatus::Disconnected, "Disconnected");
                        return;
                    };
                    match handle_raw_command(&session, &mut socket, command).await {
                        Ok(CommandOutcome::Continue) => {}
                        Ok(CommandOutcome::Disconnected) => return,
                        Err(RawCommandError::Validation(error)) => {
                            session.record(
                                RealtimeTranscriptDirection::System,
                                RealtimeTranscriptKind::Error,
                                sanitize_error(&error.to_string(), &session.secret_values),
                                None,
                                None,
                            ).await;
                        }
                        Err(RawCommandError::Transport(error)) => {
                            session.record(
                                RealtimeTranscriptDirection::System,
                                RealtimeTranscriptKind::Error,
                                sanitize_error(&error.to_string(), &session.secret_values),
                                None,
                                None,
                            ).await;
                            break;
                        }
                    }
                }
                incoming = socket.next() => {
                    match incoming {
                        Some(Ok(message)) => {
                            if handle_raw_incoming(&session, message).await {
                                return;
                            }
                        }
                        Some(Err(error)) => {
                            session.record(
                                RealtimeTranscriptDirection::System,
                                RealtimeTranscriptKind::Error,
                                sanitize_error(&error.to_string(), &session.secret_values),
                                None,
                                None,
                            ).await;
                            break;
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }

        if schedule_reconnect(&session, &reconnect, &mut attempt, &mut commands).await {
            continue;
        }
        session.emit_status(RealtimeConnectionStatus::Failed, "Connection lost");
        return;
    }
}

async fn connect_raw_socket(
    url: &str,
    query_params: &[crate::domain::requests::KeyValueRow],
    headers: &[crate::domain::requests::KeyValueRow],
    auth: &RequestAuth,
    subprotocols: &[String],
    limits: RealtimeRuntimeLimits,
) -> AppResult<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let mut url = Url::parse(url)?;
    if !matches!(url.scheme(), "ws" | "wss") {
        return Err(AppError::Message(
            "Raw WebSocket URLs must use ws:// or wss://.".to_string(),
        ));
    }
    {
        let mut pairs = url.query_pairs_mut();
        for row in query_params
            .iter()
            .filter(|row| row.enabled && !row.key.trim().is_empty())
        {
            pairs.append_pair(row.key.trim(), &row.value);
        }
        if auth.auth_type == "api-key"
            && auth.api_key_in == "query"
            && !auth.api_key_name.trim().is_empty()
        {
            pairs.append_pair(auth.api_key_name.trim(), &auth.api_key_value);
        }
    }

    let mut request = url.as_str().into_client_request().map_err(|error| {
        AppError::Message(format!("Invalid WebSocket handshake request: {error}"))
    })?;
    for row in headers
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        let name = row.key.trim().to_ascii_lowercase();
        if FORBIDDEN_WEBSOCKET_HEADERS.contains(&name.as_str()) {
            return Err(AppError::Message(format!(
                "WebSocket handshake header is managed by PostNot: {}.",
                row.key.trim()
            )));
        }
        if (name == "authorization" && !matches!(auth.auth_type.as_str(), "" | "none"))
            || (auth.auth_type == "api-key"
                && auth.api_key_in == "header"
                && name == auth.api_key_name.trim().to_ascii_lowercase())
        {
            return Err(AppError::Message(format!(
                "Header '{}' conflicts with the configured authentication.",
                row.key.trim()
            )));
        }
        request.headers_mut().append(
            HeaderName::from_bytes(row.key.trim().as_bytes())
                .map_err(|error| AppError::Message(format!("Invalid header name: {error}")))?,
            HeaderValue::from_str(&row.value)
                .map_err(|error| AppError::Message(format!("Invalid header value: {error}")))?,
        );
    }
    apply_auth_header(request.headers_mut(), auth)?;
    if !subprotocols.is_empty() {
        if subprotocols.iter().any(|value| {
            value.trim().is_empty() || value.contains(',') || value.chars().any(char::is_whitespace)
        }) {
            return Err(AppError::Message(
                "WebSocket subprotocols must be non-empty tokens without commas or spaces."
                    .to_string(),
            ));
        }
        request.headers_mut().insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_str(&subprotocols.join(", "))
                .map_err(|error| AppError::Message(format!("Invalid subprotocol: {error}")))?,
        );
    }

    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(!limits.validate_tls)
        .build()
        .map_err(|error| AppError::Message(format!("Could not configure TLS: {error}")))?;
    let mut config = WebSocketConfig::default();
    config.max_message_size = Some(limits.max_message_bytes);
    config.max_frame_size = Some(limits.max_message_bytes);
    let (socket, _) = connect_async_tls_with_config(
        request,
        Some(config),
        false,
        Some(Connector::NativeTls(tls)),
    )
    .await
    .map_err(|error| AppError::Message(error.to_string()))?;
    Ok(socket)
}

fn apply_auth_header(
    headers: &mut tokio_tungstenite::tungstenite::http::HeaderMap,
    auth: &RequestAuth,
) -> AppResult<()> {
    let value = match auth.auth_type.as_str() {
        "none" | "" => return Ok(()),
        "api-key" if auth.api_key_in == "query" => return Ok(()),
        "basic" => format!(
            "Basic {}",
            BASE64.encode(format!("{}:{}", auth.basic_username, auth.basic_password))
        ),
        "bearer" => format!("Bearer {}", auth.bearer_token),
        "oauth2" => format!("Bearer {}", auth.oauth2_access_token),
        "api-key" if auth.api_key_in == "header" => {
            let name = HeaderName::from_bytes(auth.api_key_name.trim().as_bytes())
                .map_err(|error| AppError::Message(format!("Invalid API-key header: {error}")))?;
            headers.insert(
                name,
                HeaderValue::from_str(&auth.api_key_value).map_err(|error| {
                    AppError::Message(format!("Invalid API-key header value: {error}"))
                })?,
            );
            return Ok(());
        }
        other => {
            return Err(AppError::Message(format!(
                "Unsupported realtime authentication type: {other}."
            )))
        }
    };
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&value)
            .map_err(|error| AppError::Message(format!("Invalid authorization value: {error}")))?,
    );
    Ok(())
}

enum CommandOutcome {
    Continue,
    Disconnected,
}

enum RawCommandError {
    Validation(AppError),
    Transport(AppError),
}

async fn handle_raw_command<S>(
    session: &Arc<RuntimeSession>,
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    command: SessionCommand,
) -> Result<CommandOutcome, RawCommandError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    match command {
        SessionCommand::Send {
            message: RealtimeMessageDraft::Websocket { composer, .. },
            secret_values,
            used_secret,
        } => {
            let (message, kind, payload) = build_raw_message(
                &session.payloads,
                &composer,
                session.limits,
                &secret_values,
                used_secret,
            )
            .await
            .map_err(RawCommandError::Validation)?;
            if let Err(error) = socket.send(message).await {
                if let Some(handle_id) = payload.handle_id() {
                    let _ = session.payloads.release(handle_id).await;
                }
                return Err(RawCommandError::Transport(AppError::Message(
                    error.to_string(),
                )));
            }
            session
                .record(
                    RealtimeTranscriptDirection::Sent,
                    kind,
                    "Sent",
                    None,
                    Some(payload),
                )
                .await;
            Ok(CommandOutcome::Continue)
        }
        SessionCommand::Send {
            message: RealtimeMessageDraft::Socketio { .. },
            ..
        } => Err(RawCommandError::Validation(AppError::Message(
            "A Socket.IO message cannot be sent through a raw WebSocket connection.".to_string(),
        ))),
        SessionCommand::Ping(payload) => {
            let bytes = payload.unwrap_or_default().into_bytes();
            if bytes.len() > 125 {
                return Err(RawCommandError::Validation(AppError::Message(
                    "WebSocket ping payloads cannot exceed 125 bytes.".to_string(),
                )));
            }
            socket
                .send(Message::Ping(bytes.clone().into()))
                .await
                .map_err(|error| {
                    RawCommandError::Transport(AppError::Message(error.to_string()))
                })?;
            let payload = session
                .payloads
                .store_text(String::from_utf8_lossy(&bytes).into_owned())
                .await
                .map_err(RawCommandError::Validation)?;
            session
                .record(
                    RealtimeTranscriptDirection::Sent,
                    RealtimeTranscriptKind::Ping,
                    "Ping",
                    None,
                    Some(payload),
                )
                .await;
            Ok(CommandOutcome::Continue)
        }
        SessionCommand::Close { code, reason } => {
            session.emit_status(RealtimeConnectionStatus::Disconnecting, "Disconnecting");
            socket
                .close(Some(CloseFrame {
                    code: CloseCode::from(code),
                    reason: reason.clone().into(),
                }))
                .await
                .map_err(|error| {
                    RawCommandError::Transport(AppError::Message(error.to_string()))
                })?;
            session
                .record(
                    RealtimeTranscriptDirection::Sent,
                    RealtimeTranscriptKind::Lifecycle,
                    format!("Closed ({code}) {reason}").trim().to_string(),
                    None,
                    None,
                )
                .await;
            session.emit_status(RealtimeConnectionStatus::Disconnected, "Disconnected");
            Ok(CommandOutcome::Disconnected)
        }
        SessionCommand::Disconnect => {
            session.emit_status(RealtimeConnectionStatus::Disconnecting, "Disconnecting");
            let _ = socket
                .close(Some(CloseFrame {
                    code: CloseCode::Normal,
                    reason: "PostNot disconnected".into(),
                }))
                .await;
            session
                .record(
                    RealtimeTranscriptDirection::System,
                    RealtimeTranscriptKind::Lifecycle,
                    "Disconnected by user",
                    None,
                    None,
                )
                .await;
            session.emit_status(RealtimeConnectionStatus::Disconnected, "Disconnected");
            Ok(CommandOutcome::Disconnected)
        }
    }
}

async fn handle_raw_incoming(session: &Arc<RuntimeSession>, message: Message) -> bool {
    match message {
        Message::Text(text) => {
            let text = text.to_string();
            let kind = if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
                RealtimeTranscriptKind::Json
            } else {
                RealtimeTranscriptKind::Text
            };
            match session.payloads.store_text(text).await {
                Ok(payload) => {
                    session
                        .record(
                            RealtimeTranscriptDirection::Received,
                            kind,
                            "Received",
                            None,
                            Some(payload),
                        )
                        .await
                }
                Err(error) => {
                    session
                        .record(
                            RealtimeTranscriptDirection::System,
                            RealtimeTranscriptKind::Error,
                            error.to_string(),
                            None,
                            None,
                        )
                        .await
                }
            }
            false
        }
        Message::Binary(bytes) => {
            match session.payloads.store_binary(&bytes).await {
                Ok(payload) => {
                    session
                        .record(
                            RealtimeTranscriptDirection::Received,
                            RealtimeTranscriptKind::Binary,
                            "Received binary",
                            None,
                            Some(payload),
                        )
                        .await
                }
                Err(error) => {
                    session
                        .record(
                            RealtimeTranscriptDirection::System,
                            RealtimeTranscriptKind::Error,
                            error.to_string(),
                            None,
                            None,
                        )
                        .await
                }
            }
            false
        }
        Message::Ping(bytes) => {
            match session.payloads.store_binary(&bytes).await {
                Ok(payload) => {
                    session
                        .record(
                            RealtimeTranscriptDirection::Received,
                            RealtimeTranscriptKind::Ping,
                            "Ping",
                            None,
                            Some(payload),
                        )
                        .await;
                }
                Err(error) => {
                    session
                        .record(
                            RealtimeTranscriptDirection::System,
                            RealtimeTranscriptKind::Error,
                            format!("Could not store WebSocket ping payload: {error}"),
                            None,
                            None,
                        )
                        .await;
                }
            }
            false
        }
        Message::Pong(bytes) => {
            match session.payloads.store_binary(&bytes).await {
                Ok(payload) => {
                    session
                        .record(
                            RealtimeTranscriptDirection::Received,
                            RealtimeTranscriptKind::Pong,
                            "Pong",
                            None,
                            Some(payload),
                        )
                        .await;
                }
                Err(error) => {
                    session
                        .record(
                            RealtimeTranscriptDirection::System,
                            RealtimeTranscriptKind::Error,
                            format!("Could not store WebSocket pong payload: {error}"),
                            None,
                            None,
                        )
                        .await;
                }
            }
            false
        }
        Message::Close(frame) => {
            let label = frame
                .map(|frame| format!("Closed ({}) {}", u16::from(frame.code), frame.reason))
                .unwrap_or_else(|| "Closed".to_string());
            session
                .record(
                    RealtimeTranscriptDirection::Received,
                    RealtimeTranscriptKind::Lifecycle,
                    label,
                    None,
                    None,
                )
                .await;
            session.emit_status(RealtimeConnectionStatus::Disconnected, "Disconnected");
            true
        }
        Message::Frame(_) => false,
    }
}

async fn build_raw_message(
    payloads: &RealtimePayloadStore,
    composer: &RawWebSocketComposer,
    limits: RealtimeRuntimeLimits,
    secret_values: &[String],
    binary_used_secret: bool,
) -> AppResult<(Message, RealtimeTranscriptKind, RealtimePayload)> {
    match composer.mode {
        RawMessageMode::Text => {
            ensure_message_size(composer.content.len(), limits)?;
            let payload = payloads
                .store_text(sanitize_error(&composer.content, secret_values))
                .await?;
            Ok((
                Message::Text(composer.content.clone().into()),
                RealtimeTranscriptKind::Text,
                payload,
            ))
        }
        RawMessageMode::Json => {
            let value: serde_json::Value =
                serde_json::from_str(&composer.content).map_err(|error| {
                    AppError::Message(format!("WebSocket JSON payload is invalid: {error}"))
                })?;
            let text = serde_json::to_string(&value)?;
            ensure_message_size(text.len(), limits)?;
            let payload = payloads
                .store_text(sanitize_error(&text, secret_values))
                .await?;
            Ok((
                Message::Text(text.into()),
                RealtimeTranscriptKind::Json,
                payload,
            ))
        }
        RawMessageMode::Binary => {
            let source = composer.binary.as_ref().ok_or_else(|| {
                AppError::Message("A binary payload source is required.".to_string())
            })?;
            let bytes = read_binary_source(source, limits).await?;
            let payload = if binary_used_secret {
                payloads
                    .store_text(format!("Binary payload redacted ({} bytes)", bytes.len()))
                    .await?
            } else {
                payloads.store_binary(&bytes).await?
            };
            Ok((
                Message::Binary(bytes.into()),
                RealtimeTranscriptKind::Binary,
                payload,
            ))
        }
    }
}

pub(crate) async fn read_binary_source(
    source: &BinaryPayloadSource,
    limits: RealtimeRuntimeLimits,
) -> AppResult<Vec<u8>> {
    let bytes = match source {
        BinaryPayloadSource::File { path } => tokio::fs::read(path).await?,
        BinaryPayloadSource::Hex { value } => decode_hex(value)?,
        BinaryPayloadSource::Base64 { value } => BASE64.decode(value.trim()).map_err(|error| {
            AppError::Message(format!("Binary payload is not valid base64: {error}"))
        })?,
    };
    ensure_message_size(bytes.len(), limits)?;
    Ok(bytes)
}

fn decode_hex(value: &str) -> AppResult<Vec<u8>> {
    let compact = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if compact.len() % 2 != 0 {
        return Err(AppError::Message(
            "Hex payloads must contain an even number of digits.".to_string(),
        ));
    }
    (0..compact.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|error| AppError::Message(format!("Invalid hex payload: {error}")))
        })
        .collect()
}

pub(crate) fn ensure_message_size(size: usize, limits: RealtimeRuntimeLimits) -> AppResult<()> {
    if size > limits.max_message_bytes {
        return Err(AppError::Message(format!(
            "Realtime message is {size} bytes; the configured limit is {} bytes.",
            limits.max_message_bytes
        )));
    }
    Ok(())
}

async fn schedule_reconnect(
    session: &Arc<RuntimeSession>,
    policy: &ReconnectPolicy,
    attempt: &mut u32,
    commands: &mut mpsc::Receiver<SessionCommand>,
) -> bool {
    if !policy.enabled || *attempt >= policy.max_attempts {
        return false;
    }
    *attempt = attempt.saturating_add(1);
    let multiplier = 1_u64
        .checked_shl(attempt.saturating_sub(1))
        .unwrap_or(u64::MAX);
    let base = policy
        .initial_delay_ms
        .saturating_mul(multiplier)
        .min(policy.max_delay_ms);
    let jitter_range = (base / 5).max(1);
    let random = u64::from_le_bytes(*Uuid::new_v4().as_bytes().first_chunk().unwrap());
    let jitter = random % jitter_range;
    let delay = base.saturating_add(jitter).min(policy.max_delay_ms);
    session.emit_status(
        RealtimeConnectionStatus::Reconnecting,
        format!("Reconnecting in {delay} ms"),
    );

    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(delay)) => true,
        command = commands.recv() => {
            if !matches!(command, Some(SessionCommand::Disconnect) | Some(SessionCommand::Close { .. }) | None) {
                session.record(
                    RealtimeTranscriptDirection::System,
                    RealtimeTranscriptKind::Error,
                    "Messages are not queued while reconnecting",
                    None,
                    None,
                ).await;
                true
            } else {
                session.emit_status(RealtimeConnectionStatus::Disconnected, "Disconnected");
                false
            }
        }
    }
}

fn validate_close(code: u16, reason: &str) -> AppResult<()> {
    if !(code == 1000 || (3000..=4999).contains(&code)) {
        return Err(AppError::Message(
            "Close code must be 1000 or between 3000 and 4999.".to_string(),
        ));
    }
    if reason.len() > 123 {
        return Err(AppError::Message(
            "WebSocket close reasons cannot exceed 123 bytes.".to_string(),
        ));
    }
    Ok(())
}

fn manager_error() -> AppError {
    AppError::Message("Failed to access realtime connection state.".to_string())
}

fn session_error() -> AppError {
    AppError::Message("Failed to access realtime session state.".to_string())
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        realtime::{RawWebSocketComposer, RealtimeConnectionCommon},
        requests::RequestAuth,
    };

    use super::*;

    fn limits() -> RealtimeRuntimeLimits {
        RealtimeRuntimeLimits {
            connect_timeout: Duration::from_secs(1),
            max_concurrent_sessions: 2,
            max_message_bytes: 1024,
            transcript_max_entries: 2,
            transcript_max_bytes: 1024,
            validate_tls: true,
        }
    }

    #[test]
    fn validates_close_codes_and_binary_encodings() {
        assert!(validate_close(1000, "").is_ok());
        assert!(validate_close(3999, "bye").is_ok());
        assert!(validate_close(1001, "").is_err());
        assert_eq!(decode_hex("00 ff 10").expect("hex"), vec![0, 255, 16]);
        assert!(decode_hex("0").is_err());
        assert!(ensure_message_size(1024, limits()).is_ok());
        assert!(ensure_message_size(1025, limits()).is_err());
    }

    #[tokio::test]
    async fn rejects_secret_handshake_header_overrides() {
        let error = connect_raw_socket(
            "ws://127.0.0.1:9",
            &[],
            &[crate::domain::requests::KeyValueRow {
                id: "1".to_string(),
                key: "Sec-WebSocket-Key".to_string(),
                value: "override".to_string(),
                enabled: true,
            }],
            &RequestAuth::default(),
            &[],
            limits(),
        )
        .await
        .expect_err("reserved header");
        assert!(error.to_string().contains("managed by PostNot"));
    }

    #[tokio::test]
    async fn outgoing_secret_is_sent_but_redacted_from_transcript_payload() {
        let store = RealtimePayloadStore::new(
            std::env::temp_dir().join(format!("postnot-secret-test-{}", Uuid::new_v4())),
        );
        store.reset().await.expect("reset");
        let composer = RawWebSocketComposer {
            mode: RawMessageMode::Text,
            content: "token=wire-secret".to_string(),
            binary: None,
        };
        let (message, _, payload) = build_raw_message(
            &store,
            &composer,
            limits(),
            &["wire-secret".to_string()],
            false,
        )
        .await
        .expect("build message");
        assert_eq!(message.into_text().expect("text"), "token=wire-secret");
        let RealtimePayload::Inline { text, .. } = payload else {
            panic!("small redacted transcript should be inline");
        };
        assert_eq!(text, "token=***");
    }

    #[tokio::test]
    async fn outgoing_binary_is_preserved_unless_its_source_used_a_secret() {
        let store = RealtimePayloadStore::new(
            std::env::temp_dir().join(format!("postnot-binary-test-{}", Uuid::new_v4())),
        );
        store.reset().await.expect("reset");
        let composer = RawWebSocketComposer {
            mode: RawMessageMode::Binary,
            content: String::new(),
            binary: Some(BinaryPayloadSource::Base64 {
                value: BASE64.encode([1_u8, 2, 3]),
            }),
        };
        let (_, _, payload) = build_raw_message(&store, &composer, limits(), &[], false)
            .await
            .expect("normal binary");
        let RealtimePayload::Inline { text, encoding, .. } = payload else {
            panic!("small binary should be inline");
        };
        assert!(matches!(
            encoding,
            crate::services::realtime_payload_service::RealtimePayloadEncoding::Base64
        ));
        assert_eq!(BASE64.decode(text).expect("decode"), vec![1, 2, 3]);

        let (_, _, payload) = build_raw_message(&store, &composer, limits(), &[], true)
            .await
            .expect("secret binary");
        let RealtimePayload::Inline { text, encoding, .. } = payload else {
            panic!("redaction metadata should be inline");
        };
        assert!(matches!(
            encoding,
            crate::services::realtime_payload_service::RealtimePayloadEncoding::Utf8
        ));
        assert!(text.contains("redacted"));
        assert!(!text.contains(&BASE64.encode([1_u8, 2, 3])));
    }

    #[tokio::test]
    async fn one_entry_larger_than_transcript_cap_keeps_only_bounded_metadata() {
        let store = RealtimePayloadStore::new(
            std::env::temp_dir().join(format!("postnot-cap-test-{}", Uuid::new_v4())),
        );
        store.reset().await.expect("reset");
        let (command_tx, _command_rx) = mpsc::channel(1);
        let session = RuntimeSession {
            session_id: "cap".to_string(),
            protocol: RequestType::Websocket,
            generation: 1,
            event_channel: Channel::new(|_| Ok(())),
            state: Mutex::new(SessionState {
                sequence: 0,
                status: RealtimeConnectionStatus::Connected,
                status_message: "Connected".to_string(),
                transcript: VecDeque::new(),
                transcript_size_bytes: 0,
                has_trim_marker: false,
            }),
            command_tx,
            payloads: store.clone(),
            limits: RealtimeRuntimeLimits {
                transcript_max_bytes: 1,
                ..limits()
            },
            secret_values: Vec::new(),
        };
        let payload = store
            .store_text("oversized".to_string())
            .await
            .expect("store");
        session
            .record(
                RealtimeTranscriptDirection::Received,
                RealtimeTranscriptKind::Text,
                "Received",
                None,
                Some(payload),
            )
            .await;
        let snapshot = session.snapshot().expect("snapshot");
        assert!(snapshot.transcript_size_bytes <= 1);
        assert!(matches!(
            snapshot.transcript[0].payload,
            Some(RealtimePayload::Inline {
                truncated: true,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn raw_manager_round_trips_text_with_local_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let address = listener.local_addr().expect("address");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("handshake");
            if let Some(Ok(message)) = socket.next().await {
                socket.send(message).await.expect("echo");
            }
        });

        let root = std::env::temp_dir().join(format!("postnot-manager-test-{}", Uuid::new_v4()));
        let payloads = RealtimePayloadStore::new(root);
        payloads.reset().await.expect("reset");
        let manager = RealtimeConnectionManager::new(payloads);
        let request = RealtimeConnectionDraft::Websocket {
            common: RealtimeConnectionCommon {
                name: "Echo".to_string(),
                url: format!("ws://{address}"),
                query_params: Vec::new(),
                headers: Vec::new(),
                auth: RequestAuth::default(),
                reconnect: ReconnectPolicy::default(),
            },
            subprotocols: Vec::new(),
        };
        let channel = Channel::new(|_| Ok(()));
        manager
            .connect(
                RealtimeConnectInput {
                    session_id: "echo".to_string(),
                    connection: request.clone(),
                },
                request,
                Vec::new(),
                limits(),
                channel,
            )
            .await
            .expect("connect");

        wait_for_status(&manager, "echo", RealtimeConnectionStatus::Connected).await;
        manager
            .send(
                "echo",
                RealtimeMessageDraft::Websocket {
                    name: "Invalid JSON".to_string(),
                    composer: RawWebSocketComposer {
                        mode: RawMessageMode::Json,
                        content: "{".to_string(),
                        binary: None,
                    },
                },
                Vec::new(),
                false,
            )
            .await
            .expect("queue invalid local payload");
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            manager.snapshot("echo").expect("snapshot").status,
            RealtimeConnectionStatus::Connected,
            "local validation errors must not close a healthy socket"
        );
        manager
            .send(
                "echo",
                RealtimeMessageDraft::Websocket {
                    name: "Hello".to_string(),
                    composer: RawWebSocketComposer {
                        mode: RawMessageMode::Text,
                        content: "hello".to_string(),
                        binary: None,
                    },
                },
                Vec::new(),
                false,
            )
            .await
            .expect("send");

        for _ in 0..100 {
            let snapshot = manager.snapshot("echo").expect("snapshot");
            if snapshot.transcript.iter().any(|entry| {
                matches!(entry.direction, RealtimeTranscriptDirection::Received)
                    && matches!(entry.kind, RealtimeTranscriptKind::Text)
            }) {
                manager.release("echo").await.expect("release");
                server.await.expect("server");
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("echo response was not recorded");
    }

    async fn wait_for_status(
        manager: &RealtimeConnectionManager,
        session_id: &str,
        expected: RealtimeConnectionStatus,
    ) {
        for _ in 0..100 {
            let snapshot = manager.snapshot(session_id).expect("snapshot");
            if snapshot.status == expected {
                return;
            }
            if snapshot.status == RealtimeConnectionStatus::Failed {
                panic!(
                    "session failed before reaching {expected:?}: {}",
                    snapshot.status_message
                );
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("session did not reach {expected:?}");
    }
}
