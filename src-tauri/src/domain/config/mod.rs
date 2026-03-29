use crate::domain::llm::DEFAULT_CACHE_LIMIT_BYTES;

/// アプリケーション設定
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    /// ダウンロードキャッシュの上限（バイト）
    pub cache_limit_bytes: u64,
    /// 前回ロードしたLLMモデルのファイル名
    #[serde(default)]
    pub last_loaded_model: Option<String>,
    /// 無効化されたモデルのファイル名リスト（LLMモデル選択に表示しない）
    #[serde(default)]
    pub disabled_models: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            cache_limit_bytes: DEFAULT_CACHE_LIMIT_BYTES,
            last_loaded_model: None,
            disabled_models: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.cache_limit_bytes, DEFAULT_CACHE_LIMIT_BYTES);
    }
}
