use std::path::{Path, PathBuf};

use sqlx::{Row, SqlitePool};
use tauri::AppHandle;
use uuid::Uuid;

use crate::{
    domain::{
        history::{HistoryEntryDetail, HistoryEntrySummary},
        requests::{KeyValueRow, ResponseBody, ResponsePayload, SendRequestPayload},
        storage::HistoryRetentionResult,
    },
    error::{AppError, AppResult},
    services::{
        response_body_service::{ResponseBodyStore, ResponsePresentation},
        settings_service,
    },
    storage::paths,
};

const PREVIEW_LIMIT: usize = 4_096;
#[derive(Debug)]
struct HistoryRetentionCandidate {
    id: String,
    response_body_path: Option<PathBuf>,
    executed_at: String,
}

pub async fn record_success(
    pool: &SqlitePool,
    request: &SendRequestPayload,
    response: &ResponsePayload,
    app: &AppHandle,
    body_store: &ResponseBodyStore,
) -> AppResult<()> {
    let bodies_dir = paths::response_bodies_dir(app)?;
    record_success_in_dir(pool, request, response, &bodies_dir, body_store).await
}

pub(crate) async fn record_success_in_dir(
    pool: &SqlitePool,
    request: &SendRequestPayload,
    response: &ResponsePayload,
    bodies_dir: &Path,
    body_store: &ResponseBodyStore,
) -> AppResult<()> {
    let history_id = Uuid::new_v4().to_string();
    let request_snapshot_json = serde_json::to_string(request)?;
    let response_headers_json = serde_json::to_string(&response.headers)?;
    tokio::fs::create_dir_all(bodies_dir).await?;
    let final_path = bodies_dir.join(format!("{history_id}.body"));
    let (response_body_preview, content_type, charset, presentation, handle_id) =
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
                    preview(text),
                    content_type.clone(),
                    charset.clone(),
                    *presentation,
                    None,
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
                body_store.copy_to(handle_id, &final_path).await?;
                (
                    preview(preview_text),
                    content_type.clone(),
                    charset.clone(),
                    *presentation,
                    Some(handle_id.as_str()),
                )
            }
        };

    let result = sqlx::query(
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
    .bind(path_to_string(final_path.clone()))
    .bind(response_body_preview)
    .bind(&response.error_text)
    .bind(&response.executed_at)
    .bind(i64::try_from(response.size_bytes).unwrap_or(i64::MAX))
    .bind(content_type)
    .bind(charset)
    .bind(presentation_name(presentation))
    .execute(pool)
    .await;

    if let Err(error) = result {
        delete_response_body_file(&final_path).await?;
        return Err(error.into());
    }
    if let Some(handle_id) = handle_id {
        body_store.mark_history_owned(handle_id, final_path)?;
    }

    prune(pool, Some(body_store)).await.map(|_| ())
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

    prune(pool, Some(body_store)).await.map(|_| ())
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
    let paths = delete_history_entries(
        pool,
        "DELETE FROM history_entries WHERE id IN (SELECT id FROM history_entries LIMIT -1 OFFSET ?1) RETURNING response_body_path",
        0,
    )
    .await?;
    delete_committed_paths(paths, Some(body_store)).await
}

pub async fn apply_history_retention(
    pool: &SqlitePool,
    body_store: &ResponseBodyStore,
) -> AppResult<HistoryRetentionResult> {
    prune(pool, Some(body_store)).await
}

async fn prune(
    pool: &SqlitePool,
    body_store: Option<&ResponseBodyStore>,
) -> AppResult<HistoryRetentionResult> {
    let (history_limit, retention_days, storage_limit_bytes) =
        settings_service::history_retention(pool).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, response_body_path, executed_at
        FROM history_entries
        ORDER BY executed_at DESC, id DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    let candidates = rows
        .into_iter()
        .map(|row| HistoryRetentionCandidate {
            id: row.get("id"),
            response_body_path: row
                .get::<Option<String>, _>("response_body_path")
                .map(PathBuf::from),
            executed_at: row.get("executed_at"),
        })
        .collect::<Vec<_>>();
    let cutoff = (retention_days > 0)
        .then(|| chrono::Utc::now() - chrono::Duration::days(i64::from(retention_days)));
    let mut retained_body_bytes = 0_u64;
    let mut removals = Vec::new();

    for (index, candidate) in candidates.into_iter().enumerate() {
        let body_bytes =
            response_body_storage_bytes(candidate.response_body_path.as_deref()).await?;
        let exceeds_count = index >= history_limit as usize;
        let exceeds_age = cutoff.is_some_and(|cutoff| {
            chrono::DateTime::parse_from_rfc3339(&candidate.executed_at)
                .map(|executed_at| executed_at < cutoff)
                .unwrap_or(false)
        });
        let exceeds_storage = storage_limit_bytes > 0
            && retained_body_bytes.saturating_add(body_bytes) > storage_limit_bytes;
        if exceeds_count || exceeds_age || exceeds_storage {
            removals.push((candidate.id, candidate.response_body_path, body_bytes));
        } else {
            retained_body_bytes = retained_body_bytes.saturating_add(body_bytes);
        }
    }

    if removals.is_empty() {
        return Ok(HistoryRetentionResult::default());
    }
    let mut transaction = pool.begin().await?;
    for (id, _, _) in &removals {
        sqlx::query("DELETE FROM history_entries WHERE id = ?1")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;

    let released_response_body_bytes = removals
        .iter()
        .map(|(_, _, bytes)| bytes)
        .fold(0_u64, |total, bytes| total.saturating_add(*bytes));
    let paths = removals
        .iter()
        .filter_map(|(_, path, _)| path.clone())
        .collect::<Vec<_>>();
    delete_committed_paths(paths, body_store).await?;

    Ok(HistoryRetentionResult {
        removed_entry_count: removals.len() as u64,
        released_response_body_bytes,
    })
}

async fn response_body_storage_bytes(path: Option<&Path>) -> AppResult<u64> {
    let Some(path) = path else {
        return Ok(0);
    };
    let mut total = 0_u64;
    for candidate in [path.to_path_buf(), path.with_extension("idx")] {
        match tokio::fs::metadata(candidate).await {
            Ok(metadata) => total = total.saturating_add(metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(total)
}

async fn delete_history_entries(
    pool: &SqlitePool,
    query: &str,
    offset: i64,
) -> AppResult<Vec<PathBuf>> {
    let mut transaction = pool.begin().await?;
    let paths = sqlx::query(query)
        .bind(offset)
        .fetch_all(&mut *transaction)
        .await?
        .into_iter()
        .filter_map(|row| row.get::<Option<String>, _>("response_body_path"))
        .map(PathBuf::from)
        .collect();
    transaction.commit().await?;
    Ok(paths)
}

async fn delete_committed_paths(
    paths: Vec<PathBuf>,
    body_store: Option<&ResponseBodyStore>,
) -> AppResult<()> {
    for path in paths {
        if let Some(store) = body_store {
            store.delete_path_when_released(&path)?;
        } else {
            delete_response_body_file(&path).await?;
        }
    }
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
    for path in [path.to_path_buf(), path.with_extension("idx")] {
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, services::settings_service};

    #[tokio::test]
    async fn retention_applies_age_and_actual_body_bytes_together() {
        let root = std::env::temp_dir().join(format!("postnot-retention-test-{}", Uuid::new_v4()));
        let database_path = root.join("postnot.sqlite");
        let bodies_path = root.join("bodies");
        std::fs::create_dir_all(&bodies_path).expect("create test body directory");
        let pool = db::init_path(&database_path)
            .await
            .expect("initialize test database");
        settings_service::ensure_defaults(&pool)
            .await
            .expect("seed settings");
        let mut settings = settings_service::get_settings(&pool)
            .await
            .expect("load settings");
        settings.history_limit = 10;
        settings.history_retention_days = 30;
        settings.history_storage_limit_bytes = 1024 * 1024;
        settings_service::save_settings(&pool, &settings)
            .await
            .expect("save retention settings");

        let newest_path = bodies_path.join("newest.body");
        let middle_path = bodies_path.join("middle.body");
        let old_path = bodies_path.join("old.body");
        std::fs::write(&newest_path, vec![b'a'; 700 * 1024]).expect("write newest body");
        std::fs::write(&middle_path, vec![b'b'; 700 * 1024]).expect("write middle body");
        std::fs::write(&old_path, b"old").expect("write old body");
        let now = chrono::Utc::now();
        seed_history(&pool, "newest", &newest_path, &now.to_rfc3339()).await;
        seed_history(
            &pool,
            "middle",
            &middle_path,
            &(now - chrono::Duration::seconds(1)).to_rfc3339(),
        )
        .await;
        seed_history(
            &pool,
            "old",
            &old_path,
            &(now - chrono::Duration::days(60)).to_rfc3339(),
        )
        .await;

        let store = ResponseBodyStore::new(bodies_path);
        let result = apply_history_retention(&pool, &store)
            .await
            .expect("apply retention");
        assert_eq!(result.removed_entry_count, 2);
        assert!(result.released_response_body_bytes >= 700 * 1024 + 3);
        assert!(newest_path.exists());
        assert!(!middle_path.exists());
        assert!(!old_path.exists());
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM history_entries")
            .fetch_one(&pool)
            .await
            .expect("count history");
        assert_eq!(remaining, 1);

        pool.close().await;
        let _ = std::fs::remove_dir_all(root);
    }

    async fn seed_history(pool: &SqlitePool, id: &str, body_path: &Path, executed_at: &str) {
        sqlx::query(
            r#"
            INSERT INTO history_entries
                (id, request_name, method, url, request_snapshot_json, status_code,
                 duration_ms, response_headers_json, response_body_path,
                 response_body_preview, error_text, executed_at)
            VALUES (?1, ?2, 'GET', 'https://example.test', ?3, 200, 1, '[]', ?4, '', '', ?5)
            "#,
        )
        .bind(id)
        .bind(id)
        .bind(
            r#"{"name":"test","method":"GET","url":"https://example.test","queryParams":[],"headers":[],"body":{"mode":"none","raw":"","form":[],"files":[]},"auth":{"type":"none"},"preRequestScript":"","testScript":""}"#,
        )
        .bind(path_to_string(body_path.to_path_buf()))
        .bind(executed_at)
        .execute(pool)
        .await
        .expect("insert history entry");
    }
}
