mod commands;
pub mod domain;
pub mod infra;

use commands::AppState;
use infra::config::SettingsStore;
use infra::model;
use infra::model_registry::ModelRegistry;
use infra::onnx::OnnxEmbeddingGenerator;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // モード別の環境変数ファイルを読み込む（.local が優先）
    #[cfg(debug_assertions)]
    {
        dotenvy::from_filename(".env.development").ok();
        dotenvy::from_filename(".env.development.local").ok();
    }
    #[cfg(not(debug_assertions))]
    {
        dotenvy::from_filename(".env.production").ok();
        dotenvy::from_filename(".env.production.local").ok();
    }
    let model_dir = tauri::utils::platform::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("models")))
        .unwrap_or_else(|| std::path::PathBuf::from("./models"));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            engine: Mutex::new(None),
            vector_index: Mutex::new(None),
            embedding_model: Mutex::new(None),
            llm_engine: Mutex::new(None),
            model_registry: ModelRegistry::new(&model_dir),
            settings_store: SettingsStore::new(&model_dir),
            model_dir,
            folder_path: Mutex::new(None),
            watcher: Mutex::new(None),
            loaded_llm_config: Mutex::new(None),
            cancel_token: Arc::new(AtomicBool::new(false)),
        })
        .setup(|app| {
            // TAURI_OPEN_DEVTOOLS=1 でDevToolsを自動で開く（デバッグビルドのみ）
            #[cfg(debug_assertions)]
            if std::env::var("TAURI_OPEN_DEVTOOLS").unwrap_or_default() == "1" {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            let state = app.state::<AppState>();
            let model_dir = &state.model_dir;

            if model::is_model_downloaded(model_dir) {
                let files = model::model_files(model_dir);
                match OnnxEmbeddingGenerator::new(
                    files.model_path.to_str().unwrap_or(""),
                    files.tokenizer_path.to_str().unwrap_or(""),
                ) {
                    Ok(generator) => {
                        let mut guard = state.embedding_model.lock().unwrap();
                        *guard = Some(generator);
                    }
                    Err(e) => {
                        eprintln!("embeddingモデルの自動ロード失敗: {}", e);
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_folder,
            commands::cancel_indexing,
            commands::build_index,
            commands::search,
            commands::get_index_status,
            commands::read_file_content,
            commands::is_embedding_model_ready,
            commands::download_embedding_model,
            commands::build_vector_index,
            commands::hybrid_search,
            commands::list_available_models,
            commands::download_llm_model,
            commands::load_llm_model,
            commands::is_llm_ready,
            commands::chat,
            commands::detect_system_info,
            commands::get_model_recommendations,
            commands::list_downloaded_models,
            commands::delete_model,
            commands::get_storage_usage,
            commands::register_custom_model,
            commands::unregister_custom_model,
            commands::get_loaded_model_filename,
            commands::clear_model_cache,
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
