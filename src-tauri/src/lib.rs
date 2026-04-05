mod commands;
pub mod domain;
pub mod infra;

use commands::{AppState, IndexValidation};
use infra::config::SettingsStore;
use infra::model;
use infra::model_registry::ModelRegistry;
use infra::onnx::OnnxEmbeddingGenerator;
use infra::tantivy as tantivy_infra;
use infra::vector_cache;
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let model_dir = app_data_dir.join("models");

            app.manage(AppState {
                engine: Mutex::new(None),
                vector_index: Mutex::new(None),
                embedding_model: Mutex::new(None),
                llm_engine: Mutex::new(None),
                model_registry: ModelRegistry::new(&model_dir),
                settings_store: SettingsStore::new(&app_data_dir),
                model_dir,
                folder_path: Mutex::new(None),
                watcher: Mutex::new(None),
                loaded_llm_config: Mutex::new(None),
                cancel_token: Arc::new(AtomicBool::new(false)),
                index_validation: Arc::new(IndexValidation::new()),
            });

            // TAURI_OPEN_DEVTOOLS=1 でDevToolsを自動で開く（デバッグビルドのみ）
            #[cfg(debug_assertions)]
            if std::env::var("TAURI_OPEN_DEVTOOLS").unwrap_or_default() == "1" {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            let state = app.state::<AppState>();
            let model_dir = &state.model_dir;

            // バックグラウンドでインデックス検証を開始
            if let Ok(app_data_dir) = app.path().app_data_dir() {
                let validation = state.index_validation.clone();
                std::thread::spawn(move || {
                    let cache = vector_cache::VectorCache::new(&app_data_dir);

                    // index/{hash}/ を列挙し、各ハッシュ内の fulltext/ と vector/ を検証
                    for hash_dir in cache.list_index_dirs() {
                        let hash = hash_dir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();

                        // フォルダ選択で予約済みならスキップ
                        if validation.reserved.lock().unwrap().contains(&hash) {
                            continue;
                        }

                        // 検証中ハッシュをセット
                        *validation.current_hash.lock().unwrap() = Some(hash.clone());

                        // 全文検索インデックスの検証
                        let fulltext_path = hash_dir.join("fulltext");
                        if fulltext_path.exists()
                            && !tantivy_infra::validate_index(&fulltext_path)
                        {
                            eprintln!(
                                "BG検証: 全文検索インデックスの破損を検出、削除: {:?}",
                                fulltext_path
                            );
                            let _ = std::fs::remove_dir_all(&fulltext_path);
                        }

                        // ベクトルキャッシュの検証
                        let vector_path = hash_dir.join("vector");
                        if vector_path.exists()
                            && !vector_cache::validate_cache_dir(&vector_path)
                        {
                            eprintln!(
                                "BG検証: ベクトルキャッシュの破損を検出、削除: {:?}",
                                vector_path
                            );
                            let _ = std::fs::remove_dir_all(&vector_path);
                        }

                        // 検証中ハッシュをクリア・完了に追加・通知
                        *validation.current_hash.lock().unwrap() = None;
                        validation.completed.lock().unwrap().insert(hash);
                        validation.notify.notify_all();

                        // UIの動きを止めないように優先度を下げる
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                });
            }

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
            commands::validate_folder_indexes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
