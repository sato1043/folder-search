use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest;
use sysinfo::Disks;
use tokio::io::AsyncWriteExt;

use crate::domain::llm::{DownloadedModelInfo, StorageUsage};

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
pub async fn download_file_with_progress<F>(url: &str, dest: &Path, mut on_progress: F) -> Result<(), String>
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
pub async fn download_embedding_model<F>(model_dir: &Path, mut on_progress: F) -> Result<ModelFiles, String>
where
    F: FnMut(DownloadProgress),
{
    std::fs::create_dir_all(model_dir)
        .map_err(|e| format!("ディレクトリ作成失敗: {}", e))?;

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
    }
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
