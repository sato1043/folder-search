use serde::{Deserialize, Serialize};

/// LLMモデルのチャットテンプレート
///
/// モデルファミリーごとに異なるプロンプト形式を定義する
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatTemplate {
    /// Qwen2.5, Yi 等が使用する ChatML 形式
    #[serde(alias = "ChatML")]
    Chatml,
    /// Gemma 2/3 が使用する形式
    #[serde(alias = "Gemma")]
    Gemma,
    /// Llama 3/3.1 が使用する形式
    #[serde(alias = "Llama3")]
    Llama3,
}

impl ChatTemplate {
    /// system メッセージと user メッセージを整形し、
    /// アシスタント応答の開始位置で終わるプロンプト文字列を返す
    pub fn format_prompt(&self, system: &str, user: &str) -> String {
        match self {
            ChatTemplate::Chatml => format_chatml(system, user),
            ChatTemplate::Gemma => format_gemma(system, user),
            ChatTemplate::Llama3 => format_llama3(system, user),
        }
    }
}

/// ChatML 形式（Qwen2.5, Yi）
///
/// ```text
/// <|im_start|>system
/// {system}<|im_end|>
/// <|im_start|>user
/// {user}<|im_end|>
/// <|im_start|>assistant
/// ```
fn format_chatml(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        system, user
    )
}

/// Gemma 形式（Gemma 2/3）
///
/// Gemma は system ロールを持たないため、system メッセージを user ターンの先頭に埋め込む
///
/// ```text
/// <start_of_turn>user
/// {system}
///
/// {user}<end_of_turn>
/// <start_of_turn>model
/// ```
fn format_gemma(system: &str, user: &str) -> String {
    format!(
        "<start_of_turn>user\n{}\n\n{}<end_of_turn>\n<start_of_turn>model\n",
        system, user
    )
}

/// Llama 3 形式（Llama 3/3.1）
///
/// ```text
/// <|start_header_id|>system<|end_header_id|>
///
/// {system}<|eot_id|><|start_header_id|>user<|end_header_id|>
///
/// {user}<|eot_id|><|start_header_id|>assistant<|end_header_id|>
///
/// ```
fn format_llama3(system: &str, user: &str) -> String {
    format!(
        "<|start_header_id|>system<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
        system, user
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYSTEM: &str = "あなたはアシスタントです。";
    const USER: &str = "質問です。";

    // ============================================================
    // ChatML
    // ============================================================

    #[test]
    fn test_chatml_contains_markers() {
        let prompt = ChatTemplate::Chatml.format_prompt(SYSTEM, USER);
        assert!(prompt.contains("<|im_start|>system"));
        assert!(prompt.contains("<|im_end|>"));
        assert!(prompt.contains("<|im_start|>user"));
        assert!(prompt.contains("<|im_start|>assistant"));
    }

    #[test]
    fn test_chatml_contains_messages() {
        let prompt = ChatTemplate::Chatml.format_prompt(SYSTEM, USER);
        assert!(prompt.contains(SYSTEM));
        assert!(prompt.contains(USER));
    }

    #[test]
    fn test_chatml_ends_with_assistant() {
        let prompt = ChatTemplate::Chatml.format_prompt(SYSTEM, USER);
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }

    // ============================================================
    // Gemma
    // ============================================================

    #[test]
    fn test_gemma_contains_markers() {
        let prompt = ChatTemplate::Gemma.format_prompt(SYSTEM, USER);
        assert!(prompt.contains("<start_of_turn>user"));
        assert!(prompt.contains("<end_of_turn>"));
        assert!(prompt.contains("<start_of_turn>model"));
    }

    #[test]
    fn test_gemma_system_in_user_turn() {
        // Gemma は system ロールがないため user ターンに埋め込まれる
        let prompt = ChatTemplate::Gemma.format_prompt(SYSTEM, USER);
        let user_start = prompt.find("<start_of_turn>user").unwrap();
        let system_pos = prompt.find(SYSTEM).unwrap();
        assert!(system_pos > user_start, "system は user ターン内に埋め込まれるべき");
    }

    #[test]
    fn test_gemma_no_system_marker() {
        let prompt = ChatTemplate::Gemma.format_prompt(SYSTEM, USER);
        assert!(!prompt.contains("<start_of_turn>system"));
    }

    #[test]
    fn test_gemma_ends_with_model() {
        let prompt = ChatTemplate::Gemma.format_prompt(SYSTEM, USER);
        assert!(prompt.ends_with("<start_of_turn>model\n"));
    }

    // ============================================================
    // Llama3
    // ============================================================

    #[test]
    fn test_llama3_contains_markers() {
        let prompt = ChatTemplate::Llama3.format_prompt(SYSTEM, USER);
        assert!(prompt.contains("<|start_header_id|>system<|end_header_id|>"));
        assert!(prompt.contains("<|eot_id|>"));
        assert!(prompt.contains("<|start_header_id|>user<|end_header_id|>"));
        assert!(prompt.contains("<|start_header_id|>assistant<|end_header_id|>"));
    }

    #[test]
    fn test_llama3_contains_messages() {
        let prompt = ChatTemplate::Llama3.format_prompt(SYSTEM, USER);
        assert!(prompt.contains(SYSTEM));
        assert!(prompt.contains(USER));
    }

    #[test]
    fn test_llama3_ends_with_assistant() {
        let prompt = ChatTemplate::Llama3.format_prompt(SYSTEM, USER);
        assert!(prompt.ends_with("<|start_header_id|>assistant<|end_header_id|>\n\n"));
    }

    // ============================================================
    // 共通
    // ============================================================

    #[test]
    fn test_empty_system_message() {
        // system が空でもパニックしない
        for template in [ChatTemplate::Chatml, ChatTemplate::Gemma, ChatTemplate::Llama3] {
            let prompt = template.format_prompt("", USER);
            assert!(prompt.contains(USER));
        }
    }

    #[test]
    fn test_empty_user_message() {
        for template in [ChatTemplate::Chatml, ChatTemplate::Gemma, ChatTemplate::Llama3] {
            let prompt = template.format_prompt(SYSTEM, "");
            assert!(prompt.contains(SYSTEM));
        }
    }

    #[test]
    fn test_multiline_messages() {
        let system = "行1\n行2\n行3";
        let user = "質問行1\n質問行2";
        for template in [ChatTemplate::Chatml, ChatTemplate::Gemma, ChatTemplate::Llama3] {
            let prompt = template.format_prompt(system, user);
            assert!(prompt.contains(system));
            assert!(prompt.contains(user));
        }
    }

    // ============================================================
    // Serde
    // ============================================================

    #[test]
    fn test_serde_roundtrip() {
        for template in [ChatTemplate::Chatml, ChatTemplate::Gemma, ChatTemplate::Llama3] {
            let json = serde_json::to_string(&template).unwrap();
            let deserialized: ChatTemplate = serde_json::from_str(&json).unwrap();
            assert_eq!(template, deserialized);
        }
    }

    #[test]
    fn test_serde_lowercase() {
        assert_eq!(
            serde_json::to_string(&ChatTemplate::Chatml).unwrap(),
            "\"chatml\""
        );
        assert_eq!(
            serde_json::to_string(&ChatTemplate::Gemma).unwrap(),
            "\"gemma\""
        );
        assert_eq!(
            serde_json::to_string(&ChatTemplate::Llama3).unwrap(),
            "\"llama3\""
        );
    }

    #[test]
    fn test_serde_alias_deserialization() {
        // 大文字始まりのエイリアスでもデシリアライズ可能
        let chatml: ChatTemplate = serde_json::from_str("\"ChatML\"").unwrap();
        assert_eq!(chatml, ChatTemplate::Chatml);
        let gemma: ChatTemplate = serde_json::from_str("\"Gemma\"").unwrap();
        assert_eq!(gemma, ChatTemplate::Gemma);
    }
}
