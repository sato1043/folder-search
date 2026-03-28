mod commands;
pub mod domain;
pub mod infra;

use commands::AppState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            engine: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::build_index,
            commands::search,
            commands::get_index_status,
            commands::read_file_content,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
