# インデックス設計書

全文検索・ベクトル検索のインデックス管理・検索パイプライン・検証・修復に関する設計。

## 1. 検索パイプライン

### 1.1 全文検索パイプライン

```
ユーザーのクエリ文字列
  ↓
lindera トークナイザ（IPAdic辞書による日本語形態素解析）
  ↓
tantivy クエリパーサー（title + body フィールド）
  ↓
tantivy インデックス検索
  ↓
BM25 スコアによるランキング + スニペット生成
  ↓
SearchResult { path, title, snippet, score }
```

**実装**: `infra/tantivy/mod.rs` の `TantivySearchEngine`
- `FulltextSearcher` トレイト（`domain/search/mod.rs`）を実装
- `Indexer` トレイト（`domain/indexer/mod.rs`）を実装

### 1.2 ベクトル検索パイプライン

```
ドキュメント登録時:
  ファイル本文
    → チャンク分割（500文字、100文字オーバーラップ）
    → E5プレフィックス付与（"passage: " + テキスト）
    → ONNX Runtime推論（multilingual-e5-small, 384次元）
    → HNSWインデックスに挿入

検索時:
  クエリ文字列
    → E5プレフィックス付与（"query: " + テキスト）
    → ONNX Runtime推論 → クエリembedding
    → HNSW近似最近傍検索（コサイン距離）
    → VectorSearchResult { chunk_id, source_path, distance, text }
```

**実装**:
- チャンク分割: `domain/indexer/chunker.rs` の `split_into_chunks()`
- embedding生成: `infra/onnx/mod.rs` の `OnnxEmbeddingGenerator`（`EmbeddingGenerator` トレイト実装）
- ベクトル検索: `infra/hnsw/mod.rs` の `HnswVectorIndex`（`VectorSearcher` トレイト実装）
- HNSWパラメータ: max_nb_connection=16, ef_construction=200, ef_search=200, nb_elem=50,000

### 1.3 ハイブリッド検索パイプライン

```
ユーザーのクエリ文字列
  ↓
┌─────────────────────┬──────────────────────────┐
│ 全文検索              │ ベクトル検索               │
│ TantivySearchEngine  │ OnnxEmbeddingGenerator    │
│ → BM25ランキング      │ + HnswVectorIndex         │
│ → paths_A            │ → paths_B                 │
└──────────┬──────────┴────────────┬─────────────┘
           ↓                       ↓
     reciprocal_rank_fusion(paths_A, paths_B, k=60)
           ↓
     RRF_score = Σ 1/(k + rank_i)
           ↓
     HybridSearchResult { path, title, snippet, score, source }
       source: "fulltext" | "vector" | "hybrid"
```

**実装**: `domain/search/hybrid.rs` の `reciprocal_rank_fusion()`

ハイブリッド検索はデフォルトの検索モード。ベクトルインデックスが利用不可の場合は全文検索に自動フォールバックする。

## 2. ディレクトリ構造

すべてのインデックスデータは `{appDataDir}` 配下に保存する。

```
{appDataDir}/
└── index/
    ├── fulltext/
    │   ├── {hash_A}/          ← フォルダAの全文検索インデックス（tantivy形式）
    │   └── {hash_B}/          ← フォルダBの全文検索インデックス
    └── vector/
        ├── {hash_A}/          ← フォルダAのベクトルキャッシュ
        │   ├── manifest.json  ← メタデータ（バージョン、フィンガープリント等）
        │   └── embeddings.bin ← embedding + チャンクメタ（bincode形式）
        └── {hash_B}/          ← フォルダBのベクトルキャッシュ
```

### appDataDir のプラットフォーム別パス

identifier: `com.foldersearch.desktop`

| OS | パス |
|----|------|
| Linux | `~/.local/share/com.foldersearch.desktop/` |
| Windows | `%APPDATA%/com.foldersearch.desktop/` |
| macOS | `~/Library/Application Support/com.foldersearch.desktop/` |

### フォルダハッシュ

フォルダパスからSHA256ハッシュの先頭16文字を取得する。全文検索・ベクトルキャッシュ共通。

```rust
// infra/vector_cache/mod.rs
pub fn folder_hash(folder_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(folder_path.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    hash[..16].to_string()
}
```

## 3. 全文検索インデックス

### 概要

- **エンジン**: tantivy 0.25 + lindera 2（IPAdic辞書）
- **保存場所**: `index/fulltext/{hash}/`
- **形式**: tantivy MmapDirectory
- **保持**: フォルダごとに独立保持。フォルダ切替時にフルビルド不要

### スキーマ

| フィールド | 型 | 用途 |
|-----------|-----|------|
| path | STRING \| STORED | ファイルパス（検索対象外） |
| title | TEXT \| STORED | ファイル名（日本語トークナイズ） |
| body | TEXT \| STORED | ファイル本文（日本語トークナイズ） |

### 構築フロー

1. `build_index` コマンド呼び出し（フロントエンドから `folder_path` と `total_files` を渡す）
2. Rust側で `app_data_dir + "/index/fulltext/" + folder_hash(folder_path)` を算出
3. `TantivySearchEngine::new()` でインデックスを開く（既存あれば open、なければ create）
4. `delete_all_documents()` で既存データをクリア
5. フォルダ走査（`.txt`, `.md` のみ）→ ドキュメント追加
6. `commit()` で永続化
7. キャンセル対応: `AtomicBool` トークンで100分の1ごとにチェック

### 差分更新

ファイル監視（notify クレート、2秒デバウンス）により変更を検出し、`update_files()` で対象ファイルのみ削除→再追加する。

## 4. ベクトルキャッシュ

### 概要

- **embeddingモデル**: intfloat/multilingual-e5-small（384次元、ONNX形式）
- **保存場所**: `index/vector/{hash}/`
- **形式**: `manifest.json`（メタデータ）+ `embeddings.bin`（bincode）
- **保持**: フォルダごとに独立保持

### manifest.json

```rust
pub struct CacheManifest {
    pub format_version: u32,                            // 現在: 1
    pub folder_path: String,                            // 対象フォルダパス
    pub file_fingerprints: HashMap<String, FileFingerprint>,  // ファイルメタ
    pub chunk_count: usize,                             // チャンク総数
    pub embedding_dimension: usize,                     // 384
}
```

### embeddings.bin

```rust
pub struct CachedEmbeddings {
    pub metas: Vec<ChunkMeta>,       // チャンクメタ（ID、パス、テキスト）
    pub embeddings: Vec<Embedding>,  // 384次元 f32 ベクトル
}
```

### 構築フロー（3経路）

1. **キャッシュヒット**: `is_cache_valid()` が true → キャッシュから `load()` → HNSW再構築のみ
2. **差分更新**: `compute_diff()` で変更検出 → 未変更分を再利用、変更分のみ再生成
3. **フルビルド**: キャッシュなし → 全ファイルをチャンク分割 → embedding生成 → HNSW構築 → キャッシュ保存

### 中断時の途中保存

キャンセル検知時に `save_with_fingerprints()` で処理済みファイル分のみ保存する。次回の `compute_diff()` が未処理ファイルを `added` として検出し、差分更新パスで再開する。

## 5. ファイル監視による自動更新

### 概要

フォルダ選択後、`notify` クレートによりフォルダ内の変更をリアルタイム監視する。変更検知時に全文検索・ベクトルの両インデックスを差分更新する。

### 仕組み

- **ライブラリ**: `notify` 7 + `notify-debouncer-mini`
- **監視対象**: 選択フォルダ全体（再帰）
- **対象ファイル**: `.txt`, `.md` のみ
- **デバウンス**: 2秒（短時間の連続変更をまとめる）

### 更新フロー

```
ファイル変更検知（notify）
  ↓ 2秒デバウンス
  ↓
変更ファイルパスのリスト収集
  ↓
├── 全文検索: engine.update_files()（対象ファイルの削除→再追加）
└── ベクトル: build_vector_index_incremental_inner()（差分更新）
  ↓
フロントエンドへ "index-updated" イベント通知
  { fulltext_count, vector_chunk_count }
```

### 差分更新方式

- **全文検索**: tantivy の `path` フィールドが `STRING` 型のため、`Term::from_field_text` で個別文書の削除が可能。変更ファイルは削除→再追加で更新する
- **ベクトル検索**: `VectorCache::compute_diff` でファイル変更を検出し、変更ファイルのみembeddingを再生成する。HNSWインデックスは全体再構築（hnsw_rsに削除APIがないため）

### ライフサイクル

- フォルダ選択時に `FileWatcher::start()` で監視開始
- 別フォルダ選択時に既存ウォッチャーを停止（`*watcher_guard = None`）してから新しい監視を開始
- `FileWatcher` は `AppState.watcher` に保持される
- ウォッチャーのコールバックは別スレッドで実行される。AppStateのMutexアクセスでデッドロックに注意する
- デバウンス中にアプリが終了した場合の未処理イベントは無視してよい

## 6. インデックス破損検査・自動修復

### 6.1 検証関数

#### 全文検索インデックス

```rust
// infra/tantivy/mod.rs
pub fn validate_index(index_path: &Path) -> bool
```

- ディレクトリが存在しない → true（未作成は正常）
- `MmapDirectory::open()` → `Index::open()` → `reader()` を試行
- いずれかが失敗 → false（破損）

#### ベクトルキャッシュ

```rust
// infra/vector_cache/mod.rs
pub fn validate_cache_dir(cache_dir: &Path) -> bool
```

- `manifest.json` 存在 + パース + `format_version` チェック
- `embeddings.bin` 存在 + bincode デシリアライズ試行
- いずれかが失敗 → false（破損）

### 6.2 起動時バックグラウンド検証

setup hookで `std::thread::spawn` により低優先度スレッドを起動する。

```
1. index/fulltext/ 配下のサブディレクトリを列挙
   → 各ディレクトリを validate_index() で検証
   → 破損 → ディレクトリ削除
   → 100ms sleep

2. index/vector/ 配下のサブディレクトリを列挙
   → reserved（フォルダ選択で予約済み）ならスキップ
   → current_hash をセットして検証
   → 破損 → ディレクトリ削除
   → current_hash クリア + completed に追加 + notify
   → 100ms sleep
```

100ms の sleep はUIスレッドの動きを止めないための優先度低下措置。

### 6.3 フォルダ選択時の同期検証

`validate_folder_indexes` コマンドがフォルダ選択処理の先頭で呼ばれる。

```
1. folder_hash を計算
2. reserved に追加（BGがまだ到達していなければスキップさせる）
3. completed に含まれる → BG検証済み、スキップ
4. current_hash == 自分のハッシュ → BGが検証中、Condvar で完了待ち
5. それ以外 → 自分で同期検証
   → 全文検索インデックス: validate_index()
   → ベクトルキャッシュ: validate_cache_dir()
   → 破損 → ディレクトリ削除
```

### 6.4 IndexValidation 共有状態

```rust
pub struct IndexValidation {
    pub(crate) current_hash: Mutex<Option<String>>,   // BG検証中のハッシュ
    pub(crate) reserved: Mutex<HashSet<String>>,       // フォルダ選択で予約済み
    pub(crate) completed: Mutex<HashSet<String>>,      // BG検証完了済み
    pub(crate) notify: Condvar,                        // BG検証完了通知
}
```

BGスレッドとフォルダ選択コマンドの競合を制御する。AppState に `Arc<IndexValidation>` として保持される。
