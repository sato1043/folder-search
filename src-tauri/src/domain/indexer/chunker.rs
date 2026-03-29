/// テキストチャンクを表す
#[derive(Debug, Clone)]
pub struct Chunk {
    /// 元ファイルのパス
    pub source_path: String,
    /// チャンクのインデックス（0始まり）
    pub chunk_index: usize,
    /// チャンクのテキスト
    pub text: String,
}

/// テキストをチャンクに分割する
///
/// - `chunk_size`: 1チャンクの最大文字数
/// - `overlap`: 前後のチャンクとのオーバーラップ文字数
pub fn split_into_chunks(
    source_path: &str,
    text: &str,
    chunk_size: usize,
    overlap: usize,
) -> Vec<Chunk> {
    if text.is_empty() || chunk_size == 0 {
        return Vec::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();

    if total <= chunk_size {
        return vec![Chunk {
            source_path: source_path.to_string(),
            chunk_index: 0,
            text: text.to_string(),
        }];
    }

    let step = chunk_size.saturating_sub(overlap).max(1);
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < total {
        let end = (start + chunk_size).min(total);
        let chunk_text: String = chars[start..end].iter().collect();

        chunks.push(Chunk {
            source_path: source_path.to_string(),
            chunk_index: chunks.len(),
            text: chunk_text,
        });

        start += step;
        if end == total {
            break;
        }
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_text_returns_empty() {
        let chunks = split_into_chunks("/test.md", "", 100, 20);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_short_text_returns_single_chunk() {
        let text = "短いテキスト";
        let chunks = split_into_chunks("/test.md", text, 100, 20);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, text);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[0].source_path, "/test.md");
    }

    #[test]
    fn test_long_text_splits_with_overlap() {
        // 30文字のテキスト、チャンクサイズ10、オーバーラップ3
        let text = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほ";
        let chunks = split_into_chunks("/test.md", text, 10, 3);

        assert!(chunks.len() > 1, "複数のチャンクに分割される");

        // 各チャンクが10文字以下
        for chunk in &chunks {
            assert!(chunk.text.chars().count() <= 10);
        }

        // チャンクインデックスが連番
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.chunk_index, i);
        }

        // オーバーラップがある: 隣接チャンクの末尾と先頭が重なる
        if chunks.len() >= 2 {
            let c0_chars: Vec<char> = chunks[0].text.chars().collect();
            let c1_chars: Vec<char> = chunks[1].text.chars().collect();
            let overlap_part: String = c0_chars[c0_chars.len() - 3..].iter().collect();
            let c1_start: String = c1_chars[..3].iter().collect();
            assert_eq!(overlap_part, c1_start, "オーバーラップ部分が一致する");
        }
    }

    #[test]
    fn test_chunk_covers_entire_text() {
        let text = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let chunks = split_into_chunks("/test.md", text, 10, 3);

        // 最後のチャンクがテキスト末尾を含む
        let last = chunks.last().unwrap();
        assert!(
            text.ends_with(&last.text[last.text.len() - 1..]),
            "末尾が含まれる"
        );
    }

    #[test]
    fn test_source_path_preserved() {
        let chunks = split_into_chunks("/docs/重要.md", "テスト", 100, 0);
        assert_eq!(chunks[0].source_path, "/docs/重要.md");
    }
}
