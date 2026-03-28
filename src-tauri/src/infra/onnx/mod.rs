use std::path::Path;

use ndarray::Array2;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

use crate::domain::embedding::{Embedding, EmbeddingError, EmbeddingGenerator};

/// ONNX Runtime によるembedding生成
pub struct OnnxEmbeddingGenerator {
    session: Session,
    tokenizer: Tokenizer,
}

impl OnnxEmbeddingGenerator {
    /// モデルとトークナイザを読み込んで初期化する
    pub fn new(model_path: &str, tokenizer_path: &str) -> Result<Self, EmbeddingError> {
        if !Path::new(model_path).exists() {
            return Err(EmbeddingError::ModelLoadError(format!(
                "モデルファイルが見つからない: {}",
                model_path
            )));
        }
        if !Path::new(tokenizer_path).exists() {
            return Err(EmbeddingError::ModelLoadError(format!(
                "トークナイザファイルが見つからない: {}",
                tokenizer_path
            )));
        }

        let session = ort::session::builder::SessionBuilder::new()
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?
            .with_intra_threads(4)
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?
            .commit_from_file(model_path)
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbeddingError::TokenizerError(e.to_string()))?;

        Ok(Self { session, tokenizer })
    }

    /// mean pooling + L2正規化でembeddingを取得する
    fn mean_pool_and_normalize(
        attention_mask: &[u32],
        hidden_states: &[f32],
        hidden_size: usize,
        seq_len: usize,
    ) -> Embedding {
        let mut pooled = vec![0.0f32; hidden_size];
        let mut count = 0.0f32;

        for i in 0..seq_len {
            if attention_mask[i] == 1 {
                for j in 0..hidden_size {
                    pooled[j] += hidden_states[i * hidden_size + j];
                }
                count += 1.0;
            }
        }

        if count > 0.0 {
            pooled.iter_mut().for_each(|v| *v /= count);
        }

        // L2正規化
        let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            pooled.iter_mut().for_each(|v| *v /= norm);
        }

        pooled
    }
}

impl EmbeddingGenerator for OnnxEmbeddingGenerator {
    fn generate(&mut self, text: &str) -> Result<Embedding, EmbeddingError> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| EmbeddingError::TokenizerError(e.to_string()))?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let attention_mask_raw = encoding.get_attention_mask();
        let attention_mask_i64: Vec<i64> = attention_mask_raw.iter().map(|&m| m as i64).collect();
        let seq_len = input_ids.len();

        let ids_array = Array2::from_shape_vec((1, seq_len), input_ids)
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;
        let mask_array = Array2::from_shape_vec((1, seq_len), attention_mask_i64)
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;

        let token_type_ids = vec![0i64; seq_len];
        let token_type_array = Array2::from_shape_vec((1, seq_len), token_type_ids)
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;

        let ids_tensor = Tensor::from_array(ids_array)
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;
        let mask_tensor = Tensor::from_array(mask_array)
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;
        let type_tensor = Tensor::from_array(token_type_array)
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;

        let outputs = self
            .session
            .run(ort::inputs![ids_tensor, mask_tensor, type_tensor])
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;

        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;

        let hidden_size = shape[2] as usize;

        Ok(Self::mean_pool_and_normalize(
            attention_mask_raw,
            data,
            hidden_size,
            seq_len,
        ))
    }

    fn generate_batch(&mut self, texts: &[&str]) -> Result<Vec<Embedding>, EmbeddingError> {
        texts.iter().map(|text| self.generate(text)).collect()
    }
}
