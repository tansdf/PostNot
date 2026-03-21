use tauri::Manager;

pub mod app_state;
pub mod commands;
pub mod db;
pub mod domain;
pub mod error;
pub mod services;
pub mod storage;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let database = tauri::async_runtime::block_on(db::init(app.handle()))?;
            tauri::async_runtime::block_on(services::settings_service::ensure_defaults(&database))?;
            app.manage(app_state::AppState::new(database));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::requests::send_request,
            commands::requests::cancel_active_request,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::history::list_history,
            commands::history::get_history_entry,
            commands::history::clear_history,
            commands::collections::list_collections,
            commands::collections::create_collection,
            commands::collections::update_collection,
            commands::collections::delete_collection,
            commands::collections::list_saved_requests,
            commands::collections::save_request_to_collection,
            commands::collections::update_saved_request,
            commands::collections::get_saved_request,
            commands::collections::delete_saved_request,
            commands::environments::list_environments,
            commands::environments::create_environment,
            commands::environments::get_environment,
            commands::environments::update_environment,
            commands::environments::delete_environment,
            commands::environments::set_active_environment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PostNot application");
}
