use std::num::NonZeroU32;
use std::pin::pin;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use crate::domain::llm::{estimate_gpu_layers, LlmError, LlmInference};

/// 全層オフロード失敗時の最初のフォールバック層数
const FALLBACK_MAX_LAYERS: u32 = 40;

/// llama.cpp による LLM推論エンジン
pub struct LlamaEngine {
    backend: LlamaBackend,
    model: LlamaModel,
    /// 実際に使用されたGPUオフロード層数（0 = CPU推論）
    gpu_layers: u32,
}

impl LlamaEngine {
    /// GGUFモデルをロードして初期化する（VRAM推定+適応的GPUオフロード）
    ///
    /// model_size_bytes と vram_mb からオフロード層数を推定し、
    /// 失敗時は二分探索で段階的に削減、最終的にCPUフォールバックする
    pub fn new(model_path: &str, model_size_bytes: u64, vram_mb: u64) -> Result<Self, LlmError> {
        let initial_layers = estimate_gpu_layers(model_size_bytes, vram_mb);
        Self::load_adaptive(model_path, initial_layers)
    }

    /// 適応的にGPU層数を探索してモデルをロードする
    fn load_adaptive(model_path: &str, initial_layers: u32) -> Result<Self, LlmError> {
        let backend =
            LlamaBackend::init().map_err(|e| LlmError::ModelLoadError(e.to_string()))?;

        let mut layers = initial_layers;
        loop {
            match Self::try_load_model(&backend, model_path, layers) {
                Ok(model) => {
                    if layers != initial_layers {
                        eprintln!(
                            "GPU {}層でモデルロード成功（初期推定: {}層）",
                            layers, initial_layers
                        );
                    }
                    return Ok(Self {
                        backend,
                        model,
                        gpu_layers: layers,
                    });
                }
                Err(e) => {
                    if layers == 0 {
                        return Err(e);
                    }
                    eprintln!("GPU {}層でロード失敗、削減して再試行: {}", layers, e);
                    layers = next_layer_count(layers);
                }
            }
        }
    }

    /// 指定したGPU層数でモデルのロードを試みる
    fn try_load_model(
        backend: &LlamaBackend,
        model_path: &str,
        n_gpu_layers: u32,
    ) -> Result<LlamaModel, LlmError> {
        let model_params = pin!(LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers));

        LlamaModel::load_from_file(backend, model_path, &model_params)
            .map_err(|e| LlmError::ModelLoadError(e.to_string()))
    }

    /// 実際に使用されたGPUオフロード層数を返す（0 = CPU推論）
    pub fn gpu_layers(&self) -> u32 {
        self.gpu_layers
    }

    /// GPU推論が有効かどうかを返す
    pub fn is_gpu_active(&self) -> bool {
        self.gpu_layers > 0
    }
}

/// 次に試行するレイヤー数を計算する（二分探索）
fn next_layer_count(current: u32) -> u32 {
    if current >= FALLBACK_MAX_LAYERS * 2 {
        // u32::MAX や極端に大きい値からの最初のフォールバック
        FALLBACK_MAX_LAYERS
    } else {
        current / 2
    }
}

impl LlmInference for LlamaEngine {
    fn generate<F>(
        &mut self,
        prompt: &str,
        max_tokens: u32,
        mut on_token: F,
    ) -> Result<String, LlmError>
    where
        F: FnMut(&str),
    {
        let ctx_size = NonZeroU32::new(2048).unwrap();
        let ctx_params = LlamaContextParams::default().with_n_ctx(Some(ctx_size));

        let mut ctx = self
            .model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| LlmError::InferenceError(e.to_string()))?;

        // プロンプトをトークン化
        let tokens = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| LlmError::TokenizeError(e.to_string()))?;

        // バッチにトークンを投入
        let mut batch = LlamaBatch::new(2048, 1);
        let last_idx = (tokens.len() - 1) as i32;
        for (i, token) in (0_i32..).zip(tokens.into_iter()) {
            batch
                .add(token, i, &[0], i == last_idx)
                .map_err(|e| LlmError::InferenceError(e.to_string()))?;
        }

        ctx.decode(&mut batch)
            .map_err(|e| LlmError::InferenceError(e.to_string()))?;

        // サンプラー設定
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::temp(0.7),
            LlamaSampler::dist(1234),
        ]);

        // ストリーミング生成ループ
        let mut n_cur = batch.n_tokens();
        let n_len = n_cur + max_tokens as i32;
        let mut full_response = String::new();
        let mut decoder = encoding_rs::UTF_8.new_decoder();

        while n_cur < n_len {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);

            // EOSトークンで終了
            if self.model.is_eog_token(token) {
                break;
            }

            // トークンを文字列に変換
            match self.model.token_to_piece(token, &mut decoder, true, None) {
                Ok(piece) => {
                    full_response.push_str(&piece);
                    on_token(&piece);
                }
                Err(_) => continue,
            }

            // 次のデコードの準備
            batch.clear();
            batch
                .add(token, n_cur, &[0], true)
                .map_err(|e| LlmError::InferenceError(e.to_string()))?;
            ctx.decode(&mut batch)
                .map_err(|e| LlmError::InferenceError(e.to_string()))?;
            n_cur += 1;
        }

        Ok(full_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_layer_count_from_max() {
        // u32::MAX → FALLBACK_MAX_LAYERS (40)
        assert_eq!(next_layer_count(u32::MAX), FALLBACK_MAX_LAYERS);
    }

    #[test]
    fn test_next_layer_count_from_large_value() {
        // 100 (>= 80) → FALLBACK_MAX_LAYERS
        assert_eq!(next_layer_count(100), FALLBACK_MAX_LAYERS);
    }

    #[test]
    fn test_next_layer_count_binary_search() {
        // 40 → 20 → 10 → 5 → 2 → 1 → 0
        assert_eq!(next_layer_count(40), 20);
        assert_eq!(next_layer_count(20), 10);
        assert_eq!(next_layer_count(10), 5);
        assert_eq!(next_layer_count(5), 2);
        assert_eq!(next_layer_count(2), 1);
        assert_eq!(next_layer_count(1), 0);
    }

    #[test]
    fn test_full_fallback_sequence() {
        // u32::MAX から CPU(0) までの完全なシーケンスを検証
        let mut layers = u32::MAX;
        let mut sequence = vec![layers];
        while layers > 0 {
            layers = next_layer_count(layers);
            sequence.push(layers);
        }

        // u32::MAX → 40 → 20 → 10 → 5 → 2 → 1 → 0
        assert_eq!(sequence, vec![u32::MAX, 40, 20, 10, 5, 2, 1, 0]);
    }

    #[test]
    fn test_fallback_sequence_from_partial() {
        // 部分オフロード（24層）からのフォールバック
        let mut layers = 24u32;
        let mut sequence = vec![layers];
        while layers > 0 {
            layers = next_layer_count(layers);
            sequence.push(layers);
        }

        // 24 → 12 → 6 → 3 → 1 → 0
        assert_eq!(sequence, vec![24, 12, 6, 3, 1, 0]);
    }

    // ============================================================
    // next_layer_count: 境界条件
    // ============================================================

    #[test]
    fn test_next_layer_count_threshold_boundary() {
        // FALLBACK_MAX_LAYERS * 2 = 80 が閾値
        // 79 (閾値未満) → 39 (通常の二分探索)
        assert_eq!(next_layer_count(79), 39);
        // 80 (閾値丁度) → FALLBACK_MAX_LAYERS (40)
        assert_eq!(next_layer_count(80), FALLBACK_MAX_LAYERS);
        // 81 (閾値超過) → FALLBACK_MAX_LAYERS (40)
        assert_eq!(next_layer_count(81), FALLBACK_MAX_LAYERS);
    }

    #[test]
    fn test_next_layer_count_zero_input() {
        // 0 は通常呼ばれないが、安全に0を返す
        assert_eq!(next_layer_count(0), 0);
    }

    #[test]
    fn test_next_layer_count_one_input() {
        assert_eq!(next_layer_count(1), 0);
    }

    // ============================================================
    // フォールバックシーケンス: 追加パターン
    // ============================================================

    #[test]
    fn test_fallback_sequence_from_one() {
        // 1層 → 0 の最小シーケンス
        let mut layers = 1u32;
        let mut seq = vec![layers];
        while layers > 0 {
            layers = next_layer_count(layers);
            seq.push(layers);
        }
        assert_eq!(seq, vec![1, 0]);
    }

    #[test]
    fn test_fallback_sequence_from_odd() {
        // 奇数 7 → 3 → 1 → 0
        let mut layers = 7u32;
        let mut seq = vec![layers];
        while layers > 0 {
            layers = next_layer_count(layers);
            seq.push(layers);
        }
        assert_eq!(seq, vec![7, 3, 1, 0]);
    }

    #[test]
    fn test_fallback_sequence_terminates() {
        // 任意の初期値からシーケンスが有限回で0に到達する
        for initial in [1, 2, 5, 10, 20, 40, 79, 80, 100, 1000, u32::MAX] {
            let mut layers = initial;
            let mut steps = 0u32;
            while layers > 0 {
                layers = next_layer_count(layers);
                steps += 1;
                assert!(steps < 100, "initial={}で100回以内に終了しない", initial);
            }
        }
    }

    #[test]
    fn test_fallback_sequence_strictly_decreasing() {
        // シーケンスは厳密に減少する（無限ループしない）
        let mut layers = u32::MAX;
        let mut prev = layers;
        while layers > 0 {
            layers = next_layer_count(layers);
            assert!(
                layers < prev,
                "{}→{}: 厳密減少に違反",
                prev, layers
            );
            prev = layers;
        }
    }

    #[test]
    fn test_fallback_max_steps_from_u32_max() {
        // u32::MAX からのシーケンス長が適切な範囲内
        let mut layers = u32::MAX;
        let mut steps = 0u32;
        while layers > 0 {
            layers = next_layer_count(layers);
            steps += 1;
        }
        // u32::MAX → 40 → 20 → 10 → 5 → 2 → 1 → 0 = 7ステップ
        assert_eq!(steps, 7);
    }
}
