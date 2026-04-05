use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use tauri::{Emitter, State};

use crate::domain::config::AppSettings;
use crate::domain::embedding::{EmbeddingGenerator, VectorSearcher};
use crate::domain::indexer::chunker::split_into_chunks;
use crate::domain::indexer::{IndexStatus, Indexer};
use crate::domain::llm::chat_template::ChatTemplate;
use crate::domain::llm::rag::{build_rag_prompt, extract_sources, ContextChunk, RagAnswer};
use crate::domain::llm::{DownloadedModelInfo, LlmInference, LlmModelInfo, StorageUsage};
use crate::domain::search::hybrid::{reciprocal_rank_fusion, HybridSearchResult};
use crate::domain::search::{FulltextSearcher, SearchResult};
use crate::domain::system::{recommend_models, ModelRecommendation, SystemInfo};
use crate::infra::config::SettingsStore;
use crate::infra::hnsw::HnswVectorIndex;
use crate::infra::llama::LlamaEngine;
use crate::infra::model;
use crate::infra::model_registry::ModelRegistry;
use crate::infra::onnx::OnnxEmbeddingGenerator;
use crate::infra::tantivy::{self as tantivy_infra, TantivySearchEngine};
use crate::infra::vector_cache::{self, VectorCache};
use crate::infra::watcher::FileWatcher;

use tauri::Manager;

/// ロード済みLLMモデルの設定
pub struct LoadedLlmConfig {
    pub filename: String,
    pub chat_template: ChatTemplate,
    pub context_length: u32,
}

/// インデックス検証の共有状態
pub struct IndexValidation {
    /// バックグラウンド検証が処理中のキャッシュハッシュ
    pub(crate) current_hash: Mutex<Option<String>>,
    /// フォルダ選択で予約済み（バックグラウンドがスキップすべきハッシュ）
    pub(crate) reserved: Mutex<HashSet<String>>,
    /// バックグラウンドで検証完了済みのハッシュ
    pub(crate) completed: Mutex<HashSet<String>>,
    /// バックグラウンド検証の完了通知
    pub(crate) notify: Condvar,
}

impl IndexValidation {
    pub fn new() -> Self {
        Self {
            current_hash: Mutex::new(None),
            reserved: Mutex::new(HashSet::new()),
            completed: Mutex::new(HashSet::new()),
            notify: Condvar::new(),
        }
    }
}

/// アプリの状態
pub struct AppState {
    pub engine: Mutex<Option<TantivySearchEngine>>,
    pub vector_index: Mutex<Option<HnswVectorIndex>>,
    pub embedding_model: Mutex<Option<OnnxEmbeddingGenerator>>,
    pub llm_engine: Mutex<Option<LlamaEngine>>,
    pub model_dir: PathBuf,
    pub folder_path: Mutex<Option<String>>,
    pub watcher: Mutex<Option<FileWatcher>>,
    pub loaded_llm_config: Mutex<Option<LoadedLlmConfig>>,
    pub model_registry: ModelRegistry,
    pub settings_store: SettingsStore,
    pub cancel_token: Arc<AtomicBool>,
    pub index_validation: Arc<IndexValidation>,
}

/// フォルダの軽量スキャンを実行する（メタデータのみ取得）
#[tauri::command]
pub fn scan_folder(
    folder_path: String,
) -> Result<crate::domain::indexer::FolderScanResult, String> {
    use crate::domain::indexer::FolderScanResult;
    use std::time::{Duration, Instant};

    let timeout = Duration::from_secs(5);
    let start = Instant::now();

    let mut file_count = 0u64;
    let mut total_size_bytes = 0u64;
    let mut max_file_size_bytes = 0u64;
    let mut has_symlinks = false;
    let mut timed_out = false;

    for entry in walkdir::WalkDir::new(&folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if start.elapsed() > timeout {
            timed_out = true;
            break;
        }

        if entry.path_is_symlink() {
            has_symlinks = true;
        }

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "txt" && ext != "md" {
            continue;
        }

        if let Ok(metadata) = std::fs::metadata(path) {
            let size = metadata.len();
            file_count += 1;
            total_size_bytes += size;
            if size > max_file_size_bytes {
                max_file_size_bytes = size;
            }
        }
    }

    let estimated_chunks = if total_size_bytes > 0 {
        total_size_bytes / 400
    } else {
        0
    };

    Ok(FolderScanResult {
        file_count,
        total_size_bytes,
        max_file_size_bytes,
        estimated_chunks,
        has_symlinks,
        timed_out,
    })
}

/// インデックス作成を中断する
#[tauri::command]
pub fn cancel_indexing(state: State<'_, AppState>) {
    state.cancel_token.store(true, Ordering::Relaxed);
}

/// 全文検索インデックスを構築する
#[tauri::command]
pub async fn build_index(
    app: tauri::AppHandle,
    folder_path: String,
    total_files: u64,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let index_path = app_data_dir
        .join("index")
        .join(vector_cache::folder_hash(&folder_path))
        .join("fulltext");

    // キャンセルトークンをリセット
    state.cancel_token.store(false, Ordering::Relaxed);

    // 既存のウォッチャーを停止
    {
        let mut watcher_guard = state.watcher.lock().map_err(|e| e.to_string())?;
        *watcher_guard = None;
    }

    // 既存エンジンを解放（ファイル監視コールバックのIndexWriter完了を待つ）
    {
        let mut guard = state.engine.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }

    // 重い処理をブロッキングスレッドで実行（WebViewスレッドを解放）
    let cancel_token = state.cancel_token.clone();
    let app_clone = app.clone();
    let folder_path_clone = folder_path.clone();
    let index_path_clone = index_path.clone();
    let (engine, count) = tokio::task::spawn_blocking(move || {
        let index_path_str = index_path_clone.to_string_lossy().to_string();
        let mut engine = TantivySearchEngine::new(&index_path_str)
            .map_err(|e| format!("インデックス作成失敗: {}", e))?;

        let count = match engine.index_folder_cancellable(
            &folder_path_clone,
            &cancel_token,
            total_files,
            |current, total| {
                let _ = app_clone.emit(
                    "fulltext-index-progress",
                    serde_json::json!({ "current": current, "total": total }),
                );
            },
        ) {
            Ok(count) => count,
            Err(crate::domain::indexer::IndexError::Cancelled) => {
                let _ = std::fs::remove_dir_all(&index_path_clone);
                return Err("インデックス作成が中断された".to_string());
            }
            Err(e) => return Err(format!("インデックス構築失敗: {}", e)),
        };

        Ok::<_, String>((engine, count))
    })
    .await
    .map_err(|e| format!("タスク実行失敗: {}", e))??;

    // フォルダ情報を保存
    let hash_dir = app_data_dir
        .join("index")
        .join(vector_cache::folder_hash(&folder_path));
    vector_cache::save_folder_info(&hash_dir, &folder_path);

    {
        let mut guard = state.engine.lock().map_err(|e| e.to_string())?;
        *guard = Some(engine);
    }

    {
        let mut fp = state.folder_path.lock().map_err(|e| e.to_string())?;
        *fp = Some(folder_path.clone());
    }

    // ファイル監視を開始
    let app_handle = app.app_handle().clone();
    let watch_folder = folder_path.clone();
    match FileWatcher::start(&watch_folder, move |changed_files| {
        let state: tauri::State<'_, AppState> = app_handle.state();

        // 全文検索インデックスの差分更新
        let fulltext_count = {
            let mut engine_guard = match state.engine.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if let Some(engine) = engine_guard.as_mut() {
                if let Err(e) = engine.update_files(&changed_files) {
                    eprintln!("全文検索インデックス更新失敗: {}", e);
                }
                engine.status().file_count
            } else {
                return;
            }
        };

        // ベクトルインデックスの差分更新（ベストエフォート）
        let vector_chunk_count = (|| -> Option<u64> {
            let app_data_dir = app_handle.path().app_data_dir().ok()?;
            let cache = VectorCache::new(&app_data_dir);
            let folder_path = state.folder_path.lock().ok()?.clone()?;

            let diff = cache.compute_diff(&folder_path)?;
            if !diff.has_changes() {
                return None;
            }

            let cached = cache.load(&folder_path).ok()?;
            let mut model_guard = state.embedding_model.try_lock().ok()?;
            let generator = model_guard.as_mut()?;

            match build_vector_index_incremental_inner(
                &cache,
                &folder_path,
                &diff,
                cached,
                generator,
                None, // ファイル監視の差分更新ではキャンセル不要
            ) {
                Ok((vector_index, total)) => {
                    if let Ok(mut guard) = state.vector_index.lock() {
                        *guard = Some(vector_index);
                    }
                    Some(total)
                }
                Err(e) => {
                    eprintln!("ベクトルインデックス差分更新失敗: {}", e);
                    None
                }
            }
        })();

        // フロントエンドに通知（ベクトル更新の成否に関わらず送信）
        let _ = app_handle.emit(
            "index-updated",
            serde_json::json!({
                "fulltext_count": fulltext_count,
                "vector_chunk_count": vector_chunk_count.unwrap_or(0),
            }),
        );
    }) {
        Ok(watcher) => {
            let mut watcher_guard = state.watcher.lock().map_err(|e| e.to_string())?;
            *watcher_guard = Some(watcher);
        }
        Err(e) => {
            eprintln!("ファイル監視開始失敗（無視）: {}", e);
        }
    }

    Ok(count)
}

/// 全文検索を実行する
#[tauri::command]
pub fn search(
    query: String,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
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

/// embeddingモデルがダウンロード済みかどうか
#[tauri::command]
pub fn is_embedding_model_ready(state: State<'_, AppState>) -> bool {
    model::is_model_downloaded(&state.model_dir)
}

/// embeddingモデルをダウンロードする
#[tauri::command]
pub async fn download_embedding_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let model_dir = state.model_dir.clone();

    model::download_embedding_model(&model_dir, |progress| {
        let _ = app.emit("download-progress", progress);
    })
    .await?;

    // モデルをロードする
    let files = model::model_files(&model_dir);
    let generator = OnnxEmbeddingGenerator::new(
        files.model_path.to_str().unwrap_or(""),
        files.tokenizer_path.to_str().unwrap_or(""),
    )
    .map_err(|e| format!("モデル読み込み失敗: {}", e))?;

    let mut guard = state.embedding_model.lock().map_err(|e| e.to_string())?;
    *guard = Some(generator);

    Ok(())
}

/// ベクトルインデックスを構築する
#[tauri::command]
pub async fn build_vector_index(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let folder_path = {
        let guard = state.folder_path.lock().map_err(|e| e.to_string())?;
        guard
            .clone()
            .ok_or_else(|| "フォルダが選択されていない".to_string())?
    };

    // キャッシュの準備
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir取得失敗: {}", e))?;
    let cache = VectorCache::new(&app_data_dir);

    // キャッシュ差分を計算
    let diff = cache.compute_diff(&folder_path);

    // 差分なし → キャッシュからロード（軽量なのでspawn_blocking不要）
    if let Some(ref d) = diff {
        if !d.has_changes() {
            if let Ok(cached) = cache.load(&folder_path) {
                let total = cached.metas.len() as u64;
                let vector_index = HnswVectorIndex::from_cache(cached);

                let _ = app.emit(
                    "vector-index-progress",
                    serde_json::json!({ "current": total, "total": total }),
                );

                let mut guard = state.vector_index.lock().map_err(|e| e.to_string())?;
                *guard = Some(vector_index);
                return Ok(total);
            }
        }
    } else {
    }

    // 重い処理をブロッキングスレッドで実行（WebViewスレッドを解放）
    let cancel_token = state.cancel_token.clone();
    let app_clone = app.clone();

    // キャンセルトークンをリセット（前回の中断が残っている場合に備える）
    state.cancel_token.store(false, Ordering::Relaxed);

    // embedding_model を一時的に取り出す
    let mut generator = {
        let mut model_guard = state.embedding_model.lock().map_err(|e| e.to_string())?;
        model_guard
            .take()
            .ok_or_else(|| "embeddingモデルがロードされていない".to_string())?
    };

    let (result, generator) = tokio::task::spawn_blocking(move || {
        let result = if let Some(ref d) = diff {
            if d.has_changes() {
                if let Ok(cached) = cache.load(&folder_path) {
                    // 差分更新
                    let build_result = build_vector_index_incremental_inner(
                        &cache,
                        &folder_path,
                        d,
                        cached,
                        &mut generator,
                        Some(&cancel_token),
                    );

                    match build_result {
                        Ok((vector_index, total)) => {
                            let _ = app_clone.emit(
                                "vector-index-progress",
                                serde_json::json!({ "current": total, "total": total }),
                            );
                            Ok((vector_index, total))
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    // キャッシュロード失敗 → フルビルド
                    build_vector_index_full_inner(
                        &app_clone,
                        &cache,
                        &folder_path,
                        &mut generator,
                        &cancel_token,
                    )
                }
            } else {
                // ここには来ない（差分なしは上で処理済み）
                Err("予期しない状態".to_string())
            }
        } else {
            // キャッシュ不在 → フルビルド
            build_vector_index_full_inner(
                &app_clone,
                &cache,
                &folder_path,
                &mut generator,
                &cancel_token,
            )
        };

        // 成功・失敗に関わらず generator を返却する
        (result, generator)
    })
    .await
    .map_err(|e| format!("タスク実行失敗: {}", e))?;


    // embedding_model を常に返却
    {
        let mut model_guard = state.embedding_model.lock().map_err(|e| e.to_string())?;
        *model_guard = Some(generator);
    }

    match result {
        Ok((vector_index, total)) => {
            {
                let mut guard = state.vector_index.lock().map_err(|e| e.to_string())?;
                *guard = Some(vector_index);
            }
            Ok(total)
        }
        Err(e) => Err(e),
    }
}

/// 差分更新のコアロジック（State非依存）
fn build_vector_index_incremental_inner(
    cache: &VectorCache,
    folder_path: &str,
    diff: &crate::infra::vector_cache::CacheDiff,
    cached: crate::infra::vector_cache::CachedEmbeddings,
    generator: &mut OnnxEmbeddingGenerator,
    cancel_token: Option<&AtomicBool>,
) -> Result<(HnswVectorIndex, u64), String> {
    use crate::domain::embedding::EmbeddingGenerator;
    use std::collections::HashSet;

    let changed_files: HashSet<&str> = diff
        .modified
        .iter()
        .chain(diff.deleted.iter())
        .map(|s| s.as_str())
        .collect();

    // 未変更ファイルのチャンク+embeddingを保持
    let mut all_metas = Vec::new();
    let mut all_embeddings = Vec::new();

    for (meta, embedding) in cached.metas.into_iter().zip(cached.embeddings.into_iter()) {
        if !changed_files.contains(meta.source_path.as_str()) {
            all_metas.push(meta);
            all_embeddings.push(embedding);
        }
    }

    // 未変更ファイルのパスを収集（途中保存用）
    let mut processed_files: HashSet<String> =
        all_metas.iter().map(|m| m.source_path.clone()).collect();

    // 追加・変更ファイルのチャンク分割 + embedding生成
    let target_files: Vec<&str> = diff
        .added
        .iter()
        .chain(diff.modified.iter())
        .map(|s| s.as_str())
        .collect();

    let mut new_chunks = Vec::new();
    for file_path in &target_files {
        let body = match std::fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let chunks = split_into_chunks(file_path, &body, 500, 100);
        new_chunks.extend(chunks);
    }

    let mut current_file = String::new();

    for chunk in new_chunks.iter() {
        if let Some(token) = cancel_token {
            if token.load(Ordering::Relaxed) {
                // 途中保存
                save_partial_vector_cache(
                    cache,
                    folder_path,
                    &all_metas,
                    &all_embeddings,
                    &processed_files,
                );
                return Err("ベクトルインデックス構築が中断された".to_string());
            }
        }

        // ファイル境界の検出
        if chunk.source_path != current_file {
            if !current_file.is_empty() {
                processed_files.insert(current_file.clone());
            }
            current_file = chunk.source_path.clone();
        }

        let text_with_prefix = format!("passage: {}", chunk.text);
        let embedding = generator
            .generate(&text_with_prefix)
            .map_err(|e| format!("embedding生成失敗: {}", e))?;

        all_metas.push(crate::infra::hnsw::ChunkMeta {
            chunk_id: 0,
            source_path: chunk.source_path.clone(),
            chunk_index: chunk.chunk_index,
            text: chunk.text.clone(),
        });
        all_embeddings.push(embedding);
    }

    // 最後のファイルを処理済みに追加
    if !current_file.is_empty() {
        processed_files.insert(current_file);
    }

    // chunk_idを振り直す
    for (i, meta) in all_metas.iter_mut().enumerate() {
        meta.chunk_id = i;
    }

    let total = all_metas.len() as u64;

    // HNSWを全体再構築
    let combined = crate::infra::vector_cache::CachedEmbeddings {
        metas: all_metas,
        embeddings: all_embeddings.clone(),
    };
    let vector_index = HnswVectorIndex::from_cache(combined);

    // キャッシュ保存
    if let Err(e) = cache.save(folder_path, vector_index.metas(), &all_embeddings) {
        eprintln!("ベクトルキャッシュ保存失敗（無視）: {}", e);
    }

    Ok((vector_index, total))
}

/// フルビルドでベクトルインデックスを構築する（State非依存）
fn build_vector_index_full_inner(
    app: &tauri::AppHandle,
    cache: &VectorCache,
    folder_path: &str,
    generator: &mut OnnxEmbeddingGenerator,
    cancel_token: &AtomicBool,
) -> Result<(HnswVectorIndex, u64), String> {
    use crate::domain::embedding::EmbeddingGenerator;
    use std::collections::HashSet;

    // ファイル走査・チャンク分割
    let mut all_chunks = Vec::new();
    for entry in walkdir::WalkDir::new(folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if cancel_token.load(Ordering::Relaxed) {
            return Err("ベクトルインデックス構築が中断された".to_string());
        }

        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "txt" && ext != "md" {
            continue;
        }
        let body = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let chunks = split_into_chunks(&path.to_string_lossy(), &body, 500, 100);
        all_chunks.extend(chunks);
    }

    let total = all_chunks.len() as u64;

    let mut all_metas: Vec<crate::infra::hnsw::ChunkMeta> = Vec::with_capacity(all_chunks.len());
    let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(all_chunks.len());

    // 処理済みファイルの追跡（途中保存用）
    let mut processed_files: HashSet<String> = HashSet::new();
    let mut current_file = String::new();

    let mut last_progress_time = std::time::Instant::now();

    // 開始時に total を通知
    let _ = app.emit(
        "vector-index-progress",
        serde_json::json!({ "current": 0, "total": total }),
    );

    for (i, chunk) in all_chunks.iter().enumerate() {
        if cancel_token.load(Ordering::Relaxed) {
            // current_fileは未完了なので processed_files に含めない
            // 処理済みファイル分のembeddingをキャッシュに途中保存する
            save_partial_vector_cache(
                cache,
                folder_path,
                &all_metas,
                &all_embeddings,
                &processed_files,
            );
            return Err("ベクトルインデックス構築が中断された".to_string());
        }

        // ファイル境界の検出
        if chunk.source_path != current_file {
            if !current_file.is_empty() {
                processed_files.insert(current_file.clone());
            }
            current_file = chunk.source_path.clone();
        }

        let text_with_prefix = format!("passage: {}", chunk.text);
        let embedding = generator
            .generate(&text_with_prefix)
            .map_err(|e| format!("embedding生成失敗: {}", e))?;

        all_metas.push(crate::infra::hnsw::ChunkMeta {
            chunk_id: i,
            source_path: chunk.source_path.clone(),
            chunk_index: chunk.chunk_index,
            text: chunk.text.clone(),
        });
        all_embeddings.push(embedding);

        if last_progress_time.elapsed() >= std::time::Duration::from_millis(200) {
            last_progress_time = std::time::Instant::now();
            let _ = app.emit(
                "vector-index-progress",
                serde_json::json!({
                    "current": i + 1,
                    "total": total,
                }),
            );
        }
    }

    // 最後のファイルを処理済みに追加
    if !current_file.is_empty() {
        processed_files.insert(current_file);
    }

    // HNSWインデックスを構築
    let cached = crate::infra::vector_cache::CachedEmbeddings {
        metas: all_metas,
        embeddings: all_embeddings.clone(),
    };
    let vector_index = HnswVectorIndex::from_cache(cached);

    if let Err(e) = cache.save(folder_path, vector_index.metas(), &all_embeddings) {
        eprintln!("ベクトルキャッシュ保存失敗（無視）: {}", e);
    }

    Ok((vector_index, total))
}

/// ベクトルembeddingの途中保存を行う
fn save_partial_vector_cache(
    cache: &VectorCache,
    folder_path: &str,
    metas: &[crate::infra::hnsw::ChunkMeta],
    embeddings: &[Vec<f32>],
    processed_files: &std::collections::HashSet<String>,
) {
    if processed_files.is_empty() {
        return;
    }

    // 処理済みファイルに属するチャンクのみを抽出
    let mut partial_metas = Vec::new();
    let mut partial_embeddings = Vec::new();
    for (meta, emb) in metas.iter().zip(embeddings.iter()) {
        if processed_files.contains(&meta.source_path) {
            partial_metas.push(crate::infra::hnsw::ChunkMeta {
                chunk_id: partial_metas.len(),
                source_path: meta.source_path.clone(),
                chunk_index: meta.chunk_index,
                text: meta.text.clone(),
            });
            partial_embeddings.push(emb.clone());
        }
    }

    let fingerprints = VectorCache::collect_fingerprints_for(processed_files);
    if let Err(e) = cache.save_partial(
        folder_path,
        &partial_metas,
        &partial_embeddings,
        fingerprints,
    ) {
        eprintln!("ベクトルキャッシュ途中保存失敗（無視）: {}", e);
    }
}

/// ハイブリッド検索を実行する
#[tauri::command]
pub fn hybrid_search(
    query: String,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<HybridSearchResult>, String> {
    // 全文検索
    let fulltext_results = {
        let guard = state.engine.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(engine) => engine.search(&query, limit).unwrap_or_default(),
            None => Vec::new(),
        }
    };

    // ベクトル検索
    let vector_results = {
        let mut model_guard = state.embedding_model.lock().map_err(|e| e.to_string())?;
        let vi_guard = state.vector_index.lock().map_err(|e| e.to_string())?;

        match (model_guard.as_mut(), vi_guard.as_ref()) {
            (Some(generator), Some(index)) => {
                let query_with_prefix = format!("query: {}", query);
                let query_embedding = generator
                    .generate(&query_with_prefix)
                    .map_err(|e| format!("クエリembedding生成失敗: {}", e))?;
                index
                    .search_nearest(&query_embedding, limit)
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        }
    };

    // RRFでスコア統合
    let fulltext_paths: Vec<String> = fulltext_results.iter().map(|r| r.path.clone()).collect();
    let vector_paths: Vec<String> = vector_results
        .iter()
        .map(|r| r.source_path.clone())
        .collect();
    let ranked = reciprocal_rank_fusion(&fulltext_paths, &vector_paths, 60.0);

    // 結果を構築（snippetは全文検索結果を優先、なければベクトル検索のチャンクテキスト）
    let results: Vec<HybridSearchResult> = ranked
        .into_iter()
        .take(limit)
        .map(|(path, score)| {
            let ft = fulltext_results.iter().find(|r| r.path == path);
            let vr = vector_results.iter().find(|r| r.source_path == path);

            let title = ft
                .map(|r| r.title.clone())
                .or_else(|| {
                    std::path::Path::new(&path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();

            let snippet = ft
                .map(|r| r.snippet.clone())
                .unwrap_or_else(|| vr.map(|r| r.text.clone()).unwrap_or_default());

            let source = match (ft.is_some(), vr.is_some()) {
                (true, true) => "hybrid",
                (true, false) => "fulltext",
                (false, true) => "vector",
                _ => "unknown",
            };

            HybridSearchResult {
                path,
                title,
                snippet,
                score,
                source: source.to_string(),
            }
        })
        .collect();

    Ok(results)
}

/// 利用可能なLLMモデル一覧を返す（プリセット + カスタム）
#[tauri::command]
pub fn list_available_models(state: State<'_, AppState>) -> Vec<LlmModelInfo> {
    state.model_registry.all_models()
}

/// LLMモデルをダウンロードする（サイズチェック + LRUエビクション付き）
#[tauri::command]
pub async fn download_llm_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    filename: String,
    url: String,
    size_bytes: u64,
) -> Result<Vec<String>, String> {
    let model_dir = state.model_dir.clone();
    let dest = model_dir.join(&filename);

    if dest.exists() {
        return Ok(Vec::new());
    }

    // ダウンロード前サイズチェック
    let cache_limit = state.settings_store.load().cache_limit_bytes;
    model::check_can_download(size_bytes, cache_limit)?;

    std::fs::create_dir_all(&model_dir).map_err(|e| format!("ディレクトリ作成失敗: {}", e))?;

    model::download_file_with_progress(&url, &dest, |progress| {
        let _ = app.emit("download-progress", progress);
    })
    .await?;

    // ダウンロード後LRUエビクション
    let loaded_llm = {
        let config_guard = state.loaded_llm_config.lock().map_err(|e| e.to_string())?;
        config_guard.as_ref().map(|c| c.filename.clone())
    };
    let embedding_loaded = {
        let guard = state.embedding_model.lock().map_err(|e| e.to_string())?;
        guard.is_some()
    };
    let evicted = model::evict_lru(
        &model_dir,
        cache_limit,
        loaded_llm.as_deref(),
        embedding_loaded,
    );

    Ok(evicted)
}

/// LLMモデルのロード結果
#[derive(serde::Serialize)]
pub struct LlmLoadResult {
    /// GPU推論が有効か
    pub gpu_active: bool,
    /// 使用されたGPUオフロード層数
    pub gpu_layers: u32,
}

/// LLMモデルをロードする（非同期: メインスレッドをブロックしない）
#[tauri::command]
pub async fn load_llm_model(
    filename: String,
    chat_template: String,
    context_length: u32,
    state: State<'_, AppState>,
) -> Result<LlmLoadResult, String> {
    let model_path = state.model_dir.join(&filename);
    if !model_path.exists() {
        return Err(format!(
            "モデルファイルが見つからない: {}",
            model_path.display()
        ));
    }

    // chat_template 文字列をデシリアライズ
    let template: ChatTemplate = serde_json::from_str(&format!("\"{}\"", chat_template))
        .map_err(|_| format!("不明なチャットテンプレート: {}", chat_template))?;

    // モデルファイルサイズを取得
    let model_size_bytes = std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0);

    // GPU VRAMを取得（最大VRAMを使用）
    let system_info = crate::infra::system::detect_system_info();
    let vram_mb = system_info
        .gpus
        .iter()
        .map(|g| g.vram_mb)
        .max()
        .unwrap_or(0);

    let model_path_str = model_path.to_str().unwrap_or("").to_string();

    // 重い処理をブロッキングスレッドで実行（メインスレッドを解放）
    let (engine, result) = tokio::task::spawn_blocking(move || {
        let engine = LlamaEngine::new(&model_path_str, model_size_bytes, vram_mb, context_length)
            .map_err(|e| format!("モデルロード失敗: {}", e))?;

        let result = LlmLoadResult {
            gpu_active: engine.is_gpu_active(),
            gpu_layers: engine.gpu_layers(),
        };
        Ok::<_, String>((engine, result))
    })
    .await
    .map_err(|e| format!("タスク実行失敗: {}", e))??;

    // ロード済みモデル設定を保存
    {
        let mut config_guard = state.loaded_llm_config.lock().map_err(|e| e.to_string())?;
        *config_guard = Some(LoadedLlmConfig {
            filename: filename.clone(),
            chat_template: template,
            context_length,
        });
    }

    let mut guard = state.llm_engine.lock().map_err(|e| e.to_string())?;
    *guard = Some(engine);

    Ok(result)
}

/// LLMがロード済みかどうか
#[tauri::command]
pub fn is_llm_ready(state: State<'_, AppState>) -> bool {
    state
        .llm_engine
        .lock()
        .map(|g| g.is_some())
        .unwrap_or(false)
}

/// ロード済みLLMモデルのファイル名を返す（未ロード時はnull）
#[tauri::command]
pub fn get_loaded_model_filename(state: State<'_, AppState>) -> Option<String> {
    state
        .loaded_llm_config
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|c| c.filename.clone()))
}

/// RAG質問応答（ストリーミング）
#[tauri::command]
pub fn chat(
    app: tauri::AppHandle,
    question: String,
    state: State<'_, AppState>,
) -> Result<RagAnswer, String> {
    // ハイブリッド検索でコンテキストを取得
    let context_chunks = {
        // 全文検索
        let fulltext_results = {
            let guard = state.engine.lock().map_err(|e| e.to_string())?;
            match guard.as_ref() {
                Some(engine) => engine.search(&question, 5).unwrap_or_default(),
                None => Vec::new(),
            }
        };

        // ベクトル検索
        let vector_results = {
            let mut model_guard = state.embedding_model.lock().map_err(|e| e.to_string())?;
            let vi_guard = state.vector_index.lock().map_err(|e| e.to_string())?;

            match (model_guard.as_mut(), vi_guard.as_ref()) {
                (Some(generator), Some(index)) => {
                    let query_with_prefix = format!("query: {}", question);
                    let query_embedding = generator
                        .generate(&query_with_prefix)
                        .map_err(|e| format!("embedding生成失敗: {}", e))?;
                    index
                        .search_nearest(&query_embedding, 5)
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            }
        };

        // RRFで統合してトップ5のファイルを取得
        let fulltext_paths: Vec<String> = fulltext_results.iter().map(|r| r.path.clone()).collect();
        let vector_paths: Vec<String> = vector_results
            .iter()
            .map(|r| r.source_path.clone())
            .collect();
        let ranked = reciprocal_rank_fusion(&fulltext_paths, &vector_paths, 60.0);

        // コンテキストチャンクを構築
        let mut chunks = Vec::new();
        for (path, _) in ranked.iter().take(5) {
            if let Ok(content) = std::fs::read_to_string(path) {
                // ファイル内容を最大1000文字に切り詰め
                let text: String = content.chars().take(1000).collect();
                chunks.push(ContextChunk {
                    path: path.clone(),
                    text,
                });
            }
        }
        chunks
    };

    // ロード済みモデルのテンプレートを取得（未設定時は ChatML）
    let template = {
        let config_guard = state.loaded_llm_config.lock().map_err(|e| e.to_string())?;
        config_guard
            .as_ref()
            .map(|c| c.chat_template.clone())
            .unwrap_or(crate::domain::llm::chat_template::ChatTemplate::Chatml)
    };

    // RAGプロンプトを構築
    let prompt = build_rag_prompt(&question, &context_chunks, &template);
    let source_paths: Vec<String> = context_chunks.iter().map(|c| c.path.clone()).collect();

    // LLM推論
    let mut llm_guard = state.llm_engine.lock().map_err(|e| e.to_string())?;
    let engine = llm_guard
        .as_mut()
        .ok_or_else(|| "LLMモデルがロードされていない".to_string())?;

    let answer = engine
        .generate(&prompt, 512, |token| {
            let _ = app.emit("chat-token", token);
        })
        .map_err(|e| format!("LLM推論失敗: {}", e))?;

    // 回答から参照元を抽出（LLMが参照元を明示した場合）
    let mut sources = extract_sources(&answer);
    if sources.is_empty() {
        sources = source_paths;
    }

    Ok(RagAnswer { answer, sources })
}

/// システム情報（RAM・GPU）を検出する
#[tauri::command]
pub fn detect_system_info() -> SystemInfo {
    crate::infra::system::detect_system_info()
}

/// システム情報に基づくモデル推奨リストを返す
#[tauri::command]
pub fn get_model_recommendations(state: State<'_, AppState>) -> Vec<ModelRecommendation> {
    let system = crate::infra::system::detect_system_info();
    let models = state.model_registry.all_models();
    recommend_models(&models, &system)
}

/// ダウンロード済みモデルの一覧を返す
#[tauri::command]
pub fn list_downloaded_models(state: State<'_, AppState>) -> Vec<DownloadedModelInfo> {
    model::list_downloaded_models(&state.model_dir)
}

/// モデルファイルを削除する
///
/// ロード中のモデル（LLM・embedding）の削除は拒否する
#[tauri::command]
pub fn delete_model(filename: String, state: State<'_, AppState>) -> Result<(), String> {
    // embeddingモデルがロード中の場合、embedding関連ファイルの削除を拒否
    {
        let guard = state.embedding_model.lock().map_err(|e| e.to_string())?;
        if guard.is_some() && (filename == "model.onnx" || filename == "tokenizer.json") {
            return Err("embeddingモデルがロード中のため削除できない".to_string());
        }
    }

    // ロード中のLLMモデルの削除を拒否
    {
        let config_guard = state.loaded_llm_config.lock().map_err(|e| e.to_string())?;
        if let Some(config) = config_guard.as_ref() {
            if config.filename == filename {
                return Err("ロード中のLLMモデルは削除できない".to_string());
            }
        }
    }

    model::delete_model_file(&state.model_dir, &filename)
}

/// モデルストレージの使用状況を返す
#[tauri::command]
pub fn get_storage_usage(state: State<'_, AppState>) -> StorageUsage {
    let mut usage = model::get_storage_usage(&state.model_dir);
    usage.cache_limit_bytes = state.settings_store.load().cache_limit_bytes;
    usage
}

/// カスタムモデルを登録する
#[tauri::command]
pub fn register_custom_model(
    model: LlmModelInfo,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.model_registry.add_model(model)
}

/// カスタムモデルの登録を解除する（DL済みファイルは保持）
#[tauri::command]
pub fn unregister_custom_model(filename: String, state: State<'_, AppState>) -> Result<(), String> {
    state.model_registry.remove_model(&filename)
}

/// 現在の設定を返す
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings_store.load()
}

/// 設定を保存する
#[tauri::command]
pub fn save_settings(settings: AppSettings, state: State<'_, AppState>) -> Result<(), String> {
    state.settings_store.save(&settings)
}

/// ロード中モデル以外の全キャッシュを削除する
#[tauri::command]
pub fn clear_model_cache(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let loaded_llm = {
        let config_guard = state.loaded_llm_config.lock().map_err(|e| e.to_string())?;
        config_guard.as_ref().map(|c| c.filename.clone())
    };
    let embedding_loaded = {
        let guard = state.embedding_model.lock().map_err(|e| e.to_string())?;
        guard.is_some()
    };
    Ok(model::clear_all_cache(
        &state.model_dir,
        loaded_llm.as_deref(),
        embedding_loaded,
    ))
}

/// インデックス検証結果
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexValidationResult {
    pub fulltext_removed: bool,
    pub vector_cache_removed: bool,
}

/// 選択フォルダのインデックスを検証する（フォルダ選択時に同期呼出）
///
/// バックグラウンド検証との競合を制御する。
/// - バックグラウンドがこのフォルダを検証中なら完了を待つ
/// - バックグラウンドが未到達ならスキップリストに入れて自分で検証
/// - バックグラウンドが検証済みなら再検証をスキップ
#[tauri::command]
pub fn validate_folder_indexes(
    app: tauri::AppHandle,
    folder_path: String,
    state: State<'_, AppState>,
) -> Result<IndexValidationResult, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let hash = vector_cache::folder_hash(&folder_path);
    let hash_dir = app_data_dir.join("index").join(&hash);
    let fulltext_path = hash_dir.join("fulltext");
    let cache_dir = hash_dir.join("vector");

    let validation = &state.index_validation;

    // バックグラウンド検証との競合制御
    {
        // reserved に追加（BGがまだ到達していなければスキップさせる）
        let mut reserved = validation.reserved.lock().map_err(|e| e.to_string())?;
        reserved.insert(hash.clone());
    }

    // BGが検証済みかチェック
    let already_validated = {
        let completed = validation.completed.lock().map_err(|e| e.to_string())?;
        completed.contains(&hash)
    };

    if !already_validated {
        // BGがこのフォルダを検証中なら完了を待つ
        {
            let mut current = validation.current_hash.lock().map_err(|e| e.to_string())?;
            while current.as_deref() == Some(&hash) {
                current = validation
                    .notify
                    .wait(current)
                    .map_err(|e| e.to_string())?;
            }
        }

        // BGが検証完了したか再チェック
        let validated_by_bg = {
            let completed = validation.completed.lock().map_err(|e| e.to_string())?;
            completed.contains(&hash)
        };

        if validated_by_bg {
            // BGが検証完了 → 破損していたらBGが既に削除済み
            return Ok(IndexValidationResult {
                fulltext_removed: false,
                vector_cache_removed: false,
            });
        }
    } else {
        return Ok(IndexValidationResult {
            fulltext_removed: false,
            vector_cache_removed: false,
        });
    }

    // 自分で検証する
    let mut fulltext_removed = false;
    let mut vector_cache_removed = false;

    // 全文検索インデックスの検証
    if !tantivy_infra::validate_index(&fulltext_path) {
        eprintln!("全文検索インデックスの破損を検出、削除: {:?}", fulltext_path);
        let _ = std::fs::remove_dir_all(&fulltext_path);
        fulltext_removed = true;
    }

    // ベクトルキャッシュの検証
    if cache_dir.exists() && !vector_cache::validate_cache_dir(&cache_dir) {
        eprintln!("ベクトルキャッシュの破損を検出、削除: {:?}", cache_dir);
        let _ = std::fs::remove_dir_all(&cache_dir);
        vector_cache_removed = true;
    }

    Ok(IndexValidationResult {
        fulltext_removed,
        vector_cache_removed,
    })
}

/// インデックス済みフォルダの情報
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexedFolderInfo {
    pub folder_path: String,
    pub has_fulltext: bool,
    pub vector_complete: bool,
}

/// インデックス済みフォルダの一覧を返す
#[tauri::command]
pub async fn list_indexed_folders(app: tauri::AppHandle) -> Result<Vec<IndexedFolderInfo>, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let cache = VectorCache::new(&app_data_dir);

    let mut folders = Vec::new();
    for hash_dir in cache.list_index_dirs() {
        if let Some(folder_path) = vector_cache::resolve_folder_path(&hash_dir) {
            let has_fulltext = hash_dir.join("fulltext").exists();
            let vector_complete = vector_cache::is_vector_complete(&hash_dir);
            folders.push(IndexedFolderInfo {
                folder_path,
                has_fulltext,
                vector_complete,
            });
        }
    }

    folders.sort_by(|a, b| a.folder_path.cmp(&b.folder_path));
    Ok(folders)
}

/// 既存インデックスのオープン結果
#[derive(Debug, Clone, serde::Serialize)]
pub struct OpenIndexedFolderResult {
    pub fulltext_count: u64,
    pub vector_chunk_count: u64,
}

/// 既存インデックスを読み込む（再構築しない）
#[tauri::command]
pub async fn open_indexed_folder(
    app: tauri::AppHandle,
    folder_path: String,
    state: State<'_, AppState>,
) -> Result<OpenIndexedFolderResult, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let hash = vector_cache::folder_hash(&folder_path);
    let index_path = app_data_dir
        .join("index")
        .join(&hash)
        .join("fulltext");

    // 既存のウォッチャーを停止
    {
        let mut watcher_guard = state.watcher.lock().map_err(|e| e.to_string())?;
        *watcher_guard = None;
    }

    // 全文検索エンジンを読み込む
    let fulltext_count = if index_path.exists() {
        let index_path_str = index_path.to_string_lossy().to_string();
        let engine = TantivySearchEngine::new(&index_path_str)
            .map_err(|e| format!("全文検索インデックス読み込み失敗: {}", e))?;
        let count = engine.status().file_count;
        let mut guard = state.engine.lock().map_err(|e| e.to_string())?;
        *guard = Some(engine);
        count
    } else {
        0
    };

    // ベクトルキャッシュを読み込む
    let vector_chunk_count = {
        let cache = VectorCache::new(&app_data_dir);
        if let Ok(cached) = cache.load(&folder_path) {
            let total = cached.metas.len() as u64;
            let vector_index = HnswVectorIndex::from_cache(cached);
            let mut guard = state.vector_index.lock().map_err(|e| e.to_string())?;
            *guard = Some(vector_index);
            total
        } else {
            0
        }
    };

    // フォルダパスを更新
    {
        let mut fp = state.folder_path.lock().map_err(|e| e.to_string())?;
        *fp = Some(folder_path.clone());
    }

    // ファイル監視を開始
    let app_handle = app.app_handle().clone();
    let watch_folder = folder_path.clone();
    match FileWatcher::start(&watch_folder, move |changed_files| {
        let state: tauri::State<'_, AppState> = app_handle.state();

        let fulltext_count = {
            let mut engine_guard = match state.engine.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if let Some(engine) = engine_guard.as_mut() {
                if let Err(e) = engine.update_files(&changed_files) {
                    eprintln!("全文検索インデックス更新失敗: {}", e);
                }
                engine.status().file_count
            } else {
                return;
            }
        };

        let vector_chunk_count = (|| -> Option<u64> {
            let app_data_dir = app_handle.path().app_data_dir().ok()?;
            let cache = VectorCache::new(&app_data_dir);
            let folder_path = state.folder_path.lock().ok()?.clone()?;

            let diff = cache.compute_diff(&folder_path)?;
            if !diff.has_changes() {
                return None;
            }

            let cached = cache.load(&folder_path).ok()?;
            let mut model_guard = state.embedding_model.try_lock().ok()?;
            let generator = model_guard.as_mut()?;

            match build_vector_index_incremental_inner(
                &cache,
                &folder_path,
                &diff,
                cached,
                generator,
                None,
            ) {
                Ok((vector_index, total)) => {
                    if let Ok(mut guard) = state.vector_index.lock() {
                        *guard = Some(vector_index);
                    }
                    Some(total)
                }
                Err(e) => {
                    eprintln!("ベクトルインデックス差分更新失敗: {}", e);
                    None
                }
            }
        })();

        let _ = app_handle.emit(
            "index-updated",
            serde_json::json!({
                "fulltext_count": fulltext_count,
                "vector_chunk_count": vector_chunk_count.unwrap_or(0),
            }),
        );
    }) {
        Ok(watcher) => {
            let mut watcher_guard = state.watcher.lock().map_err(|e| e.to_string())?;
            *watcher_guard = Some(watcher);
        }
        Err(e) => {
            eprintln!("ファイル監視開始失敗（無視）: {}", e);
        }
    }

    Ok(OpenIndexedFolderResult {
        fulltext_count,
        vector_chunk_count,
    })
}

/// インデックスを削除する
#[tauri::command]
pub async fn delete_indexed_folder(
    app: tauri::AppHandle,
    folder_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let hash = vector_cache::folder_hash(&folder_path);
    let hash_dir = app_data_dir.join("index").join(&hash);

    // 現在選択中のフォルダと同じ場合はエンジン・監視を解放
    let is_current = {
        let fp = state.folder_path.lock().map_err(|e| e.to_string())?;
        fp.as_deref() == Some(&folder_path)
    };

    if is_current {
        {
            let mut watcher_guard = state.watcher.lock().map_err(|e| e.to_string())?;
            *watcher_guard = None;
        }
        {
            let mut guard = state.engine.lock().map_err(|e| e.to_string())?;
            *guard = None;
        }
        {
            let mut guard = state.vector_index.lock().map_err(|e| e.to_string())?;
            *guard = None;
        }
        {
            let mut fp = state.folder_path.lock().map_err(|e| e.to_string())?;
            *fp = None;
        }
    }

    if hash_dir.exists() {
        std::fs::remove_dir_all(&hash_dir)
            .map_err(|e| format!("インデックス削除失敗: {}", e))?;
    }

    Ok(())
}
