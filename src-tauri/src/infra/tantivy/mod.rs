use std::path::Path;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera_tantivy::tokenizer::LinderaTokenizer;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::snippet::SnippetGenerator;
use tantivy::{Index, IndexWriter, ReloadPolicy, TantivyDocument};
use walkdir::WalkDir;

use crate::domain::indexer::{Document, IndexError, IndexStatus, Indexer};
use crate::domain::search::{FulltextSearcher, SearchError, SearchResult};

const TOKENIZER_NAME: &str = "lang_ja";

/// tantivy + lindera による全文検索エンジン
pub struct TantivySearchEngine {
    index: Index,
    schema: Schema,
    path_field: Field,
    title_field: Field,
    body_field: Field,
    index_path: String,
}

impl TantivySearchEngine {
    /// 新しい検索エンジンを作成する（ディスク上にインデックスを作成）
    pub fn new(index_path: &str) -> Result<Self, IndexError> {
        let schema = Self::build_schema();
        let path_field = schema.get_field("path").unwrap();
        let title_field = schema.get_field("title").unwrap();
        let body_field = schema.get_field("body").unwrap();

        let dir = Path::new(index_path);
        std::fs::create_dir_all(dir)?;

        let directory = tantivy::directory::MmapDirectory::open(dir)
            .map_err(|e| IndexError::CreateError(e.to_string()))?;

        let index =
            if Index::exists(&directory).map_err(|e| IndexError::CreateError(e.to_string()))? {
                Index::open(directory).map_err(|e| IndexError::CreateError(e.to_string()))?
            } else {
                Index::create(directory, schema.clone(), tantivy::IndexSettings::default())
                    .map_err(|e| IndexError::CreateError(e.to_string()))?
            };

        Self::register_tokenizer(&index)?;

        Ok(Self {
            index,
            schema,
            path_field,
            title_field,
            body_field,
            index_path: index_path.to_string(),
        })
    }

    /// RAMインデックスで作成する（テスト用）
    pub fn new_in_ram() -> Result<Self, IndexError> {
        let schema = Self::build_schema();
        let path_field = schema.get_field("path").unwrap();
        let title_field = schema.get_field("title").unwrap();
        let body_field = schema.get_field("body").unwrap();

        let index = Index::create_in_ram(schema.clone());
        Self::register_tokenizer(&index)?;

        Ok(Self {
            index,
            schema,
            path_field,
            title_field,
            body_field,
            index_path: String::new(),
        })
    }

    /// フォルダを走査してインデックスを構築する
    pub fn index_folder(&mut self, folder_path: &str) -> Result<u64, IndexError> {
        let mut writer: IndexWriter<TantivyDocument> = self
            .index
            .writer(50_000_000)
            .map_err(|e| IndexError::CreateError(e.to_string()))?;

        // 既存のインデックスをクリア
        writer
            .delete_all_documents()
            .map_err(|e| IndexError::CommitError(e.to_string()))?;

        let mut count = 0u64;
        for entry in WalkDir::new(folder_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "txt" && ext != "md" {
                continue;
            }

            let body = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let title = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let doc = Document {
                path: path.to_string_lossy().to_string(),
                title,
                body,
            };

            Self::add_doc_to_writer(&mut writer, &self.schema, &doc)?;
            count += 1;
        }

        writer
            .commit()
            .map_err(|e| IndexError::CommitError(e.to_string()))?;

        Ok(count)
    }

    fn build_schema() -> Schema {
        let mut schema_builder = Schema::builder();

        // pathフィールド: 保存のみ（検索対象外）
        schema_builder.add_text_field("path", STRING | STORED);

        // titleフィールド: 日本語トークナイズ + 保存
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer(TOKENIZER_NAME)
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();
        schema_builder.add_text_field("title", text_options.clone());

        // bodyフィールド: 日本語トークナイズ + 保存
        schema_builder.add_text_field("body", text_options);

        schema_builder.build()
    }

    fn register_tokenizer(index: &Index) -> Result<(), IndexError> {
        let dictionary = load_dictionary("embedded://ipadic")
            .map_err(|e| IndexError::CreateError(format!("辞書の読み込みに失敗: {}", e)))?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        let tokenizer = LinderaTokenizer::from_segmenter(segmenter);
        index.tokenizers().register(TOKENIZER_NAME, tokenizer);
        Ok(())
    }

    fn add_doc_to_writer(
        writer: &mut IndexWriter<TantivyDocument>,
        schema: &Schema,
        doc: &Document,
    ) -> Result<(), IndexError> {
        let path_field = schema.get_field("path").unwrap();
        let title_field = schema.get_field("title").unwrap();
        let body_field = schema.get_field("body").unwrap();

        let mut tantivy_doc = TantivyDocument::new();
        tantivy_doc.add_text(path_field, &doc.path);
        tantivy_doc.add_text(title_field, &doc.title);
        tantivy_doc.add_text(body_field, &doc.body);

        writer
            .add_document(tantivy_doc)
            .map_err(|e| IndexError::AddError(e.to_string()))?;

        Ok(())
    }
}

impl TantivySearchEngine {
    /// 指定パスのドキュメントを削除する
    pub fn delete_by_path(&mut self, path: &str) -> Result<(), IndexError> {
        let mut writer: IndexWriter<TantivyDocument> = self
            .index
            .writer(50_000_000)
            .map_err(|e| IndexError::CreateError(e.to_string()))?;

        let term = tantivy::Term::from_field_text(self.path_field, path);
        writer.delete_term(term);
        writer
            .commit()
            .map_err(|e| IndexError::CommitError(e.to_string()))?;

        Ok(())
    }

    /// 変更されたファイルのインデックスを更新する（削除→再追加）
    pub fn update_files(&mut self, files: &[String]) -> Result<u64, IndexError> {
        let mut writer: IndexWriter<TantivyDocument> = self
            .index
            .writer(50_000_000)
            .map_err(|e| IndexError::CreateError(e.to_string()))?;

        // 対象ファイルの既存ドキュメントを削除
        for file_path in files {
            let term = tantivy::Term::from_field_text(self.path_field, file_path);
            writer.delete_term(term);
        }

        // 存在するファイルを再追加
        let mut count = 0u64;
        for file_path in files {
            let path = std::path::Path::new(file_path);
            if !path.is_file() {
                continue;
            }
            let body = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let title = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let doc = Document {
                path: file_path.clone(),
                title,
                body,
            };
            Self::add_doc_to_writer(&mut writer, &self.schema, &doc)?;
            count += 1;
        }

        writer
            .commit()
            .map_err(|e| IndexError::CommitError(e.to_string()))?;

        Ok(count)
    }
}

impl Indexer for TantivySearchEngine {
    fn add_document(&mut self, doc: &Document) -> Result<(), IndexError> {
        let mut writer: IndexWriter<TantivyDocument> = self
            .index
            .writer(50_000_000)
            .map_err(|e| IndexError::CreateError(e.to_string()))?;
        Self::add_doc_to_writer(&mut writer, &self.schema, doc)?;
        writer
            .commit()
            .map_err(|e| IndexError::CommitError(e.to_string()))?;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), IndexError> {
        let mut writer: IndexWriter<TantivyDocument> = self
            .index
            .writer(50_000_000)
            .map_err(|e| IndexError::CreateError(e.to_string()))?;
        writer
            .commit()
            .map_err(|e| IndexError::CommitError(e.to_string()))?;
        Ok(())
    }

    fn status(&self) -> IndexStatus {
        let reader = self.index.reader().ok();
        let file_count = reader
            .as_ref()
            .map(|r| {
                let searcher = r.searcher();
                searcher.num_docs()
            })
            .unwrap_or(0);

        IndexStatus {
            file_count,
            index_path: self.index_path.clone(),
            is_ready: reader.is_some(),
        }
    }
}

impl FulltextSearcher for TantivySearchEngine {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, SearchError> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e: tantivy::TantivyError| SearchError::InternalError(e.to_string()))?;

        let searcher = reader.searcher();

        let query_parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.body_field]);
        let parsed_query = query_parser
            .parse_query(query)
            .map_err(|e| SearchError::QueryParseError(e.to_string()))?;

        let top_docs = searcher
            .search(&parsed_query, &TopDocs::with_limit(limit))
            .map_err(|e| SearchError::InternalError(e.to_string()))?;

        let snippet_generator = SnippetGenerator::create(&searcher, &parsed_query, self.body_field)
            .map_err(|e: tantivy::TantivyError| SearchError::InternalError(e.to_string()))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| SearchError::InternalError(e.to_string()))?;

            let path = retrieved_doc
                .get_first(self.path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title = retrieved_doc
                .get_first(self.title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let snippet = snippet_generator.snippet_from_doc(&retrieved_doc);
            let snippet_text = snippet.to_html();

            results.push(SearchResult {
                path,
                title,
                snippet: snippet_text,
                score,
            });
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::indexer::{Document, Indexer};
    use crate::domain::search::FulltextSearcher;

    fn create_test_engine() -> TantivySearchEngine {
        let mut engine = TantivySearchEngine::new_in_ram().unwrap();

        let docs = vec![
            Document {
                path: "/test/rust入門.md".to_string(),
                title: "rust入門.md".to_string(),
                body: "Rustは安全性とパフォーマンスを両立するプログラミング言語です。所有権システムによりメモリ安全性を保証します。".to_string(),
            },
            Document {
                path: "/test/tauri開発.txt".to_string(),
                title: "tauri開発.txt".to_string(),
                body: "TauriはRustで書かれたデスクトップアプリケーションフレームワークです。フロントエンドはWeb技術で構築できます。".to_string(),
            },
            Document {
                path: "/test/検索エンジン.md".to_string(),
                title: "検索エンジン.md".to_string(),
                body: "tantivyはRust製の全文検索エンジンです。日本語の検索にはlinderaトークナイザを使用します。".to_string(),
            },
        ];

        for doc in &docs {
            engine.add_document(doc).unwrap();
        }

        engine
    }

    #[test]
    fn test_search_japanese_keyword() {
        let engine = create_test_engine();
        let results = engine.search("Rust", 10).unwrap();
        assert!(!results.is_empty(), "Rustで検索して結果が返る");
        assert!(results.len() >= 2, "Rustを含む文書が2件以上見つかる");
    }

    #[test]
    fn test_search_returns_correct_fields() {
        let engine = create_test_engine();
        let results = engine.search("tantivy", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "検索エンジン.md");
        assert!(results[0].path.contains("検索エンジン"));
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_no_results() {
        let engine = create_test_engine();
        let results = engine.search("存在しないキーワード12345", 10).unwrap();
        assert!(results.is_empty(), "存在しないキーワードでは結果が空");
    }

    #[test]
    fn test_search_japanese_morpheme() {
        let engine = create_test_engine();
        // 「安全性」で検索 → Rust入門の文書がヒットする
        let results = engine.search("安全性", 10).unwrap();
        assert!(!results.is_empty(), "日本語の形態素で検索できる");
        assert!(results[0].path.contains("rust入門"));
    }

    #[test]
    fn test_index_status() {
        let engine = create_test_engine();
        let status = engine.status();
        assert_eq!(
            status.file_count, 3,
            "3件のドキュメントがインデックスされている"
        );
        assert!(status.is_ready);
    }

    #[test]
    fn test_search_limit() {
        let engine = create_test_engine();
        let results = engine.search("Rust", 1).unwrap();
        assert_eq!(results.len(), 1, "limitで結果数が制限される");
    }

    #[test]
    fn test_delete_by_path() {
        let mut engine = create_test_engine();
        assert_eq!(engine.status().file_count, 3);

        engine.delete_by_path("/test/tauri開発.txt").unwrap();

        assert_eq!(engine.status().file_count, 2);
        let results = engine.search("Tauri", 10).unwrap();
        assert!(results.is_empty(), "削除したドキュメントは検索に出ない");
    }

    #[test]
    fn test_update_files_add_new() {
        let mut engine = create_test_engine();
        assert_eq!(engine.status().file_count, 3);

        // 一時ファイルを作成して追加
        let temp_dir = tempfile::tempdir().unwrap();
        let new_file = temp_dir.path().join("新規.md");
        std::fs::write(&new_file, "ベクトル検索はセマンティックな類似性を発見する").unwrap();

        let new_path = new_file.to_string_lossy().to_string();
        engine.update_files(&[new_path]).unwrap();

        assert_eq!(engine.status().file_count, 4);
        let results = engine.search("ベクトル検索", 10).unwrap();
        assert_eq!(results.len(), 1, "追加したファイルが検索可能");
    }

    #[test]
    fn test_update_files_modify() {
        let mut engine = create_test_engine();

        // 一時ファイルを作成して初期データをインデックス
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("変更対象.md");
        std::fs::write(&file, "変更前の内容").unwrap();

        let path = file.to_string_lossy().to_string();
        engine.update_files(&[path.clone()]).unwrap();

        let results = engine.search("変更前", 10).unwrap();
        assert_eq!(results.len(), 1);

        // ファイル内容を変更して再更新
        std::fs::write(&file, "変更後の新しい内容").unwrap();
        engine.update_files(&[path]).unwrap();

        let results_old = engine.search("変更前", 10).unwrap();
        assert!(results_old.is_empty(), "変更前の内容は検索に出ない");

        let results_new = engine.search("変更後", 10).unwrap();
        assert_eq!(results_new.len(), 1, "変更後の内容が検索可能");
    }

    #[test]
    fn test_update_files_delete() {
        let mut engine = create_test_engine();
        assert_eq!(engine.status().file_count, 3);

        // 存在しないパスをupdate_filesに渡す → 削除のみ実行
        engine
            .update_files(&["/test/tauri開発.txt".to_string()])
            .unwrap();

        assert_eq!(engine.status().file_count, 2);
        let results = engine.search("Tauri", 10).unwrap();
        assert!(results.is_empty(), "削除されたファイルは検索に出ない");

        // 残りのドキュメントは影響なし
        let results = engine.search("Rust", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_index_folder() {
        let temp_dir = tempfile::tempdir().unwrap();
        let folder_path = temp_dir.path();

        // テスト用ファイルを作成
        std::fs::write(folder_path.join("test1.md"), "Rustプログラミングの基礎").unwrap();
        std::fs::write(
            folder_path.join("test2.txt"),
            "TypeScriptによるフロントエンド開発",
        )
        .unwrap();
        std::fs::write(
            folder_path.join("test3.rs"),
            "fn main() {} // rsファイルは対象外",
        )
        .unwrap();

        let mut engine = TantivySearchEngine::new_in_ram().unwrap();
        let count = engine.index_folder(folder_path.to_str().unwrap()).unwrap();

        assert_eq!(count, 2, ".mdと.txtのみインデックスされる");

        let results = engine.search("Rust", 10).unwrap();
        assert_eq!(results.len(), 1);
    }
}
