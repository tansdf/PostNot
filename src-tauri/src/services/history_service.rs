use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{
    domain::{
        history::HistoryEntrySummary,
        requests::{ResponsePayload, SendRequestPayload},
    },
    error::AppResult,
    services::settings_service,
};

const PREVIEW_LIMIT: usize = 4_096;
const DEFAULT_HISTORY_LIMIT: u32 = 200;

pub async fn record_success(
    pool: &SqlitePool,
    request: &SendRequestPayload,
    response: &ResponsePayload,
) -> AppResult<()> {
    let request_snapshot_json = serde_json::to_string(request)?;
    let response_headers_json = serde_json::to_string(&response.headers)?;

    sqlx::query(
        "INSERT INTO history_entries (id, request_name, method, url, request_snapshot_json, status_code, duration_ms, response_headers_json, response_body_path, response_body_preview, error_text, executed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, ?11)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&request.name)
    .bind(&request.method)
    .bind(&request.url)
    .bind(request_snapshot_json)
    .bind(response.status_code.map(i64::from))
    .bind(response.duration_ms as i64)
    .bind(response_headers_json)
    .bind(preview(&response.body_text))
    .bind(&response.error_text)
    .bind(&response.executed_at)
    .execute(pool)
    .await?;

    prune(pool).await
}

pub async fn record_failure(pool: &SqlitePool, request: &SendRequestPayload, error_text: &str) -> AppResult<()> {
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

    prune(pool).await
}

pub async fn list_history(pool: &SqlitePool, limit: Option<u32>) -> AppResult<Vec<HistoryEntrySummary>> {
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

async fn prune(pool: &SqlitePool) -> AppResult<()> {
    let history_limit = settings_service::history_limit(pool)
        .await
        .unwrap_or(DEFAULT_HISTORY_LIMIT);

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
