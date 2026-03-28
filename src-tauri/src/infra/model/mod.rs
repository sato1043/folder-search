use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest;
use tokio::io::AsyncWriteExt;

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
}
