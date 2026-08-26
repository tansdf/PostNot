use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::db::DATABASE_FILE_NAME;
use crate::error::{AppError, AppResult};

pub const APP_IDENTIFIER: &str = "com.postnot.app";

pub fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|error| AppError::Message(error.to_string()))
}

pub fn database_path(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join(DATABASE_FILE_NAME))
}

pub fn headless_app_data_dir() -> AppResult<PathBuf> {
    dirs::data_dir()
        .map(|path| path.join(APP_IDENTIFIER))
        .ok_or_else(|| {
            AppError::Message("The operating system app-data directory is unavailable.".to_string())
        })
}

pub fn headless_database_path(data_dir: Option<PathBuf>) -> AppResult<PathBuf> {
    Ok(data_dir
        .unwrap_or(headless_app_data_dir()?)
        .join(DATABASE_FILE_NAME))
}

pub fn response_bodies_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join("history-response-bodies"))
}

pub fn realtime_payloads_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join("realtime-session-payloads"))
}

pub fn window_state_path(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join("window-state.json"))
}
