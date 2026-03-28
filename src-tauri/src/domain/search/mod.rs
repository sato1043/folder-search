use serde::Serialize;

/// 検索結果の1件を表す
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    /// ファイルパス
    pub path: String,
    /// ファイル名（タイトル）
    pub title: String,
    /// マッチ箇所のスニペット
    pub snippet: String,
    /// 関連度スコア
    pub score: f32,
}

/// 全文検索のトレイト
pub trait FulltextSearcher {
    /// クエリ文字列で検索し、結果を返す
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, SearchError>;
}

/// 検索エラー
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("インデックスが存在しない: {0}")]
    IndexNotFound(String),
    #[error("クエリのパースに失敗: {0}")]
    QueryParseError(String),
    #[error("検索中にエラーが発生: {0}")]
    InternalError(String),
}
