use serde::Serialize;

use super::chat_template::ChatTemplate;
use crate::domain::embedding::VectorSearchResult;
use crate::domain::search::hybrid::{reciprocal_rank_fusion, RRF_K};
use crate::domain::search::SearchResult;

/// RAGで使用するコンテキストチャンク
#[derive(Debug, Clone, Serialize)]
pub struct ContextChunk {
    /// ファイルパス
    pub path: String,
    /// チャンクテキスト
    pub text: String,
}

/// RAGの回答結果
#[derive(Debug, Clone, Serialize)]
pub struct RagAnswer {
    /// LLMの回答テキスト
    pub answer: String,
    /// 参照元ファイルの一覧
    pub sources: Vec<String>,
}

/// RAGのコンテキストに載せる量の上限
#[derive(Debug, Clone)]
pub struct ContextLimits {
    /// コンテキストに載せるファイルの数
    pub max_files: usize,
    /// 1ファイルから載せるチャンクの数
    pub max_chunks_per_file: usize,
    /// 1ファイルあたりの本文の文字数（区切りの文字列を除く）
    pub chars_per_file: usize,
}

impl Default for ContextLimits {
    fn default() -> Self {
        Self {
            max_files: 5,
            max_chunks_per_file: 2,
            chars_per_file: 1000,
        }
    }
}

/// 同じファイルの離れたチャンクをつなぐ区切り
pub const CHUNK_SEPARATOR: &str = "\n…\n";

/// 検索結果からRAGのコンテキストを組み立てる
///
/// RRFでファイルを順位付けし、上位のファイルごとに今の本文から載せる部分を選ぶ。
/// 当たった箇所がファイルの後半にあっても、LLMへ届けるためである。
///
/// 本文は `read_file` で読み直す。読めないファイル（インデックス作成後に消えた等）は除き、
/// 次の順位のファイルで上限まで詰める。インデックスにだけ残る古い本文を渡さないためである。
pub fn select_context_chunks(
    fulltext: &[SearchResult],
    vector: &[VectorSearchResult],
    limits: &ContextLimits,
    read_file: impl Fn(&str) -> Option<String>,
) -> Vec<ContextChunk> {
    let fulltext_paths: Vec<String> = fulltext.iter().map(|r| r.path.clone()).collect();
    let vector_paths: Vec<String> = vector.iter().map(|r| r.source_path.clone()).collect();
    let ranked = reciprocal_rank_fusion(&fulltext_paths, &vector_paths, RRF_K);

    ranked
        .iter()
        .filter_map(|(path, _)| {
            let content = read_file(path)?;
            let fragment = fulltext
                .iter()
                .find(|r| &r.path == path)
                .map_or("", |r| r.fragment.as_str());
            Some(ContextChunk {
                path: path.clone(),
                text: file_context_text(&content, path, fragment, vector, limits),
            })
        })
        .take(limits.max_files)
        .collect()
}

/// 1ファイルの今の本文から、コンテキストに載せる部分を選んでつなぐ
///
/// - ベクトル検索で当たったチャンクのうち、今の本文に残るものを順位の高い順に選ぶ。
///   全文検索の一致箇所がどれにも含まれなければ、最後の枠を一致箇所を中心とする窓に替える
/// - 残るチャンクが無ければ、一致箇所を中心とする窓。見つからなければ先頭
///
/// 選んだ部分はファイル内の順に並べる。
fn file_context_text(
    content: &str,
    path: &str,
    fragment: &str,
    vector: &[VectorSearchResult],
    limits: &ContextLimits,
) -> String {
    let mut parts: Vec<(usize, &str)> = vector
        .iter()
        .filter(|r| r.source_path == path)
        .filter_map(|r| {
            content
                .find(r.text.as_str())
                .map(|pos| (pos, r.text.as_str()))
        })
        .take(limits.max_chunks_per_file)
        .collect();
    if parts.is_empty() {
        return window_around(content, fragment, limits.chars_per_file)
            .1
            .to_string();
    }

    // 一致箇所が今の本文に無ければ（インデックスが古い等）、窓は先頭になるので替えない
    let fragment_missed =
        content.contains(fragment) && !parts.iter().any(|(_, text)| text.contains(fragment));
    if fragment_missed {
        parts.truncate(limits.max_chunks_per_file.saturating_sub(1));
        let width = limits.chars_per_file / limits.max_chunks_per_file.max(1);
        parts.push(window_around(content, fragment, width));
    }
    parts.sort_by_key(|(pos, _)| *pos);
    join_capped(&parts, limits.chars_per_file)
}

/// 部分を区切りでつなぐ。本文は合わせて `max_chars` 文字までに切る
fn join_capped(parts: &[(usize, &str)], max_chars: usize) -> String {
    let mut remaining = max_chars;
    parts
        .iter()
        .map(|(_, text)| {
            let part: String = text.chars().take(remaining).collect();
            remaining -= part.chars().count();
            part
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(CHUNK_SEPARATOR)
}

/// `fragment` を中心とする `width` 文字の窓を切り出し、本文での開始位置（バイト）と組で返す
///
/// `fragment` が空か本文に見つからなければ先頭から切り出す。
/// ファイルの端では窓をずらし、本文が足りる限り `width` 文字を保つ。
fn window_around<'a>(content: &'a str, fragment: &str, width: usize) -> (usize, &'a str) {
    let found = (!fragment.is_empty())
        .then(|| content.find(fragment))
        .flatten();

    let start_char = match found {
        Some(byte_pos) => {
            let fragment_start = content[..byte_pos].chars().count();
            let center = fragment_start + fragment.chars().count() / 2;
            let end = (center.saturating_sub(width / 2) + width).min(content.chars().count());
            end.saturating_sub(width)
        }
        None => 0,
    };
    let start = byte_offset(content, start_char);
    let end = byte_offset(content, start_char + width);
    (start, &content[start..end])
}

/// 先頭から `char_pos` 文字目のバイト位置。本文の長さを超えれば本文の長さ
fn byte_offset(content: &str, char_pos: usize) -> usize {
    content
        .char_indices()
        .nth(char_pos)
        .map_or(content.len(), |(pos, _)| pos)
}

/// RAG用のシステムメッセージ
const SYSTEM_MESSAGE: &str = "\
あなたはナレッジベースに基づいて質問に回答するアシスタントです。\n\
以下のコンテキストに基づいて質問に回答してください。\n\
コンテキストに情報がない場合は「情報が見つかりませんでした」と回答してください。\n\
回答の最後に、参照したファイルのパスを[参照: ファイルパス]の形式で記載してください。";

/// コンテキストと質問からユーザーメッセージを組み立てる
fn build_user_message(question: &str, context_chunks: &[ContextChunk]) -> String {
    let mut user = String::new();
    user.push_str("## コンテキスト\n\n");

    for (i, chunk) in context_chunks.iter().enumerate() {
        user.push_str(&format!("### ファイル{}: {}\n", i + 1, chunk.path));
        user.push_str(&chunk.text);
        user.push_str("\n\n");
    }

    user.push_str("## 質問\n");
    user.push_str(question);
    user
}

/// 検索結果からRAGプロンプトを構築する
pub fn build_rag_prompt(
    question: &str,
    context_chunks: &[ContextChunk],
    template: &ChatTemplate,
) -> String {
    let user_message = build_user_message(question, context_chunks);
    template.format_prompt(SYSTEM_MESSAGE, &user_message)
}

/// LLMの回答から参照元ファイルを抽出する
pub fn extract_sources(answer: &str) -> Vec<String> {
    let mut sources = Vec::new();
    for line in answer.lines() {
        // [参照: /path/to/file] 形式を抽出
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("[参照:") {
            if let Some(path) = rest.strip_suffix(']') {
                let path = path.trim().to_string();
                if !path.is_empty() && !sources.contains(&path) {
                    sources.push(path);
                }
            }
        }
    }
    sources
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_chunks() -> Vec<ContextChunk> {
        vec![
            ContextChunk {
                path: "/docs/rust.md".to_string(),
                text: "RustはMozillaが開発したプログラミング言語".to_string(),
            },
            ContextChunk {
                path: "/docs/tauri.md".to_string(),
                text: "TauriはRust製のデスクトップフレームワーク".to_string(),
            },
        ]
    }

    #[test]
    fn test_build_rag_prompt_chatml() {
        let chunks = test_chunks();
        let prompt = build_rag_prompt("Rustとは何ですか？", &chunks, &ChatTemplate::Chatml);

        assert!(prompt.contains("Rustとは何ですか？"));
        assert!(prompt.contains("/docs/rust.md"));
        assert!(prompt.contains("RustはMozillaが開発した"));
        assert!(prompt.contains("<|im_start|>assistant"));
    }

    #[test]
    fn test_build_rag_prompt_gemma() {
        let chunks = test_chunks();
        let prompt = build_rag_prompt("Rustとは何ですか？", &chunks, &ChatTemplate::Gemma);

        assert!(prompt.contains("Rustとは何ですか？"));
        assert!(prompt.contains("/docs/rust.md"));
        assert!(prompt.contains("<start_of_turn>model"));
        // Gemma は system ロールがないため im_start は含まれない
        assert!(!prompt.contains("<|im_start|>"));
    }

    #[test]
    fn test_build_rag_prompt_llama3() {
        let chunks = test_chunks();
        let prompt = build_rag_prompt("Rustとは何ですか？", &chunks, &ChatTemplate::Llama3);

        assert!(prompt.contains("Rustとは何ですか？"));
        assert!(prompt.contains("<|start_header_id|>assistant<|end_header_id|>"));
        assert!(prompt.contains("<|start_header_id|>system<|end_header_id|>"));
    }

    #[test]
    fn test_build_rag_prompt_empty_context() {
        let prompt = build_rag_prompt("テスト質問", &[], &ChatTemplate::Chatml);
        assert!(prompt.contains("テスト質問"));
        assert!(prompt.contains("assistant"));
    }

    #[test]
    fn test_build_rag_prompt_contains_system_message() {
        for template in [
            ChatTemplate::Chatml,
            ChatTemplate::Gemma,
            ChatTemplate::Llama3,
        ] {
            let prompt = build_rag_prompt("質問", &[], &template);
            assert!(
                prompt.contains("ナレッジベースに基づいて"),
                "template {:?} にシステムメッセージが含まれるべき",
                template
            );
        }
    }

    #[test]
    fn test_extract_sources() {
        let answer = "Rustは安全な言語です。\n[参照: /docs/rust.md]\n[参照: /docs/safety.md]";
        let sources = extract_sources(answer);
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0], "/docs/rust.md");
        assert_eq!(sources[1], "/docs/safety.md");
    }

    #[test]
    fn test_extract_sources_no_references() {
        let answer = "特に参照なしの回答です。";
        let sources = extract_sources(answer);
        assert!(sources.is_empty());
    }

    #[test]
    fn test_extract_sources_dedup() {
        let answer = "[参照: /docs/a.md]\n[参照: /docs/a.md]";
        let sources = extract_sources(answer);
        assert_eq!(sources.len(), 1);
    }

    // --- select_context_chunks ---

    fn fulltext_hit(path: &str, fragment: &str) -> SearchResult {
        SearchResult {
            path: path.to_string(),
            title: path.to_string(),
            snippet: String::new(),
            fragment: fragment.to_string(),
            score: 1.0,
        }
    }

    fn vector_hit(path: &str, chunk_id: usize, text: &str) -> VectorSearchResult {
        VectorSearchResult {
            chunk_id,
            source_path: path.to_string(),
            distance: 0.1,
            text: text.to_string(),
        }
    }

    fn body_chars(chunk: &ContextChunk) -> usize {
        chunk.text.replace(CHUNK_SEPARATOR, "").chars().count()
    }

    #[test]
    fn test_context_uses_vector_chunk_text_beyond_file_head() {
        // 答えはファイルの後半（7番目のチャンク）にだけある
        let content = format!("{}料理店の名前は山猫軒である", "前置き".repeat(1000));
        let vector = vec![vector_hit("/a.md", 6, "料理店の名前は山猫軒である")];

        let chunks = select_context_chunks(&[], &vector, &ContextLimits::default(), |_| {
            Some(content.clone())
        });

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].path, "/a.md");
        assert!(chunks[0].text.contains("山猫軒"));
        assert!(!chunks[0].text.contains("前置き"));
    }

    #[test]
    fn test_context_skips_deleted_vector_hit_file() {
        // インデックス作成後に消えたファイルは、ベクトル検索で当たっても載せない
        let vector = vec![
            vector_hit("/gone.md", 0, "消えた本文"),
            vector_hit("/b.md", 0, "残った本文"),
        ];

        let chunks = select_context_chunks(&[], &vector, &ContextLimits::default(), |path| {
            (path == "/b.md").then(|| "残った本文".to_string())
        });

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].path, "/b.md");
    }

    #[test]
    fn test_context_ignores_stale_chunk_not_in_current_content() {
        // インデックス作成後に書き換えたファイルは、古いチャンクでなく今の本文を使う
        let vector = vec![vector_hit("/a.md", 0, "書き換える前の本文")];

        let chunks = select_context_chunks(&[], &vector, &ContextLimits::default(), |_| {
            Some("書き換えた後の本文".to_string())
        });

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "書き換えた後の本文");
    }

    #[test]
    fn test_context_skips_stale_chunk_and_takes_next_current_one() {
        let vector = vec![
            vector_hit("/a.md", 1, "消した段落"),
            vector_hit("/a.md", 3, "残った段落その一"),
            vector_hit("/a.md", 5, "残った段落その二"),
        ];

        let chunks = select_context_chunks(&[], &vector, &ContextLimits::default(), |_| {
            Some("残った段落その一。残った段落その二。".to_string())
        });

        assert_eq!(
            chunks[0].text,
            format!("残った段落その一{}残った段落その二", CHUNK_SEPARATOR)
        );
    }

    #[test]
    fn test_context_takes_top_two_chunks_in_file_order() {
        // 順位順に 5, 1, 3。上位2つ（5と1）をファイル内の順（1, 5）に並べる
        let vector = vec![
            vector_hit("/a.md", 5, "六番目"),
            vector_hit("/a.md", 1, "二番目"),
            vector_hit("/a.md", 3, "四番目"),
        ];

        let chunks = select_context_chunks(&[], &vector, &ContextLimits::default(), |_| {
            Some("一番目二番目三番目四番目五番目六番目".to_string())
        });

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, format!("二番目{}六番目", CHUNK_SEPARATOR));
    }

    #[test]
    fn test_context_vector_text_is_capped_per_file() {
        let first = "あ".repeat(600);
        let second = "い".repeat(600);
        let content = format!("{}{}", first, second);
        let vector = vec![
            vector_hit("/a.md", 0, &first),
            vector_hit("/a.md", 1, &second),
        ];

        let chunks = select_context_chunks(&[], &vector, &ContextLimits::default(), |_| {
            Some(content.clone())
        });

        assert_eq!(body_chars(&chunks[0]), 1000);
    }

    #[test]
    fn test_context_fulltext_only_uses_window_around_fragment() {
        let content = format!("{}注文の多い料理店{}", "前".repeat(3000), "後".repeat(3000));
        let fulltext = vec![fulltext_hit("/b.md", "注文の多い料理店")];

        let chunks = select_context_chunks(&fulltext, &[], &ContextLimits::default(), |_| {
            Some(content.clone())
        });

        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("注文の多い料理店"));
        assert_eq!(chunks[0].text.chars().count(), 1000);
        // 一致箇所が窓のほぼ中央に来る
        let pos = chunks[0].text.find("注文の多い料理店").unwrap();
        let before = chunks[0].text[..pos].chars().count();
        assert!((490..=500).contains(&before), "窓の前側 {} 文字", before);
    }

    #[test]
    fn test_context_fulltext_window_near_file_end_keeps_full_width() {
        let content = format!("{}末尾の一致", "前".repeat(3000));
        let fulltext = vec![fulltext_hit("/b.md", "末尾の一致")];

        let chunks = select_context_chunks(&fulltext, &[], &ContextLimits::default(), |_| {
            Some(content.clone())
        });

        assert_eq!(chunks[0].text.chars().count(), 1000);
        assert!(chunks[0].text.ends_with("末尾の一致"));
    }

    #[test]
    fn test_context_fulltext_falls_back_to_head_when_fragment_not_found() {
        let content = format!("{}{}", "頭".repeat(1000), "尾".repeat(1000));
        let fulltext = vec![fulltext_hit("/b.md", "ファイルに無い断片")];

        let chunks = select_context_chunks(&fulltext, &[], &ContextLimits::default(), |_| {
            Some(content.clone())
        });

        assert_eq!(chunks[0].text, "頭".repeat(1000));
    }

    #[test]
    fn test_context_fulltext_short_file_is_used_whole() {
        let fulltext = vec![fulltext_hit("/b.md", "短い")];
        let chunks = select_context_chunks(&fulltext, &[], &ContextLimits::default(), |_| {
            Some("短いファイル".to_string())
        });
        assert_eq!(chunks[0].text, "短いファイル");
    }

    #[test]
    fn test_context_skips_unreadable_fulltext_only_file() {
        let fulltext = vec![
            fulltext_hit("/gone.md", "断片"),
            fulltext_hit("/b.md", "断片"),
        ];
        let chunks = select_context_chunks(&fulltext, &[], &ContextLimits::default(), |path| {
            (path == "/b.md").then(|| "断片を含む本文".to_string())
        });
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].path, "/b.md");
    }

    #[test]
    fn test_context_limits_number_of_files() {
        let vector: Vec<VectorSearchResult> = (0..7)
            .map(|i| vector_hit(&format!("/{}.md", i), 0, "本文"))
            .collect();

        let chunks = select_context_chunks(&[], &vector, &ContextLimits::default(), |_| {
            Some("本文".to_string())
        });

        assert_eq!(chunks.len(), 5);
        let paths: Vec<&str> = chunks.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, vec!["/0.md", "/1.md", "/2.md", "/3.md", "/4.md"]);
    }

    #[test]
    fn test_context_fills_file_count_after_skipping_unreadable_files() {
        // 読めないファイルを除いたあとで上限まで詰める
        let limits = ContextLimits {
            max_files: 2,
            ..ContextLimits::default()
        };
        let vector = vec![
            vector_hit("/gone.md", 0, "本文"),
            vector_hit("/a.md", 0, "本文"),
            vector_hit("/b.md", 0, "本文"),
        ];

        let chunks = select_context_chunks(&[], &vector, &limits, |path| {
            (path != "/gone.md").then(|| "本文".to_string())
        });

        let paths: Vec<&str> = chunks.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, vec!["/a.md", "/b.md"]);
    }

    #[test]
    fn test_context_hit_by_both_keeps_chunks_containing_fragment() {
        let fulltext = vec![fulltext_hit("/a.md", "山猫軒")];
        let vector = vec![
            vector_hit("/a.md", 4, "料理店の名前は山猫軒である"),
            vector_hit("/a.md", 0, "冒頭の段落"),
        ];

        let chunks = select_context_chunks(&fulltext, &vector, &ContextLimits::default(), |_| {
            Some(format!(
                "冒頭の段落{}料理店の名前は山猫軒である",
                "中".repeat(2000)
            ))
        });

        assert_eq!(
            chunks[0].text,
            format!("冒頭の段落{}料理店の名前は山猫軒である", CHUNK_SEPARATOR)
        );
    }

    #[test]
    fn test_context_hit_by_both_uses_fragment_window_when_chunks_miss_it() {
        // 全文検索の一致箇所がどのチャンクにも無ければ、2つ目の枠を一致箇所の窓に替える
        let content = format!(
            "冒頭の段落{}注文の多い料理店{}末尾の段落",
            "前".repeat(2000),
            "後".repeat(2000)
        );
        let fulltext = vec![fulltext_hit("/a.md", "注文の多い料理店")];
        let vector = vec![
            vector_hit("/a.md", 9, "末尾の段落"),
            vector_hit("/a.md", 0, "冒頭の段落"),
        ];

        let chunks = select_context_chunks(&fulltext, &vector, &ContextLimits::default(), |_| {
            Some(content.clone())
        });

        let parts: Vec<&str> = chunks[0].text.split(CHUNK_SEPARATOR).collect();
        assert_eq!(parts.len(), 2);
        // ファイル内の順に、一致箇所の窓・順位1位のチャンクと並ぶ
        assert!(parts[0].contains("注文の多い料理店"));
        assert_eq!(parts[0].chars().count(), 500);
        assert_eq!(parts[1], "末尾の段落");
    }

    #[test]
    fn test_context_hit_by_both_keeps_chunks_when_fragment_is_not_in_content() {
        // 一致箇所が今の本文に無ければ、窓は先頭になるだけなのでチャンクを替えない
        let content = format!("冒頭の段落{}末尾の段落", "中".repeat(2000));
        let fulltext = vec![fulltext_hit("/a.md", "書き換える前の一致箇所")];
        let vector = vec![
            vector_hit("/a.md", 9, "末尾の段落"),
            vector_hit("/a.md", 0, "冒頭の段落"),
        ];

        let chunks = select_context_chunks(&fulltext, &vector, &ContextLimits::default(), |_| {
            Some(content.clone())
        });

        assert_eq!(
            chunks[0].text,
            format!("冒頭の段落{}末尾の段落", CHUNK_SEPARATOR)
        );
    }
}
