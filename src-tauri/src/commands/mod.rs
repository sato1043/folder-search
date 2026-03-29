use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{Emitter, State};

use crate::domain::embedding::{EmbeddingGenerator, VectorSearcher};
use crate::domain::indexer::chunker::split_into_chunks;
use crate::domain::indexer::{Indexer, IndexStatus};
use crate::domain::llm::rag::{build_rag_prompt, extract_sources, ContextChunk, RagAnswer};
use crate::domain::llm::{available_models, LlmInference, LlmModelInfo};
use crate::domain::search::hybrid::{reciprocal_rank_fusion, HybridSearchResult};
use crate::domain::search::{FulltextSearcher, SearchResult};
use crate::infra::hnsw::HnswVectorIndex;
use crate::infra::llama::LlamaEngine;
use crate::infra::model;
use crate::infra::onnx::OnnxEmbeddingGenerator;
use crate::infra::tantivy::TantivySearchEngine;
use crate::infra::vector_cache::VectorCache;

use tauri::Manager;

/// アプリの状態
pub struct AppState {
    pub engine: Mutex<Option<TantivySearchEngine>>,
    pub vector_index: Mutex<Option<HnswVectorIndex>>,
    pub embedding_model: Mutex<Option<OnnxEmbeddingGenerator>>,
    pub llm_engine: Mutex<Option<LlamaEngine>>,
    pub model_dir: PathBuf,
    pub folder_path: Mutex<Option<String>>,
}

/// 全文検索インデックスを構築する
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

    let mut fp = state.folder_path.lock().map_err(|e| e.to_string())?;
    *fp = Some(folder_path);

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
pub fn build_vector_index(
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

    // キャッシュが有効ならロードして返す
    if cache.is_cache_valid(&folder_path) {
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

    // キャッシュ無効 → フルビルド
    let mut model_guard = state.embedding_model.lock().map_err(|e| e.to_string())?;
    let generator = model_guard
        .as_mut()
        .ok_or_else(|| "embeddingモデルがロードされていない".to_string())?;

    // ファイル走査・チャンク分割
    let mut all_chunks = Vec::new();
    for entry in walkdir::WalkDir::new(&folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
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
        let chunks = split_into_chunks(
            &path.to_string_lossy(),
            &body,
            500,  // チャンクサイズ: 500文字
            100,  // オーバーラップ: 100文字
        );
        all_chunks.extend(chunks);
    }

    let total = all_chunks.len() as u64;

    // embedding生成 + HNSWインデックス構築
    let mut vector_index = HnswVectorIndex::new();
    let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(all_chunks.len());

    let progress_interval = std::cmp::max(total as usize / 100, 1);

    for (i, chunk) in all_chunks.iter().enumerate() {
        let text_with_prefix = format!("passage: {}", chunk.text);
        let embedding = generator
            .generate(&text_with_prefix)
            .map_err(|e| format!("embedding生成失敗: {}", e))?;
        vector_index.add(chunk, &embedding);
        all_embeddings.push(embedding);

        if i % progress_interval == 0 {
            let _ = app.emit(
                "vector-index-progress",
                serde_json::json!({
                    "current": i + 1,
                    "total": total,
                }),
            );
        }
    }

    // キャッシュ保存（失敗しても続行）
    if let Err(e) = cache.save(&folder_path, vector_index.metas(), &all_embeddings) {
        eprintln!("ベクトルキャッシュ保存失敗（無視）: {}", e);
    }

    let mut guard = state.vector_index.lock().map_err(|e| e.to_string())?;
    *guard = Some(vector_index);

    Ok(total)
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
                index.search_nearest(&query_embedding, limit).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    };

    // RRFでスコア統合
    let fulltext_paths: Vec<String> = fulltext_results.iter().map(|r| r.path.clone()).collect();
    let vector_paths: Vec<String> = vector_results.iter().map(|r| r.source_path.clone()).collect();
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

/// 利用可能なLLMモデル一覧を返す
#[tauri::command]
pub fn list_available_models() -> Vec<LlmModelInfo> {
    available_models()
}

/// LLMモデルをダウンロードする
#[tauri::command]
pub async fn download_llm_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    filename: String,
    url: String,
) -> Result<(), String> {
    let model_dir = state.model_dir.clone();
    let dest = model_dir.join(&filename);

    if dest.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(&model_dir)
        .map_err(|e| format!("ディレクトリ作成失敗: {}", e))?;

    model::download_file_with_progress(&url, &dest, |progress| {
        let _ = app.emit("download-progress", progress);
    })
    .await?;

    Ok(())
}

/// LLMモデルをロードする
#[tauri::command]
pub fn load_llm_model(
    filename: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let model_path = state.model_dir.join(&filename);
    if !model_path.exists() {
        return Err(format!("モデルファイルが見つからない: {}", model_path.display()));
    }

    let engine = LlamaEngine::new(model_path.to_str().unwrap_or(""))
        .map_err(|e| format!("モデルロード失敗: {}", e))?;

    let mut guard = state.llm_engine.lock().map_err(|e| e.to_string())?;
    *guard = Some(engine);

    Ok(())
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
                    index.search_nearest(&query_embedding, 5).unwrap_or_default()
                }
                _ => Vec::new(),
            }
        };

        // RRFで統合してトップ5のファイルを取得
        let fulltext_paths: Vec<String> = fulltext_results.iter().map(|r| r.path.clone()).collect();
        let vector_paths: Vec<String> =
            vector_results.iter().map(|r| r.source_path.clone()).collect();
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

    // RAGプロンプトを構築
    let prompt = build_rag_prompt(&question, &context_chunks);
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
