pub mod app_state;
pub mod commands;
pub mod db;
pub mod domain;
pub mod error;
pub mod services;
pub mod storage;

pub fn run() {
    tauri::Builder::default()
        .manage(app_state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::requests::send_request,
            commands::settings::get_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PostNot application");
}
