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

/// KVキャッシュ・ワークスペースのオーバーヘッド見積もり（MB）
const GPU_OVERHEAD_MB: u64 = 512;

/// モデル層数の推定上限（Qwen2.5系の最大層数を想定）
const ESTIMATED_MAX_LAYERS: u32 = 40;

/// GPU VRAMとモデルサイズからオフロードすべきレイヤー数を推定する
///
/// - VRAM がモデル全体+オーバーヘッドを収容可能 → 全層オフロード（u32::MAX）
/// - VRAM に一部収まる → 比例配分
/// - VRAM なしまたは不足 → 0（CPU推論）
pub fn estimate_gpu_layers(model_size_bytes: u64, vram_mb: u64) -> u32 {
    let model_mb = model_size_bytes / (1024 * 1024);
    if model_mb == 0 {
        return 0;
    }

    let available = vram_mb.saturating_sub(GPU_OVERHEAD_MB);

    if available >= model_mb {
        u32::MAX
    } else if available > 0 {
        let ratio = available as f64 / model_mb as f64;
        let layers = (ratio * ESTIMATED_MAX_LAYERS as f64) as u32;
        layers.max(1) // 最低1層はオフロードする
    } else {
        0
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // 1MB = 1024*1024 bytes
    const MB: u64 = 1024 * 1024;

    #[test]
    fn test_estimate_full_offload_when_vram_sufficient() {
        // 7Bモデル(4680MB) + overhead(512MB) = 5192MB が必要
        // VRAM 8192MB → 全層オフロード
        let layers = estimate_gpu_layers(4680 * MB, 8192);
        assert_eq!(layers, u32::MAX);
    }

    #[test]
    fn test_estimate_partial_offload_when_vram_limited() {
        // 7Bモデル(4680MB), VRAM 3000MB → available = 2488MB
        // ratio = 2488 / 4680 ≈ 0.53 → 約21層
        let layers = estimate_gpu_layers(4680 * MB, 3000);
        assert!(layers > 0 && layers < u32::MAX);
        assert!(layers >= 20 && layers <= 25, "expected ~21, got {}", layers);
    }

    #[test]
    fn test_estimate_zero_when_no_vram() {
        let layers = estimate_gpu_layers(4680 * MB, 0);
        assert_eq!(layers, 0);
    }

    #[test]
    fn test_estimate_zero_when_vram_below_overhead() {
        // VRAM 400MB < overhead 512MB → 0層
        let layers = estimate_gpu_layers(4680 * MB, 400);
        assert_eq!(layers, 0);
    }

    #[test]
    fn test_estimate_minimum_one_layer() {
        // VRAM がわずかにoverheadを超える → 最低1層
        let layers = estimate_gpu_layers(4680 * MB, 520);
        assert!(layers >= 1);
    }

    #[test]
    fn test_estimate_zero_model_size() {
        let layers = estimate_gpu_layers(0, 8192);
        assert_eq!(layers, 0);
    }

    #[test]
    fn test_estimate_small_model_large_vram() {
        // 0.5Bモデル(491MB), VRAM 8192MB → 全層オフロード
        let layers = estimate_gpu_layers(491 * MB, 8192);
        assert_eq!(layers, u32::MAX);
    }

    #[test]
    fn test_estimate_14b_model_12gb_vram() {
        // 14Bモデル(8990MB), VRAM 12288MB → available = 11776MB > 8990MB → 全層
        let layers = estimate_gpu_layers(8990 * MB, 12288);
        assert_eq!(layers, u32::MAX);
    }

    #[test]
    fn test_estimate_14b_model_8gb_vram() {
        // 14Bモデル(8990MB), VRAM 8192MB → available = 7680MB < 8990MB → 部分オフロード
        let layers = estimate_gpu_layers(8990 * MB, 8192);
        assert!(layers > 0 && layers < u32::MAX);
    }
}
