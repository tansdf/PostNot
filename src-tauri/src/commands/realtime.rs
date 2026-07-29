use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD as BASE64, write::EncoderWriter};
use tauri::{ipc::Channel, State};

use crate::{
    app_state::AppState,
    domain::exports::ExportResult,
    error::AppResult,
    services::{
        environments_service,
        realtime_payload_service::RealtimePayload,
        realtime_resolution_service,
        realtime_service::{
            RealtimeConnectInput, RealtimeRuntimeEvent, RealtimeRuntimeLimits, RealtimeSendMessage,
            RealtimeSessionSnapshot,
        },
        settings_service,
    },
};

#[tauri::command]
pub async fn get_realtime_workspace_state(
    state: State<'_, AppState>,
) -> AppResult<Option<serde_json::Value>> {
    settings_service::get_realtime_workspace_state(state.db()).await
}

#[tauri::command]
pub async fn save_realtime_workspace_state(
    app_state: State<'_, AppState>,
    state: serde_json::Value,
) -> AppResult<()> {
    settings_service::save_realtime_workspace_state(app_state.db(), &state).await
}

#[tauri::command]
pub async fn connect_realtime_connection(
    state: State<'_, AppState>,
    input: RealtimeConnectInput,
    on_event: Channel<RealtimeRuntimeEvent>,
) -> AppResult<RealtimeSessionSnapshot> {
    let active_environment =
        environments_service::get_active_environment(state.db(), state.secret_store()).await?;
    let resolved =
        realtime_resolution_service::resolve_request(&input.request, active_environment.as_ref());
    let settings = settings_service::get_settings(state.db()).await?;
    let limits = RealtimeRuntimeLimits {
        connect_timeout: Duration::from_millis(settings.realtime_connect_timeout_ms),
        max_concurrent_sessions: settings.realtime_max_concurrent_sessions as usize,
        max_message_bytes: usize::try_from(settings.realtime_max_message_bytes)
            .unwrap_or(usize::MAX),
        transcript_max_entries: settings.realtime_transcript_max_entries as usize,
        transcript_max_bytes: settings.realtime_transcript_max_bytes,
        validate_tls: settings.validate_tls,
    };
    state
        .realtime_connections()
        .connect(
            input,
            resolved.request,
            resolved.secret_values,
            limits,
            on_event,
        )
        .await
}

#[tauri::command]
pub async fn disconnect_realtime_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> AppResult<()> {
    state
        .realtime_connections()
        .disconnect(&connection_id)
        .await
}

#[tauri::command]
pub async fn release_realtime_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> AppResult<()> {
    state.realtime_connections().release(&connection_id).await
}

#[tauri::command]
pub async fn send_realtime_message(
    state: State<'_, AppState>,
    connection_id: String,
    message: RealtimeSendMessage,
) -> AppResult<()> {
    let active_environment =
        environments_service::get_active_environment(state.db(), state.secret_store()).await?;
    let secret_values =
        environments_service::active_environment_secret_values(active_environment.as_ref());
    let message = match message {
        RealtimeSendMessage::Websocket { composer } => RealtimeSendMessage::Websocket {
            composer: realtime_resolution_service::resolve_raw_composer(
                &composer,
                active_environment.as_ref(),
            ),
        },
        RealtimeSendMessage::Socketio { composer } => RealtimeSendMessage::Socketio {
            composer: realtime_resolution_service::resolve_socketio_composer(
                &composer,
                active_environment.as_ref(),
            ),
        },
    };
    state
        .realtime_connections()
        .send(&connection_id, message, secret_values)
        .await
}

#[tauri::command]
pub async fn ping_realtime_connection(
    state: State<'_, AppState>,
    connection_id: String,
    payload: Option<String>,
) -> AppResult<()> {
    state
        .realtime_connections()
        .ping(&connection_id, payload)
        .await
}

#[tauri::command]
pub async fn close_realtime_connection(
    state: State<'_, AppState>,
    connection_id: String,
    code: u16,
    reason: String,
) -> AppResult<()> {
    state
        .realtime_connections()
        .close(&connection_id, code, reason)
        .await
}

#[tauri::command]
pub fn get_realtime_session_snapshot(
    state: State<'_, AppState>,
    connection_id: String,
) -> AppResult<RealtimeSessionSnapshot> {
    state.realtime_connections().snapshot(&connection_id)
}

#[tauri::command]
pub async fn clear_realtime_transcript(
    state: State<'_, AppState>,
    connection_id: String,
) -> AppResult<()> {
    state
        .realtime_connections()
        .clear_transcript(&connection_id)
        .await
}

#[tauri::command]
pub async fn read_realtime_payload(
    state: State<'_, AppState>,
    handle_id: String,
) -> AppResult<String> {
    state
        .realtime_connections()
        .payloads()
        .read(&handle_id)
        .await
}

#[tauri::command]
pub async fn save_realtime_payload(
    state: State<'_, AppState>,
    handle_id: String,
    suggested_name: Option<String>,
) -> AppResult<Option<String>> {
    let suggested_name = suggested_name
        .filter(|name| !name.trim().is_empty())
        .map(|name| sanitize_suggested_file_name(&name))
        .unwrap_or_else(|| "realtime-payload.bin".to_string());
    let path = tauri::async_runtime::spawn_blocking(move || {
        rfd::FileDialog::new()
            .set_title("Save realtime payload")
            .set_file_name(&suggested_name)
            .save_file()
    })
    .await?;
    let Some(path) = path else {
        return Ok(None);
    };
    state
        .realtime_connections()
        .payloads()
        .copy_to(&handle_id, &path)
        .await?;
    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn export_realtime_transcript(
    state: State<'_, AppState>,
    connection_id: String,
) -> AppResult<Option<ExportResult>> {
    let (snapshot, retained_handles) = state
        .realtime_connections()
        .snapshot_for_export(&connection_id)?;
    let suggested_name = format!(
        "{}-transcript.json",
        sanitize_file_stem(&connection_id, "realtime")
    );
    let path = match choose_transcript_path(suggested_name).await {
        Ok(path) => path,
        Err(error) => {
            for handle in retained_handles {
                let _ = state
                    .realtime_connections()
                    .payloads()
                    .release(&handle)
                    .await;
            }
            return Err(error);
        }
    };
    let Some(path) = path else {
        for handle in retained_handles {
            state
                .realtime_connections()
                .payloads()
                .release(&handle)
                .await?;
        }
        return Ok(None);
    };
    let payloads = state.realtime_connections().payloads().clone();
    let export_path = path.clone();
    let export_result = tauri::async_runtime::spawn_blocking(move || {
        write_transcript_export(&export_path, &snapshot, &payloads)
    })
    .await;
    for handle in retained_handles {
        state
            .realtime_connections()
            .payloads()
            .release(&handle)
            .await?;
    }
    export_result??;
    Ok(Some(ExportResult {
        file_path: path.to_string_lossy().to_string(),
    }))
}

async fn choose_transcript_path(suggested_name: String) -> AppResult<Option<std::path::PathBuf>> {
    let path = tauri::async_runtime::spawn_blocking(move || {
        rfd::FileDialog::new()
            .set_title("Export realtime transcript")
            .set_file_name(&suggested_name)
            .add_filter("JSON", &["json"])
            .save_file()
    })
    .await?;
    Ok(path)
}

fn sanitize_file_stem(input: &str, fallback: &str) -> String {
    let sanitized = input
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        fallback.to_string()
    } else {
        sanitized
    }
}

fn sanitize_suggested_file_name(input: &str) -> String {
    let file_name = input
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("realtime-payload.bin");
    let (stem, extension) = file_name
        .rsplit_once('.')
        .filter(|(_, extension)| {
            !extension.is_empty()
                && extension.len() <= 16
                && extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .unwrap_or((file_name, "bin"));
    format!(
        "{}.{}",
        sanitize_file_stem(stem, "realtime-payload"),
        extension
    )
}

fn write_transcript_export(
    path: &Path,
    snapshot: &RealtimeSessionSnapshot,
    payloads: &crate::services::realtime_payload_service::RealtimePayloadStore,
) -> AppResult<()> {
    let mut writer = BufWriter::new(File::create(path)?);
    write!(writer, "{{\"connectionId\":")?;
    serde_json::to_writer(&mut writer, &snapshot.connection_id)?;
    write!(
        writer,
        ",\"generation\":{},\"lastSequence\":{},\"status\":",
        snapshot.generation, snapshot.last_sequence
    )?;
    serde_json::to_writer(&mut writer, &snapshot.status)?;
    write!(writer, ",\"statusMessage\":")?;
    serde_json::to_writer(&mut writer, &snapshot.status_message)?;
    write!(
        writer,
        ",\"transcriptSizeBytes\":{},\"transcript\":[",
        snapshot.transcript_size_bytes
    )?;
    for (index, entry) in snapshot.transcript.iter().enumerate() {
        if index > 0 {
            writer.write_all(b",")?;
        }
        match entry.payload.as_ref() {
            Some(RealtimePayload::File {
                handle_id,
                size_bytes,
                encoding,
                ..
            }) => {
                let (source, stored_encoding) = payloads.export_source(handle_id)?;
                write_export_entry_prefix(&mut writer, entry)?;
                write!(writer, "{{\"mode\":\"inline\",\"text\":")?;
                match stored_encoding {
                    crate::services::realtime_payload_service::RealtimePayloadEncoding::Utf8 => {
                        write_json_string_file(&mut writer, &source)?;
                    }
                    crate::services::realtime_payload_service::RealtimePayloadEncoding::Base64 => {
                        writer.write_all(b"\"")?;
                        let mut source = BufReader::new(File::open(source)?);
                        {
                            let mut encoder = EncoderWriter::new(&mut writer, &BASE64);
                            std::io::copy(&mut source, &mut encoder)?;
                            let _ = encoder.finish()?;
                        }
                        writer.write_all(b"\"")?;
                    }
                }
                write!(writer, ",\"sizeBytes\":{},\"encoding\":", size_bytes)?;
                serde_json::to_writer(&mut writer, encoding)?;
                writer.write_all(b",\"truncated\":false}}")?;
            }
            _ => serde_json::to_writer(&mut writer, entry)?,
        }
    }
    writer.write_all(b"]}")?;
    writer.flush()?;
    Ok(())
}

fn write_export_entry_prefix(
    writer: &mut impl Write,
    entry: &crate::services::realtime_service::RealtimeTranscriptEntry,
) -> AppResult<()> {
    writer.write_all(b"{\"id\":")?;
    serde_json::to_writer(&mut *writer, &entry.id)?;
    writer.write_all(b",\"connectionId\":")?;
    serde_json::to_writer(&mut *writer, &entry.connection_id)?;
    write!(
        writer,
        ",\"generation\":{},\"sequence\":{},\"occurredAt\":",
        entry.generation, entry.sequence
    )?;
    serde_json::to_writer(&mut *writer, &entry.occurred_at)?;
    writer.write_all(b",\"direction\":")?;
    serde_json::to_writer(&mut *writer, &entry.direction)?;
    writer.write_all(b",\"kind\":")?;
    serde_json::to_writer(&mut *writer, &entry.kind)?;
    writer.write_all(b",\"label\":")?;
    serde_json::to_writer(&mut *writer, &entry.label)?;
    writer.write_all(b",\"eventName\":")?;
    serde_json::to_writer(&mut *writer, &entry.event_name)?;
    writer.write_all(b",\"payload\":")?;
    Ok(())
}

fn write_json_string_file(writer: &mut impl Write, path: &Path) -> AppResult<()> {
    writer.write_all(b"\"")?;
    let mut source = BufReader::new(File::open(path)?);
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            match *byte {
                b'"' => writer.write_all(br#"\""#)?,
                b'\\' => writer.write_all(br#"\\"#)?,
                b'\n' => writer.write_all(br#"\n"#)?,
                b'\r' => writer.write_all(br#"\r"#)?,
                b'\t' => writer.write_all(br#"\t"#)?,
                0x00..=0x1f => write!(writer, "\\u{:04x}", byte)?,
                byte => writer.write_all(&[byte])?,
            }
        }
    }
    writer.write_all(b"\"")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use uuid::Uuid;

    use crate::services::{
        realtime_payload_service::{RealtimePayloadStore, REALTIME_INLINE_PAYLOAD_LIMIT},
        realtime_service::{
            RealtimeConnectionStatus, RealtimeSessionSnapshot, RealtimeTranscriptDirection,
            RealtimeTranscriptEntry, RealtimeTranscriptKind,
        },
    };

    use super::*;

    #[tokio::test]
    async fn large_file_backed_payload_export_streams_complete_base64() {
        let root = std::env::temp_dir().join(format!("postnot-export-test-{}", Uuid::new_v4()));
        let store = RealtimePayloadStore::new(root.clone());
        store.reset().await.expect("reset");
        let bytes = vec![0x5a; REALTIME_INLINE_PAYLOAD_LIMIT + 1];
        let payload = store.store_binary(&bytes).await.expect("store");
        assert!(matches!(payload, RealtimePayload::File { .. }));
        let snapshot = RealtimeSessionSnapshot {
            connection_id: "large-export".to_string(),
            generation: 2,
            last_sequence: 4,
            status: RealtimeConnectionStatus::Disconnected,
            status_message: "Disconnected".to_string(),
            transcript_size_bytes: bytes.len() as u64,
            transcript: vec![RealtimeTranscriptEntry {
                id: "entry".to_string(),
                connection_id: "large-export".to_string(),
                generation: 2,
                sequence: 4,
                occurred_at: "2026-01-01T00:00:00Z".to_string(),
                direction: RealtimeTranscriptDirection::Received,
                kind: RealtimeTranscriptKind::Binary,
                label: "Received binary".to_string(),
                event_name: None,
                payload: Some(payload),
            }],
        };
        let destination = root.join("export.json");
        write_transcript_export(&destination, &snapshot, &store).expect("export");
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&destination).expect("read export"))
                .expect("valid JSON");
        let encoded = json["transcript"][0]["payload"]["text"]
            .as_str()
            .expect("base64 text");
        assert_eq!(BASE64.decode(encoded).expect("decode"), bytes);
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[test]
    fn suggested_payload_names_drop_paths_and_control_characters() {
        assert_eq!(
            sanitize_suggested_file_name("../bad/name\u{0}.json"),
            "name.json"
        );
        assert_eq!(
            sanitize_suggested_file_name("payload.very-long-invalid-extension"),
            "payload-very-long-invalid-extension.bin"
        );
    }
}
