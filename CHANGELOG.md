# Changelog

このプロジェクトのすべての注目すべき変更はこのファイルに記録する。

フォーマットは [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に準拠する。
バージョニングは [Semantic Versioning](https://semver.org/lang/ja/) に従う。

## [Unreleased]

### Added

- ベクトルインデックスのフォルダごとのディスクキャッシュ（embedding+メタデータをbincodeで永続化、キャッシュヒット時はHNSW再構築のみで高速復元）
- embedding差分更新（変更ファイルのみembedding再生成、未変更ファイルはキャッシュ再利用）
- ファイル監視による自動インデックス更新（notify クレート、2秒デバウンス）
- 全文検索インデックスの差分更新（tantivy Term削除による個別文書更新）
- システム情報検出（RAM・GPU VRAM）とLLMモデル自動推奨（sysinfo + プラットフォーム別GPU検出）
- GPU推論の適応的レイヤーオフロード（VRAM推定→二分探索フォールバック→CPU）
- macOS Metal 自動有効化、CUDA オプショナルフィーチャー（`--features cuda`）
- モデルストレージ管理（DL済みモデル一覧・ファイルサイズ表示・削除機能・ストレージ使用量表示）
- チャットテンプレートシステム（ChatML / Gemma / Llama3 の3テンプレート対応）
- Gemma 3 モデルプリセット追加（1B / 4B / 12B）
- LLMモデルごとのコンテキスト長設定（固定2048から可変に）
- モデルレジストリ（デフォルトプリセット + カスタムモデルのJSON永続化管理）
- カスタムモデル登録・登録解除機能（登録とダウンロードキャッシュの分離）
- カスタムモデル登録UI（モデル名・ファイル名・URL・テンプレート・コンテキスト長の入力フォーム）
- ダウンロードキャッシュ管理（100GBデフォルト上限、ダウンロード前サイズチェック）
- LRUエビクション（キャッシュ上限超過時に古いモデルファイルを自動削除、ロード中モデルは保護）
- 全キャッシュ削除機能（ロード中モデル以外の一括削除ボタン）
- アプリ設定の永続化（JSON形式、settings.json）
- 設定ダイアログ（歯車アイコン → モーダル、左: 設定入力 / 右: セクション選択）
- 一般セクション: ダウンロードキャッシュサイズスライダー（5GB刻み、サジェスト表示）
- 一般セクション: LLMモデル選択UI（モデルリスト、推奨バッジ、DLステータス、行内ロードボタン）
- モデル管理セクション: 全モデル一覧（DL済み/未DL）、有効/無効チェックボックス、DL/削除ボタン、推奨バッジ
- Llama 3 モデルプリセット追加（1B / 3B / 8B）
- 前回ロードしたLLMモデルの起動時自動ロード（settings.json に永続化）
- 起動時LLMロード中のローディングオーバーレイ（スピナー表示、操作無効化）
- モデル有効/無効設定の永続化（disabled_models in settings.json）
- 環境変数ファイル対応（dotenvy: .env.development / .env.production）
- デバッグビルド時のDevTools自動起動（TAURI_OPEN_DEVTOOLS=1）
- ロード中モデルの削除・無効化防止（フロントエンド・バックエンド両方でガード）
- モデルファイル削除時の確認ダイアログ

### Changed

- `list_available_models` をModelRegistry経由に変更（プリセット + カスタムモデルを統合して返す）
- `download_llm_model` にサイズチェックとLRUエビクションを追加（戻り値で自動削除されたファイルを通知）
- `load_llm_model` を非同期化（spawn_blockingでメインスレッド非ブロック化）
- LlamaBackend を OnceLock シングルトン化（二重初期化エラー防止）
- LLMモデル選択・管理UIをサイドバーから設定ダイアログに移動
- フォルダ選択後またはembeddingモデルロード後にベクトルインデックスを自動構築するよう変更（手動ボタン廃止）
- ベクトルインデックス構築のプログレス送信間隔をスケーラブルに変更（max(total/100, 1)チャンクごと）
- ベクトルインデックス構築中のステータス表示を改善（Embeddingモデルと同じ書式で表示）

### Fixed

- アプリ起動時にダウンロード済みembeddingモデルを自動ロードするよう修正（setupフック追加）
- ONNX推論にtoken_type_idsテンソルを追加し、embedding生成の失敗を修正

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
