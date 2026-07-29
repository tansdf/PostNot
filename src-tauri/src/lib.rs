use std::{env, fs, path::PathBuf, sync::Once};

#[cfg(target_os = "windows")]
use std::process::Command;

use tauri::Manager;

pub mod app_state;
pub mod commands;
pub mod db;
pub mod domain;
pub mod error;
pub mod mcp;
pub mod services;
pub mod storage;

static PANIC_HOOK: Once = Once::new();

pub fn run() -> Result<(), String> {
    install_panic_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let database = tauri::async_runtime::block_on(db::init(app.handle()))?;
            tauri::async_runtime::block_on(db::ensure_application_defaults(&database))?;
            let secret_store = services::secret_store_service::default_secret_store();
            let response_bodies = services::response_body_service::ResponseBodyStore::new(
                storage::paths::response_bodies_dir(app.handle())?,
            );
            let referenced_bodies = tauri::async_runtime::block_on(
                services::history_service::stored_response_body_paths(&database),
            )?;
            tauri::async_runtime::block_on(response_bodies.reconcile(&referenced_bodies))?;
            let realtime_payloads = services::realtime_payload_service::RealtimePayloadStore::new(
                storage::paths::realtime_payloads_dir(app.handle())?,
            );
            tauri::async_runtime::block_on(realtime_payloads.reset())?;
            let realtime_connections =
                services::realtime_service::RealtimeConnectionManager::new(realtime_payloads);
            app.manage(app_state::AppState::new(
                database,
                secret_store,
                response_bodies,
                realtime_connections,
            ));
            if let Some(window) = app.get_webview_window("main") {
                services::window_state_service::restore_and_track_main_window(&window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::activity::list_agent_activity,
            commands::requests::send_request,
            commands::requests::preview_request,
            commands::requests::cancel_active_request,
            commands::requests::pick_multipart_files,
            commands::realtime::connect_realtime_connection,
            commands::realtime::get_realtime_workspace_state,
            commands::realtime::save_realtime_workspace_state,
            commands::realtime::disconnect_realtime_connection,
            commands::realtime::release_realtime_connection,
            commands::realtime::send_realtime_message,
            commands::realtime::ping_realtime_connection,
            commands::realtime::close_realtime_connection,
            commands::realtime::get_realtime_session_snapshot,
            commands::realtime::clear_realtime_transcript,
            commands::realtime::read_realtime_payload,
            commands::realtime::save_realtime_payload,
            commands::realtime::export_realtime_transcript,
            commands::responses::read_response_body_window,
            commands::responses::search_response_body,
            commands::responses::cancel_response_search,
            commands::responses::find_response_match,
            commands::responses::read_response_body_text,
            commands::responses::retain_response_body,
            commands::responses::release_response_body,
            commands::responses::get_response_body_path,
            commands::responses::save_response_body,
            commands::responses::format_response_body,
            commands::responses::cancel_response_body_job,
            commands::settings::get_settings,
            commands::settings::get_mcp_setup_info,
            commands::settings::update_settings,
            commands::settings::get_request_workspace_state,
            commands::settings::save_request_workspace_state,
            commands::updates::check_for_updates,
            commands::updates::install_update,
            commands::history::list_history,
            commands::history::get_history_entry,
            commands::history::clear_history,
            commands::collections::list_collections,
            commands::collections::search_collection_entities,
            commands::collections::get_collection_sidebar_state,
            commands::collections::save_collection_sidebar_state,
            commands::collections::create_collection,
            commands::collections::list_collection_items,
            commands::collections::create_collection_folder,
            commands::collections::update_collection_folder,
            commands::collections::move_collection_item,
            commands::collections::update_collection,
            commands::collections::delete_collection,
            commands::collections::list_saved_requests,
            commands::collections::save_request_to_collection,
            commands::collections::update_saved_request,
            commands::collections::get_saved_request,
            commands::collections::list_saved_realtime_requests,
            commands::collections::save_realtime_request_to_collection,
            commands::collections::update_saved_realtime_request,
            commands::collections::get_saved_realtime_request,
            commands::collections::delete_collection_item,
            commands::collections::delete_saved_request,
            commands::collections::delete_saved_realtime_request,
            commands::collections::export_collection,
            commands::playbooks::list_playbooks,
            commands::playbooks::create_playbook,
            commands::playbooks::get_playbook,
            commands::playbooks::update_playbook,
            commands::playbooks::duplicate_playbook,
            commands::playbooks::delete_playbook,
            commands::playbooks::add_playbook_step,
            commands::playbooks::update_playbook_step,
            commands::playbooks::reorder_playbook_steps,
            commands::playbooks::delete_playbook_step,
            commands::playbooks::get_playbook_execution_context,
            commands::playbooks::create_playbook_run,
            commands::playbooks::finish_playbook_run,
            commands::playbooks::record_playbook_run_step,
            commands::playbooks::list_playbook_runs,
            commands::playbooks::get_playbook_run,
            commands::environments::list_environments,
            commands::environments::create_environment,
            commands::environments::get_environment,
            commands::environments::update_environment,
            commands::environments::delete_environment,
            commands::environments::set_active_environment,
            commands::environments::import_postman_environment,
            commands::environments::export_environment,
            commands::imports::import_requests,
            commands::imports::import_curl_request_to_draft,
            commands::imports::import_openapi_request_to_draft,
        ])
        .run(tauri::generate_context!())
        .map_err(|error| error.to_string())
}

pub fn report_startup_failure(message: &str) {
    let log_path = write_startup_log(message);
    let summary = format!(
        "PostNot failed to start.\n\n{}\n\nStartup log: {}",
        message,
        log_path.display()
    );

    eprintln!("{}", summary);

    #[cfg(target_os = "windows")]
    {
        show_windows_error_dialog(&summary);
    }
}

fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|panic_info| {
            let message = match panic_info.payload().downcast_ref::<&str>() {
                Some(payload) => (*payload).to_string(),
                None => match panic_info.payload().downcast_ref::<String>() {
                    Some(payload) => payload.clone(),
                    None => "Unknown panic".to_string(),
                },
            };

            let location = panic_info
                .location()
                .map(|location| {
                    format!(
                        "{}:{}:{}",
                        location.file(),
                        location.line(),
                        location.column()
                    )
                })
                .unwrap_or_else(|| "unknown location".to_string());

            report_startup_failure(&format!("panic at {}: {}", location, message));
        }));
    });
}

fn write_startup_log(message: &str) -> PathBuf {
    let log_path = env::temp_dir().join("postnot-startup.log");
    let log_body = format!("PostNot startup failure\n\n{}", message);
    let _ = fs::write(&log_path, log_body);
    log_path
}

#[cfg(target_os = "windows")]
fn show_windows_error_dialog(message: &str) {
    let escaped_message = message.replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.MessageBox]::Show('{escaped}', 'PostNot failed to start') | Out-Null",
        escaped = escaped_message
    );

    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .spawn();
}
