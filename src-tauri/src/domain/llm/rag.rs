use serde::Serialize;

use super::chat_template::ChatTemplate;

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
}
