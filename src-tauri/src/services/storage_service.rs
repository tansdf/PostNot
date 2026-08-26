use std::path::{Path, PathBuf};

use sqlx::SqlitePool;
use tauri::AppHandle;

use crate::{domain::storage::StorageSummary, error::AppResult, storage::paths};

pub async fn get_summary(pool: &SqlitePool, app: &AppHandle) -> AppResult<StorageSummary> {
    let data_directory = paths::app_data_dir(app)?;
    let database_path = paths::database_path(app)?;
    let history_directory = paths::response_bodies_dir(app)?;
    let realtime_directory = paths::realtime_payloads_dir(app)?;
    let (database_size_bytes, history_response_body_bytes, realtime_temporary_bytes) =
        tokio::task::spawn_blocking(move || {
            Ok::<_, std::io::Error>((
                database_size(&database_path)?,
                directory_size(&history_directory)?,
                directory_size(&realtime_directory)?,
            ))
        })
        .await??;

    Ok(StorageSummary {
        data_directory: data_directory.to_string_lossy().to_string(),
        database_size_bytes,
        history_entry_count: table_count(pool, "history_entries").await?,
        history_response_body_bytes,
        realtime_temporary_bytes,
        collection_count: table_count(pool, "collections").await?,
        collection_item_count: table_count(pool, "collection_items").await?,
        realtime_connection_count: table_count(pool, "realtime_connections").await?,
        environment_count: table_count(pool, "environments").await?,
        playbook_count: table_count(pool, "playbooks").await?,
        playbook_run_count: table_count(pool, "playbook_runs").await?,
        agent_activity_count: table_count(pool, "agent_activity").await?,
    })
}

async fn table_count(pool: &SqlitePool, table: &str) -> AppResult<u64> {
    let count = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await?;
    Ok(u64::try_from(count).unwrap_or_default())
}

fn file_size(path: &Path) -> std::io::Result<u64> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error),
    }
}

fn database_size(path: &Path) -> std::io::Result<u64> {
    [
        path.to_path_buf(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ]
    .iter()
    .try_fold(0_u64, |total, candidate| {
        Ok(total.saturating_add(file_size(candidate)?))
    })
}

fn directory_size(path: &Path) -> std::io::Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0_u64;
    let mut pending = vec![PathBuf::from(path)];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    Ok(total)
}
