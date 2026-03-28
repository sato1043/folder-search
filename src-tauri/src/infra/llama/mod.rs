use std::num::NonZeroU32;
use std::pin::pin;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use crate::domain::llm::{LlmError, LlmInference};

/// llama.cpp による LLM推論エンジン
pub struct LlamaEngine {
    backend: LlamaBackend,
    model: LlamaModel,
}

impl LlamaEngine {
    /// GGUFモデルをロードして初期化する
    pub fn new(model_path: &str) -> Result<Self, LlmError> {
        let backend =
            LlamaBackend::init().map_err(|e| LlmError::ModelLoadError(e.to_string()))?;

        let model_params = pin!(LlamaModelParams::default());

        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .map_err(|e| LlmError::ModelLoadError(e.to_string()))?;

        Ok(Self { backend, model })
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
