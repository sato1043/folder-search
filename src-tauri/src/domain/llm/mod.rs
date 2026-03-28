pub mod rag;

/// LLM推論のトレイト
pub trait LlmInference {
    /// テキストを生成する（ストリーミングコールバック付き）
    fn generate<F>(&mut self, prompt: &str, max_tokens: u32, on_token: F) -> Result<String, LlmError>
    where
        F: FnMut(&str);
}

/// LLMエラー
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("モデルの読み込みに失敗: {0}")]
    ModelLoadError(String),
    #[error("推論に失敗: {0}")]
    InferenceError(String),
    #[error("トークン化に失敗: {0}")]
    TokenizeError(String),
}

/// 利用可能なLLMモデル情報
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LlmModelInfo {
    /// モデル名
    pub name: String,
    /// ファイル名
    pub filename: String,
    /// ダウンロードURL
    pub url: String,
    /// ファイルサイズ（バイト）
    pub size_bytes: u64,
    /// 推奨最小VRAMサイズ（MB）。0はCPU専用
    pub min_vram_mb: u64,
    /// パラメータ数の概算
    pub params: String,
    /// 量子化レベル
    pub quantization: String,
}

/// デフォルトのモデルリスト
pub fn available_models() -> Vec<LlmModelInfo> {
    vec![
        LlmModelInfo {
            name: "Qwen2.5-0.5B-Instruct (Q4_K_M)".to_string(),
            filename: "qwen2.5-0.5b-instruct-q4_k_m.gguf".to_string(),
            url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf".to_string(),
            size_bytes: 491 * 1024 * 1024,
            min_vram_mb: 0,
            params: "0.5B".to_string(),
            quantization: "Q4_K_M".to_string(),
        },
        LlmModelInfo {
            name: "Qwen2.5-1.5B-Instruct (Q4_K_M)".to_string(),
            filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
            url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
            size_bytes: 1_120 * 1024 * 1024,
            min_vram_mb: 2048,
            params: "1.5B".to_string(),
            quantization: "Q4_K_M".to_string(),
        },
        LlmModelInfo {
            name: "Qwen2.5-7B-Instruct (Q4_K_M)".to_string(),
            filename: "qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            url: "https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF/resolve/main/qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            size_bytes: 4_680 * 1024 * 1024,
            min_vram_mb: 6144,
            params: "7B".to_string(),
            quantization: "Q4_K_M".to_string(),
        },
        LlmModelInfo {
            name: "Qwen2.5-14B-Instruct (Q4_K_M)".to_string(),
            filename: "qwen2.5-14b-instruct-q4_k_m.gguf".to_string(),
            url: "https://huggingface.co/Qwen/Qwen2.5-14B-Instruct-GGUF/resolve/main/qwen2.5-14b-instruct-q4_k_m.gguf".to_string(),
            size_bytes: 8_990 * 1024 * 1024,
            min_vram_mb: 12288,
            params: "14B".to_string(),
            quantization: "Q4_K_M".to_string(),
        },
    ]
}
