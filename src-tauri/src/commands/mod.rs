use std::sync::Mutex;

use tauri::State;

use crate::domain::indexer::{Indexer, IndexStatus};
use crate::domain::search::{FulltextSearcher, SearchResult};
use crate::infra::tantivy::TantivySearchEngine;

/// アプリの状態として検索エンジンを保持する
pub struct AppState {
    pub engine: Mutex<Option<TantivySearchEngine>>,
}

/// フォルダを選択してインデックスを構築する
#[tauri::command]
pub fn build_index(
    folder_path: String,
    index_path: String,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let mut engine =
        TantivySearchEngine::new(&index_path).map_err(|e| format!("インデックス作成失敗: {}", e))?;

    let count = engine
        .index_folder(&folder_path)
        .map_err(|e| format!("インデックス構築失敗: {}", e))?;

    let mut guard = state.engine.lock().map_err(|e| e.to_string())?;
    *guard = Some(engine);

    Ok(count)
}

/// 検索を実行する
#[tauri::command]
pub fn search(query: String, limit: usize, state: State<'_, AppState>) -> Result<Vec<SearchResult>, String> {
    let guard = state.engine.lock().map_err(|e| e.to_string())?;
    let engine = guard
        .as_ref()
        .ok_or_else(|| "インデックスが構築されていない".to_string())?;

    engine.search(&query, limit).map_err(|e| e.to_string())
}

/// インデックスの状態を取得する
#[tauri::command]
pub fn get_index_status(state: State<'_, AppState>) -> Result<IndexStatus, String> {
    let guard = state.engine.lock().map_err(|e| e.to_string())?;
    let engine = guard
        .as_ref()
        .ok_or_else(|| "インデックスが構築されていない".to_string())?;

    Ok(engine.status())
}

/// ファイルの内容を読み取る
#[tauri::command]
pub fn read_file_content(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("ファイル読み込み失敗: {}", e))
}
