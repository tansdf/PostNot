use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::FutureExt;
use rust_socketio::{
    asynchronous::{Client, ClientBuilder},
    Payload, TransportType,
};
use url::Url;

use crate::{
    domain::{
        realtime::{
            RealtimeConnectionDraft, RealtimeMessageDraft, SocketIoComposer, SocketIoTransport,
        },
        requests::RequestAuth,
    },
    error::{AppError, AppResult},
    services::{
        realtime_payload_service::RealtimePayload,
        realtime_resolution_service::sanitize_error,
        realtime_service::{
            ensure_message_size, read_binary_source, RealtimeConnectionStatus,
            RealtimeTranscriptDirection, RealtimeTranscriptKind, RuntimeSession, SessionCommand,
        },
    },
};

pub(crate) async fn run_socketio(
    session: Arc<RuntimeSession>,
    request: RealtimeConnectionDraft,
    mut commands: tokio::sync::mpsc::Receiver<SessionCommand>,
) {
    let RealtimeConnectionDraft::Socketio {
        common,
        path,
        namespace,
        auth_payload,
        transport,
        ..
    } = request
    else {
        return;
    };
    let (terminal_tx, mut terminal_rx) = tokio::sync::mpsc::unbounded_channel();
    let _terminal_guard = terminal_tx.clone();
    let (connected_tx, mut connected_rx) = tokio::sync::watch::channel(false);

    let builder = match build_socketio_client(
        &session,
        &common.url,
        &common.query_params,
        &common.headers,
        &common.auth,
        &path,
        &namespace,
        auth_payload,
        transport,
        &common.reconnect,
        terminal_tx.clone(),
        terminal_tx,
        connected_tx,
    ) {
        Ok(builder) => builder,
        Err(error) => {
            session.emit_status(
                RealtimeConnectionStatus::Failed,
                sanitize_error(&error.to_string(), session.secret_values()),
            );
            return;
        }
    };

    let client =
        match tokio::time::timeout(session.limits().connect_timeout, builder.connect()).await {
            Ok(Ok(client)) => client,
            Ok(Err(error)) => {
                session.emit_status(
                    RealtimeConnectionStatus::Failed,
                    sanitize_error(&error.to_string(), session.secret_values()),
                );
                return;
            }
            Err(_) => {
                session.emit_status(
                    RealtimeConnectionStatus::Failed,
                    "Socket.IO connection timed out.",
                );
                return;
            }
        };

    let namespace_connected = tokio::time::timeout(session.limits().connect_timeout, async {
        while !*connected_rx.borrow() {
            connected_rx.changed().await.map_err(|_| ())?;
        }
        Ok::<(), ()>(())
    })
    .await;
    if !matches!(namespace_connected, Ok(Ok(()))) {
        let _ = client.disconnect().await;
        session.emit_status(
            RealtimeConnectionStatus::Failed,
            "Socket.IO namespace connection timed out.",
        );
        return;
    }

    session.emit_status(RealtimeConnectionStatus::Connected, "Connected");
    session
        .record(
            RealtimeTranscriptDirection::System,
            RealtimeTranscriptKind::Lifecycle,
            "Socket.IO connected",
            None,
            None,
        )
        .await;

    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else {
                    break;
                };
                match handle_socketio_command(&session, &client, command).await {
                    Ok(SocketIoCommandOutcome::Continue) => {}
                    Ok(SocketIoCommandOutcome::Disconnected) => return,
                    Err(error) => {
                        session
                            .record(
                                RealtimeTranscriptDirection::System,
                                RealtimeTranscriptKind::Error,
                                sanitize_error(&error.to_string(), session.secret_values()),
                                None,
                                None,
                            )
                            .await;
                    }
                }
            }
            terminal = terminal_rx.recv() => {
                match terminal {
                    Some(SocketIoTerminalEvent::ReconnectExhausted) => {
                        let message = format!(
                            "Socket.IO reconnect attempts exhausted after {} attempts.",
                            common.reconnect.max_attempts
                        );
                        session
                            .record(
                                RealtimeTranscriptDirection::System,
                                RealtimeTranscriptKind::Error,
                                &message,
                                None,
                                None,
                            )
                            .await;
                        session.emit_status(RealtimeConnectionStatus::Failed, message);
                    }
                    Some(SocketIoTerminalEvent::Closed) => {
                        session
                            .record(
                                RealtimeTranscriptDirection::System,
                                RealtimeTranscriptKind::Lifecycle,
                                "Socket.IO connection closed",
                                None,
                                None,
                            )
                            .await;
                        session.emit_status(
                            RealtimeConnectionStatus::Disconnected,
                            "Disconnected",
                        );
                    }
                    None => {}
                }
                return;
            }
        }
    }

    let _ = client.disconnect().await;
    session
        .record(
            RealtimeTranscriptDirection::System,
            RealtimeTranscriptKind::Lifecycle,
            "Socket.IO command channel closed",
            None,
            None,
        )
        .await;
    session.emit_status(RealtimeConnectionStatus::Disconnected, "Disconnected");
}

#[derive(Clone, Copy)]
enum SocketIoTerminalEvent {
    Closed,
    ReconnectExhausted,
}

#[allow(clippy::too_many_arguments)]
fn build_socketio_client(
    session: &Arc<RuntimeSession>,
    url: &str,
    query_params: &[crate::domain::requests::KeyValueRow],
    headers: &[crate::domain::requests::KeyValueRow],
    auth: &RequestAuth,
    path: &str,
    namespace: &str,
    auth_payload: serde_json::Value,
    transport: SocketIoTransport,
    reconnect: &crate::domain::realtime::ReconnectPolicy,
    closed_tx: tokio::sync::mpsc::UnboundedSender<SocketIoTerminalEvent>,
    reconnect_failed_tx: tokio::sync::mpsc::UnboundedSender<SocketIoTerminalEvent>,
    connected_tx: tokio::sync::watch::Sender<bool>,
) -> AppResult<ClientBuilder> {
    let mut url = Url::parse(url)?;
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
        return Err(AppError::Message(
            "Socket.IO URLs must use http://, https://, ws://, or wss://.".to_string(),
        ));
    }
    if transport == SocketIoTransport::Auto {
        let normalized_scheme = match url.scheme() {
            "ws" => Some("http"),
            "wss" => Some("https"),
            _ => None,
        };
        if let Some(scheme) = normalized_scheme {
            url.set_scheme(scheme).map_err(|_| {
                AppError::Message("Could not normalize the Socket.IO URL scheme.".to_string())
            })?;
        }
    }
    let path = path.trim();
    if !path.starts_with('/') || path.chars().any(char::is_whitespace) {
        return Err(AppError::Message(
            "Socket.IO path must start with '/' and contain no whitespace.".to_string(),
        ));
    }
    url.set_path(path);
    let namespace = namespace.trim();
    if !namespace.starts_with('/') || namespace.chars().any(char::is_whitespace) {
        return Err(AppError::Message(
            "Socket.IO namespace must start with '/' and contain no whitespace.".to_string(),
        ));
    }
    if !auth_payload.is_object() {
        return Err(AppError::Message(
            "Socket.IO auth payload must be a JSON object.".to_string(),
        ));
    }
    if reconnect.enabled {
        if !(1..=u8::MAX.into()).contains(&reconnect.max_attempts) {
            return Err(AppError::Message(format!(
                "Socket.IO reconnect attempts must be between 1 and {}.",
                u8::MAX
            )));
        }
        if reconnect.initial_delay_ms == 0 || reconnect.max_delay_ms < reconnect.initial_delay_ms {
            return Err(AppError::Message(
                "Socket.IO reconnect delays must be positive and the maximum must not be shorter than the initial delay."
                    .to_string(),
            ));
        }
    }
    {
        let mut pairs = url.query_pairs_mut();
        for row in query_params
            .iter()
            .filter(|row| row.enabled && !row.key.trim().is_empty())
        {
            if matches!(
                row.key.trim().to_ascii_lowercase().as_str(),
                "eio" | "transport" | "sid"
            ) {
                return Err(AppError::Message(format!(
                    "Socket.IO query key is managed by PostNot: {}.",
                    row.key.trim()
                )));
            }
            pairs.append_pair(row.key.trim(), &row.value);
        }
        if auth.auth_type == "api-key"
            && auth.api_key_in == "query"
            && !auth.api_key_name.trim().is_empty()
        {
            if matches!(
                auth.api_key_name.trim().to_ascii_lowercase().as_str(),
                "eio" | "transport" | "sid"
            ) {
                return Err(AppError::Message(format!(
                    "Socket.IO query key is managed by PostNot: {}.",
                    auth.api_key_name.trim()
                )));
            }
            pairs.append_pair(auth.api_key_name.trim(), &auth.api_key_value);
        }
    }

    let reconnect_session = Arc::clone(session);
    let reconnect_failed_session = Arc::clone(session);
    let incoming_session = Arc::clone(session);
    let error_session = Arc::clone(session);
    let close_session = Arc::clone(session);
    let connect_session = Arc::clone(session);
    let connected_notification = connected_tx;
    let mut builder = ClientBuilder::new(url.to_string())
        .namespace(namespace)
        .auth(auth_payload)
        .transport_type(match transport {
            SocketIoTransport::Auto => TransportType::Any,
            SocketIoTransport::WebsocketOnly => TransportType::Websocket,
        })
        .reconnect(reconnect.enabled)
        .reconnect_on_disconnect(false)
        .reconnect_delay(reconnect.initial_delay_ms, reconnect.max_delay_ms)
        .max_reconnect_attempts(reconnect.max_attempts as u8)
        .on_reconnect(move || {
            let session = Arc::clone(&reconnect_session);
            async move {
                session.emit_status(
                    RealtimeConnectionStatus::Reconnecting,
                    "Socket.IO reconnecting",
                );
                session
                    .record(
                        RealtimeTranscriptDirection::System,
                        RealtimeTranscriptKind::Lifecycle,
                        "Socket.IO reconnecting",
                        None,
                        None,
                    )
                    .await;
                rust_socketio::asynchronous::ReconnectSettings::new()
            }
            .boxed()
        })
        .on_reconnect_failed(move || {
            let session = Arc::clone(&reconnect_failed_session);
            let sender = reconnect_failed_tx.clone();
            async move {
                session.emit_status(
                    RealtimeConnectionStatus::Failed,
                    "Socket.IO reconnect attempts exhausted.",
                );
                let _ = sender.send(SocketIoTerminalEvent::ReconnectExhausted);
            }
            .boxed()
        })
        .on_connection_closed(move || {
            let sender = closed_tx.clone();
            async move {
                let _ = sender.send(SocketIoTerminalEvent::Closed);
            }
            .boxed()
        })
        .on_any(move |event, payload, _| {
            let session = Arc::clone(&incoming_session);
            async move {
                record_socketio_payload(
                    &session,
                    RealtimeTranscriptDirection::Received,
                    RealtimeTranscriptKind::Event,
                    event.to_string(),
                    Some(event.to_string()),
                    payload,
                )
                .await;
            }
            .boxed()
        })
        .on("error", move |payload, _| {
            let session = Arc::clone(&error_session);
            async move {
                record_socketio_payload(
                    &session,
                    RealtimeTranscriptDirection::System,
                    RealtimeTranscriptKind::Error,
                    "Socket.IO error".to_string(),
                    None,
                    payload,
                )
                .await;
            }
            .boxed()
        })
        .on("open", move |_, _| {
            let session = Arc::clone(&connect_session);
            let connected_notification = connected_notification.clone();
            async move {
                let _ = connected_notification.send(true);
                session.emit_status(RealtimeConnectionStatus::Connected, "Connected");
            }
            .boxed()
        })
        .on("close", move |payload, _| {
            let session = Arc::clone(&close_session);
            async move {
                record_socketio_payload(
                    &session,
                    RealtimeTranscriptDirection::System,
                    RealtimeTranscriptKind::Lifecycle,
                    "Socket.IO disconnected".to_string(),
                    None,
                    payload,
                )
                .await;
                session.emit_status(RealtimeConnectionStatus::Disconnected, "Disconnected");
            }
            .boxed()
        });

    for (name, value) in coalesce_headers(headers, auth)? {
        builder = builder.opening_header(name, value);
    }
    if let Some((name, value)) = socketio_auth_header(auth)? {
        builder = builder.opening_header(name, value);
    }
    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(!session.limits().validate_tls)
        .build()
        .map_err(|error| AppError::Message(format!("Could not configure TLS: {error}")))?;
    Ok(builder.tls_config(tls))
}

fn coalesce_headers(
    headers: &[crate::domain::requests::KeyValueRow],
    auth: &RequestAuth,
) -> AppResult<Vec<(String, String)>> {
    let mut positions = HashMap::<String, usize>::new();
    let mut coalesced = Vec::<(String, String)>::new();
    for row in headers
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        validate_header(row.key.trim(), &row.value)?;
        let lower = row.key.trim().to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "host"
                | "connection"
                | "upgrade"
                | "sec-websocket-key"
                | "sec-websocket-version"
                | "sec-websocket-extensions"
                | "sec-websocket-protocol"
        ) {
            return Err(AppError::Message(format!(
                "Socket.IO transport header is managed by PostNot: {}.",
                row.key.trim()
            )));
        }
        if (lower == "authorization" && !matches!(auth.auth_type.as_str(), "" | "none"))
            || (auth.auth_type == "api-key"
                && auth.api_key_in == "header"
                && lower == auth.api_key_name.trim().to_ascii_lowercase())
        {
            return Err(AppError::Message(format!(
                "Header '{}' conflicts with the configured authentication.",
                row.key.trim()
            )));
        }
        if let Some(index) = positions.get(&lower).copied() {
            let separator = if lower == "cookie" { "; " } else { ", " };
            coalesced[index].1.push_str(separator);
            coalesced[index].1.push_str(&row.value);
        } else {
            positions.insert(lower, coalesced.len());
            coalesced.push((row.key.trim().to_string(), row.value.clone()));
        }
    }
    Ok(coalesced)
}

enum SocketIoCommandOutcome {
    Continue,
    Disconnected,
}

async fn handle_socketio_command(
    session: &Arc<RuntimeSession>,
    client: &Client,
    command: SessionCommand,
) -> AppResult<SocketIoCommandOutcome> {
    match command {
        SessionCommand::Send {
            message: RealtimeMessageDraft::Socketio { composer, .. },
            secret_values,
            used_secret,
        } => {
            send_socketio_message(session, client, composer, &secret_values, used_secret).await?;
            Ok(SocketIoCommandOutcome::Continue)
        }
        SessionCommand::Send {
            message: RealtimeMessageDraft::Websocket { .. },
            ..
        } => Err(AppError::Message(
            "A raw WebSocket message cannot be sent through a Socket.IO connection.".to_string(),
        )),
        SessionCommand::Ping(_) => Err(AppError::Message(
            "Socket.IO manages Engine.IO ping and pong frames automatically.".to_string(),
        )),
        SessionCommand::Close { .. } | SessionCommand::Disconnect => {
            session.emit_status(RealtimeConnectionStatus::Disconnecting, "Disconnecting");
            client
                .disconnect()
                .await
                .map_err(|error| AppError::Message(error.to_string()))?;
            session
                .record(
                    RealtimeTranscriptDirection::System,
                    RealtimeTranscriptKind::Lifecycle,
                    "Socket.IO disconnected by user",
                    None,
                    None,
                )
                .await;
            session.emit_status(RealtimeConnectionStatus::Disconnected, "Disconnected");
            Ok(SocketIoCommandOutcome::Disconnected)
        }
    }
}

async fn send_socketio_message(
    session: &Arc<RuntimeSession>,
    client: &Client,
    composer: SocketIoComposer,
    secret_values: &[String],
    binary_used_secret: bool,
) -> AppResult<()> {
    const MAX_ACK_TIMEOUT_MS: u64 = 120_000;

    let event = composer.event.trim().to_string();
    if event.is_empty() {
        return Err(AppError::Message(
            "Socket.IO event name is required.".to_string(),
        ));
    }
    let safe_event = sanitize_error(&event, secret_values);
    let (payload, transcript_payload) = if let Some(source) = composer.binary.as_ref() {
        if composer
            .arguments
            .as_array()
            .is_none_or(|arguments| !arguments.is_empty())
        {
            return Err(AppError::Message(
                "A Socket.IO binary event cannot also contain JSON arguments.".to_string(),
            ));
        }
        let bytes = read_binary_source(source, session.limits()).await?;
        let transcript = if binary_used_secret {
            session
                .payloads()
                .store_text(format!(
                    "Binary event payload redacted ({} bytes)",
                    bytes.len()
                ))
                .await?
        } else {
            session.payloads().store_binary(&bytes).await?
        };
        (Payload::Binary(bytes.into()), transcript)
    } else {
        let arguments = composer.arguments.as_array().ok_or_else(|| {
            AppError::Message("Socket.IO event arguments must be a JSON array.".to_string())
        })?;
        let text = serde_json::to_string(arguments)?;
        ensure_message_size(text.len(), session.limits())?;
        let transcript = session
            .payloads()
            .store_text(sanitize_error(&text, secret_values))
            .await?;
        (Payload::Text(arguments.clone()), transcript)
    };

    if composer.wait_for_ack && !(1..=MAX_ACK_TIMEOUT_MS).contains(&composer.ack_timeout_ms) {
        release_unrecorded_payload(session, &transcript_payload).await;
        return Err(AppError::Message(format!(
            "Socket.IO acknowledgement timeout must be between 1 and {MAX_ACK_TIMEOUT_MS} ms."
        )));
    }

    if composer.wait_for_ack {
        const ACK_PENDING: u8 = 0;
        const ACKED: u8 = 1;
        const ACK_TIMED_OUT: u8 = 2;

        let acknowledgement_state = Arc::new(AtomicU8::new(ACK_PENDING));
        let callback_state = Arc::clone(&acknowledgement_state);
        let callback_session = Arc::clone(session);
        let callback_event = safe_event.clone();
        let emit_result = client
            .emit_with_ack(
                event.clone(),
                payload,
                Duration::from_millis(composer.ack_timeout_ms),
                move |payload, _| {
                    let session = Arc::clone(&callback_session);
                    let event = callback_event.clone();
                    let state = Arc::clone(&callback_state);
                    async move {
                        if state
                            .compare_exchange(
                                ACK_PENDING,
                                ACKED,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            )
                            .is_ok()
                        {
                            record_socketio_payload(
                                &session,
                                RealtimeTranscriptDirection::Received,
                                RealtimeTranscriptKind::Ack,
                                format!("Acknowledged {event}"),
                                Some(event),
                                payload,
                            )
                            .await;
                        }
                    }
                    .boxed()
                },
            )
            .await;
        if let Err(error) = emit_result {
            release_unrecorded_payload(session, &transcript_payload).await;
            return Err(AppError::Message(sanitize_error(
                &error.to_string(),
                secret_values,
            )));
        }

        let timeout_session = Arc::clone(session);
        let timeout_event = safe_event.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(composer.ack_timeout_ms)).await;
            if acknowledgement_state
                .compare_exchange(
                    ACK_PENDING,
                    ACK_TIMED_OUT,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                timeout_session
                    .record(
                        RealtimeTranscriptDirection::System,
                        RealtimeTranscriptKind::Error,
                        format!("Acknowledgement timed out for {timeout_event}"),
                        Some(timeout_event),
                        None,
                    )
                    .await;
            }
        });
    } else {
        if let Err(error) = client.emit(event.clone(), payload).await {
            release_unrecorded_payload(session, &transcript_payload).await;
            return Err(AppError::Message(sanitize_error(
                &error.to_string(),
                secret_values,
            )));
        }
    }
    session
        .record(
            RealtimeTranscriptDirection::Sent,
            RealtimeTranscriptKind::Event,
            format!("Sent {safe_event}"),
            Some(safe_event),
            Some(transcript_payload),
        )
        .await;
    Ok(())
}

async fn release_unrecorded_payload(session: &RuntimeSession, payload: &RealtimePayload) {
    if let Some(handle_id) = payload.handle_id() {
        let _ = session.payloads().release(handle_id).await;
    }
}

async fn record_socketio_payload(
    session: &Arc<RuntimeSession>,
    direction: RealtimeTranscriptDirection,
    kind: RealtimeTranscriptKind,
    label: String,
    event_name: Option<String>,
    payload: Payload,
) {
    #[allow(deprecated)]
    let stored_result = match payload {
        Payload::Text(values) => {
            let text = serde_json::to_string(&values)
                .unwrap_or_else(|_| "[unserializable Socket.IO payload]".to_string());
            if ensure_message_size(text.len(), session.limits()).is_err() {
                session
                    .record(
                        RealtimeTranscriptDirection::System,
                        RealtimeTranscriptKind::Error,
                        format!(
                            "Received Socket.IO event exceeded the configured {} byte limit",
                            session.limits().max_message_bytes
                        ),
                        event_name,
                        None,
                    )
                    .await;
                return;
            }
            session
                .payloads()
                .store_text(sanitize_error(&text, session.secret_values()))
                .await
        }
        Payload::Binary(bytes) => {
            if ensure_message_size(bytes.len(), session.limits()).is_err() {
                session
                    .record(
                        RealtimeTranscriptDirection::System,
                        RealtimeTranscriptKind::Error,
                        format!(
                            "Received Socket.IO binary event exceeded the configured {} byte limit",
                            session.limits().max_message_bytes
                        ),
                        event_name,
                        None,
                    )
                    .await;
                return;
            }
            let contains_secret = session.secret_values().iter().any(|secret| {
                !secret.is_empty()
                    && bytes
                        .windows(secret.len())
                        .any(|window| window == secret.as_bytes())
            });
            if contains_secret {
                session
                    .payloads()
                    .store_text(format!(
                        "Binary Socket.IO payload redacted ({} bytes)",
                        bytes.len()
                    ))
                    .await
            } else {
                session.payloads().store_binary(&bytes).await
            }
        }
        Payload::String(value) => {
            if ensure_message_size(value.len(), session.limits()).is_err() {
                session
                    .record(
                        RealtimeTranscriptDirection::System,
                        RealtimeTranscriptKind::Error,
                        format!(
                            "Received Socket.IO event exceeded the configured {} byte limit",
                            session.limits().max_message_bytes
                        ),
                        event_name,
                        None,
                    )
                    .await;
                return;
            }
            session
                .payloads()
                .store_text(sanitize_error(&value, session.secret_values()))
                .await
        }
    };
    let stored = match stored_result {
        Ok(payload) => payload,
        Err(error) => {
            session
                .record(
                    RealtimeTranscriptDirection::System,
                    RealtimeTranscriptKind::Error,
                    format!("Could not store Socket.IO payload for {label}: {error}"),
                    event_name,
                    None,
                )
                .await;
            return;
        }
    };
    session
        .record(direction, kind, label, event_name, Some(stored))
        .await;
}

fn socketio_auth_header(auth: &RequestAuth) -> AppResult<Option<(String, String)>> {
    Ok(match auth.auth_type.as_str() {
        "" | "none" => None,
        "basic" => Some((
            "authorization".to_string(),
            format!(
                "Basic {}",
                BASE64.encode(format!("{}:{}", auth.basic_username, auth.basic_password))
            ),
        )),
        "bearer" => Some((
            "authorization".to_string(),
            format!("Bearer {}", auth.bearer_token),
        )),
        "oauth2" => Some((
            "authorization".to_string(),
            format!("Bearer {}", auth.oauth2_access_token),
        )),
        "api-key" if auth.api_key_in == "query" => None,
        "api-key" if auth.api_key_in == "header" => {
            validate_header(auth.api_key_name.trim(), &auth.api_key_value)?;
            Some((
                auth.api_key_name.trim().to_string(),
                auth.api_key_value.clone(),
            ))
        }
        other => {
            return Err(AppError::Message(format!(
                "Unsupported realtime authentication type: {other}."
            )))
        }
    })
}

fn validate_header(name: &str, value: &str) -> AppResult<()> {
    tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(name.as_bytes())
        .map_err(|error| AppError::Message(format!("Invalid header name: {error}")))?;
    tokio_tungstenite::tungstenite::http::HeaderValue::from_str(value)
        .map_err(|error| AppError::Message(format!("Invalid header value: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufRead, BufReader},
        process::{Child, Command, Stdio},
    };

    use tauri::ipc::Channel;
    use uuid::Uuid;

    use crate::domain::{
        realtime::{
            BinaryPayloadSource, RealtimeConnectionCommon, RealtimeConnectionDraft,
            RealtimeMessageDraft, ReconnectPolicy, SocketIoComposer,
        },
        requests::RequestAuth,
    };
    use crate::services::{
        realtime_payload_service::{RealtimePayload, RealtimePayloadStore},
        realtime_service::{
            RealtimeConnectInput, RealtimeConnectionManager, RealtimeConnectionStatus,
            RealtimeRuntimeLimits, RealtimeTranscriptKind,
        },
    };

    use super::*;

    #[test]
    fn rejects_reserved_transport_query_keys() {
        let request = RealtimeConnectionCommon {
            name: "Socket".to_string(),
            url: "http://localhost:3000".to_string(),
            query_params: vec![crate::domain::requests::KeyValueRow {
                id: "eio".to_string(),
                key: "EIO".to_string(),
                value: "3".to_string(),
                enabled: true,
            }],
            headers: Vec::new(),
            auth: RequestAuth::default(),
            reconnect: ReconnectPolicy::default(),
        };
        let result = validate_socketio_request_without_session(
            &request.url,
            &request.query_params,
            "/socket.io/",
            "/",
            &serde_json::json!({}),
        );
        assert!(result
            .expect_err("reserved query")
            .to_string()
            .contains("managed by PostNot"));
    }

    #[test]
    fn repeated_opening_headers_are_coalesced_without_silent_loss() {
        let headers = vec![
            crate::domain::requests::KeyValueRow {
                id: "1".to_string(),
                key: "X-Trace".to_string(),
                value: "one".to_string(),
                enabled: true,
            },
            crate::domain::requests::KeyValueRow {
                id: "2".to_string(),
                key: "x-trace".to_string(),
                value: "two".to_string(),
                enabled: true,
            },
            crate::domain::requests::KeyValueRow {
                id: "3".to_string(),
                key: "Cookie".to_string(),
                value: "a=1".to_string(),
                enabled: true,
            },
            crate::domain::requests::KeyValueRow {
                id: "4".to_string(),
                key: "cookie".to_string(),
                value: "b=2".to_string(),
                enabled: true,
            },
        ];
        let coalesced = coalesce_headers(&headers, &RequestAuth::default()).expect("coalesce");
        assert_eq!(
            coalesced,
            vec![
                ("X-Trace".to_string(), "one, two".to_string()),
                ("Cookie".to_string(), "a=1; b=2".to_string())
            ]
        );
    }

    #[tokio::test]
    async fn interoperates_with_socketio_4_auto_and_websocket_only() {
        let mut fixture = SocketIoFixture::start();
        let root = std::env::temp_dir().join(format!("postnot-socketio-test-{}", Uuid::new_v4()));
        let payloads = RealtimePayloadStore::new(root.clone());
        payloads.reset().await.expect("reset");
        let manager = RealtimeConnectionManager::new(payloads);

        exercise_transport(&manager, fixture.port, "auto", SocketIoTransport::Auto).await;
        exercise_transport(
            &manager,
            fixture.port,
            "websocket",
            SocketIoTransport::WebsocketOnly,
        )
        .await;

        fixture.stop();
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn reconnects_after_an_unexpected_socketio_transport_loss() {
        let mut fixture = SocketIoFixture::start();
        let root =
            std::env::temp_dir().join(format!("postnot-socketio-reconnect-{}", Uuid::new_v4()));
        let payloads = RealtimePayloadStore::new(root.clone());
        payloads.reset().await.expect("reset");
        let manager = RealtimeConnectionManager::new(payloads);
        let session_id = "reconnect";
        let request = socketio_fixture_request(
            fixture.port,
            SocketIoTransport::Auto,
            ReconnectPolicy {
                enabled: true,
                max_attempts: 4,
                initial_delay_ms: 10,
                max_delay_ms: 50,
            },
        );
        manager
            .connect(
                RealtimeConnectInput {
                    session_id: session_id.to_string(),
                    connection: request.clone(),
                },
                request,
                Vec::new(),
                fixture_limits(),
                Channel::new(|_| Ok(())),
            )
            .await
            .expect("connect fixture");
        wait_for_status(&manager, session_id, RealtimeConnectionStatus::Connected).await;
        manager
            .send(
                session_id,
                RealtimeMessageDraft::Socketio {
                    name: "Drop transport".to_string(),
                    composer: SocketIoComposer {
                        event: "drop-transport".to_string(),
                        arguments: serde_json::json!([]),
                        binary: None,
                        wait_for_ack: false,
                        ack_timeout_ms: 2_000,
                    },
                },
                Vec::new(),
                false,
            )
            .await
            .expect("force transport loss");

        for _ in 0..500 {
            let snapshot = manager.snapshot(session_id).expect("snapshot");
            let connection_meta_events = snapshot
                .transcript
                .iter()
                .filter(|entry| entry.event_name.as_deref() == Some("connection-meta"))
                .count();
            let saw_reconnect = snapshot.transcript.iter().any(|entry| {
                entry.kind == RealtimeTranscriptKind::Lifecycle && entry.label.contains("reconnect")
            }) || snapshot.status == RealtimeConnectionStatus::Reconnecting;
            if connection_meta_events >= 2
                && saw_reconnect
                && snapshot.status == RealtimeConnectionStatus::Connected
            {
                manager.release(session_id).await.expect("release");
                fixture.stop();
                let _ = tokio::fs::remove_dir_all(root).await;
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let snapshot = manager.snapshot(session_id).expect("final snapshot");
        panic!(
            "Socket.IO fixture did not reconnect after transport loss: {}",
            serde_json::to_string(&snapshot).expect("snapshot JSON")
        );
    }

    #[tokio::test]
    async fn reconnect_exhaustion_transitions_the_session_to_failed() {
        let mut fixture = SocketIoFixture::start();
        let root =
            std::env::temp_dir().join(format!("postnot-socketio-exhaustion-{}", Uuid::new_v4()));
        let payloads = RealtimePayloadStore::new(root.clone());
        payloads.reset().await.expect("reset");
        let manager = RealtimeConnectionManager::new(payloads);
        let session_id = "reconnect-exhaustion";
        let request = socketio_fixture_request(
            fixture.port,
            SocketIoTransport::Auto,
            ReconnectPolicy {
                enabled: true,
                max_attempts: 2,
                initial_delay_ms: 10,
                max_delay_ms: 20,
            },
        );
        manager
            .connect(
                RealtimeConnectInput {
                    session_id: session_id.to_string(),
                    connection: request.clone(),
                },
                request,
                Vec::new(),
                fixture_limits(),
                Channel::new(|_| Ok(())),
            )
            .await
            .expect("connect fixture");
        wait_for_status(&manager, session_id, RealtimeConnectionStatus::Connected).await;
        manager
            .send(
                session_id,
                RealtimeMessageDraft::Socketio {
                    name: "Drop server".to_string(),
                    composer: SocketIoComposer {
                        event: "drop-server".to_string(),
                        arguments: serde_json::json!([]),
                        binary: None,
                        wait_for_ack: false,
                        ack_timeout_ms: 2_000,
                    },
                },
                Vec::new(),
                false,
            )
            .await
            .expect("stop fixture transport");

        for _ in 0..500 {
            let snapshot = manager.snapshot(session_id).expect("snapshot");
            if snapshot.status == RealtimeConnectionStatus::Failed {
                assert!(snapshot
                    .transcript
                    .iter()
                    .any(|entry| entry.label.contains("attempts exhausted")));
                manager.release(session_id).await.expect("release");
                fixture.stop();
                let _ = tokio::fs::remove_dir_all(root).await;
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let snapshot = manager.snapshot(session_id).expect("final snapshot");
        panic!(
            "Socket.IO reconnect exhaustion did not terminate the session: {}",
            serde_json::to_string(&snapshot).expect("snapshot JSON")
        );
    }

    #[tokio::test]
    async fn transport_loss_without_reconnect_transitions_to_disconnected() {
        let mut fixture = SocketIoFixture::start();
        let root =
            std::env::temp_dir().join(format!("postnot-socketio-no-reconnect-{}", Uuid::new_v4()));
        let payloads = RealtimePayloadStore::new(root.clone());
        payloads.reset().await.expect("reset");
        let manager = RealtimeConnectionManager::new(payloads);
        let session_id = "no-reconnect";
        let request = socketio_fixture_request(
            fixture.port,
            SocketIoTransport::Auto,
            ReconnectPolicy::default(),
        );
        manager
            .connect(
                RealtimeConnectInput {
                    session_id: session_id.to_string(),
                    connection: request.clone(),
                },
                request,
                Vec::new(),
                fixture_limits(),
                Channel::new(|_| Ok(())),
            )
            .await
            .expect("connect fixture");
        wait_for_status(&manager, session_id, RealtimeConnectionStatus::Connected).await;
        manager
            .send(
                session_id,
                RealtimeMessageDraft::Socketio {
                    name: "Drop server".to_string(),
                    composer: SocketIoComposer {
                        event: "drop-server".to_string(),
                        arguments: serde_json::json!([]),
                        binary: None,
                        wait_for_ack: false,
                        ack_timeout_ms: 2_000,
                    },
                },
                Vec::new(),
                false,
            )
            .await
            .expect("stop fixture transport");

        wait_for_status(
            &manager,
            session_id,
            RealtimeConnectionStatus::Disconnected,
        )
        .await;
        let snapshot = manager.snapshot(session_id).expect("snapshot");
        assert!(snapshot
            .transcript
            .iter()
            .any(|entry| entry.label == "Socket.IO connection closed"));
        manager.release(session_id).await.expect("release");
        fixture.stop();
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    async fn exercise_transport(
        manager: &RealtimeConnectionManager,
        port: u16,
        session_id: &str,
        transport: SocketIoTransport,
    ) {
        let request = socketio_fixture_request(port, transport, ReconnectPolicy::default());
        manager
            .connect(
                RealtimeConnectInput {
                    session_id: session_id.to_string(),
                    connection: request.clone(),
                },
                request,
                Vec::new(),
                fixture_limits(),
                Channel::new(|_| Ok(())),
            )
            .await
            .expect("connect fixture");
        wait_for_status(manager, session_id, RealtimeConnectionStatus::Connected).await;
        manager
            .send(
                session_id,
                RealtimeMessageDraft::Socketio {
                    name: "Echo".to_string(),
                    composer: SocketIoComposer {
                        event: "echo".to_string(),
                        arguments: serde_json::json!([{"hello": "world"}]),
                        binary: None,
                        wait_for_ack: true,
                        ack_timeout_ms: 2_000,
                    },
                },
                Vec::new(),
                false,
            )
            .await
            .expect("send fixture event");
        manager
            .send(
                session_id,
                RealtimeMessageDraft::Socketio {
                    name: "Binary echo".to_string(),
                    composer: SocketIoComposer {
                        event: "binary-echo".to_string(),
                        arguments: serde_json::json!([]),
                        binary: Some(BinaryPayloadSource::Base64 {
                            value: BASE64.encode([0_u8, 1, 2, 255]),
                        }),
                        wait_for_ack: true,
                        ack_timeout_ms: 2_000,
                    },
                },
                Vec::new(),
                false,
            )
            .await
            .expect("send fixture binary event");

        for _ in 0..300 {
            let snapshot = manager.snapshot(session_id).expect("snapshot");
            let has_json_ack = snapshot.transcript.iter().any(|entry| {
                entry.kind == RealtimeTranscriptKind::Ack
                    && entry.event_name.as_deref() == Some("echo")
            });
            let has_binary_ack = snapshot.transcript.iter().any(|entry| {
                entry.kind == RealtimeTranscriptKind::Ack
                    && entry.event_name.as_deref() == Some("binary-echo")
                    && entry.payload.as_ref().is_some_and(|payload| {
                        matches!(
                            payload,
                            RealtimePayload::Inline {
                                encoding: crate::services::realtime_payload_service::RealtimePayloadEncoding::Base64,
                                text,
                                ..
                            } if BASE64.decode(text).ok().as_deref() == Some(&[0_u8, 1, 2, 255])
                        )
                    })
            });
            let meta = snapshot
                .transcript
                .iter()
                .find(|entry| entry.event_name.as_deref() == Some("connection-meta"));
            if has_json_ack && has_binary_ack && meta.is_some_and(meta_contains_fixture_values) {
                manager.release(session_id).await.expect("release");
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let snapshot = manager.snapshot(session_id).expect("final snapshot");
        panic!(
            "Socket.IO fixture did not produce metadata and acknowledgement: {}",
            serde_json::to_string(&snapshot).expect("snapshot JSON")
        );
    }

    fn socketio_fixture_request(
        port: u16,
        transport: SocketIoTransport,
        reconnect: ReconnectPolicy,
    ) -> RealtimeConnectionDraft {
        RealtimeConnectionDraft::Socketio {
            common: RealtimeConnectionCommon {
                name: "Socket.IO fixture".to_string(),
                url: format!("http://127.0.0.1:{port}"),
                query_params: vec![crate::domain::requests::KeyValueRow {
                    id: "fixture".to_string(),
                    key: "fixture".to_string(),
                    value: "query-ok".to_string(),
                    enabled: true,
                }],
                headers: vec![crate::domain::requests::KeyValueRow {
                    id: "header".to_string(),
                    key: "X-PostNot-Fixture".to_string(),
                    value: "header-ok".to_string(),
                    enabled: true,
                }],
                auth: RequestAuth::default(),
                reconnect,
            },
            path: "/custom-socket/".to_string(),
            namespace: "/admin".to_string(),
            auth_payload: serde_json::json!({"token": "auth-ok"}),
            transport,
        }
    }

    fn fixture_limits() -> RealtimeRuntimeLimits {
        RealtimeRuntimeLimits {
            connect_timeout: Duration::from_secs(5),
            max_concurrent_sessions: 4,
            max_message_bytes: 1024 * 1024,
            transcript_max_entries: 100,
            transcript_max_bytes: 1024 * 1024,
            validate_tls: true,
        }
    }

    fn meta_contains_fixture_values(
        entry: &crate::services::realtime_service::RealtimeTranscriptEntry,
    ) -> bool {
        let Some(RealtimePayload::Inline { text, .. }) = entry.payload.as_ref() else {
            return false;
        };
        text.contains("auth-ok") && text.contains("query-ok") && text.contains("header-ok")
    }

    async fn wait_for_status(
        manager: &RealtimeConnectionManager,
        session_id: &str,
        expected: RealtimeConnectionStatus,
    ) {
        for _ in 0..300 {
            let snapshot = manager.snapshot(session_id).expect("snapshot");
            if snapshot.status == expected {
                return;
            }
            if snapshot.status == RealtimeConnectionStatus::Failed {
                panic!("Socket.IO connection failed: {}", snapshot.status_message);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("Socket.IO connection did not reach {expected:?}");
    }

    struct SocketIoFixture {
        child: Option<Child>,
        port: u16,
    }

    impl SocketIoFixture {
        fn start() -> Self {
            let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/socketio-server.mjs");
            let mut child = Command::new("node")
                .arg(fixture)
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("start Socket.IO fixture");
            let stdout = child.stdout.take().expect("fixture stdout");
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            reader.read_line(&mut line).expect("fixture ready line");
            let ready: serde_json::Value = serde_json::from_str(&line).expect("fixture ready JSON");
            Self {
                child: Some(child),
                port: ready["port"].as_u64().expect("fixture port") as u16,
            }
        }

        fn stop(&mut self) {
            if let Some(mut child) = self.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    impl Drop for SocketIoFixture {
        fn drop(&mut self) {
            self.stop();
        }
    }

    fn validate_socketio_request_without_session(
        url: &str,
        query_params: &[crate::domain::requests::KeyValueRow],
        path: &str,
        namespace: &str,
        auth_payload: &serde_json::Value,
    ) -> AppResult<()> {
        let url = Url::parse(url)?;
        if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
            return Err(AppError::Message("invalid scheme".to_string()));
        }
        if !path.starts_with('/') || !namespace.starts_with('/') || !auth_payload.is_object() {
            return Err(AppError::Message("invalid Socket.IO options".to_string()));
        }
        for row in query_params.iter().filter(|row| row.enabled) {
            if matches!(
                row.key.trim().to_ascii_lowercase().as_str(),
                "eio" | "transport" | "sid"
            ) {
                return Err(AppError::Message(format!(
                    "Socket.IO query key is managed by PostNot: {}.",
                    row.key
                )));
            }
        }
        Ok(())
    }
}
