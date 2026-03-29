# アーキテクチャ設計書

## 1. 全体アーキテクチャ

Tauri v2のアーキテクチャに従い、フロントエンド（WebView）とネイティブレイヤー（Rust）の2層構成をとる。

```
┌──────────────────────────────────────────────────────┐
│  フロントエンド（WebView）                              │
│  React 19 + TypeScript 5.8 + Vite 6                   │
│  ├── components/     UIコンポーネント                   │
│  │   ├── layout/     Sidebar, MainPanel               │
│  │   ├── search/     SearchBar, ResultList, Preview    │
│  │   ├── chat/       ChatMessage                      │
│  │   └── settings/   SettingsDialog, GeneralSection,  │
│  │                    ModelManagementSection           │
│  ├── lib/tauri.ts    Tauri IPCラッパー（24関数）        │
│  ├── types/index.ts  共通型定義                        │
│  └── test/           テストセットアップ・モック          │
├──────────────────────────────────────────────────────┤
│  Tauri IPC ブリッジ（invoke / listen / emit）           │
├──────────────────────────────────────────────────────┤
│  ネイティブレイヤー（Rust）                              │
│  ├── commands/mod.rs  Tauri コマンド（24コマンド）       │
│  ├── domain/          ドメインロジック（トレイト・型）    │
│  │   ├── config/      アプリケーション設定型              │
│  │   ├── indexer/     インデックス構築・チャンク分割      │
│  │   ├── search/      全文検索・ハイブリッド検索(RRF)    │
│  │   ├── embedding/   ベクトル生成・検索のトレイト       │
│  │   └── llm/         LLM推論トレイト・RAGパイプライン  │
│  └── infra/           インフラ実装                      │
│      ├── config/      設定永続化（JSON）                 │
│      ├── tantivy/     全文検索エンジン                   │
│      ├── onnx/        ONNX Runtime embedding生成       │
│      ├── hnsw/        HNSWベクトルインデックス           │
│      ├── llama/       llama.cpp LLM推論                │
│      ├── model/       モデルダウンロード・管理           │
│      └── model_registry/ モデルレジストリ（JSON永続化） │
└──────────────────────────────────────────────────────┘
```

### 設計原則

- **クリーンアーキテクチャ**: domain層は外部ライブラリに依存しない。infra層がdomain層のトレイトを実装する
- **依存性の方向**: commands → domain ← infra（domain層が中心）
- **IPC境界**: フロントエンドとRust間のデータ型は `serde` でシリアライズする
- **TDD**: テストコードを先に書き、失敗を確認してから実装する

## 2. 検索パイプライン

### 2.1 全文検索パイプライン

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
- tantivy スキーマ: `path`（STRING|STORED）, `title`（TEXT|STORED）, `body`（TEXT|STORED）
- トークナイザ名: `lang_ja`（lindera + IPAdic）

### 2.2 ベクトル検索パイプライン

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

### 2.3 ハイブリッド検索パイプライン

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

### 2.4 RAGパイプライン

```
ユーザーの自然言語による質問
  ↓
ハイブリッド検索（上記2.3）→ 上位5件のファイル
  ↓
コンテキスト構築（各ファイルの先頭1000文字）
  ↓
プロンプト構築（ChatTemplate に応じた形式）:
  system メッセージ + コンテキスト + 質問 を組み立て
    → ChatTemplate::format_prompt() でモデル固有の形式に整形
  ↓
llama.cpp推論（ストリーミング、max 512トークン、context_length はモデル設定に従う）
  → Tauri event "chat-token" でトークン単位送信
  ↓
回答テキスト + 参照元ファイルパス抽出
  ↓
RagAnswer { answer, sources }
```

**実装**:
- チャットテンプレート: `domain/llm/chat_template.rs` の `ChatTemplate`（ChatML / Gemma / Llama3）
- プロンプト構築: `domain/llm/rag.rs` の `build_rag_prompt()`（テンプレートを受け取り整形を委譲）
- 参照元抽出: `domain/llm/rag.rs` の `extract_sources()`
- LLM推論: `infra/llama/mod.rs` の `LlamaEngine`（`LlmInference` トレイト実装、context_length 可変）

## 3. コンポーネント設計（実装済み）

### 3.1 フロントエンドコンポーネント

```
src/
├── App.tsx                         ← ルートコンポーネント
│                                     検索モード/チャットモード切り替え
│                                     全状態管理（useState）
├── components/
│   ├── layout/
│   │   ├── Sidebar.tsx             ← サイドバー（children受け取り）
│   │   ├── Sidebar.test.tsx
│   │   ├── MainPanel.tsx           ← メインパネル（children受け取り）
│   │   └── MainPanel.test.tsx
│   ├── search/
│   │   ├── SearchBar.tsx           ← 検索/質問入力（Enter実行、placeholder可変）
│   │   ├── SearchBar.test.tsx
│   │   ├── ResultList.tsx          ← 検索結果一覧（HTMLスニペット、クリック選択）
│   │   ├── ResultList.test.tsx
│   │   ├── Preview.tsx             ← 原文プレビュー（pre表示）
│   │   └── Preview.test.tsx
│   ├── chat/
│   │   ├── ChatMessage.tsx         ← LLM回答表示（ストリーミングカーソル、参照元リンク）
│   │   └── ChatMessage.test.tsx
│   └── settings/
│       ├── SettingsDialog.tsx      ← 設定モーダルダイアログ（セクション切替）
│       ├── SettingsDialog.test.tsx
│       ├── GeneralSection.tsx      ← 一般セクション（LLMモデル選択、キャッシュスライダー）
│       ├── GeneralSection.test.tsx
│       ├── ModelManagementSection.tsx  ← モデル管理セクション（DL/削除/有効無効）
│       └── ModelManagementSection.test.tsx
├── lib/
│   └── tauri.ts                    ← Tauri IPCラッパー（24関数）
├── types/
│   └── index.ts                    ← 共通型定義（10型）
├── test/
│   ├── setup.ts                    ← テストセットアップ
│   └── tauri-mock.ts               ← Tauri APIモック（テスト環境用）
├── main.tsx                        ← エントリポイント
├── index.css                       ← 全スタイル（ダークモード対応）
└── vite-env.d.ts                   ← Vite型定義
```

### 3.2 Rustモジュール構成

```
src-tauri/src/
├── main.rs                         ← エントリポイント
├── lib.rs                          ← Tauriアプリ設定、AppState初期化、コマンド登録
│                                     setupフック: ダウンロード済みembeddingモデルの自動ロード
├── commands/
│   └── mod.rs                      ← 全Tauriコマンド（24コマンド）
│                                     AppState構造体定義
│                                     ベクトルインデックス構築: キャッシュヒット/差分更新/フルビルドの3経路
├── domain/
│   ├── mod.rs
│   ├── config/
│   │   └── mod.rs                  ← AppSettings型、DEFAULT_CACHE_LIMIT_BYTES
│   ├── indexer/
│   │   ├── mod.rs                  ← Document, IndexStatus, Indexerトレイト, IndexError
│   │   └── chunker.rs              ← split_into_chunks()（オーバーラップ付き分割）
│   ├── search/
│   │   ├── mod.rs                  ← SearchResult, FulltextSearcherトレイト, SearchError
│   │   └── hybrid.rs               ← HybridSearchResult, reciprocal_rank_fusion()
│   ├── embedding/
│   │   └── mod.rs                  ← Embedding型, EmbeddingGeneratorトレイト,
│   │                                 VectorSearcherトレイト, VectorSearchResult
│   ├── llm/
│   │   ├── mod.rs                  ← LlmInferenceトレイト, LlmModelInfo, available_models()
│   │   ├── chat_template.rs        ← ChatTemplate enum, format_prompt()
│   │   └── rag.rs                  ← ContextChunk, RagAnswer, build_rag_prompt(),
│   │                                 extract_sources()
│   └── system/
│       └── mod.rs                  ← SystemInfo, GpuInfo, ModelRecommendation,
│                                     RecommendationStatus, recommend_models()
└── infra/
    ├── mod.rs
    ├── config/
    │   └── mod.rs                  ← SettingsStore（設定のJSON永続化）
    │                                 .env.development/.env.production の読み込み（dotenvy）
    ├── tantivy/
    │   └── mod.rs                  ← TantivySearchEngine（FulltextSearcher + Indexer実装）
    │                                 tantivy + lindera(IPAdic) による日本語全文検索
    ├── onnx/
    │   └── mod.rs                  ← OnnxEmbeddingGenerator（EmbeddingGenerator実装）
    │                                 ONNX Runtime + tokenizers による embedding生成
    ├── hnsw/
    │   └── mod.rs                  ← HnswVectorIndex（VectorSearcher実装）
    │                                 hnsw_rs + anndists(DistCosine) によるベクトル検索
    │                                 from_cache(): キャッシュからのHNSW再構築
    ├── vector_cache/
    │   └── mod.rs                  ← VectorCache（ベクトルインデックスのディスクキャッシュ）
    │                                 フォルダごとのembedding+メタデータ永続化
    │                                 CacheDiff: ファイル変更の差分計算
    ├── watcher/
    │   └── mod.rs                  ← FileWatcher（ファイル監視）
    │                                 notify + debouncer によるフォルダ再帰監視
    ├── llama/
    │   └── mod.rs                  ← LlamaEngine（LlmInference実装）
    │                                 llama-cpp-2 による GGUF モデル推論
    ├── system/
    │   └── mod.rs                  ← システム情報検出（RAM, GPU VRAM）
    │                                 プラットフォーム別GPU検出（Windows/macOS/Linux）
    ├── model/
    │   └── mod.rs                  ← モデルDL管理（download_file_with_progress,
    │                                 download_embedding_model, is_model_downloaded）
    └── model_registry/
        └── mod.rs                  ← ModelRegistry（デフォルトプリセット+カスタムモデル管理）
                                      JSON永続化（custom_models.json）
                                      add_model, remove_model, all_models
```

## 4. Tauri IPCコマンド一覧

| コマンド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `build_index` | folder_path, index_path | u64 | 全文検索インデックス構築 |
| `search` | query, limit | Vec\<SearchResult\> | 全文検索実行 |
| `hybrid_search` | query, limit | Vec\<HybridSearchResult\> | ハイブリッド検索実行 |
| `get_index_status` | — | IndexStatus | インデックス状態取得 |
| `read_file_content` | path | String | ファイル内容読み取り |
| `is_embedding_model_ready` | — | bool | embeddingモデルの準備状態 |
| `download_embedding_model` | — | () | embeddingモデルDL（イベント通知付き） |
| `build_vector_index` | — | u64 | ベクトルインデックス構築（イベント通知付き） |
| `list_available_models` | — | Vec\<LlmModelInfo\> | 利用可能LLMモデル一覧（プリセット+カスタム） |
| `download_llm_model` | filename, url, size_bytes | () | LLMモデルDL（サイズチェック+LRUエビクション付き） |
| `load_llm_model` | filename, chat_template, context_length | LlmLoadResult | LLMモデルロード（非同期、spawn_blocking） |
| `is_llm_ready` | — | bool | LLMの準備状態 |
| `get_loaded_model_filename` | — | Option\<String\> | ロード済みLLMモデルのファイル名 |
| `chat` | question | RagAnswer | RAG質問応答（ストリーミング） |
| `detect_system_info` | — | SystemInfo | システムRAM・GPU情報検出 |
| `get_model_recommendations` | — | Vec\<ModelRecommendation\> | システム情報に基づくモデル推奨 |
| `list_downloaded_models` | — | Vec\<DownloadedModelInfo\> | DL済みモデル一覧（ファイル名・サイズ） |
| `delete_model` | filename | () | モデルファイル削除（ロード中は拒否） |
| `get_storage_usage` | — | StorageUsage | モデルストレージ使用状況 |
| `register_custom_model` | model (JSON) | () | カスタムモデル登録 |
| `unregister_custom_model` | filename | () | カスタムモデル登録解除（ファイル保持） |
| `clear_model_cache` | — | Vec\<String\> | ロード中以外の全モデルファイル削除 |
| `get_settings` | — | AppSettings | 現在の設定を取得 |
| `save_settings` | settings (JSON) | () | 設定を保存 |

コマンド数: 24

### Tauri イベント一覧

| イベント名 | ペイロード | 発生元 |
|---|---|---|
| `download-progress` | DownloadProgress | モデルDL中 |
| `vector-index-progress` | {current, total} | ベクトルインデックス構築中 |
| `chat-token` | String（トークン文字列） | LLM推論中 |

## 5. AppState（アプリケーション状態）

```rust
pub struct AppState {
    pub engine: Mutex<Option<TantivySearchEngine>>,         // 全文検索エンジン
    pub vector_index: Mutex<Option<HnswVectorIndex>>,       // ベクトルインデックス
    pub embedding_model: Mutex<Option<OnnxEmbeddingGenerator>>, // embedding生成モデル
    pub llm_engine: Mutex<Option<LlamaEngine>>,             // LLM推論エンジン
    pub model_dir: PathBuf,                                 // モデル保存ディレクトリ
    pub folder_path: Mutex<Option<String>>,                 // 選択中のフォルダパス
    pub loaded_llm_config: Mutex<Option<LoadedLlmConfig>>,  // ロード済みLLM設定
    pub model_registry: ModelRegistry,                      // モデルレジストリ
}
```

## 6. 起動シーケンス

### 6.1 Rust側（setup フック）

```
1. AppState 初期化（全フィールド None / model_dir 設定）
2. setup フック実行
   └─ embeddingモデルがダウンロード済みか確認
      ├─ DL済み → OnnxEmbeddingGenerator を自動ロード
      └─ 未DL → スキップ
3. コマンドハンドラ登録（15コマンド）
4. WebView 起動 → フロントエンド初期化へ
```

LLMモデルの自動ロードは未実装。ユーザーが手動でモデルを選択・ロードする必要がある。

### 6.2 フロントエンド初期化（useEffect）

以下の5つのIPCコマンドが**並列**に発火する（依存関係なし）:

```
isEmbeddingModelReady()      → modelReady 設定
isLlmReady()                 → llmReady 設定
listAvailableModels()        → llmModels 設定
detectSystemInfo()           → systemInfo 設定
getModelRecommendations()    → recommendations 設定 + best_fit を selectedModel に設定
```

### 6.3 ユーザー操作トリガー

```
フォルダ選択
  → build_index（全文検索インデックス構築 + ファイル監視開始）
  → modelReady の場合、自動で build_vector_index

Embeddingモデル DL ボタン（indexCount > 0 かつ !modelReady 時のみ表示）
  → download_embedding_model → ロード → modelReady = true
  → indexCount > 0 の場合、自動で build_vector_index

LLMモデル取得・ロード ボタン
  → download_llm_model → load_llm_model（適応的GPUオフロード）
  → llmReady = true + GPU/CPUステータス表示
```

### 6.4 注意事項

- `listAvailableModels` と `getModelRecommendations` が並列に走るため、`selectedModel` の初期値設定に競合の可能性がある。`getModelRecommendations` が先に完了した場合、`llmModels` がまだ空で `selectedModel` だけ設定される。現状は `<select>` の `value` が `llmModels` とずれる期間が短いため実害はないが、初期化の依存関係を整理するとより堅牢になる
- embeddingモデルは setup フックで自動ロードされるが、LLMモデルは自動ロードされない。これは LLMモデルが複数あり、どれをロードすべきか判断が必要なため

## 7. 技術選定の詳細（確定版）

### 7.1 全文検索: tantivy 0.25 + lindera 2

- tantivy: Rust製全文検索エンジン。BM25スコアリング、スニペット生成
- lindera: IPAdic辞書による日本語形態素解析
- lindera-tantivy: linderaをtantivyトークナイザとして統合するアダプタ
- スキーマ: path(STRING|STORED), title(TEXT|STORED), body(TEXT|STORED)
- 全フィールドで`lang_ja`トークナイザを使用

### 7.2 ベクトル検索

- **embeddingモデル**: `intfloat/multilingual-e5-small`（384次元、ONNX形式、約470MB）
  - 94言語以上対応（日本語含む）
  - E5プレフィックス方式: クエリには`"query: "`、文書には`"passage: "`を付与
- **推論エンジン**: ort 2.0.0-rc.12（ONNX Runtime for Rust）
  - CPU推論（デフォルト）。GPU対応はfeature flagで追加可能
- **トークナイザ**: tokenizers 0.22（HuggingFace公式、tokenizer.jsonを使用）
- **ベクトルインデックス**: hnsw_rs 0.3 + anndists 0.1
  - HNSW（Hierarchical Navigable Small World）アルゴリズム
  - コサイン距離（DistCosine）
  - パラメータ: max_nb_connection=16, ef_construction=200, ef_search=200
- **チャンク分割**: 500文字、100文字オーバーラップ
- **スコア統合**: RRF（Reciprocal Rank Fusion、k=60）

### 7.3 ローカルLLM: llama-cpp-2 0.1.140

- llama.cppのRustバインディング（C++ソースを自動コンパイル）
- GGUF形式モデルをサポート
- ストリーミング推論（トークン単位でコールバック）
- サンプリング: temperature=0.7
- コンテキスト長: モデル設定に従う（プリセットは32768トークン）
- 最大生成トークン数: 512

### 7.4 利用可能なLLMモデル（プリセット）

| モデル | サイズ | 推奨VRAM | 用途 |
|---|---|---|---|
| Gemma 3 1B Instruct Q4_K_M | 714MB | CPU可 | 軽量・高速 |
| Gemma 3 4B Instruct Q4_K_M | 2.9GB | 4GB+ | バランス |
| Gemma 3 12B Instruct Q4_K_M | 7.3GB | 8GB+ | 高品質 |
| Llama 3.2 1B Instruct Q4_K_M | 750MB | CPU可 | 軽量・高速 |
| Llama 3.2 3B Instruct Q4_K_M | 2.0GB | 3GB+ | バランス |
| Llama 3.1 8B Instruct Q4_K_M | 4.9GB | 6GB+ | 高品質 |

プリセット6種（Gemma3×3、Llama3×3）に加え、ユーザーがカスタムモデルを登録可能。HuggingFaceから動的ダウンロード。

## 8. 永続化

### 8.1 データ保存場所

| データ | 保存場所 | 形式 |
|---|---|---|
| 全文検索インデックス | `{app_data}/index/fulltext/` | tantivy独自形式 |
| ベクトルインデックス | メモリ上（永続化未実装） | — |
| embeddingモデル | `{model_dir}/model.onnx`, `tokenizer.json` | ONNX, JSON |
| LLMモデル | `{model_dir}/*.gguf` | GGUF |

`{model_dir}` は実行ファイルと同階層の `models/` ディレクトリ。
`{app_data}` はTauri APIが提供するプラットフォーム別パス。

### 8.2 未実装の永続化（今後の課題）

- ベクトルインデックスのディスク永続化（hnsw_rsのhnswioモジュール）
- アプリ設定のJSON永続化
- ファイルメタデータのSQLite管理

## 9. テスト構成

### 9.1 テスト総数: 211件

| カテゴリ | ファイル | 件数 |
|---|---|---|
| tantivy全文検索 | `infra/tantivy/mod.rs` | 11 |
| チャンク分割 | `domain/indexer/chunker.rs` | 5 |
| RRFハイブリッド | `domain/search/hybrid.rs` | 5 |
| HNSWベクトル検索 | `infra/hnsw/mod.rs` | 6 |
| ベクトルキャッシュ | `infra/vector_cache/mod.rs` | 9 |
| システム情報検出 | `infra/system/mod.rs` | 3 |
| モデル推奨ロジック | `domain/system/mod.rs` | 10 |
| GPU推定ロジック | `domain/llm/mod.rs` | 33 |
| ChatTemplate | `domain/llm/chat_template.rs` | 16 |
| GPUフォールバック | `infra/llama/mod.rs` | 13 |
| モデル管理 | `infra/model/mod.rs` | 22 |
| モデルレジストリ | `infra/model_registry/mod.rs` | 13 |
| RAGパイプライン | `domain/llm/rag.rs` | 8 |
| アプリ設定(domain) | `domain/config/mod.rs` | 1 |
| 設定永続化(infra) | `infra/config/mod.rs` | 7 |
| Appコンポーネント | `src/App.test.tsx` | 1 |
| Sidebar | `src/components/layout/Sidebar.test.tsx` | 2 |
| MainPanel | `src/components/layout/MainPanel.test.tsx` | 2 |
| SearchBar | `src/components/search/SearchBar.test.tsx` | 3 |
| ResultList | `src/components/search/ResultList.test.tsx` | 4 |
| Preview | `src/components/search/Preview.test.tsx` | 2 |
| ChatMessage | `src/components/chat/ChatMessage.test.tsx` | 4 |
| SettingsDialog | `src/components/settings/SettingsDialog.test.tsx` | 5 |
| GeneralSection | `src/components/settings/GeneralSection.test.tsx` | 18 |
| ModelManagement | `src/components/settings/ModelManagementSection.test.tsx` | 7 |

### 9.2 E2Eテスト

- **自動**: WebdriverIO v8 + tauri-driver（3件）
  - アプリウィンドウ表示、サイドバー、メインパネル
- **手動**: `docs/manual-e2e-tests.md`（3項目）
  - MET-001: embeddingモデルDL
  - MET-002: ベクトルインデックス構築
  - MET-003: ハイブリッド検索

## 10. 依存関係（Rust）

### 10.1 主要クレート

| クレート | バージョン | 用途 |
|---|---|---|
| tauri | 2 | アプリケーションフレームワーク |
| tauri-plugin-dialog | 2 | フォルダ選択ダイアログ |
| tauri-plugin-fs | 2 | ファイルシステムアクセス |
| tantivy | 0.25 | 全文検索エンジン |
| lindera | 2 (embed-ipadic) | 日本語形態素解析 |
| lindera-tantivy | 2 (embed-ipadic) | lindera-tantivy統合 |
| ort | 2.0.0-rc.12 | ONNX Runtime推論 |
| ndarray | 0.17 | テンソル操作 |
| hnsw_rs | 0.3 | HNSWベクトルインデックス |
| anndists | 0.1 | 距離関数（コサイン距離） |
| tokenizers | 0.22 | HuggingFaceトークナイザ |
| llama-cpp-2 | 0.1.140 | llama.cpp Rustバインディング |
| walkdir | 2 | ファイル走査 |
| reqwest | 0.13 (stream) | HTTPダウンロード |
| serde | 1 (derive) | シリアライズ |
| thiserror | 2 | エラー型定義 |
| sysinfo | 0.32 | システムRAM検出 |

### 10.2 ビルド依存

| ツール | 用途 |
|---|---|
| CMake | llama-cpp-2のC++コンパイル |
| libclang-dev | llama-cpp-2のbindgen |
| webkit2gtk-4.1-dev | Tauri WebView (Linux) |
| webkit2gtk-driver | E2Eテスト (Linux) |

## 11. ファイル監視（設計メモ）

### 11.1 概要

フォルダ内のファイル変更を検知し、全文検索インデックスとベクトルインデックスを自動更新する。

```
フォルダ選択 → インデックス構築 → notify::RecommendedWatcher 開始
                                        ↓
                                  ファイル変更イベント
                                        ↓
                                  デバウンス（2秒）
                                        ↓
                              全文検索インデックス差分更新
                                        ↓
                              ベクトルインデックス差分更新
                                        ↓
                              フロントエンドに通知（Tauri event）
```

### 11.2 差分更新方式

- **全文検索**: tantivy の `path` フィールドが `STRING` 型のため、`Term::from_field_text` で個別文書の削除が可能。変更ファイルは削除→再追加で更新する
- **ベクトル検索**: 既存の `build_vector_index` を再呼び出しする。`VectorCache::compute_diff` でファイル変更を検出し、変更ファイルのみembeddingを再生成する。HNSWインデックスは全体再構築（hnsw_rsに削除APIがないため）

### 11.3 注意事項

- ウォッチャーのコールバックは別スレッドで実行される。AppStateのMutexアクセスでデッドロックに注意する
- フォルダ再選択時に前のウォッチャーを確実に停止してから新しいウォッチャーを開始する
- デバウンス中にアプリが終了した場合の未処理イベントは無視してよい
- WSL2環境ではnotifyのinotifyがホストファイルシステム（/mnt/c/等）の変更を検知できない可能性がある。WSL2ネイティブのファイルシステムでは動作する

## 12. システム情報検出・モデル推奨

### 12.1 概要

システムのRAMとGPU情報を検出し、LLMモデルの推奨を自動的に行う。CPU推論時はシステムRAMベース、GPU推論時はVRAMベースで判定する。

### 12.2 検出方式

- **システムRAM**: `sysinfo` クレートで取得（クロスプラットフォーム）
- **GPU情報**: プラットフォーム別のコマンド実行+パース
  - Windows: `wmic path win32_VideoController get Name,AdapterRAM`
  - macOS: `system_profiler SPDisplaysDataType -json`
  - Linux: `lspci` + `nvidia-smi`（ベストエフォート）
- GPU検出は失敗してもエラーにしない（GPU情報なしとして扱う）

### 12.3 推奨ロジック

```
利用可能メモリ =
  GPU推論可能時 → 最大GPU VRAMを基準（将来のフェーズB）
  CPU推論時    → システムRAM − 2GB（OS/アプリ使用分）

各モデルの判定:
  min_vram_mb <= 利用可能メモリ          → Recommended
  min_vram_mb <= 利用可能メモリ × 1.5    → Warning（動くが遅い可能性）
  それ以外                               → TooLarge（非推奨）

best_fit = Recommended の中で最大サイズのモデル
```

### 12.4 データフロー

```
アプリ起動 / LLMセクション表示
  ↓
detect_system_info コマンド
  → SystemInfo { total_ram_mb, gpus, gpu_inference_available }
  ↓
get_model_recommendations コマンド
  → Vec<ModelRecommendation> { filename, status, is_best_fit, reason }
  ↓
フロントエンド表示:
  - モデルドロップダウンに推奨/警告バッジ
  - best_fit モデルを初期選択
  - サイドバーにシステム情報を簡易表示
```

### 12.5 GPU推論（フェーズB）

#### GPU バックエンド

| プラットフォーム | バックエンド | 有効化方法 |
|---|---|---|
| macOS | Metal | 自動（フィーチャーフラグ不要） |
| Windows/Linux (NVIDIA) | CUDA | `--features cuda` でビルド |

Cargo.toml でオプショナルフィーチャーとして定義する:
```toml
[features]
default = []
cuda = ["llama-cpp-2/cuda"]
```

#### 適応的GPUレイヤーオフロード

GPU推論をデフォルトとし、VRAM不足時は段階的にオフロード量を削減する。

```
モデルロード要求
  ↓
estimate_gpu_layers(model_size_bytes, vram_mb)
  → VRAM推定でオフロードレイヤー数を算出
  ↓
try_load(path, estimated_layers)
  → 成功 → 完了（GPU推論）
  → 失敗 ↓
layers /= 2 で半減して再試行（二分探索）
  → 成功 → 完了（部分GPU推論）
  → 失敗 → さらに半減…
  ↓
layers == 0 で最終試行
  → 成功 → 完了（CPU推論フォールバック）
  → 失敗 → エラー
```

推定ロジック（`domain/llm/mod.rs`）:
```
overhead = 512MB（KVキャッシュ+ワークスペース）
available = vram_mb - overhead

available >= model_mb → 全層オフロード（u32::MAX）
available > 0         → 比例配分（available / model_mb × 推定層数）
available <= 0        → CPU推論（0層）
```

#### gpu_inference_available の判定

コンパイル時フィーチャーフラグで判定する:
```rust
gpu_inference_available: cfg!(target_os = "macos") || cfg!(feature = "cuda")
```

GPU推論が有効な場合、推奨ロジックはVRAMベースに自動切り替えされる（フェーズA実装済み）。

#### LlamaEngine のインタフェース変更

```rust
// 変更前
pub fn new(model_path: &str) -> Result<Self, LlmError>

// 変更後
pub fn new(model_path: &str, model_size_bytes: u64, vram_mb: u64) -> Result<Self, LlmError>
```

戻り値の `LlamaEngine` に実際に使用された `n_gpu_layers` を保持し、フロントエンドにGPU/CPU推論のステータスを返す。

## 13. モデルレジストリ

### 13.1 概要

LLMモデルの管理を「プリセット（コードに定義）」と「カスタム（ユーザー登録、JSON永続化）」の2系統で行う。`ModelRegistry` がこれらをマージして統一的なモデル一覧を提供する。

### 13.2 アーキテクチャ

```
┌─────────────────────────────────────┐
│ ModelRegistry                       │
│   ├── default_models()  ← コード定義│
│   ├── custom_models     ← JSON読込  │
│   └── all_models()      ← マージ    │
└──────────────┬──────────────────────┘
               ↓
    {app_data_dir}/custom_models.json
```

### 13.3 custom_models.json スキーマ

```json
{
  "format_version": 1,
  "models": [
    {
      "name": "My Custom Model",
      "filename": "my-model.gguf",
      "url": "https://huggingface.co/...",
      "size_bytes": 0,
      "min_vram_mb": 0,
      "params": "",
      "quantization": "",
      "chat_template": "chatml",
      "context_length": 4096
    }
  ]
}
```

### 13.4 登録とキャッシュの分離

| 操作 | 対象 | 効果 |
|---|---|---|
| モデル登録 | `custom_models.json` | エントリ追加 |
| モデル登録解除 | `custom_models.json` | エントリ削除（DL済みファイルは残る） |
| キャッシュ削除 | `model_dir` のファイル | ファイル削除（登録は残る） |

デフォルトプリセットは登録解除不可（コードに定義されているため常に表示される）。

### 13.5 データフロー

```
アプリ起動
  ↓
ModelRegistry::new(model_dir)
  → custom_models.json 読み込み（存在しなければ空）
  ↓
list_available_models コマンド
  → ModelRegistry::all_models()
  → デフォルトプリセット + カスタムモデルをマージして返す
  ↓
register_custom_model コマンド
  → ModelRegistry::add_model(model)
  → custom_models.json に保存
  ↓
unregister_custom_model コマンド
  → ModelRegistry::remove_model(filename)
  → custom_models.json から削除（DL済みファイルは保持）
```

## 14. ダウンロードキャッシュ管理

### 14.1 概要

モデルダウンロードキャッシュにサイズ上限を設け、超過時にLRUエビクションで古いファイルを自動削除する。

### 14.2 キャッシュ上限

デフォルト上限: 100 GB（`DEFAULT_CACHE_LIMIT_BYTES` 定数）。将来、設定ダイアログでユーザー変更可能にする。

### 14.3 ダウンロードフロー

```
ダウンロード要求 (filename, url, size_bytes)
  ↓
size_bytes > cache_limit?
  → YES: エラー「キャッシュ上限を超えるためダウンロードできない」
  ↓ NO
ダウンロード実行
  ↓
キャッシュ合計 > cache_limit?
  → YES: LRUエビクション
         ロード中でないGGUFファイルを更新日時の古い順にソート
         合計が上限以下になるまで削除
         削除されたファイル名をフロントエンドに返す
  ↓
完了
```

### 14.4 削除対象の判定

- **削除不可**: 現在ロード中のLLMモデル、ロード中のembeddingモデル
- **削除対象**: 上記以外のGGUFファイル
- **ソート基準**: ファイルの更新日時（`metadata().modified()`）、古い順に削除

### 14.5 全キャッシュ削除

`clear_model_cache` コマンドでロード中モデル以外の全GGUFファイルを一括削除する。
