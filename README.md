# Folder Search

ローカルナレッジベースから高速・高精度に知識を引き出すデスクトップアプリケーション。

指定フォルダ内のプレーンテキスト・マークダウンファイルに対し、全文検索・ベクトル検索・ローカルLLMによるRAGを提供する。クラウドサービスに依存せず、完全ローカルで動作する。

## 主な機能

- **全文検索** — tantivy + lindera（IPAdic）による日本語対応の高速キーワード検索（BM25スコアリング）
- **ベクトル検索** — multilingual-e5-small（ONNX）による意味的な類似検索（HNSW近傍探索）
- **ハイブリッド検索** — 全文検索とベクトル検索をRRF（Reciprocal Rank Fusion）で統合
- **RAG質問応答** — ローカルLLM（llama.cpp）による検索結果に基づく自然言語回答（ストリーミング表示）
- **モデル管理** — HuggingFaceからembeddingモデル・LLMモデルを動的ダウンロード

## 技術スタック

| レイヤー | 技術 |
|---|---|
| フレームワーク | Tauri v2 |
| フロントエンド | React 19 + TypeScript 5.8 + Vite 6 |
| バックエンド | Rust（ネイティブレイヤー） |
| 全文検索 | tantivy 0.25 + lindera 2（IPAdic辞書） |
| ベクトル検索 | ONNX Runtime（ort 2.0） + hnsw_rs 0.3 |
| ローカルLLM | llama-cpp-2 0.1.140（GGUF形式） |
| テスト | Vitest（フロントエンド）/ cargo test（Rust）/ WebdriverIO（E2E） |

## 対応プラットフォーム

- Windows 10以降
- macOS 12（Monterey）以降

## 必要環境

### 実行時

- Windows 10以降（WebView2は標準同梱）
- macOS 12（Monterey）以降（WKWebViewは標準搭載）

追加のランタイム依存はない。

### 開発時

- Node.js >= 20
- Rust >= 1.77
- pnpm
- CMake（llama.cppのビルドに必要）
- libclang-dev（llama.cppのbindgenに必要）

#### Linux追加要件

- webkit2gtk-4.1-dev（Tauri WebView）
- webkit2gtk-driver（E2Eテスト）

## セットアップ

```bash
# リポジトリのクローン
git clone <repository-url>
cd folder-search

# フロントエンド依存のインストール
pnpm install

# 開発サーバー起動
pnpm tauri dev
```

## コマンド一覧

```bash
# 開発
pnpm tauri dev          # 開発サーバー起動
pnpm tauri build        # リリースビルド

# テスト
pnpm test               # フロントエンドユニットテスト（Vitest）
cargo test --manifest-path src-tauri/Cargo.toml  # Rustユニットテスト
pnpm test:e2e           # E2Eテスト（WebdriverIO）

# コード品質
pnpm lint               # ESLint
pnpm format             # Prettier
```

## プロジェクト構成

```
folder-search/
├── src/                          # フロントエンド（React + TypeScript）
│   ├── components/
│   │   ├── layout/               #   Sidebar, MainPanel
│   │   ├── search/               #   SearchBar, ResultList, Preview
│   │   └── chat/                 #   ChatMessage
│   ├── lib/tauri.ts              #   Tauri IPCラッパー
│   └── types/index.ts            #   共通型定義
├── src-tauri/                    # バックエンド（Rust）
│   └── src/
│       ├── commands/             #   Tauriコマンド
│       ├── domain/               #   ドメインロジック
│       │   ├── indexer/          #     インデックス構築・チャンク分割
│       │   ├── search/           #     全文検索・ハイブリッド検索
│       │   ├── embedding/        #     ベクトル生成・検索トレイト
│       │   └── llm/              #     LLM推論・RAGパイプライン
│       └── infra/                #   インフラ実装
│           ├── tantivy/          #     全文検索エンジン
│           ├── onnx/             #     ONNX Runtime embedding
│           ├── hnsw/             #     HNSWベクトルインデックス
│           ├── llama/            #     llama.cpp LLM推論
│           └── model/            #     モデルDL・管理
├── docs/                         # ドキュメント
├── e2e/                          # E2Eテスト
└── public/                       # 静的ファイル
```

## アーキテクチャ

Tauri v2の2層構成（WebView + Rustネイティブレイヤー）を採用している。

- **クリーンアーキテクチャ** — domain層は外部ライブラリに依存しない。infra層がdomain層のトレイトを実装する
- **依存性の方向** — commands → domain ← infra（domain層が中心）
- **IPC境界** — フロントエンドとRust間のデータ型は`serde`でシリアライズする

詳細は [docs/architecture.md](docs/architecture.md) を参照。

## 利用可能なLLMモデル

アプリ内からダウンロード・切り替えが可能。全モデルがQwen2.5ベースで日本語に対応する。

| モデル | サイズ | 推奨VRAM | 用途 |
|---|---|---|---|
| Qwen2.5-0.5B-Instruct Q4_K_M | 491MB | CPU可 | テスト・軽量用途 |
| Qwen2.5-1.5B-Instruct Q4_K_M | 1.12GB | 2GB+ | 日常用途 |
| Qwen2.5-7B-Instruct Q4_K_M | 4.68GB | 6GB+ | 高品質 |
| Qwen2.5-14B-Instruct Q4_K_M | 8.99GB | 12GB+ | 最高品質 |

## ドキュメント

- [要件定義書](docs/requirements.md)
- [アーキテクチャ設計書](docs/architecture.md)
- [機能一覧・TODO](docs/features.md)
- [コントリビューションガイド](docs/CONTRIBUTING.md)
- [手動E2Eテスト項目](docs/manual-e2e-tests.md)

## ライセンス

[MIT License](LICENSE)
