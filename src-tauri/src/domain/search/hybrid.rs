use std::collections::{HashMap, HashSet};

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

/// 画面へ渡すスニペットを選ぶ
///
/// 全文検索のスニペットを優先し、なければベクトル検索のチャンクテキストを使う。
pub fn display_snippet(fulltext_snippet: Option<&str>, chunk_text: Option<&str>) -> String {
    fulltext_snippet
        .or(chunk_text)
        .map(str::to_string)
        .unwrap_or_default()
}

/// RRFの調整パラメータ（検索モードのハイブリッド検索とRAGで共有する）
pub const RRF_K: f32 = 60.0;

/// RRF（Reciprocal Rank Fusion）によるスコア統合
///
/// `k` は調整パラメータ（通常は [`RRF_K`] を渡す）
///
/// 各リストで同じパスは初出の1件だけを数え、順位は重複を除いた並びで付ける。
/// ベクトル検索の結果はチャンク単位で同じファイルが複数回現れるため、
/// そのまま数えるとファイルのスコアが重ねて加算される。
pub fn reciprocal_rank_fusion(
    fulltext_paths: &[String],
    vector_paths: &[String],
    k: f32,
) -> Vec<(String, f32)> {
    let mut scores: HashMap<String, f32> = HashMap::new();

    for list in [fulltext_paths, vector_paths] {
        let mut seen: HashSet<&str> = HashSet::new();
        let unique_paths = list.iter().filter(|path| seen.insert(path.as_str()));
        for (rank, path) in unique_paths.enumerate() {
            *scores.entry(path.clone()).or_insert(0.0) += 1.0 / (k + rank as f32 + 1.0);
        }
    }

    // 同点はパスの順に並べ、順位を実行ごとに変えない
    let mut ranked: Vec<(String, f32)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
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
    fn test_display_snippet_prefers_fulltext_snippet() {
        let snippet = display_snippet(Some("<b>Rust</b>の資料"), Some("チャンク"));
        assert_eq!(snippet, "<b>Rust</b>の資料");
    }

    #[test]
    fn test_display_snippet_falls_back_to_chunk_text() {
        assert_eq!(display_snippet(None, Some("チャンク")), "チャンク");
        assert_eq!(display_snippet(None, None), "");
    }

    #[test]
    fn test_display_snippet_escapes_chunk_text() {
        // 画面は snippet を HTML として描画するので、生のチャンクテキストに含まれる
        // マークアップを実行させない
        let snippet = display_snippet(None, Some(r#"<img src=x onerror="alert(1)"> & 'R&D'"#));
        assert_eq!(
            snippet,
            "&lt;img src=x onerror=&quot;alert(1)&quot;&gt; &amp; &#39;R&amp;D&#39;"
        );
    }

    #[test]
    fn test_rrf_orders_tied_scores_by_path() {
        // 各順位で全文とベクトルの1件ずつが同点になる。同点はパスの順に並ぶ
        let fulltext: Vec<String> = ["h.md", "f.md", "d.md", "b.md"]
            .iter()
            .map(|p| p.to_string())
            .collect();
        let vector: Vec<String> = ["g.md", "e.md", "c.md", "a.md"]
            .iter()
            .map(|p| p.to_string())
            .collect();

        let results = reciprocal_rank_fusion(&fulltext, &vector, 60.0);

        let paths: Vec<&str> = results.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            vec!["g.md", "h.md", "e.md", "f.md", "c.md", "d.md", "a.md", "b.md"]
        );
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

    #[test]
    fn test_rrf_counts_duplicate_path_once_per_list() {
        // ベクトル検索はチャンク単位なので、同じファイルが複数回現れる
        let vector = vec![
            "a.md".to_string(),
            "a.md".to_string(),
            "b.md".to_string(),
            "a.md".to_string(),
        ];

        let results = reciprocal_rank_fusion(&[], &vector, 60.0);
        let score_of = |path: &str| {
            results
                .iter()
                .find(|(p, _)| p == path)
                .map(|(_, s)| *s)
                .unwrap()
        };

        // a.md は初出の1位だけ、b.md は重複を除いた順位の2位で数える
        assert_eq!(results.len(), 2);
        assert!((score_of("a.md") - 1.0 / 61.0).abs() < 1e-6);
        assert!((score_of("b.md") - 1.0 / 62.0).abs() < 1e-6);
    }
}
