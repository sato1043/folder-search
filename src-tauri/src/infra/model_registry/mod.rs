use std::path::{Path, PathBuf};

use crate::domain::llm::{available_models, LlmModelInfo};

/// カスタムモデルのJSONマニフェスト
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CustomModelsManifest {
    format_version: u32,
    models: Vec<LlmModelInfo>,
}

impl Default for CustomModelsManifest {
    fn default() -> Self {
        Self {
            format_version: 1,
            models: Vec::new(),
        }
    }
}

/// モデルレジストリ
///
/// デフォルトプリセット（コード定義）とカスタムモデル（JSON永続化）を統合管理する
pub struct ModelRegistry {
    manifest_path: PathBuf,
}

impl ModelRegistry {
    /// 新しいModelRegistryを作成する
    ///
    /// `model_dir` はモデルファイルの保存ディレクトリ。
    /// カスタムモデルのマニフェストは `{model_dir}/custom_models.json` に保存される。
    pub fn new(model_dir: &Path) -> Self {
        Self {
            manifest_path: model_dir.join("custom_models.json"),
        }
    }

    /// カスタムモデルをJSON から読み込む
    ///
    /// ファイルが存在しない場合は空のリストを返す
    pub fn load_custom_models(&self) -> Vec<LlmModelInfo> {
        let content = match std::fs::read_to_string(&self.manifest_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        match serde_json::from_str::<CustomModelsManifest>(&content) {
            Ok(manifest) => manifest.models,
            Err(_) => Vec::new(),
        }
    }

    /// カスタムモデルをJSON に保存する
    fn save_custom_models(&self, models: &[LlmModelInfo]) -> Result<(), String> {
        let manifest = CustomModelsManifest {
            format_version: 1,
            models: models.to_vec(),
        };
        let json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("JSON変換に失敗: {}", e))?;

        if let Some(parent) = self.manifest_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("ディレクトリ作成に失敗: {}", e))?;
        }

        std::fs::write(&self.manifest_path, json)
            .map_err(|e| format!("ファイル書き込みに失敗: {}", e))?;
        Ok(())
    }

    /// カスタムモデルを追加する
    ///
    /// 同じfilenameのモデルが既に存在する場合は上書きする
    pub fn add_model(&self, model: LlmModelInfo) -> Result<(), String> {
        // デフォルトプリセットと同名は拒否
        let defaults = available_models();
        if defaults.iter().any(|m| m.filename == model.filename) {
            return Err(format!(
                "プリセットモデルと同名のファイル名は登録できない: {}",
                model.filename
            ));
        }

        let mut custom = self.load_custom_models();
        // 同名を上書き
        custom.retain(|m| m.filename != model.filename);
        custom.push(model);
        self.save_custom_models(&custom)
    }

    /// カスタムモデルを登録解除する
    ///
    /// ダウンロード済みファイルは削除しない。
    /// デフォルトプリセットの削除は拒否する。
    pub fn remove_model(&self, filename: &str) -> Result<(), String> {
        let defaults = available_models();
        if defaults.iter().any(|m| m.filename == filename) {
            return Err("プリセットモデルは登録解除できない".to_string());
        }

        let mut custom = self.load_custom_models();
        let before_len = custom.len();
        custom.retain(|m| m.filename != filename);
        if custom.len() == before_len {
            return Err(format!("カスタムモデルが見つからない: {}", filename));
        }
        self.save_custom_models(&custom)
    }

    /// デフォルトプリセット + カスタムモデルを統合して返す
    ///
    /// デフォルトが先、カスタムが後の順序。
    /// filenameの重複があった場合はデフォルト側を優先する。
    pub fn all_models(&self) -> Vec<LlmModelInfo> {
        let defaults = available_models();
        let custom = self.load_custom_models();

        let default_filenames: std::collections::HashSet<String> =
            defaults.iter().map(|m| m.filename.clone()).collect();

        let mut result = defaults;
        for model in custom {
            if !default_filenames.contains(&model.filename) {
                result.push(model);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::llm::chat_template::ChatTemplate;
    use tempfile::TempDir;

    fn make_test_model(filename: &str) -> LlmModelInfo {
        LlmModelInfo {
            name: format!("Test Model ({})", filename),
            filename: filename.to_string(),
            url: "https://example.com/model.gguf".to_string(),
            size_bytes: 1000 * 1024 * 1024,
            min_vram_mb: 0,
            params: "1B".to_string(),
            quantization: "Q4_K_M".to_string(),
            chat_template: ChatTemplate::Chatml,
            context_length: 4096,
            is_preset: false,
        }
    }

    #[test]
    fn test_new_registry_returns_only_defaults() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());
        let models = registry.all_models();
        assert_eq!(models.len(), available_models().len());
    }

    #[test]
    fn test_load_custom_models_empty_when_no_file() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());
        assert!(registry.load_custom_models().is_empty());
    }

    #[test]
    fn test_add_and_load_custom_model() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        let model = make_test_model("my-custom.gguf");
        registry.add_model(model.clone()).unwrap();

        let custom = registry.load_custom_models();
        assert_eq!(custom.len(), 1);
        assert_eq!(custom[0].filename, "my-custom.gguf");
    }

    #[test]
    fn test_add_model_overwrites_same_filename() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        let model1 = make_test_model("same.gguf");
        registry.add_model(model1).unwrap();

        let mut model2 = make_test_model("same.gguf");
        model2.name = "Updated Name".to_string();
        registry.add_model(model2).unwrap();

        let custom = registry.load_custom_models();
        assert_eq!(custom.len(), 1);
        assert_eq!(custom[0].name, "Updated Name");
    }

    #[test]
    fn test_add_model_rejects_preset_filename() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        let defaults = available_models();
        let mut model = make_test_model(&defaults[0].filename);
        model.name = "Fake preset".to_string();

        let result = registry.add_model(model);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("プリセットモデルと同名"));
    }

    #[test]
    fn test_remove_custom_model() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        registry
            .add_model(make_test_model("to-remove.gguf"))
            .unwrap();
        assert_eq!(registry.load_custom_models().len(), 1);

        registry.remove_model("to-remove.gguf").unwrap();
        assert!(registry.load_custom_models().is_empty());
    }

    #[test]
    fn test_remove_preset_model_rejected() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        let defaults = available_models();
        let result = registry.remove_model(&defaults[0].filename);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("プリセットモデルは登録解除できない"));
    }

    #[test]
    fn test_remove_nonexistent_model() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        let result = registry.remove_model("nonexistent.gguf");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("カスタムモデルが見つからない"));
    }

    #[test]
    fn test_all_models_includes_defaults_and_custom() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        registry
            .add_model(make_test_model("custom-a.gguf"))
            .unwrap();
        registry
            .add_model(make_test_model("custom-b.gguf"))
            .unwrap();

        let all = registry.all_models();
        assert_eq!(all.len(), available_models().len() + 2);

        // デフォルトが先
        let defaults = available_models();
        for (i, d) in defaults.iter().enumerate() {
            assert_eq!(all[i].filename, d.filename);
        }
        // カスタムが後
        assert_eq!(all[defaults.len()].filename, "custom-a.gguf");
        assert_eq!(all[defaults.len() + 1].filename, "custom-b.gguf");
    }

    #[test]
    fn test_all_models_deduplicates_by_filename() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        // JSONに直接プリセットと同名のエントリを書き込む（add_modelはガードするため）
        let defaults = available_models();
        let mut dup = make_test_model("dummy");
        dup.name = "Duplicate".to_string();
        dup.filename = defaults[0].filename.clone();
        let manifest = CustomModelsManifest {
            format_version: 1,
            models: vec![dup],
        };
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        std::fs::write(dir.path().join("custom_models.json"), json).unwrap();

        let all = registry.all_models();
        // 重複は除外されるため、デフォルト数と同じ
        assert_eq!(all.len(), defaults.len());
        // デフォルト側の名前が保持される
        assert_eq!(all[0].name, defaults[0].name);
    }

    #[test]
    fn test_json_roundtrip_preserves_all_fields() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        let model = LlmModelInfo {
            name: "日本語テストモデル".to_string(),
            filename: "test-日本語.gguf".to_string(),
            url: "https://example.com/日本語.gguf".to_string(),
            size_bytes: 123_456_789,
            min_vram_mb: 4096,
            params: "3B".to_string(),
            quantization: "Q5_K_S".to_string(),
            chat_template: ChatTemplate::Gemma,
            context_length: 8192,
            is_preset: false,
        };
        registry.add_model(model.clone()).unwrap();

        let loaded = registry.load_custom_models();
        assert_eq!(loaded.len(), 1);
        let m = &loaded[0];
        assert_eq!(m.name, "日本語テストモデル");
        assert_eq!(m.filename, "test-日本語.gguf");
        assert_eq!(m.url, "https://example.com/日本語.gguf");
        assert_eq!(m.size_bytes, 123_456_789);
        assert_eq!(m.min_vram_mb, 4096);
        assert_eq!(m.params, "3B");
        assert_eq!(m.quantization, "Q5_K_S");
        assert_eq!(m.chat_template, ChatTemplate::Gemma);
        assert_eq!(m.context_length, 8192);
    }

    #[test]
    fn test_corrupt_json_returns_empty() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("custom_models.json"), "invalid json{").unwrap();

        let registry = ModelRegistry::new(dir.path());
        assert!(registry.load_custom_models().is_empty());
    }

    #[test]
    fn test_multiple_operations_consistency() {
        let dir = TempDir::new().unwrap();
        let registry = ModelRegistry::new(dir.path());

        registry.add_model(make_test_model("a.gguf")).unwrap();
        registry.add_model(make_test_model("b.gguf")).unwrap();
        registry.add_model(make_test_model("c.gguf")).unwrap();
        assert_eq!(registry.load_custom_models().len(), 3);

        registry.remove_model("b.gguf").unwrap();
        let remaining = registry.load_custom_models();
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].filename, "a.gguf");
        assert_eq!(remaining[1].filename, "c.gguf");
    }
}
