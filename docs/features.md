# 機能一覧・TODO

## Phase 0: ドキュメント整備

- [x] 要件定義書（docs/requirements.md）
- [x] 機能一覧・TODO（docs/features.md）
- [x] アーキテクチャ設計書（docs/architecture.md）
- [x] コントリビューションガイド（docs/CONTRIBUTING.md）
- [x] 手動E2Eテスト項目リスト（docs/manual-e2e-tests.md）
- [x] プロジェクト開発ガイドライン（.claude/CLAUDE.md）

## Phase 1: 基盤構築

### セットアップ

- [x] Tauri v2 + React + TypeScript プロジェクト初期化
- [x] ESLint + Prettier 設定
- [x] Rust clippy + rustfmt 設定
- [x] .gitattributes（クロスプラットフォーム改行コード LF統一）
- [x] .gitignore

### テスト基盤

- [x] Vitest 導入（フロントエンドユニットテスト）
- [x] cargo test 設定（Rustユニットテスト）
- [x] E2Eテスト基盤の導入（WebdriverIO v8 + tauri-driver）
- [ ] E2EテストのCI統合
- [x] 手動E2Eテスト項目リスト運用開始

### 基本UI

- [x] アプリケーションウィンドウの表示
- [x] 基本レイアウト（Sidebar + MainPanel コンポーネント）
- [x] アプリアイコン（プレースホルダー）

## Phase 2: 全文検索

### インデックス構築

- [x] tantivy 0.25 クレート統合
- [x] lindera 2 トークナイザ統合（IPAdic辞書、日本語形態素解析）
- [x] フォルダ選択ダイアログ（Tauri plugin-dialog）
- [x] ファイル走査（walkdir）・インデックス構築
- [ ] ファイル監視（notify クレート）・差分インデックス更新
- [x] インデックス状態の表示（件数）

### 検索機能

- [x] 検索クエリ入力UI（SearchBar、Enter実行）
- [x] tantivy による全文検索の実行（BM25スコアリング）
- [x] 検索結果のランキング表示（ResultList）
- [x] 検索結果の原文プレビュー（Preview）
- [x] スニペット生成（tantivy SnippetGenerator）
- [ ] 原文ファイルを外部エディタで開く機能

### 品質検証

- [ ] 検索応答時間のベンチマーク（目標: 100ms以内）
- [x] 日本語検索の精度検証（テストで確認済み）

## Phase 3: ベクトル検索 → ハイブリッド化

### Embedding生成

- [x] embeddingモデル選定: intfloat/multilingual-e5-small（384次元、ONNX）
- [x] ONNX Runtime 統合（ort 2.0.0-rc.12）
- [x] tokenizers 統合（0.22、tokenizer.json方式）
- [x] テキストのチャンク分割（500文字、100文字オーバーラップ）
- [x] embeddingのバッチ生成
- [ ] embedding差分更新

### ベクトル検索

- [x] HNSWインデックス構築（hnsw_rs 0.3 + anndists DistCosine）
- [x] ベクトル近似最近傍検索
- [x] バッチ挿入（parallel_insert）

### ハイブリッド検索

- [x] RRF（Reciprocal Rank Fusion、k=60）によるスコア統合
- [x] ハイブリッド検索UIの統合（検索モード切り替え）
- [ ] 検索精度評価（Recall@10 目標: 80%以上）

### モデル管理

- [x] embeddingモデルのダウンロード機能（HuggingFaceから）
- [x] ダウンロード進捗表示（Tauri event通知）
- [x] モデルの存在チェック・キャッシュ
- [x] ベクトルインデックス構築の進捗表示

### 品質検証

- [ ] ベクトル検索応答時間のベンチマーク（目標: 500ms以内）
- [ ] ハイブリッド検索応答時間のベンチマーク（目標: 1秒以内）
- [ ] セマンティック検索の精度検証

## Phase 4: ローカルLLM統合

### llama.cpp統合

- [x] llama-cpp-2 0.1.140 Rustバインディング統合
- [x] GGUFモデルのロード・推論
- [x] ストリーミング推論（トークン単位コールバック）
- [ ] GPU VRAM検出機能
- [x] モデル選択UI（プルダウン）

### モデル管理

- [x] GGUF形式モデルのダウンロード機能
- [x] ダウンロード進捗表示
- [x] モデル切り替え機能
- [ ] モデルストレージ管理（削除、サイズ表示）
- [x] 利用可能モデル一覧（Qwen2.5系 4種）

### RAGパイプライン

- [x] 検索結果のコンテキスト構築（上位5件、各1000文字）
- [x] LLMへのプロンプト生成（Qwen2 ChatML形式）
- [x] ストリーミング回答表示（ChatMessageコンポーネント）
- [x] 回答への参照元ファイルパス付記
- [x] 参照元の原文ワンクリック表示

### UI

- [x] 検索モード / チャットモード切り替え
- [x] ChatMessageコンポーネント（ストリーミングカーソル、参照元リンク）
- [x] Tauri APIモック（テスト環境用）

### 品質検証

- [ ] LLM最初のトークン表示時間のベンチマーク（目標: 3秒以内）
- [ ] RAG回答精度の検証
- [ ] 日本語モデルの評価

## 未実装項目（今後の課題）

### 機能

- [ ] ファイル監視による自動インデックス更新（notify クレート）
- [ ] 原文ファイルを外部エディタで開く機能
- [ ] GPU VRAM検出・自動モデル推奨
- [ ] ベクトルインデックスのディスク永続化
- [ ] アプリ設定の永続化（JSON）
- [ ] 複数フォルダの登録対応
- [ ] モデルストレージ管理（削除、サイズ表示）
- [ ] embedding差分更新

### 品質

- [ ] 検索応答時間のベンチマーク
- [ ] 検索精度評価（Recall@10）
- [ ] E2EテストのCI統合
