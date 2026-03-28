# アーキテクチャ設計書

## 1. 全体アーキテクチャ

Tauri v2のアーキテクチャに従い、フロントエンド（WebView）とネイティブレイヤー（Rust）の2層構成をとる。

```
┌──────────────────────────────────────────────┐
│  フロントエンド（WebView）                      │
│  React + TypeScript                           │
│  ├── UI コンポーネント                          │
│  ├── 状態管理                                  │
│  └── Tauri IPC（invoke / listen）              │
├──────────────────────────────────────────────┤
│  Tauri IPC ブリッジ                             │
├──────────────────────────────────────────────┤
│  ネイティブレイヤー（Rust）                      │
│  ├── commands/    ← Tauri コマンド（IPC API）   │
│  ├── domain/      ← ドメインロジック            │
│  │   ├── indexer/   ファイル走査・インデックス   │
│  │   ├── search/    検索エンジン                │
│  │   ├── embedding/ ベクトル生成・検索          │
│  │   └── llm/       LLM推論・RAG              │
│  ├── infra/       ← インフラストラクチャ        │
│  │   ├── fs/        ファイルシステム操作        │
│  │   ├── tantivy/   全文検索エンジン            │
│  │   ├── onnx/      ONNX Runtime             │
│  │   └── llama/     llama.cpp バインディング   │
│  └── config/      ← 設定管理                   │
└──────────────────────────────────────────────┘
```

### 設計原則

- **クリーンアーキテクチャ**: domain層は外部ライブラリに依存しない。infra層が具体的な実装を提供する
- **依存性の方向**: commands → domain ← infra（domain層が中心、infra層はdomain層のトレイトを実装する）
- **IPC境界**: フロントエンドとRust間のデータ型は `serde` でシリアライズする

## 2. 検索パイプライン

### 2.1 全文検索パイプライン（Phase 2）

```
ユーザーのクエリ文字列
  ↓
lindera トークナイザ（日本語形態素解析）
  ↓
tantivy クエリパーサー
  ↓
tantivy インデックス検索
  ↓
BM25 スコアによるランキング
  ↓
検索結果（ファイルパス、スコア、マッチ箇所のスニペット）
```

### 2.2 ハイブリッド検索パイプライン（Phase 3）

```
ユーザーのクエリ文字列
  ↓
┌────────────────────┬────────────────────────┐
│ 全文検索            │ ベクトル検索             │
│ (tantivy + lindera)│ (ONNX + HNSW)          │
│ BM25スコア          │ コサイン類似度           │
└────────┬───────────┴───────────┬────────────┘
         ↓                      ↓
    ランキングA             ランキングB
         ↓                      ↓
         └──────────┬───────────┘
                    ↓
          RRF（Reciprocal Rank Fusion）
                    ↓
          統合ランキング
                    ↓
          上位N件の検索結果
```

### 2.3 RAGパイプライン（Phase 4）

```
ユーザーの自然言語による質問
  ↓
ハイブリッド検索（上記2.2）
  ↓
上位N件のチャンク
  ↓
プロンプト構築:
  「以下のコンテキストに基づいて質問に回答してください。
   回答には参照元のファイル名と該当箇所を付記してください。
   [コンテキスト: チャンク1, チャンク2, ...]
   [質問: ユーザーの質問]」
  ↓
llama.cpp 推論（ストリーミング）
  ↓
回答テキスト + 参照元情報
  ↓
UI: ストリーミング表示 + 原文参照リンク
```

## 3. コンポーネント設計

### 3.1 フロントエンドコンポーネント

```
src/
├── App.tsx                  ← ルートコンポーネント
├── components/
│   ├── layout/
│   │   ├── Sidebar.tsx      ← サイドバー（フォルダ一覧、設定）
│   │   └── MainPanel.tsx    ← メインパネル
│   ├── search/
│   │   ├── SearchBar.tsx    ← 検索クエリ入力
│   │   ├── ResultList.tsx   ← 検索結果一覧
│   │   └── Preview.tsx      ← 原文プレビュー
│   ├── chat/
│   │   ├── ChatInput.tsx    ← 質問入力
│   │   ├── ChatMessage.tsx  ← 回答表示（ストリーミング対応）
│   │   └── SourceRef.tsx    ← 参照元リンク
│   └── settings/
│       ├── FolderConfig.tsx ← フォルダ設定
│       ├── ModelConfig.tsx  ← モデル設定・ダウンロード
│       └── IndexStatus.tsx  ← インデックス状態表示
├── hooks/
│   ├── useSearch.ts         ← 検索ロジック
│   ├── useChat.ts           ← チャットロジック
│   └── useIndex.ts          ← インデックス管理ロジック
├── types/
│   └── index.ts             ← 共通型定義
└── lib/
    └── tauri.ts             ← Tauri IPC ラッパー
```

### 3.2 Rustモジュール構成

```
src-tauri/src/
├── main.rs                  ← エントリポイント
├── lib.rs                   ← Tauriアプリ設定
├── commands/
│   ├── mod.rs
│   ├── search.rs            ← 検索コマンド
│   ├── index.rs             ← インデックス管理コマンド
│   ├── folder.rs            ← フォルダ管理コマンド
│   ├── model.rs             ← モデル管理コマンド
│   └── chat.rs              ← チャットコマンド
├── domain/
│   ├── mod.rs
│   ├── indexer/
│   │   ├── mod.rs
│   │   ├── scanner.rs       ← ファイル走査
│   │   ├── watcher.rs       ← ファイル監視
│   │   └── chunker.rs       ← チャンク分割
│   ├── search/
│   │   ├── mod.rs
│   │   ├── fulltext.rs      ← 全文検索トレイト
│   │   ├── vector.rs        ← ベクトル検索トレイト
│   │   └── hybrid.rs        ← ハイブリッド検索（RRF）
│   ├── embedding/
│   │   ├── mod.rs
│   │   └── generator.rs     ← embedding生成トレイト
│   └── llm/
│       ├── mod.rs
│       ├── inference.rs      ← LLM推論トレイト
│       ├── model_manager.rs  ← モデル管理
│       └── rag.rs            ← RAGパイプライン
├── infra/
│   ├── mod.rs
│   ├── fs/
│   │   └── mod.rs            ← ファイルシステム操作
│   ├── tantivy/
│   │   ├── mod.rs
│   │   ├── indexer.rs        ← tantivyインデックス実装
│   │   └── searcher.rs       ← tantivy検索実装
│   ├── onnx/
│   │   └── mod.rs            ← ONNX Runtime実装
│   └── llama/
│       └── mod.rs            ← llama.cppバインディング実装
└── config/
    ├── mod.rs
    └── settings.rs           ← アプリ設定
```

## 4. データフロー

### 4.1 インデックス構築フロー

```
フォルダ登録
  → ファイル走査（scanner）
  → 各ファイルの内容読み取り
  → lindera で日本語トークン化
  → tantivy インデックスに追加
  → （Phase 3）チャンク分割 → embedding生成 → HNSWインデックスに追加
```

### 4.2 インデックス更新フロー

```
ファイル監視（watcher）がイベント検知
  → 変更種別の判定（作成 / 更新 / 削除）
  → 該当ファイルのインデックスを差分更新
  → フロントエンドにイベント通知（Tauri event）
```

### 4.3 検索フロー

```
フロントエンド: SearchBar → invoke("search", { query })
  → Rust: commands::search::search()
  → domain::search::hybrid::search()
  → 結果をシリアライズして返却
  → フロントエンド: ResultList に表示
```

### 4.4 RAGフロー

```
フロントエンド: ChatInput → invoke("chat", { question })
  → Rust: commands::chat::chat()
  → domain::search::hybrid::search() で関連チャンク取得
  → domain::llm::rag::generate() でプロンプト構築 + 推論
  → Tauri event でストリーミング送信
  → フロントエンド: ChatMessage にストリーミング表示
```

## 5. 技術選定の詳細

### 5.1 全文検索: tantivy

- Rust製の全文検索エンジン。Apache Lucene相当の機能を提供する
- BM25スコアリング、フレーズ検索、ファセット検索をサポートする
- lindera tokenizer でIPAdic辞書を用いた日本語形態素解析に対応する

### 5.2 ベクトル検索: ONNX Runtime + HNSW

- ONNX Runtimeでsentence-transformersモデル（多言語対応）を実行する
- embedding候補: `intfloat/multilingual-e5-small`（384次元、約100MB）
- HNSWアルゴリズムでベクトル近似最近傍検索を行う
- Rustクレート: `ort`（ONNX Runtime）、`hnsw`（近傍探索）

### 5.3 ローカルLLM: llama.cpp

- llama.cppのRustバインディング（`llama-cpp-rs` / `llama-cpp-2`）を使用する
- GGUF形式のモデルをサポートする
- GPU VRAMに応じたモデルサイズの推奨と切り替え機能を提供する
- モデルはHuggingFace Hubから動的にダウンロードする

### 5.4 ハイブリッド検索: RRF

- Reciprocal Rank Fusion でBM25スコアとベクトル類似度のランキングを統合する
- 計算式: `RRF_score = Σ 1 / (k + rank_i)` （k=60が一般的）
- 全文検索とベクトル検索の長所を組み合わせ、検索精度を向上させる

## 6. 永続化

### 6.1 インデックスデータ

| データ | 保存形式 | 保存場所 |
|---|---|---|
| 全文検索インデックス | tantivy独自形式 | `{app_data}/index/fulltext/` |
| ベクトルインデックス | HNSWバイナリ | `{app_data}/index/vector/` |
| ファイルメタデータ | SQLite | `{app_data}/metadata.db` |

### 6.2 アプリ設定

| データ | 保存形式 | 保存場所 |
|---|---|---|
| アプリ設定 | JSON | `{app_config}/settings.json` |
| ダウンロード済みモデル | GGUF | `{app_data}/models/` |

`{app_data}` と `{app_config}` はTauri APIが提供するプラットフォーム別のパスを使用する。
