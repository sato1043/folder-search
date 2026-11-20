/// embeddingベクトル
pub type Embedding = Vec<f32>;

/// embedding生成のトレイト
pub trait EmbeddingGenerator {
    /// テキストからembeddingベクトルを生成する
    fn generate(&mut self, text: &str) -> Result<Embedding, EmbeddingError>;

    /// 複数テキストのembeddingをバッチ生成する
    fn generate_batch(&mut self, texts: &[&str]) -> Result<Vec<Embedding>, EmbeddingError>;
}

/// ベクトル検索のトレイト
pub trait VectorSearcher {
    /// クエリベクトルに最も近いチャンクを検索する
    fn search_nearest(
        &self,
        query_embedding: &Embedding,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, VectorSearchError>;
}

/// ベクトル検索結果
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    /// チャンクの識別子（source_path + chunk_index）
    pub chunk_id: usize,
    /// 元ファイルのパス
    pub source_path: String,
    /// 距離（小さいほど類似）
    pub distance: f32,
    /// チャンクテキスト
    pub text: String,
}

/// embeddingエラー
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("モデルの読み込みに失敗: {0}")]
    ModelLoadError(String),
    #[error("embedding生成に失敗: {0}")]
    GenerationError(String),
    #[error("トークナイザエラー: {0}")]
    TokenizerError(String),
}

/// ベクトル検索エラー
#[derive(Debug, thiserror::Error)]
pub enum VectorSearchError {
    #[error("インデックスが存在しない")]
    IndexNotFound,
    #[error("検索に失敗: {0}")]
    SearchError(String),
}
