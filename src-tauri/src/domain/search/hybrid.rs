use std::collections::HashMap;

use serde::Serialize;

/// ハイブリッド検索結果
#[derive(Debug, Clone, Serialize)]
pub struct HybridSearchResult {
    /// ファイルパス
    pub path: String,
    /// ファイル名
    pub title: String,
    /// スニペット（全文検索由来、なければチャンクテキスト）
    pub snippet: String,
    /// RRFスコア
    pub score: f32,
    /// 検索ソース（"fulltext", "vector", "hybrid"）
    pub source: String,
}

/// RRF（Reciprocal Rank Fusion）によるスコア統合
///
/// `k` は調整パラメータ（一般的に60が使われる）
pub fn reciprocal_rank_fusion(
    fulltext_paths: &[String],
    vector_paths: &[String],
    k: f32,
) -> Vec<(String, f32)> {
    let mut scores: HashMap<String, f32> = HashMap::new();

    for (rank, path) in fulltext_paths.iter().enumerate() {
        *scores.entry(path.clone()).or_insert(0.0) += 1.0 / (k + rank as f32 + 1.0);
    }

    for (rank, path) in vector_paths.iter().enumerate() {
        *scores.entry(path.clone()).or_insert(0.0) += 1.0 / (k + rank as f32 + 1.0);
    }

    let mut ranked: Vec<(String, f32)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_basic() {
        let fulltext = vec!["a.md".to_string(), "b.md".to_string(), "c.md".to_string()];
        let vector = vec!["b.md".to_string(), "a.md".to_string(), "d.md".to_string()];

        let results = reciprocal_rank_fusion(&fulltext, &vector, 60.0);

        // a.md と b.md は両方に出現するためスコアが高い
        assert!(!results.is_empty());
        let top_paths: Vec<&str> = results.iter().take(2).map(|(p, _)| p.as_str()).collect();
        assert!(top_paths.contains(&"a.md"), "aは上位に来る");
        assert!(top_paths.contains(&"b.md"), "bは上位に来る");
    }

    #[test]
    fn test_rrf_same_rank_higher_score() {
        let fulltext = vec!["a.md".to_string()];
        let vector = vec!["a.md".to_string()];

        let results = reciprocal_rank_fusion(&fulltext, &vector, 60.0);
        assert_eq!(results.len(), 1);

        // 両方で1位 → スコアは 2 * 1/(60+1) ≈ 0.0328
        let (_, score) = &results[0];
        let expected = 2.0 / 61.0;
        assert!((score - expected).abs() < 0.001);
    }

    #[test]
    fn test_rrf_disjoint_results() {
        let fulltext = vec!["a.md".to_string()];
        let vector = vec!["b.md".to_string()];

        let results = reciprocal_rank_fusion(&fulltext, &vector, 60.0);
        assert_eq!(results.len(), 2);

        // 同じスコア（両方とも 1/(60+1)）
        let (_, s1) = &results[0];
        let (_, s2) = &results[1];
        assert!((s1 - s2).abs() < 0.001);
    }

    #[test]
    fn test_rrf_empty_inputs() {
        let results = reciprocal_rank_fusion(&[], &[], 60.0);
        assert!(results.is_empty());
    }

    #[test]
    fn test_rrf_one_empty() {
        let fulltext = vec!["a.md".to_string(), "b.md".to_string()];
        let results = reciprocal_rank_fusion(&fulltext, &[], 60.0);
        assert_eq!(results.len(), 2);
    }
}
