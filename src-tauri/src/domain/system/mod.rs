use serde::{Deserialize, Serialize};

use super::llm::LlmModelInfo;

/// GPU情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU名称
    pub name: String,
    /// VRAMサイズ（MB）
    pub vram_mb: u64,
}

/// システム情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// システムRAM合計（MB）
    pub total_ram_mb: u64,
    /// 検出されたGPU一覧
    pub gpus: Vec<GpuInfo>,
    /// GPU推論が利用可能か（フェーズBまでfalse）
    pub gpu_inference_available: bool,
}

/// モデル推奨ステータス
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendationStatus {
    /// メモリ十分、推奨
    Recommended,
    /// メモリが不足気味、動作するが遅い可能性
    Warning,
    /// メモリ不足、非推奨
    TooLarge,
}

/// モデル推奨情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecommendation {
    /// モデルファイル名
    pub filename: String,
    /// 推奨ステータス
    pub status: RecommendationStatus,
    /// 推奨群の中で最高品質か
    pub is_best_fit: bool,
    /// 推奨理由
    pub reason: String,
}

/// OS・アプリが使用するメモリの見積もり（MB）
const OS_OVERHEAD_MB: u64 = 2048;

/// 利用可能メモリを算出する
fn available_memory_mb(system: &SystemInfo) -> u64 {
    if system.gpu_inference_available {
        // GPU推論時: 最大VRAMを基準とする
        system.gpus.iter().map(|g| g.vram_mb).max().unwrap_or(0)
    } else {
        // CPU推論時: システムRAMからOS分を差し引く
        system.total_ram_mb.saturating_sub(OS_OVERHEAD_MB)
    }
}

/// システム情報に基づきモデルの推奨リストを生成する
pub fn recommend_models(
    models: &[LlmModelInfo],
    system: &SystemInfo,
) -> Vec<ModelRecommendation> {
    let available = available_memory_mb(system);

    let mut recommendations: Vec<ModelRecommendation> = models
        .iter()
        .map(|model| {
            let (status, reason) = evaluate_model(model.min_vram_mb, available);
            ModelRecommendation {
                filename: model.filename.clone(),
                status,
                is_best_fit: false,
                reason,
            }
        })
        .collect();

    // Recommendedの中で最大のmin_vram_mbを持つモデルをbest_fitにする
    let best_fit_index = recommendations
        .iter()
        .enumerate()
        .filter(|(_, r)| r.status == RecommendationStatus::Recommended)
        .max_by_key(|(i, _)| models[*i].min_vram_mb)
        .map(|(i, _)| i);

    if let Some(idx) = best_fit_index {
        recommendations[idx].is_best_fit = true;
    }

    recommendations
}

/// 個別モデルのメモリ適合性を評価する
fn evaluate_model(min_vram_mb: u64, available_mb: u64) -> (RecommendationStatus, String) {
    // min_vram_mb == 0 はCPU専用の軽量モデルを示す
    if min_vram_mb == 0 {
        return (
            RecommendationStatus::Recommended,
            "軽量モデル — 全環境で動作可能".to_string(),
        );
    }

    if min_vram_mb <= available_mb {
        (
            RecommendationStatus::Recommended,
            format!("必要メモリ {}MB — 十分な空きメモリあり", min_vram_mb),
        )
    } else if min_vram_mb <= available_mb * 3 / 2 {
        (
            RecommendationStatus::Warning,
            format!(
                "必要メモリ {}MB — メモリが不足気味（動作が遅い可能性）",
                min_vram_mb
            ),
        )
    } else {
        (
            RecommendationStatus::TooLarge,
            format!("必要メモリ {}MB — メモリ不足", min_vram_mb),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_system(total_ram_mb: u64) -> SystemInfo {
        SystemInfo {
            total_ram_mb,
            gpus: vec![],
            gpu_inference_available: false,
        }
    }

    fn test_models() -> Vec<LlmModelInfo> {
        vec![
            LlmModelInfo {
                name: "0.5B".to_string(),
                filename: "model-0.5b.gguf".to_string(),
                url: String::new(),
                size_bytes: 491 * 1024 * 1024,
                min_vram_mb: 0,
                params: "0.5B".to_string(),
                quantization: "Q4_K_M".to_string(),
                chat_template: crate::domain::llm::chat_template::ChatTemplate::Chatml,
                context_length: 32768,
            },
            LlmModelInfo {
                name: "1.5B".to_string(),
                filename: "model-1.5b.gguf".to_string(),
                url: String::new(),
                size_bytes: 1_120 * 1024 * 1024,
                min_vram_mb: 2048,
                params: "1.5B".to_string(),
                quantization: "Q4_K_M".to_string(),
                chat_template: crate::domain::llm::chat_template::ChatTemplate::Chatml,
                context_length: 32768,
            },
            LlmModelInfo {
                name: "7B".to_string(),
                filename: "model-7b.gguf".to_string(),
                url: String::new(),
                size_bytes: 4_680 * 1024 * 1024,
                min_vram_mb: 6144,
                params: "7B".to_string(),
                quantization: "Q4_K_M".to_string(),
                chat_template: crate::domain::llm::chat_template::ChatTemplate::Chatml,
                context_length: 32768,
            },
            LlmModelInfo {
                name: "14B".to_string(),
                filename: "model-14b.gguf".to_string(),
                url: String::new(),
                size_bytes: 8_990 * 1024 * 1024,
                min_vram_mb: 12288,
                params: "14B".to_string(),
                quantization: "Q4_K_M".to_string(),
                chat_template: crate::domain::llm::chat_template::ChatTemplate::Chatml,
                context_length: 32768,
            },
        ]
    }

    #[test]
    fn test_available_memory_cpu_mode() {
        let system = test_system(16384); // 16GB
        assert_eq!(available_memory_mb(&system), 16384 - 2048);
    }

    #[test]
    fn test_available_memory_gpu_mode() {
        let system = SystemInfo {
            total_ram_mb: 16384,
            gpus: vec![
                GpuInfo {
                    name: "GPU0".to_string(),
                    vram_mb: 4096,
                },
                GpuInfo {
                    name: "GPU1".to_string(),
                    vram_mb: 8192,
                },
            ],
            gpu_inference_available: true,
        };
        // GPU推論時は最大VRAMを基準とする
        assert_eq!(available_memory_mb(&system), 8192);
    }

    #[test]
    fn test_available_memory_low_ram() {
        let system = test_system(1024); // 1GB
        // OS_OVERHEAD以下の場合は0になる
        assert_eq!(available_memory_mb(&system), 0);
    }

    #[test]
    fn test_recommend_16gb_system() {
        // 16GB RAM → available = 14336MB
        let system = test_system(16384);
        let models = test_models();
        let recs = recommend_models(&models, &system);

        // 0.5B (0MB): Recommended
        assert_eq!(recs[0].status, RecommendationStatus::Recommended);
        // 1.5B (2048MB): Recommended
        assert_eq!(recs[1].status, RecommendationStatus::Recommended);
        // 7B (6144MB): Recommended
        assert_eq!(recs[2].status, RecommendationStatus::Recommended);
        // 14B (12288MB): Recommended (14336 >= 12288)
        assert_eq!(recs[3].status, RecommendationStatus::Recommended);
        // best_fit は最大のRecommended = 14B
        assert!(recs[3].is_best_fit);
    }

    #[test]
    fn test_recommend_8gb_system() {
        // 8GB RAM → available = 6144MB
        let system = test_system(8192);
        let models = test_models();
        let recs = recommend_models(&models, &system);

        // 0.5B: Recommended
        assert_eq!(recs[0].status, RecommendationStatus::Recommended);
        // 1.5B (2048MB): Recommended
        assert_eq!(recs[1].status, RecommendationStatus::Recommended);
        // 7B (6144MB): Recommended (6144 <= 6144)
        assert_eq!(recs[2].status, RecommendationStatus::Recommended);
        // 14B (12288MB): TooLarge (12288 > 6144 * 1.5 = 9216)
        assert_eq!(recs[3].status, RecommendationStatus::TooLarge);
        // best_fit = 7B
        assert!(recs[2].is_best_fit);
    }

    #[test]
    fn test_recommend_4gb_system() {
        // 4GB RAM → available = 2048MB
        let system = test_system(4096);
        let models = test_models();
        let recs = recommend_models(&models, &system);

        // 0.5B: Recommended
        assert_eq!(recs[0].status, RecommendationStatus::Recommended);
        // 1.5B (2048MB): Recommended (2048 <= 2048)
        assert_eq!(recs[1].status, RecommendationStatus::Recommended);
        // 7B (6144MB): TooLarge (6144 > 2048 * 1.5 = 3072)
        assert_eq!(recs[2].status, RecommendationStatus::TooLarge);
        // 14B: TooLarge
        assert_eq!(recs[3].status, RecommendationStatus::TooLarge);
        // best_fit = 1.5B
        assert!(recs[1].is_best_fit);
    }

    #[test]
    fn test_recommend_warning_zone() {
        // 7GB RAM → available = 5120MB
        // 7B (6144MB): 6144 > 5120 だが 6144 <= 5120 * 1.5 = 7680
        // → Warning
        let system = test_system(7168);
        let models = test_models();
        let recs = recommend_models(&models, &system);

        assert_eq!(recs[2].status, RecommendationStatus::Warning);
    }

    #[test]
    fn test_recommend_no_models() {
        let system = test_system(16384);
        let recs = recommend_models(&[], &system);
        assert!(recs.is_empty());
    }

    #[test]
    fn test_best_fit_is_unique() {
        let system = test_system(16384);
        let models = test_models();
        let recs = recommend_models(&models, &system);

        let best_fit_count = recs.iter().filter(|r| r.is_best_fit).count();
        assert_eq!(best_fit_count, 1);
    }

    #[test]
    fn test_evaluate_zero_vram_model() {
        let (status, _) = evaluate_model(0, 1024);
        assert_eq!(status, RecommendationStatus::Recommended);
    }
}
