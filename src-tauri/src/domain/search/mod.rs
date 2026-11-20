pub mod hybrid;

use serde::Serialize;

/// 検索結果の1件を表す
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    /// ファイルパス
    pub path: String,
    /// ファイル名（タイトル）
    pub title: String,
    /// マッチ箇所のスニペット（画面表示用、HTMLエスケープ・強調タグ付き）
    pub snippet: String,
    /// マッチ箇所の断片（本文の文字列そのまま）。RAGで本文中の一致位置を探すために使い、
    /// フロントエンドへは渡さない
    #[serde(skip_serializing)]
    pub fragment: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_result_serialization_omits_fragment() {
        let result = SearchResult {
            path: "/a.md".to_string(),
            title: "a.md".to_string(),
            snippet: "<b>Rust</b>".to_string(),
            fragment: "Rust".to_string(),
            score: 1.0,
        };

        let value = serde_json::to_value(&result).unwrap();
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();

        // フロントエンドへ渡す形は変えない（fragment はバックエンド内部でだけ使う）
        assert_eq!(keys.len(), 4);
        assert!(!keys.contains(&"fragment"));
    }
}
