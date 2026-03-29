use std::path::{Path, PathBuf};

use crate::domain::config::AppSettings;

/// 設定のJSONマニフェスト
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SettingsManifest {
    format_version: u32,
    #[serde(flatten)]
    settings: AppSettings,
}

/// 設定の永続化ストア
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    /// 新しいSettingsStoreを作成する
    ///
    /// `app_data_dir` はアプリのデータディレクトリ。
    /// 設定ファイルは `{app_data_dir}/settings.json` に保存される。
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            path: app_data_dir.join("settings.json"),
        }
    }

    /// 設定を読み込む
    ///
    /// ファイルが存在しないか読み込みに失敗した場合はデフォルト値を返す
    pub fn load(&self) -> AppSettings {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(_) => return AppSettings::default(),
        };
        match serde_json::from_str::<SettingsManifest>(&content) {
            Ok(manifest) => manifest.settings,
            Err(_) => AppSettings::default(),
        }
    }

    /// 設定を保存する
    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        let manifest = SettingsManifest {
            format_version: 1,
            settings: settings.clone(),
        };
        let json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("JSON変換に失敗: {}", e))?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("ディレクトリ作成に失敗: {}", e))?;
        }

        std::fs::write(&self.path, json).map_err(|e| format!("ファイル書き込みに失敗: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::llm::DEFAULT_CACHE_LIMIT_BYTES;
    use tempfile::TempDir;

    #[test]
    fn test_load_default_when_no_file() {
        let dir = TempDir::new().unwrap();
        let store = SettingsStore::new(dir.path());
        let settings = store.load();
        assert_eq!(settings.cache_limit_bytes, DEFAULT_CACHE_LIMIT_BYTES);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let store = SettingsStore::new(dir.path());

        let settings = AppSettings {
            cache_limit_bytes: 50 * 1024 * 1024 * 1024,
            last_loaded_model: None,
            disabled_models: Vec::new(),
        };
        store.save(&settings).unwrap();

        let loaded = store.load();
        assert_eq!(loaded.cache_limit_bytes, 50 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_load_corrupt_json_returns_default() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("settings.json"), "invalid{").unwrap();

        let store = SettingsStore::new(dir.path());
        let settings = store.load();
        assert_eq!(settings.cache_limit_bytes, DEFAULT_CACHE_LIMIT_BYTES);
    }

    #[test]
    fn test_save_creates_directory() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("subdir");
        let store = SettingsStore::new(&nested);

        let settings = AppSettings {
            cache_limit_bytes: 25 * 1024 * 1024 * 1024,
            last_loaded_model: None,
            disabled_models: Vec::new(),
        };
        store.save(&settings).unwrap();

        let loaded = store.load();
        assert_eq!(loaded.cache_limit_bytes, 25 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_overwrite_existing_settings() {
        let dir = TempDir::new().unwrap();
        let store = SettingsStore::new(dir.path());

        store
            .save(&AppSettings {
                cache_limit_bytes: 50 * 1024 * 1024 * 1024,
                last_loaded_model: None,
                disabled_models: Vec::new(),
            })
            .unwrap();
        store
            .save(&AppSettings {
                cache_limit_bytes: 75 * 1024 * 1024 * 1024,
                last_loaded_model: None,
                disabled_models: Vec::new(),
            })
            .unwrap();

        let loaded = store.load();
        assert_eq!(loaded.cache_limit_bytes, 75 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_disabled_models_roundtrip() {
        let dir = TempDir::new().unwrap();
        let store = SettingsStore::new(dir.path());

        let settings = AppSettings {
            cache_limit_bytes: DEFAULT_CACHE_LIMIT_BYTES,
            last_loaded_model: Some("model-a.gguf".to_string()),
            disabled_models: vec!["model-b.gguf".to_string(), "model-c.gguf".to_string()],
        };
        store.save(&settings).unwrap();

        let loaded = store.load();
        assert_eq!(loaded.disabled_models.len(), 2);
        assert!(loaded.disabled_models.contains(&"model-b.gguf".to_string()));
        assert!(loaded.disabled_models.contains(&"model-c.gguf".to_string()));
        assert_eq!(loaded.last_loaded_model, Some("model-a.gguf".to_string()));
    }

    #[test]
    fn test_disabled_models_defaults_to_empty() {
        let dir = TempDir::new().unwrap();
        // disabled_modelsフィールドがないJSONでもデフォルト空で読める
        let json = r#"{"format_version":1,"cache_limit_bytes":100}"#;
        std::fs::write(dir.path().join("settings.json"), json).unwrap();

        let store = SettingsStore::new(dir.path());
        let loaded = store.load();
        assert!(loaded.disabled_models.is_empty());
    }
}
