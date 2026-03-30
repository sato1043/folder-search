use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::embedding::Embedding;
use crate::infra::hnsw::ChunkMeta;

const FORMAT_VERSION: u32 = 1;

/// フォルダパスからキャッシュ用ハッシュ文字列（SHA256先頭16文字）を生成する
pub fn folder_hash(folder_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(folder_path.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    hash[..16].to_string()
}

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

/// キャッシュとの差分情報
#[derive(Debug)]
pub struct CacheDiff {
    pub unchanged: Vec<String>,
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
}

impl CacheDiff {
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.modified.is_empty() || !self.deleted.is_empty()
    }
}

/// ベクトルインデックスのキャッシュ管理
pub struct VectorCache {
    base_dir: PathBuf,
}

impl VectorCache {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            base_dir: app_data_dir.join("index"),
        }
    }

    /// フォルダパスからキャッシュディレクトリを算出する
    pub fn cache_dir_for(&self, folder_path: &str) -> PathBuf {
        self.base_dir
            .join(folder_hash(folder_path))
            .join("vector")
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

    /// キャッシュとの差分を計算する
    ///
    /// キャッシュが存在しない場合はNoneを返す。
    pub fn compute_diff(&self, folder_path: &str) -> Option<CacheDiff> {
        let cache_dir = self.cache_dir_for(folder_path);
        let manifest_path = cache_dir.join("manifest.json");
        let embeddings_path = cache_dir.join("embeddings.bin");

        if !manifest_path.exists() || !embeddings_path.exists() {
            return None;
        }

        let manifest: CacheManifest = std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())?;

        if manifest.format_version != FORMAT_VERSION {
            return None;
        }

        let current = Self::scan_fingerprints(folder_path);
        let cached = &manifest.file_fingerprints;

        let mut unchanged = Vec::new();
        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut deleted = Vec::new();

        // 現在のファイルをチェック
        for (path, fp) in &current {
            match cached.get(path) {
                Some(cached_fp) if cached_fp == fp => unchanged.push(path.clone()),
                Some(_) => modified.push(path.clone()),
                None => added.push(path.clone()),
            }
        }

        // 削除されたファイルをチェック
        for path in cached.keys() {
            if !current.contains_key(path) {
                deleted.push(path.clone());
            }
        }

        Some(CacheDiff {
            unchanged,
            added,
            modified,
            deleted,
        })
    }

    /// キャッシュからembeddingデータをロードする
    pub fn load(&self, folder_path: &str) -> Result<CachedEmbeddings, String> {
        let cache_dir = self.cache_dir_for(folder_path);
        let embeddings_path = cache_dir.join("embeddings.bin");

        let data = std::fs::read(&embeddings_path)
            .map_err(|e| format!("キャッシュ読み込み失敗: {}", e))?;

        bincode::deserialize(&data).map_err(|e| format!("キャッシュデシリアライズ失敗: {}", e))
    }

    /// embeddingデータをキャッシュに保存する
    pub fn save(
        &self,
        folder_path: &str,
        metas: &[ChunkMeta],
        embeddings: &[Embedding],
    ) -> Result<(), String> {
        let fingerprints = Self::scan_fingerprints(folder_path);
        self.save_with_fingerprints(folder_path, metas, embeddings, fingerprints)
    }

    /// embeddingデータをキャッシュに保存する（フィンガープリント明示指定版）
    ///
    /// 途中保存時に使用する。処理済みファイルのフィンガープリントのみを渡すことで、
    /// 次回の `compute_diff` が未処理ファイルを `added` として正しく検出する。
    pub fn save_with_fingerprints(
        &self,
        folder_path: &str,
        metas: &[ChunkMeta],
        embeddings: &[Embedding],
        fingerprints: HashMap<String, FileFingerprint>,
    ) -> Result<(), String> {
        let cache_dir = self.cache_dir_for(folder_path);
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("キャッシュディレクトリ作成失敗: {}", e))?;

        let cached = CachedEmbeddings {
            metas: metas.to_vec(),
            embeddings: embeddings.to_vec(),
        };
        let bin = bincode::serialize(&cached)
            .map_err(|e| format!("キャッシュシリアライズ失敗: {}", e))?;
        // embeddings.bin を先に書く（manifest.jsonが存在しなければキャッシュ無効と判定される）
        std::fs::write(cache_dir.join("embeddings.bin"), bin)
            .map_err(|e| format!("キャッシュ書き込み失敗: {}", e))?;

        let dim = embeddings.first().map(|e| e.len()).unwrap_or(0);
        let manifest = CacheManifest {
            format_version: FORMAT_VERSION,
            folder_path: folder_path.to_string(),
            file_fingerprints: fingerprints,
            chunk_count: metas.len(),
            embedding_dimension: dim,
        };
        let json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("マニフェストシリアライズ失敗: {}", e))?;
        std::fs::write(cache_dir.join("manifest.json"), json)
            .map_err(|e| format!("マニフェスト書き込み失敗: {}", e))?;

        Ok(())
    }

    /// 全ベクトルキャッシュディレクトリを列挙する（index/{hash}/vector/）
    pub fn list_cache_dirs(&self) -> Vec<PathBuf> {
        if !self.base_dir.exists() {
            return Vec::new();
        }
        std::fs::read_dir(&self.base_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.path().join("vector"))
            .filter(|p| p.exists())
            .collect()
    }

    /// 全インデックスハッシュディレクトリを列挙する（index/{hash}/）
    pub fn list_index_dirs(&self) -> Vec<PathBuf> {
        if !self.base_dir.exists() {
            return Vec::new();
        }
        std::fs::read_dir(&self.base_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect()
    }

    /// 指定ファイルのフィンガープリントを収集する
    pub fn collect_fingerprints_for(
        file_paths: &std::collections::HashSet<String>,
    ) -> HashMap<String, FileFingerprint> {
        let mut fingerprints = HashMap::new();
        for path_str in file_paths {
            let path = std::path::Path::new(path_str);
            if let Ok(metadata) = std::fs::metadata(path) {
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                fingerprints.insert(
                    path_str.clone(),
                    FileFingerprint {
                        size: metadata.len(),
                        modified,
                    },
                );
            }
        }
        fingerprints
    }
}

/// ベクトルキャッシュディレクトリの破損を検査する
///
/// manifest.jsonのパース・format_versionチェック・embeddings.binのデシリアライズを試行する。
/// フォルダとの整合性チェックではなく、ファイル内容の健全性のみを検査する。
pub fn validate_cache_dir(cache_dir: &Path) -> bool {
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

    let data = match std::fs::read(&embeddings_path) {
        Ok(d) => d,
        Err(_) => return false,
    };

    let result: Result<CachedEmbeddings, _> = bincode::deserialize(&data);
    result.is_ok()
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
        cache.save(folder_path, &[], &[]).unwrap();

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

    #[test]
    fn test_compute_diff_no_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());

        let folder_dir = tempfile::tempdir().unwrap();
        let test_file = folder_dir.path().join("test.md");
        {
            let mut f = std::fs::File::create(&test_file).unwrap();
            writeln!(f, "テスト").unwrap();
        }

        let folder_path = folder_dir.path().to_str().unwrap();
        cache.save(folder_path, &[], &[]).unwrap();

        let diff = cache.compute_diff(folder_path).unwrap();
        assert!(!diff.has_changes());
        assert_eq!(diff.unchanged.len(), 1);
        assert!(diff.added.is_empty());
        assert!(diff.modified.is_empty());
        assert!(diff.deleted.is_empty());
    }

    #[test]
    fn test_compute_diff_file_added() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());

        let folder_dir = tempfile::tempdir().unwrap();
        let test_file = folder_dir.path().join("test.md");
        {
            let mut f = std::fs::File::create(&test_file).unwrap();
            writeln!(f, "テスト").unwrap();
        }

        let folder_path = folder_dir.path().to_str().unwrap();
        cache.save(folder_path, &[], &[]).unwrap();

        // ファイルを追加
        let new_file = folder_dir.path().join("new.txt");
        {
            let mut f = std::fs::File::create(&new_file).unwrap();
            writeln!(f, "新規").unwrap();
        }

        let diff = cache.compute_diff(folder_path).unwrap();
        assert!(diff.has_changes());
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.unchanged.len(), 1);
    }

    #[test]
    fn test_compute_diff_file_deleted() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());

        let folder_dir = tempfile::tempdir().unwrap();
        let file_a = folder_dir.path().join("a.md");
        let file_b = folder_dir.path().join("b.md");
        {
            let mut f = std::fs::File::create(&file_a).unwrap();
            writeln!(f, "ファイルA").unwrap();
            let mut f = std::fs::File::create(&file_b).unwrap();
            writeln!(f, "ファイルB").unwrap();
        }

        let folder_path = folder_dir.path().to_str().unwrap();
        cache.save(folder_path, &[], &[]).unwrap();

        // ファイルを削除
        std::fs::remove_file(&file_b).unwrap();

        let diff = cache.compute_diff(folder_path).unwrap();
        assert!(diff.has_changes());
        assert_eq!(diff.deleted.len(), 1);
        assert_eq!(diff.unchanged.len(), 1);
    }

    #[test]
    fn test_compute_diff_no_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());
        assert!(cache.compute_diff("/nonexistent/path").is_none());
    }

    #[test]
    fn test_validate_cache_dir_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());

        let folder_dir = tempfile::tempdir().unwrap();
        let test_file = folder_dir.path().join("test.md");
        std::fs::write(&test_file, "テスト").unwrap();

        let folder_path = folder_dir.path().to_str().unwrap();
        let metas = vec![ChunkMeta {
            chunk_id: 0,
            source_path: test_file.to_string_lossy().to_string(),
            chunk_index: 0,
            text: "テスト".to_string(),
        }];
        let embeddings = vec![vec![0.1f32; 384]];
        cache.save(folder_path, &metas, &embeddings).unwrap();

        let cache_dir = cache.cache_dir_for(folder_path);
        assert!(validate_cache_dir(&cache_dir));
    }

    #[test]
    fn test_validate_cache_dir_nonexistent() {
        assert!(!validate_cache_dir(Path::new("/tmp/nonexistent-cache-xyz")));
    }

    #[test]
    fn test_validate_cache_dir_corrupted_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("corrupted");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join("manifest.json"), "invalid json").unwrap();
        std::fs::write(cache_dir.join("embeddings.bin"), "invalid bin").unwrap();

        assert!(!validate_cache_dir(&cache_dir));
    }

    #[test]
    fn test_validate_cache_dir_corrupted_embeddings() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("corrupted");
        std::fs::create_dir_all(&cache_dir).unwrap();

        let manifest = CacheManifest {
            format_version: FORMAT_VERSION,
            folder_path: "/test".to_string(),
            file_fingerprints: HashMap::new(),
            chunk_count: 0,
            embedding_dimension: 384,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        std::fs::write(cache_dir.join("manifest.json"), json).unwrap();
        std::fs::write(cache_dir.join("embeddings.bin"), "invalid bin data").unwrap();

        assert!(!validate_cache_dir(&cache_dir));
    }

    #[test]
    fn test_list_cache_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());

        // base_dirが存在しない場合は空
        assert!(cache.list_cache_dirs().is_empty());

        // index/{hash}/vector/ 構造を作成
        let base = tmp.path().join("index");
        std::fs::create_dir_all(base.join("abc123").join("vector")).unwrap();
        std::fs::create_dir_all(base.join("def456").join("vector")).unwrap();
        // vectorサブディレクトリがないハッシュは含まれない
        std::fs::create_dir_all(base.join("ghi789").join("fulltext")).unwrap();
        // ファイルはリストに含まれない
        std::fs::write(base.join("not-a-dir.txt"), "x").unwrap();

        let dirs = cache.list_cache_dirs();
        assert_eq!(dirs.len(), 2);
    }

    #[test]
    fn test_list_index_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = VectorCache::new(tmp.path());

        assert!(cache.list_index_dirs().is_empty());

        let base = tmp.path().join("index");
        std::fs::create_dir_all(base.join("abc123")).unwrap();
        std::fs::create_dir_all(base.join("def456")).unwrap();
        std::fs::write(base.join("not-a-dir.txt"), "x").unwrap();

        let dirs = cache.list_index_dirs();
        assert_eq!(dirs.len(), 2);
    }
}
