# Changelog

このプロジェクトのすべての注目すべき変更はこのファイルに記録する。

フォーマットは [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に準拠する。
バージョニングは [Semantic Versioning](https://semver.org/lang/ja/) に従う。

## [Unreleased]

### Fixed

- アプリ起動時にダウンロード済みembeddingモデルを自動ロードするよう修正（setupフック追加）

## [0.1.0] - 2026-03-28

### Added

#### Phase 0: ドキュメント整備

- 要件定義書（docs/requirements.md）
- アーキテクチャ設計書（docs/architecture.md）
- 機能一覧・TODO（docs/features.md）
- コントリビューションガイド（docs/CONTRIBUTING.md）
- 手動E2Eテスト項目リスト（docs/manual-e2e-tests.md）
- プロジェクト開発ガイドライン（.claude/CLAUDE.md）

#### Phase 1: 基盤構築

- Tauri v2 + React 19 + TypeScript 5.8 プロジェクト初期化
- ESLint + Prettier + clippy + rustfmt 設定
- 基本レイアウト（Sidebar + MainPanel コンポーネント）
- Vitest によるフロントエンドユニットテスト基盤
- cargo test によるRustユニットテスト基盤
- WebdriverIO v8 + tauri-driver によるE2Eテスト基盤

#### Phase 2: 全文検索

- tantivy 0.25 + lindera 2（IPAdic辞書）による日本語全文検索エンジン
- フォルダ選択ダイアログ（Tauri plugin-dialog）
- ファイル走査（walkdir）・インデックス構築
- BM25スコアリング + スニペット生成
- 検索UI（SearchBar, ResultList, Preview コンポーネント）
- Tauri IPCラッパー（lib/tauri.ts）・共通型定義（types/index.ts）

#### Phase 3: ベクトル検索・ハイブリッド検索

- ONNX Runtime（ort 2.0）+ multilingual-e5-small によるembedding生成
- テキストのチャンク分割（500文字、100文字オーバーラップ）
- HNSW近似最近傍検索（hnsw_rs + コサイン距離）
- RRF（Reciprocal Rank Fusion、k=60）によるハイブリッド検索
- embeddingモデルのダウンロード機能（HuggingFaceから、進捗表示付き）
- ベクトルインデックス構築の進捗表示
- ハイブリッド検索UIの統合（検索モード切り替え）
- Tauri APIモック（テスト環境用）

#### Phase 4: ローカルLLM統合

- llama-cpp-2 0.1.140 によるGGUFモデル推論
- ストリーミング推論（トークン単位でTauri event送信）
- RAGパイプライン（ハイブリッド検索 → コンテキスト構築 → プロンプト生成 → LLM推論）
- Qwen2.5系モデル4種のプリセット（0.5B / 1.5B / 7B / 14B）
- LLMモデルのダウンロード・切り替え機能
- チャットUI（ChatMessage コンポーネント、ストリーミングカーソル、参照元リンク）
- 検索モード / チャットモード切り替え
