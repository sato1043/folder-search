pub mod chat_template;
pub mod rag;

/// LLM推論のトレイト
pub trait LlmInference {
    /// テキストを生成する（ストリーミングコールバック付き）
    fn generate<F>(
        &mut self,
        prompt: &str,
        max_tokens: u32,
        on_token: F,
    ) -> Result<String, LlmError>
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

use chat_template::ChatTemplate;

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
    /// チャットテンプレート
    pub chat_template: ChatTemplate,
    /// コンテキスト長（トークン数）
    pub context_length: u32,
    /// プリセットモデルか（trueならコード定義、falseならカスタム登録）
    #[serde(default = "default_is_preset")]
    pub is_preset: bool,
}

fn default_is_preset() -> bool {
    false
}

/// ダウンロード済みモデルの情報
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DownloadedModelInfo {
    /// ファイル名
    pub filename: String,
    /// ファイルサイズ（バイト）
    pub size_bytes: u64,
    /// embeddingモデルのファイルか（model.onnx / tokenizer.json）
    pub is_embedding: bool,
}

/// モデルストレージ使用状況
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageUsage {
    /// モデルファイルの合計サイズ（バイト）
    pub total_used_bytes: u64,
    /// ディスク空き容量（バイト）。取得できない場合は0
    pub disk_free_bytes: u64,
    /// キャッシュ上限（バイト）
    pub cache_limit_bytes: u64,
}

/// ダウンロードキャッシュのデフォルト上限（100 GB）
pub const DEFAULT_CACHE_LIMIT_BYTES: u64 = 100 * 1024 * 1024 * 1024;

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
        // --- Gemma 3 ---
        LlmModelInfo {
            name: "Gemma 3 1B Instruct (Q4_K_M)".to_string(),
            filename: "gemma-3-1b-it-Q4_K_M.gguf".to_string(),
            url: "https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf".to_string(),
            size_bytes: 714 * 1024 * 1024,
            min_vram_mb: 0,
            params: "1B".to_string(),
            quantization: "Q4_K_M".to_string(),
            chat_template: ChatTemplate::Gemma,
            context_length: 32768,
            is_preset: true,
        },
        LlmModelInfo {
            name: "Gemma 3 4B Instruct (Q4_K_M)".to_string(),
            filename: "google_gemma-3-4b-it-Q4_K_M.gguf".to_string(),
            url: "https://huggingface.co/bartowski/google_gemma-3-4b-it-GGUF/resolve/main/google_gemma-3-4b-it-Q4_K_M.gguf".to_string(),
            size_bytes: 2_900 * 1024 * 1024,
            min_vram_mb: 4096,
            params: "4B".to_string(),
            quantization: "Q4_K_M".to_string(),
            chat_template: ChatTemplate::Gemma,
            context_length: 32768,
            is_preset: true,
        },
        LlmModelInfo {
            name: "Gemma 3 12B Instruct (Q4_K_M)".to_string(),
            filename: "google_gemma-3-12b-it-Q4_K_M.gguf".to_string(),
            url: "https://huggingface.co/bartowski/google_gemma-3-12b-it-GGUF/resolve/main/google_gemma-3-12b-it-Q4_K_M.gguf".to_string(),
            size_bytes: 7_300 * 1024 * 1024,
            min_vram_mb: 8192,
            params: "12B".to_string(),
            quantization: "Q4_K_M".to_string(),
            chat_template: ChatTemplate::Gemma,
            context_length: 32768,
            is_preset: true,
        },
        // --- Llama 3 ---
        LlmModelInfo {
            name: "Llama 3.2 1B Instruct (Q4_K_M)".to_string(),
            filename: "Llama-3.2-1B-Instruct-Q4_K_M.gguf".to_string(),
            url: "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf".to_string(),
            size_bytes: 750 * 1024 * 1024,
            min_vram_mb: 0,
            params: "1B".to_string(),
            quantization: "Q4_K_M".to_string(),
            chat_template: ChatTemplate::Llama3,
            context_length: 32768,
            is_preset: true,
        },
        LlmModelInfo {
            name: "Llama 3.2 3B Instruct (Q4_K_M)".to_string(),
            filename: "Llama-3.2-3B-Instruct-Q4_K_M.gguf".to_string(),
            url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf".to_string(),
            size_bytes: 2_020 * 1024 * 1024,
            min_vram_mb: 3072,
            params: "3B".to_string(),
            quantization: "Q4_K_M".to_string(),
            chat_template: ChatTemplate::Llama3,
            context_length: 32768,
            is_preset: true,
        },
        LlmModelInfo {
            name: "Llama 3.1 8B Instruct (Q4_K_M)".to_string(),
            filename: "Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf".to_string(),
            url: "https://huggingface.co/bartowski/Meta-Llama-3.1-8B-Instruct-GGUF/resolve/main/Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf".to_string(),
            size_bytes: 4_920 * 1024 * 1024,
            min_vram_mb: 6144,
            params: "8B".to_string(),
            quantization: "Q4_K_M".to_string(),
            chat_template: ChatTemplate::Llama3,
            context_length: 32768,
            is_preset: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_models_all_preset() {
        let models = available_models();
        assert_eq!(models.len(), 6);
        for model in &models {
            assert!(model.is_preset, "{} should be preset", model.name);
            assert!(!model.name.is_empty());
            assert!(!model.filename.is_empty());
            assert!(!model.url.is_empty());
            assert!(model.size_bytes > 0);
            assert!(model.context_length > 0);
        }
    }

    #[test]
    fn test_llm_model_info_deserialize_default_is_preset() {
        let json = r#"{
            "name": "Test",
            "filename": "test.gguf",
            "url": "",
            "size_bytes": 1000,
            "min_vram_mb": 0,
            "params": "",
            "quantization": "",
            "chat_template": "chatml",
            "context_length": 4096
        }"#;
        let model: LlmModelInfo = serde_json::from_str(json).unwrap();
        assert!(!model.is_preset, "is_preset should default to false");
    }

    #[test]
    fn test_default_cache_limit() {
        assert_eq!(DEFAULT_CACHE_LIMIT_BYTES, 100 * 1024 * 1024 * 1024);
    }

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

    // ============================================================
    // 境界条件: VRAM と overhead の関係
    // ============================================================

    #[test]
    fn test_boundary_vram_equals_overhead() {
        // VRAM = overhead(512) → available = 0 → 0層
        assert_eq!(estimate_gpu_layers(4680 * MB, GPU_OVERHEAD_MB), 0);
    }

    #[test]
    fn test_boundary_vram_one_above_overhead() {
        // VRAM = 513 → available = 1 → ratio極小 → 最低1層を保証
        let layers = estimate_gpu_layers(4680 * MB, GPU_OVERHEAD_MB + 1);
        assert_eq!(layers, 1, "available=1でも最低1層��保証する");
    }

    // ============================================================
    // 境界条件: 全層オフロード境界
    // ============================================================

    #[test]
    fn test_boundary_full_offload_exact_fit() {
        // VRAM = model_mb + overhead 丁度 → 全層オフロード
        let model_mb = 4680u64;
        let vram = model_mb + GPU_OVERHEAD_MB;
        assert_eq!(estimate_gpu_layers(model_mb * MB, vram), u32::MAX);
    }

    #[test]
    fn test_boundary_one_below_full_offload() {
        // VRAM = model_mb + overhead - 1 → 部分オフロード
        let model_mb = 4680u64;
        let vram = model_mb + GPU_OVERHEAD_MB - 1;
        let layers = estimate_gpu_layers(model_mb * MB, vram);
        assert!(
            layers > 0 && layers < u32::MAX,
            "全層境界の1MB下は部分オフロードになるべき: got {}",
            layers
        );
        // available = 4679, ratio ≈ 0.9998 → 約39層
        assert!(layers >= 38, "ほぼ全VRAMが使える: got {}", layers);
    }

    // ============================================================
    // 境界条件: model_size_bytes が小さい場合
    // ============================================================

    #[test]
    fn test_boundary_model_size_below_1mb() {
        // model_mb = 0 になるケース → 早期return 0
        assert_eq!(estimate_gpu_layers(500_000, 8192), 0);
        assert_eq!(estimate_gpu_layers(1, 8192), 0);
        assert_eq!(estimate_gpu_layers(MB - 1, 8192), 0);
    }

    #[test]
    fn test_boundary_model_size_exactly_1mb() {
        // model_mb = 1, VRAM十分 → 全層
        assert_eq!(estimate_gpu_layers(MB, 8192), u32::MAX);
    }

    #[test]
    fn test_boundary_minimum_one_layer_large_model() {
        // 巨大モデルでも available > 0 なら最低1層
        assert_eq!(estimate_gpu_layers(100_000 * MB, GPU_OVERHEAD_MB + 1), 1);
    }

    // ============================================================
    // 極大値・オーバーフロー安全性
    // ============================================================

    #[test]
    fn test_extreme_large_model() {
        // 100GB モデル、24GB VRAM → パニックしない + 部分オフロード
        let layers = estimate_gpu_layers(100_000 * MB, 24_576);
        assert!(layers > 0 && layers < u32::MAX);
    }

    #[test]
    fn test_extreme_large_vram() {
        // 通常モデル、256GB VRAM → 全層
        assert_eq!(estimate_gpu_layers(4680 * MB, 256 * 1024), u32::MAX);
    }

    #[test]
    fn test_extreme_both_large_no_panic() {
        // 巨大モデル + 巨大VRAM → パニックしないことが重要
        let _ = estimate_gpu_layers(u64::MAX / 2, u64::MAX / (1024 * 1024));
    }

    #[test]
    fn test_extreme_max_u64_model_no_panic() {
        // u64::MAX → model_mb は巨大だが panic しない
        let _ = estimate_gpu_layers(u64::MAX, 8192);
    }

    #[test]
    fn test_extreme_max_u64_vram_no_panic() {
        let _ = estimate_gpu_layers(4680 * MB, u64::MAX);
    }

    // ============================================================
    // 全プリセットモデル × 典型的 VRAM
    // ============================================================

    #[test]
    fn test_preset_models_4gb_vram() {
        let vram = 4096; // available = 3584
        assert_eq!(estimate_gpu_layers(491 * MB, vram), u32::MAX);
        assert_eq!(estimate_gpu_layers(1120 * MB, vram), u32::MAX);
        let l7b = estimate_gpu_layers(4680 * MB, vram);
        let l14b = estimate_gpu_layers(8990 * MB, vram);
        assert!(l7b > 0 && l7b < u32::MAX);
        assert!(l14b > 0 && l14b < l7b, "14Bは7Bより層数が少ないべき");
    }

    #[test]
    fn test_preset_models_8gb_vram() {
        let vram = 8192; // available = 7680
        assert_eq!(estimate_gpu_layers(491 * MB, vram), u32::MAX);
        assert_eq!(estimate_gpu_layers(1120 * MB, vram), u32::MAX);
        assert_eq!(estimate_gpu_layers(4680 * MB, vram), u32::MAX);
        let l14b = estimate_gpu_layers(8990 * MB, vram);
        assert!(l14b > 0 && l14b < u32::MAX);
    }

    #[test]
    fn test_preset_models_16gb_vram() {
        let vram = 16384; // 全モデル全層オフロード可能
        for &model_mb in &[491u64, 1120, 4680, 8990] {
            assert_eq!(estimate_gpu_layers(model_mb * MB, vram), u32::MAX);
        }
    }

    #[test]
    fn test_preset_models_no_gpu() {
        for &model_mb in &[491u64, 1120, 4680, 8990] {
            assert_eq!(estimate_gpu_layers(model_mb * MB, 0), 0);
        }
    }

    // ============================================================
    // 比例配分の正確性
    // ============================================================

    #[test]
    fn test_ratio_half() {
        // model=4000MB, VRAM=2512 → available=2000, ratio=0.5 → 20層
        assert_eq!(estimate_gpu_layers(4000 * MB, 2512), 20);
    }

    #[test]
    fn test_ratio_quarter() {
        // model=4000MB, VRAM=1512 → available=1000, ratio=0.25 → 10層
        assert_eq!(estimate_gpu_layers(4000 * MB, 1512), 10);
    }

    #[test]
    fn test_ratio_three_quarters() {
        // model=4000MB, VRAM=3512 → available=3000, ratio=0.75 → 30層
        assert_eq!(estimate_gpu_layers(4000 * MB, 3512), 30);
    }

    #[test]
    fn test_monotonically_increasing_with_vram() {
        // VRAMが増えるにつれ層数は単調非減少
        let model_bytes = 4680 * MB;
        let mut prev = 0u32;
        for vram in (600..=5200).step_by(100) {
            let layers = estimate_gpu_layers(model_bytes, vram);
            assert!(
                layers >= prev,
                "VRAM {}MBで{}層 < 前回{}層: 単調性違反",
                vram,
                layers,
                prev
            );
            prev = layers;
        }
    }

    #[test]
    fn test_monotonically_decreasing_with_model_size() {
        // モデルサイズが増えるにつれ層数は単調非増加（VRAM固定）
        let vram = 8192;
        let mut prev = u32::MAX;
        for model_mb in (1000..=10000).step_by(500) {
            let layers = estimate_gpu_layers(model_mb * MB, vram);
            assert!(
                layers <= prev,
                "model {}MBで{}層 > 前回{}層: 単調性違反",
                model_mb,
                layers,
                prev
            );
            prev = layers;
        }
    }
}
