#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
use tauri::utils::{config::BundleType, platform::bundle_type};
use tauri::AppHandle;
#[cfg(target_os = "linux")]
use tauri_plugin_updater::Update;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

use crate::{
    app_state::AppState,
    domain::updates::{AvailableUpdate, UpdateCheckResult},
    error::{AppError, AppResult},
    services::settings_service,
};

const DEFAULT_UPDATE_ENDPOINT: &str =
    "https://github.com/tansdf/PostNot/releases/latest/download/latest.json";
const UPDATER_PUBLIC_KEY: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDlFM0FDRjNBNTMzRkJERjkKUldUNXZUOVRPczg2bmlUNzdBdlFvS1Z0RHBzc1JyMXgxUDNCclRuVmsvZGVZUCttcEVaTzNySmgK";
#[cfg(target_os = "linux")]
const PACKAGE_INSTALL_TIMEOUT: Duration = Duration::from_secs(300);

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
struct LinuxPackageInstaller {
    format_name: &'static str,
    extension: &'static str,
    magic_bytes: &'static [u8],
    install_command: &'static str,
    install_arg: &'static str,
}

#[cfg(target_os = "linux")]
const DEB_INSTALLER: LinuxPackageInstaller = LinuxPackageInstaller {
    format_name: "Debian",
    extension: "deb",
    magic_bytes: b"!<arch>\n",
    install_command: "dpkg",
    install_arg: "-i",
};

#[cfg(target_os = "linux")]
const RPM_INSTALLER: LinuxPackageInstaller = LinuxPackageInstaller {
    format_name: "RPM",
    extension: "rpm",
    magic_bytes: &[0xed, 0xab, 0xee, 0xdb],
    install_command: "rpm",
    install_arg: "-U",
};

pub async fn check_for_updates(app: &AppHandle, state: &AppState) -> AppResult<UpdateCheckResult> {
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

    settings_service::save_last_update_checked_at(state.db(), &chrono::Utc::now().to_rfc3339())
        .await?;

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

    state.clear_pending_update()?;

    Ok(UpdateCheckResult {
        configured: true,
        update: None,
    })
}

pub async fn install_update(_app: &AppHandle, state: &AppState) -> AppResult<()> {
    let Some(update) = state.take_pending_update()? else {
        return Err(AppError::Message(
            "Check for updates first so PostNot knows what to install.".to_string(),
        ));
    };

    #[cfg(target_os = "linux")]
    match bundle_type() {
        Some(BundleType::Deb) => {
            install_linux_package_update(update, DEB_INSTALLER).await?;
            _app.restart();
        }
        Some(BundleType::Rpm) => {
            install_linux_package_update(update, RPM_INSTALLER).await?;
            _app.restart();
        }
        _ => {}
    }

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(map_updater_error)?;

    #[cfg(not(target_os = "windows"))]
    _app.restart();

    #[cfg(target_os = "windows")]
    Ok(())
}

fn default_update_endpoint() -> AppResult<Url> {
    Url::parse(DEFAULT_UPDATE_ENDPOINT).map_err(AppError::from)
}

fn map_updater_error(error: impl std::fmt::Display) -> AppError {
    AppError::Message(error.to_string())
}

#[cfg(target_os = "linux")]
async fn install_linux_package_update(
    update: Update,
    installer: LinuxPackageInstaller,
) -> AppResult<()> {
    let bytes = update
        .download(
            |chunk_length, content_length| {
                log::debug!(
                    "downloaded update chunk: {chunk_length} bytes of {:?}",
                    content_length
                );
            },
            || {
                log::debug!("update download finished");
            },
        )
        .await
        .map_err(map_updater_error)?;

    if !bytes.starts_with(installer.magic_bytes) {
        return Err(AppError::Message(format!(
            "The downloaded Linux update is not a {} package. Check the latest.json Linux artifact URL.",
            installer.format_name
        )));
    }

    let package_dir = std::env::temp_dir().join(format!("postnot-update-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&package_dir).await?;

    let package_path = package_dir.join(format!("postnot-update.{}", installer.extension));
    tokio::fs::write(&package_path, bytes).await?;

    let install_result = install_linux_package_with_pkexec(&package_path, installer).await;

    if let Err(error) = tokio::fs::remove_dir_all(&package_dir).await {
        log::warn!("failed to remove temporary update package: {error}");
    }

    install_result
}

#[cfg(target_os = "linux")]
async fn install_linux_package_with_pkexec(
    package_path: &Path,
    installer: LinuxPackageInstaller,
) -> AppResult<()> {
    let mut command = tokio::process::Command::new("pkexec");
    command
        .arg(installer.install_command)
        .arg(installer.install_arg)
        .arg(package_path)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .kill_on_drop(true);

    let output = tokio::time::timeout(PACKAGE_INSTALL_TIMEOUT, command.output())
        .await
        .map_err(|_| {
            AppError::Message(
                "Timed out waiting for the system authentication prompt. Make sure a PolicyKit authentication agent is running, then try the update again.".to_string(),
            )
        })?
        .map_err(|error| {
            AppError::Message(format!(
                "Unable to start the system authentication prompt with pkexec: {error}. Install PolicyKit or download and install the latest Linux package release manually."
            ))
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let details = stderr.trim();

    if details.is_empty() {
        return Err(AppError::Message(format!(
            "The system authentication prompt was cancelled or the {} package install failed.",
            installer.format_name
        )));
    }

    Err(AppError::Message(format!(
        "The {} package install failed: {details}",
        installer.format_name
    )))
}
