use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::embedding::Embedding;
use crate::infra::hnsw::ChunkMeta;

const FORMAT_VERSION: u32 = 1;

/// キャッシュのマニフェスト
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheManifest {
    pub format_version: u32,
    pub folder_path: String,
    pub file_fingerprints: HashMap<String, FileFingerprint>,
    pub chunk_count: usize,
    pub embedding_dimension: usize,
}

/// ファイルのフィンガープリント
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FileFingerprint {
    pub size: u64,
    pub modified: u64,
}

/// キャッシュされたembeddingデータ
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedEmbeddings {
    pub metas: Vec<ChunkMeta>,
    pub embeddings: Vec<Embedding>,
}

/// ベクトルインデックスのキャッシュ管理
pub struct VectorCache {
    base_dir: PathBuf,
}

impl VectorCache {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            base_dir: app_data_dir.join("index").join("vector"),
        }
    }

    /// フォルダパスからキャッシュディレクトリを算出する
    pub fn cache_dir_for(&self, folder_path: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(folder_path.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.base_dir.join(&hash[..16])
    }

    /// フォルダ内の対象ファイルのフィンガープリントを収集する
    pub fn scan_fingerprints(folder_path: &str) -> HashMap<String, FileFingerprint> {
        let mut fingerprints = HashMap::new();

        for entry in walkdir::WalkDir::new(folder_path)
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
            if let Ok(metadata) = std::fs::metadata(path) {
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                fingerprints.insert(
                    path.to_string_lossy().to_string(),
                    FileFingerprint {
                        size: metadata.len(),
                        modified,
                    },
                );
            }
        }

        fingerprints
    }

    /// キャッシュが有効かどうかを判定する
    pub fn is_cache_valid(&self, folder_path: &str) -> bool {
        let cache_dir = self.cache_dir_for(folder_path);
        let manifest_path = cache_dir.join("manifest.json");
        let embeddings_path = cache_dir.join("embeddings.bin");

        if !manifest_path.exists() || !embeddings_path.exists() {
            return false;
        }

        let manifest: CacheManifest = match std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(m) => m,
            None => return false,
        };

        if manifest.format_version != FORMAT_VERSION {
            return false;
        }

        let current = Self::scan_fingerprints(folder_path);
        manifest.file_fingerprints == current
    }

    /// キャッシュからembeddingデータをロードする
    pub fn load(&self, folder_path: &str) -> Result<CachedEmbeddings, String> {
        let cache_dir = self.cache_dir_for(folder_path);
        let embeddings_path = cache_dir.join("embeddings.bin");

        let data =
            std::fs::read(&embeddings_path).map_err(|e| format!("キャッシュ読み込み失敗: {}", e))?;

        bincode::deserialize(&data).map_err(|e| format!("キャッシュデシリアライズ失敗: {}", e))
    }

    /// embeddingデータをキャッシュに保存する
    pub fn save(
        &self,
        folder_path: &str,
        metas: &[ChunkMeta],
        embeddings: &[Embedding],
    ) -> Result<(), String> {
        let cache_dir = self.cache_dir_for(folder_path);
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("キャッシュディレクトリ作成失敗: {}", e))?;

        let cached = CachedEmbeddings {
            metas: metas.to_vec(),
            embeddings: embeddings.to_vec(),
        };
        let bin =
            bincode::serialize(&cached).map_err(|e| format!("キャッシュシリアライズ失敗: {}", e))?;
        std::fs::write(cache_dir.join("embeddings.bin"), bin)
            .map_err(|e| format!("キャッシュ書き込み失敗: {}", e))?;

        let dim = embeddings.first().map(|e| e.len()).unwrap_or(0);
        let manifest = CacheManifest {
            format_version: FORMAT_VERSION,
            folder_path: folder_path.to_string(),
            file_fingerprints: Self::scan_fingerprints(folder_path),
            chunk_count: metas.len(),
            embedding_dimension: dim,
        };
        let json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("マニフェストシリアライズ失敗: {}", e))?;
        std::fs::write(cache_dir.join("manifest.json"), json)
            .map_err(|e| format!("マニフェスト書き込み失敗: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_cache_dir_hash_consistency() {
        let cache = VectorCache::new(Path::new("/tmp/test-cache"));
        let dir1 = cache.cache_dir_for("/home/user/docs");
        let dir2 = cache.cache_dir_for("/home/user/docs");
        assert_eq!(dir1, dir2);
    }

    #[test]
    fn test_cache_dir_different_paths() {
        let cache = VectorCache::new(Path::new("/tmp/test-cache"));
        let dir1 = cache.cache_dir_for("/home/user/docs");
        let dir2 = cache.cache_dir_for("/home/user/other");
        assert_ne!(dir1, dir2);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());

        let folder_dir = tempfile::tempdir().unwrap();
        let test_file = folder_dir.path().join("test.md");
        {
            let mut f = std::fs::File::create(&test_file).unwrap();
            writeln!(f, "テスト文書").unwrap();
        }

        let folder_path = folder_dir.path().to_str().unwrap();
        let metas = vec![ChunkMeta {
            chunk_id: 0,
            source_path: test_file.to_string_lossy().to_string(),
            chunk_index: 0,
            text: "テスト文書".to_string(),
        }];
        let embeddings = vec![vec![0.1f32, 0.2, 0.3]];

        cache.save(folder_path, &metas, &embeddings).unwrap();

        let loaded = cache.load(folder_path).unwrap();
        assert_eq!(loaded.metas.len(), 1);
        assert_eq!(loaded.metas[0].text, "テスト文書");
        assert_eq!(loaded.embeddings.len(), 1);
        assert_eq!(loaded.embeddings[0], vec![0.1f32, 0.2, 0.3]);
    }

    #[test]
    fn test_cache_valid_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());

        let folder_dir = tempfile::tempdir().unwrap();
        let test_file = folder_dir.path().join("test.md");
        {
            let mut f = std::fs::File::create(&test_file).unwrap();
            writeln!(f, "テスト").unwrap();
        }

        let folder_path = folder_dir.path().to_str().unwrap();
        let metas = vec![ChunkMeta {
            chunk_id: 0,
            source_path: test_file.to_string_lossy().to_string(),
            chunk_index: 0,
            text: "テスト".to_string(),
        }];
        let embeddings = vec![vec![0.1f32; 384]];

        cache.save(folder_path, &metas, &embeddings).unwrap();
        assert!(cache.is_cache_valid(folder_path));
    }

    #[test]
    fn test_cache_invalid_after_file_change() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());

        let folder_dir = tempfile::tempdir().unwrap();
        let test_file = folder_dir.path().join("test.md");
        {
            let mut f = std::fs::File::create(&test_file).unwrap();
            writeln!(f, "テスト").unwrap();
        }

        let folder_path = folder_dir.path().to_str().unwrap();
        cache
            .save(folder_path, &[], &[])
            .unwrap();

        // ファイルを追加
        let new_file = folder_dir.path().join("new.txt");
        {
            let mut f = std::fs::File::create(&new_file).unwrap();
            writeln!(f, "新規").unwrap();
        }

        assert!(!cache.is_cache_valid(folder_path));
    }

    #[test]
    fn test_cache_invalid_when_not_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());
        assert!(!cache.is_cache_valid("/nonexistent/path"));
    }
}
