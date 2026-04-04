use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

use crate::{
    app_state::AppState,
    domain::updates::{AvailableUpdate, UpdateCheckResult},
    error::{AppError, AppResult},
};

const DEFAULT_UPDATE_ENDPOINT: &str =
    "https://github.com/tansdf/PostNot/releases/latest/download/latest.json";
const UPDATER_PUBLIC_KEY: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDlFM0FDRjNBNTMzRkJERjkKUldUNXZUOVRPczg2bmlUNzdBdlFvS1Z0RHBzc1JyMXgxUDNCclRuVmsvZGVZUCttcEVaTzNySmgK";

pub async fn check_for_updates(app: &AppHandle, state: &AppState) -> AppResult<UpdateCheckResult> {
    state.clear_pending_update()?;

    let update = app
        .updater_builder()
        .pubkey(UPDATER_PUBLIC_KEY)
        .endpoints(vec![default_update_endpoint()?])
        .map_err(map_updater_error)?
        .build()
        .map_err(map_updater_error)?
        .check()
        .await
        .map_err(map_updater_error)?;

    if let Some(update) = update {
        let metadata = AvailableUpdate {
            current_version: update.current_version.clone(),
            version: update.version.clone(),
            date: update.date.map(|value| value.to_string()),
            body: update.body.clone(),
        };

        state.set_pending_update(update)?;

        return Ok(UpdateCheckResult {
            configured: true,
            update: Some(metadata),
        });
    }

    Ok(UpdateCheckResult {
        configured: true,
        update: None,
    })
}

pub async fn install_update(app: &AppHandle, state: &AppState) -> AppResult<()> {
    let Some(update) = state.take_pending_update()? else {
        return Err(AppError::Message(
            "Check for updates first so PostNot knows what to install.".to_string(),
        ));
    };

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(map_updater_error)?;

    #[cfg(not(target_os = "windows"))]
    app.restart();

    #[cfg(target_os = "windows")]
    Ok(())
}

fn default_update_endpoint() -> AppResult<Url> {
    Url::parse(DEFAULT_UPDATE_ENDPOINT).map_err(AppError::from)
}

fn map_updater_error(error: impl std::fmt::Display) -> AppError {
    AppError::Message(error.to_string())
}
