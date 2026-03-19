use crate::domain::settings::AppSettings;
use crate::error::AppResult;
use crate::services::settings_service;

#[tauri::command]
pub async fn get_settings() -> AppResult<AppSettings> {
    Ok(settings_service::default_settings())
}
