use std::path::{Path, PathBuf};

use sqlx::{Row, SqlitePool};
use tauri::AppHandle;
use uuid::Uuid;

use crate::{
    domain::{
        history::{HistoryEntryDetail, HistoryEntrySummary},
        requests::{KeyValueRow, ResponseBody, ResponsePayload, SendRequestPayload},
    },
    error::{AppError, AppResult},
    services::{
        response_body_service::{ResponseBodyStore, ResponsePresentation},
        settings_service,
    },
    storage::paths,
};

const PREVIEW_LIMIT: usize = 4_096;
const DEFAULT_HISTORY_LIMIT: u32 = 200;

pub async fn record_success(
    pool: &SqlitePool,
    request: &SendRequestPayload,
    response: &ResponsePayload,
    app: &AppHandle,
    body_store: &ResponseBodyStore,
) -> AppResult<()> {
    let history_id = Uuid::new_v4().to_string();
    let request_snapshot_json = serde_json::to_string(request)?;
    let response_headers_json = serde_json::to_string(&response.headers)?;
    let bodies_dir = paths::response_bodies_dir(app)?;
    tokio::fs::create_dir_all(&bodies_dir).await?;
    let final_path = bodies_dir.join(format!("{history_id}.body"));
    let (response_body_path, response_body_preview, content_type, charset, presentation) =
        match &response.body {
            ResponseBody::Inline {
                text,
                content_type,
                charset,
                presentation,
                ..
            } => {
                tokio::fs::write(&final_path, text.as_bytes()).await?;
                (
                    Some(path_to_string(final_path)),
                    preview(text),
                    content_type.clone(),
                    charset.clone(),
                    *presentation,
                )
            }
            ResponseBody::File {
                handle_id,
                preview_text,
                content_type,
                charset,
                presentation,
                ..
            } => {
                let source = body_store.path_for(handle_id)?;
                ResponseBodyStore::move_file(&source, &final_path).await?;
                body_store.mark_history_owned(handle_id, final_path.clone())?;
                (
                    Some(path_to_string(final_path)),
                    preview(preview_text),
                    content_type.clone(),
                    charset.clone(),
                    *presentation,
                )
            }
        };

    sqlx::query(
        "INSERT INTO history_entries (id, request_name, method, url, request_snapshot_json, status_code, duration_ms, response_headers_json, response_body_path, response_body_preview, error_text, executed_at, response_size_bytes, response_content_type, response_charset, response_presentation) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
    )
    .bind(&history_id)
    .bind(&request.name)
    .bind(&request.method)
    .bind(&request.url)
    .bind(request_snapshot_json)
    .bind(response.status_code.map(i64::from))
    .bind(response.duration_ms as i64)
    .bind(response_headers_json)
    .bind(response_body_path)
    .bind(response_body_preview)
    .bind(&response.error_text)
    .bind(&response.executed_at)
    .bind(i64::try_from(response.size_bytes).unwrap_or(i64::MAX))
    .bind(content_type)
    .bind(charset)
    .bind(presentation_name(presentation))
    .execute(pool)
    .await?;

    prune(pool, Some(body_store)).await
}

pub async fn record_failure(
    pool: &SqlitePool,
    request: &SendRequestPayload,
    error_text: &str,
    body_store: &ResponseBodyStore,
) -> AppResult<()> {
    let request_snapshot_json = serde_json::to_string(request)?;

    sqlx::query(
        "INSERT INTO history_entries (id, request_name, method, url, request_snapshot_json, status_code, duration_ms, response_headers_json, response_body_path, response_body_preview, error_text, executed_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, '[]', NULL, '', ?6, ?7)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&request.name)
    .bind(&request.method)
    .bind(&request.url)
    .bind(request_snapshot_json)
    .bind(error_text)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await?;

    prune(pool, Some(body_store)).await
}

pub async fn list_history(
    pool: &SqlitePool,
    limit: Option<u32>,
) -> AppResult<Vec<HistoryEntrySummary>> {
    let row_limit = i64::from(limit.unwrap_or(50));
    let rows = sqlx::query(
        "SELECT id, request_name, method, url, status_code, duration_ms, response_body_preview, error_text, executed_at FROM history_entries ORDER BY executed_at DESC LIMIT ?1",
    )
    .bind(row_limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| HistoryEntrySummary {
            id: row.get("id"),
            request_name: row.get("request_name"),
            method: row.get("method"),
            url: row.get("url"),
            status_code: row.get("status_code"),
            duration_ms: row.get("duration_ms"),
            response_body_preview: row.get("response_body_preview"),
            error_text: row.get("error_text"),
            executed_at: row.get("executed_at"),
        })
        .collect())
}

pub async fn get_history_entry(
    pool: &SqlitePool,
    body_store: &ResponseBodyStore,
    id: &str,
) -> AppResult<HistoryEntryDetail> {
    let row = sqlx::query(
        "SELECT id, request_name, method, url, request_snapshot_json, status_code, duration_ms, response_headers_json, response_body_path, response_body_preview, error_text, executed_at, response_size_bytes, response_content_type, response_charset, response_presentation FROM history_entries WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("History entry not found.".to_string()))?;

    let request_snapshot: SendRequestPayload =
        serde_json::from_str(&row.get::<String, _>("request_snapshot_json"))?;
    let response_headers: Vec<KeyValueRow> =
        serde_json::from_str(&row.get::<String, _>("response_headers_json"))?;
    let response_body_path: Option<String> = row.get("response_body_path");
    let response_body_preview: String = row.get("response_body_preview");
    let response_body = match response_body_path {
        Some(path) => {
            let path = PathBuf::from(path);
            let size = ResponseBodyStore::file_size(&path)
                .await
                .unwrap_or_else(|_| row.get::<i64, _>("response_size_bytes").max(0) as u64);
            let preview_bytes = ResponseBodyStore::read_preview(&path)
                .await
                .unwrap_or_else(|_| response_body_preview.as_bytes().to_vec());
            body_store
                .register_existing(path, row.get("response_content_type"), &preview_bytes, size)?
                .into()
        }
        None => ResponseBody::Inline {
            text: response_body_preview.clone(),
            size_bytes: response_body_preview.len() as u64,
            content_type: row.get("response_content_type"),
            charset: row.get("response_charset"),
            presentation: presentation_from_name(&row.get::<String, _>("response_presentation")),
        },
    };

    Ok(HistoryEntryDetail {
        id: row.get("id"),
        request_name: row.get("request_name"),
        method: row.get("method"),
        url: row.get("url"),
        status_code: row.get("status_code"),
        duration_ms: row.get("duration_ms"),
        request_snapshot,
        response_headers,
        response_body,
        error_text: row.get("error_text"),
        executed_at: row.get("executed_at"),
    })
}

pub async fn clear_history(pool: &SqlitePool, body_store: &ResponseBodyStore) -> AppResult<()> {
    for path in stored_response_body_paths(pool).await? {
        body_store.delete_path_when_released(&path)?;
    }

    sqlx::query("DELETE FROM history_entries")
        .execute(pool)
        .await?;
    Ok(())
}

async fn prune(pool: &SqlitePool, body_store: Option<&ResponseBodyStore>) -> AppResult<()> {
    let history_limit = settings_service::history_limit(pool)
        .await
        .unwrap_or(DEFAULT_HISTORY_LIMIT);

    let paths_to_delete = sqlx::query(
        "SELECT response_body_path FROM history_entries WHERE id IN (SELECT id FROM history_entries ORDER BY executed_at DESC LIMIT -1 OFFSET ?1)",
    )
    .bind(i64::from(history_limit))
    .fetch_all(pool)
    .await?;

    for row in paths_to_delete {
        let path: Option<String> = row.get("response_body_path");
        if let Some(path) = path {
            if let Some(store) = body_store {
                store.delete_path_when_released(Path::new(&path))?;
            } else {
                delete_response_body_file(Path::new(&path)).await?;
            }
        }
    }

    sqlx::query(
        "DELETE FROM history_entries WHERE id IN (SELECT id FROM history_entries ORDER BY executed_at DESC LIMIT -1 OFFSET ?1)",
    )
    .bind(i64::from(history_limit))
    .execute(pool)
    .await?;

    Ok(())
}

fn preview(body: &str) -> String {
    body.chars().take(PREVIEW_LIMIT).collect()
}

fn presentation_name(value: ResponsePresentation) -> &'static str {
    match value {
        ResponsePresentation::Text => "text",
        ResponsePresentation::Json => "json",
        ResponsePresentation::Image => "image",
        ResponsePresentation::Binary => "binary",
    }
}

fn presentation_from_name(value: &str) -> ResponsePresentation {
    match value {
        "json" => ResponsePresentation::Json,
        "image" => ResponsePresentation::Image,
        "binary" => ResponsePresentation::Binary,
        _ => ResponsePresentation::Text,
    }
}

async fn delete_response_body_file(path: &Path) -> AppResult<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn stored_response_body_paths(pool: &SqlitePool) -> AppResult<Vec<PathBuf>> {
    let rows = sqlx::query(
        "SELECT response_body_path FROM history_entries WHERE response_body_path IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| row.get::<Option<String>, _>("response_body_path"))
        .map(PathBuf::from)
        .collect())
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}
