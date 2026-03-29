mod commands;
pub mod domain;
pub mod infra;

use commands::AppState;
use infra::model;
use infra::onnx::OnnxEmbeddingGenerator;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            engine: Mutex::new(None),
            vector_index: Mutex::new(None),
            embedding_model: Mutex::new(None),
            llm_engine: Mutex::new(None),
            model_dir: tauri::utils::platform::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("models")))
                .unwrap_or_else(|| std::path::PathBuf::from("./models")),
            folder_path: Mutex::new(None),
            watcher: Mutex::new(None),
        })
        .setup(|app| {
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
