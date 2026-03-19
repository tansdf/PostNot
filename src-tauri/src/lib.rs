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
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::history::list_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PostNot application");
}
