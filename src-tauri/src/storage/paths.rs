use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::db::DATABASE_FILE_NAME;
use crate::error::{AppError, AppResult};

pub fn database_path(app: &AppHandle) -> AppResult<PathBuf> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::Message(error.to_string()))?;

    Ok(app_dir.join(DATABASE_FILE_NAME))
}

pub fn response_bodies_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::Message(error.to_string()))?;

    Ok(app_dir.join("history-response-bodies"))
}
