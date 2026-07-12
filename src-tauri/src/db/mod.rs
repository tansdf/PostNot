use std::{fs, str::FromStr, time::Duration};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};
use tauri::AppHandle;

use crate::{
    error::{AppError, AppResult},
    storage::paths,
};

pub const DATABASE_FILE_NAME: &str = "postnot.sqlite";
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn init(app: &AppHandle) -> AppResult<SqlitePool> {
    let database_path = paths::database_path(app)?;

    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", database_path.display()))
        .map_err(|error| AppError::Message(error.to_string()))?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;

    MIGRATOR
        .run(&pool)
        .await
        .map_err(|error| AppError::Message(error.to_string()))?;

    Ok(pool)
}
