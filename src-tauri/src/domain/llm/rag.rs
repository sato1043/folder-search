use serde::Serialize;

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

/// 検索結果からRAGプロンプトを構築する
pub fn build_rag_prompt(question: &str, context_chunks: &[ContextChunk]) -> String {
    let mut prompt = String::new();

    prompt.push_str("<|im_start|>system\n");
    prompt.push_str("あなたはナレッジベースに基づいて質問に回答するアシスタントです。\n");
    prompt.push_str("以下のコンテキストに基づいて質問に回答してください。\n");
    prompt.push_str("コンテキストに情報がない場合は「情報が見つかりませんでした」と回答してください。\n");
    prompt.push_str("回答の最後に、参照したファイルのパスを[参照: ファイルパス]の形式で記載してください。\n");
    prompt.push_str("<|im_end|>\n");

    prompt.push_str("<|im_start|>user\n");
    prompt.push_str("## コンテキスト\n\n");

    for (i, chunk) in context_chunks.iter().enumerate() {
        prompt.push_str(&format!("### ファイル{}: {}\n", i + 1, chunk.path));
        prompt.push_str(&chunk.text);
        prompt.push_str("\n\n");
    }

    prompt.push_str("## 質問\n");
    prompt.push_str(question);
    prompt.push_str("\n<|im_end|>\n");

    prompt.push_str("<|im_start|>assistant\n");

    prompt
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

    #[test]
    fn test_build_rag_prompt_contains_context() {
        let chunks = vec![
            ContextChunk {
                path: "/docs/rust.md".to_string(),
                text: "RustはMozillaが開発したプログラミング言語".to_string(),
            },
            ContextChunk {
                path: "/docs/tauri.md".to_string(),
                text: "TauriはRust製のデスクトップフレームワーク".to_string(),
            },
        ];

        let prompt = build_rag_prompt("Rustとは何ですか？", &chunks);

        assert!(prompt.contains("Rustとは何ですか？"));
        assert!(prompt.contains("/docs/rust.md"));
        assert!(prompt.contains("RustはMozillaが開発した"));
        assert!(prompt.contains("/docs/tauri.md"));
        assert!(prompt.contains("<|im_start|>assistant"));
    }

    #[test]
    fn test_build_rag_prompt_empty_context() {
        let prompt = build_rag_prompt("テスト質問", &[]);
        assert!(prompt.contains("テスト質問"));
        assert!(prompt.contains("assistant"));
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
}
