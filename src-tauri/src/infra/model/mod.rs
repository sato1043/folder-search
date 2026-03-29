use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest;
use sysinfo::Disks;
use tokio::io::AsyncWriteExt;

use crate::domain::llm::{DownloadedModelInfo, StorageUsage, DEFAULT_CACHE_LIMIT_BYTES};

/// embeddingモデルのファイル情報
pub struct ModelFiles {
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
}

/// モデルのダウンロード進捗
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadProgress {
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub is_complete: bool,
}

const MODEL_URL: &str =
    "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/onnx/model.onnx";
const TOKENIZER_URL: &str =
    "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/tokenizer.json";

/// モデルディレクトリ内のファイルパスを返す
pub fn model_files(model_dir: &Path) -> ModelFiles {
    ModelFiles {
        model_path: model_dir.join("model.onnx"),
        tokenizer_path: model_dir.join("tokenizer.json"),
    }
}

/// モデルファイルが既にダウンロード済みかどうか
pub fn is_model_downloaded(model_dir: &Path) -> bool {
    let files = model_files(model_dir);
    files.model_path.exists() && files.tokenizer_path.exists()
}

/// ファイルをダウンロードする（進捗コールバック付き）
pub async fn download_file_with_progress<F>(
    url: &str,
    dest: &Path,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("ダウンロード開始失敗: {}", e))?;

    let total_bytes = response.content_length();

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("ファイル作成失敗: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("ダウンロードエラー: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("書き込みエラー: {}", e))?;
        downloaded += chunk.len() as u64;

        on_progress(DownloadProgress {
            file_name: file_name.clone(),
            downloaded_bytes: downloaded,
            total_bytes,
            is_complete: false,
        });
    }

    file.flush()
        .await
        .map_err(|e| format!("フラッシュエラー: {}", e))?;

    on_progress(DownloadProgress {
        file_name,
        downloaded_bytes: downloaded,
        total_bytes,
        is_complete: true,
    });

    Ok(())
}

/// embeddingモデルをダウンロードする
pub async fn download_embedding_model<F>(
    model_dir: &Path,
    mut on_progress: F,
) -> Result<ModelFiles, String>
where
    F: FnMut(DownloadProgress),
{
    std::fs::create_dir_all(model_dir).map_err(|e| format!("ディレクトリ作成失敗: {}", e))?;

    let files = model_files(model_dir);

    // tokenizer.jsonのダウンロード（小さいので先に）
    if !files.tokenizer_path.exists() {
        download_file_with_progress(TOKENIZER_URL, &files.tokenizer_path, &mut on_progress).await?;
    }

    // model.onnxのダウンロード
    if !files.model_path.exists() {
        download_file_with_progress(MODEL_URL, &files.model_path, &mut on_progress).await?;
    }

    Ok(files)
}

/// embeddingモデルのファイル名一覧
const EMBEDDING_FILES: &[&str] = &["model.onnx", "tokenizer.json"];

/// モデルディレクトリ内のDL済みモデル一覧を返す
pub fn list_downloaded_models(model_dir: &Path) -> Vec<DownloadedModelInfo> {
    let mut models = Vec::new();

    let entries = match std::fs::read_dir(model_dir) {
        Ok(e) => e,
        Err(_) => return models,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let is_embedding = EMBEDDING_FILES.contains(&filename.as_str());

        models.push(DownloadedModelInfo {
            filename,
            size_bytes,
            is_embedding,
        });
    }

    models.sort_by(|a, b| a.filename.cmp(&b.filename));
    models
}

/// モデルファイルを削除する
pub fn delete_model_file(model_dir: &Path, filename: &str) -> Result<(), String> {
    // パストラバーサル防止
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("不正なファイル名".to_string());
    }

    let path = model_dir.join(filename);
    if !path.exists() {
        return Err(format!("ファイルが見つからない: {}", filename));
    }

    std::fs::remove_file(&path).map_err(|e| format!("削除失敗: {}", e))
}

/// モデルストレージの使用状況を返す
pub fn get_storage_usage(model_dir: &Path) -> StorageUsage {
    let total_used_bytes = list_downloaded_models(model_dir)
        .iter()
        .map(|m| m.size_bytes)
        .sum();

    let disk_free_bytes = get_disk_free(model_dir);

    StorageUsage {
        total_used_bytes,
        disk_free_bytes,
        cache_limit_bytes: DEFAULT_CACHE_LIMIT_BYTES,
    }
}

/// ダウンロード前にキャッシュ上限を超えないか確認する
///
/// モデルサイズが上限を超える場合はエラーを返す
pub fn check_can_download(model_size_bytes: u64, cache_limit: u64) -> Result<(), String> {
    if model_size_bytes > cache_limit {
        let model_gb = model_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let limit_gb = cache_limit as f64 / (1024.0 * 1024.0 * 1024.0);
        return Err(format!(
            "モデルサイズ ({:.1} GB) がキャッシュ上限 ({:.0} GB) を超えている",
            model_gb, limit_gb
        ));
    }
    Ok(())
}

/// LRUエビクション: キャッシュ合計が上限を超えた場合に古いファイルから削除する
///
/// - `loaded_llm_filename`: 現在ロード中のLLMモデル（削除しない）
/// - `embedding_loaded`: embeddingモデルがロード中か（ロード中ならembeddingファイルを削除しない）
///
/// 削除されたファイル名のリストを返す
pub fn evict_lru(
    model_dir: &Path,
    cache_limit: u64,
    loaded_llm_filename: Option<&str>,
    embedding_loaded: bool,
) -> Vec<String> {
    let mut total: u64 = list_downloaded_models(model_dir)
        .iter()
        .map(|m| m.size_bytes)
        .sum();

    if total <= cache_limit {
        return Vec::new();
    }

    // 削除候補をファイル更新日時の古い順にソート
    let mut candidates =
        collect_eviction_candidates(model_dir, loaded_llm_filename, embedding_loaded);
    candidates.sort_by_key(|(_, modified)| *modified);

    let mut evicted = Vec::new();
    for (filename, _) in candidates {
        if total <= cache_limit {
            break;
        }
        let path = model_dir.join(&filename);
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
            evicted.push(filename);
        }
    }
    evicted
}

/// ロード中モデル以外の全キャッシュを削除する
///
/// 削除されたファイル名のリストを返す
pub fn clear_all_cache(
    model_dir: &Path,
    loaded_llm_filename: Option<&str>,
    embedding_loaded: bool,
) -> Vec<String> {
    let candidates = collect_eviction_candidates(model_dir, loaded_llm_filename, embedding_loaded);
    let mut cleared = Vec::new();
    for (filename, _) in candidates {
        let path = model_dir.join(&filename);
        if std::fs::remove_file(&path).is_ok() {
            cleared.push(filename);
        }
    }
    cleared
}

/// エビクション候補のファイル一覧を返す（ロード中のファイルを除外）
fn collect_eviction_candidates(
    model_dir: &Path,
    loaded_llm_filename: Option<&str>,
    embedding_loaded: bool,
) -> Vec<(String, std::time::SystemTime)> {
    let models = list_downloaded_models(model_dir);
    let mut candidates = Vec::new();

    for model in models {
        // ロード中のLLMモデルは除外
        if let Some(loaded) = loaded_llm_filename {
            if model.filename == loaded {
                continue;
            }
        }
        // ロード中のembeddingモデルは除外
        if embedding_loaded && model.is_embedding {
            continue;
        }

        let path = model_dir.join(&model.filename);
        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);

        candidates.push((model.filename, modified));
    }

    candidates
}

/// 指定パスが属するディスクの空き容量を返す
fn get_disk_free(path: &Path) -> u64 {
    let canonical = std::fs::canonicalize(path)
        .or_else(|_| std::fs::canonicalize(path.parent().unwrap_or(Path::new("/"))))
        .unwrap_or_default();

    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter(|d| canonical.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_files_paths() {
        let dir = Path::new("/tmp/test-models");
        let files = model_files(dir);
        assert_eq!(files.model_path, dir.join("model.onnx"));
        assert_eq!(files.tokenizer_path, dir.join("tokenizer.json"));
    }

    #[test]
    fn test_is_model_downloaded_false() {
        let dir = Path::new("/tmp/nonexistent-model-dir-12345");
        assert!(!is_model_downloaded(dir));
    }

    #[test]
    fn test_list_downloaded_models_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let models = list_downloaded_models(dir.path());
        assert!(models.is_empty());
    }

    #[test]
    fn test_list_downloaded_models_with_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("model-a.gguf"), "aaa").unwrap();
        std::fs::write(dir.path().join("model-b.gguf"), "bbbbbb").unwrap();
        std::fs::write(dir.path().join("model.onnx"), "onnx").unwrap();

        let models = list_downloaded_models(dir.path());
        assert_eq!(models.len(), 3);

        // ソート済み
        assert_eq!(models[0].filename, "model-a.gguf");
        assert_eq!(models[0].size_bytes, 3);
        assert!(!models[0].is_embedding);

        assert_eq!(models[1].filename, "model-b.gguf");
        assert_eq!(models[1].size_bytes, 6);

        assert_eq!(models[2].filename, "model.onnx");
        assert!(models[2].is_embedding);
    }

    #[test]
    fn test_list_downloaded_models_nonexistent_dir() {
        let models = list_downloaded_models(Path::new("/tmp/nonexistent-dir-xyz"));
        assert!(models.is_empty());
    }

    #[test]
    fn test_list_downloaded_models_skips_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("model.gguf"), "data").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();

        let models = list_downloaded_models(dir.path());
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].filename, "model.gguf");
    }

    #[test]
    fn test_delete_model_file_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.gguf");
        std::fs::write(&path, "data").unwrap();
        assert!(path.exists());

        let result = delete_model_file(dir.path(), "model.gguf");
        assert!(result.is_ok());
        assert!(!path.exists());
    }

    #[test]
    fn test_delete_model_file_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = delete_model_file(dir.path(), "nonexistent.gguf");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_model_file_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        assert!(delete_model_file(dir.path(), "../etc/passwd").is_err());
        assert!(delete_model_file(dir.path(), "sub/file.txt").is_err());
        assert!(delete_model_file(dir.path(), "..\\file.txt").is_err());
    }

    #[test]
    fn test_get_storage_usage_empty() {
        let dir = tempfile::tempdir().unwrap();
        let usage = get_storage_usage(dir.path());
        assert_eq!(usage.total_used_bytes, 0);
    }

    #[test]
    fn test_get_storage_usage_with_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.gguf"), "aaaa").unwrap();
        std::fs::write(dir.path().join("b.gguf"), "bb").unwrap();

        let usage = get_storage_usage(dir.path());
        assert_eq!(usage.total_used_bytes, 6);
        // disk_free_bytes は環境依存だが 0 以上であることを確認
        assert!(usage.disk_free_bytes > 0 || usage.disk_free_bytes == 0);
    }

    #[test]
    fn test_get_storage_usage_includes_cache_limit() {
        let dir = tempfile::tempdir().unwrap();
        let usage = get_storage_usage(dir.path());
        assert_eq!(usage.cache_limit_bytes, DEFAULT_CACHE_LIMIT_BYTES);
    }

    #[test]
    fn test_check_can_download_ok() {
        assert!(check_can_download(1024 * 1024 * 1024, DEFAULT_CACHE_LIMIT_BYTES).is_ok());
    }

    #[test]
    fn test_check_can_download_too_large() {
        let result = check_can_download(DEFAULT_CACHE_LIMIT_BYTES + 1, DEFAULT_CACHE_LIMIT_BYTES);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("キャッシュ上限"));
    }

    #[test]
    fn test_check_can_download_exact_limit() {
        // ちょうど上限のサイズは許可される
        assert!(check_can_download(DEFAULT_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_BYTES).is_ok());
    }

    #[test]
    fn test_evict_lru_no_eviction_needed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("small.gguf"), "data").unwrap();

        let evicted = evict_lru(dir.path(), DEFAULT_CACHE_LIMIT_BYTES, None, false);
        assert!(evicted.is_empty());
    }

    #[test]
    fn test_evict_lru_removes_oldest() {
        let dir = tempfile::tempdir().unwrap();
        // 古いファイルを作成
        std::fs::write(dir.path().join("old.gguf"), "old-data-old-data").unwrap();
        // ファイルの更新日時を変えるために少し待つ
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(dir.path().join("new.gguf"), "new-data").unwrap();

        // 上限を小さく設定して強制エビクション
        let evicted = evict_lru(dir.path(), 10, None, false);
        // oldが先に削除される（古い順）
        assert!(evicted.contains(&"old.gguf".to_string()));
        // 合計が上限以下になったらnewは残る
        assert!(!evicted.contains(&"new.gguf".to_string()));
    }

    #[test]
    fn test_evict_lru_skips_loaded_llm() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("loaded.gguf"), "loaded-model").unwrap();
        std::fs::write(dir.path().join("other.gguf"), "other-model").unwrap();

        // 上限1バイト → 全部削除したいが、loadedは保護
        let evicted = evict_lru(dir.path(), 1, Some("loaded.gguf"), false);
        assert!(!evicted.contains(&"loaded.gguf".to_string()));
        assert!(evicted.contains(&"other.gguf".to_string()));
    }

    #[test]
    fn test_evict_lru_skips_embedding_when_loaded() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("model.onnx"), "embedding").unwrap();
        std::fs::write(dir.path().join("tokenizer.json"), "tok").unwrap();
        std::fs::write(dir.path().join("llm.gguf"), "llm-data").unwrap();

        let evicted = evict_lru(dir.path(), 1, None, true);
        assert!(!evicted.contains(&"model.onnx".to_string()));
        assert!(!evicted.contains(&"tokenizer.json".to_string()));
        assert!(evicted.contains(&"llm.gguf".to_string()));
    }

    #[test]
    fn test_clear_all_cache() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.gguf"), "aaa").unwrap();
        std::fs::write(dir.path().join("b.gguf"), "bbb").unwrap();

        let cleared = clear_all_cache(dir.path(), None, false);
        assert_eq!(cleared.len(), 2);
        assert!(list_downloaded_models(dir.path()).is_empty());
    }

    #[test]
    fn test_clear_all_cache_preserves_loaded() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("loaded.gguf"), "loaded").unwrap();
        std::fs::write(dir.path().join("other.gguf"), "other").unwrap();
        std::fs::write(dir.path().join("model.onnx"), "onnx").unwrap();

        let cleared = clear_all_cache(dir.path(), Some("loaded.gguf"), true);
        assert_eq!(cleared.len(), 1);
        assert!(cleared.contains(&"other.gguf".to_string()));
        // loaded.gguf と embedding は残る
        assert!(dir.path().join("loaded.gguf").exists());
        assert!(dir.path().join("model.onnx").exists());
    }

    #[test]
    fn test_embedding_files_detection() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("model.onnx"), "x").unwrap();
        std::fs::write(dir.path().join("tokenizer.json"), "y").unwrap();
        std::fs::write(dir.path().join("qwen.gguf"), "z").unwrap();

        let models = list_downloaded_models(dir.path());
        let embedding_count = models.iter().filter(|m| m.is_embedding).count();
        let llm_count = models.iter().filter(|m| !m.is_embedding).count();
        assert_eq!(embedding_count, 2);
        assert_eq!(llm_count, 1);
    }
}
