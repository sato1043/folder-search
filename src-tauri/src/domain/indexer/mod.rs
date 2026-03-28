use serde::Serialize;

/// インデックスの状態を表す
#[derive(Debug, Clone, Serialize)]
pub struct IndexStatus {
    /// インデックス済みファイル数
    pub file_count: u64,
    /// インデックスのパス
    pub index_path: String,
    /// インデックスの準備状態
    pub is_ready: bool,
}

/// インデックスに追加するドキュメントを表す
#[derive(Debug, Clone)]
pub struct Document {
    /// ファイルパス
    pub path: String,
    /// ファイル名（タイトル）
    pub title: String,
    /// ファイル本文
    pub body: String,
}

/// インデックス構築のトレイト
pub trait Indexer {
    /// ドキュメントをインデックスに追加する
    fn add_document(&mut self, doc: &Document) -> Result<(), IndexError>;
    /// インデックスをコミットする
    fn commit(&mut self) -> Result<(), IndexError>;
    /// インデックスの状態を取得する
    fn status(&self) -> IndexStatus;
}

/// インデックスエラー
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("インデックスの作成に失敗: {0}")]
    CreateError(String),
    #[error("ドキュメントの追加に失敗: {0}")]
    AddError(String),
    #[error("コミットに失敗: {0}")]
    CommitError(String),
    #[error("IOエラー: {0}")]
    IoError(#[from] std::io::Error),
}
