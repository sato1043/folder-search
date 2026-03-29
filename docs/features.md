# 機能ロードマップ

現在の機能セットと将来の計画を示す。
`[x]` は実装済み、`[ ]` は未実装（計画中）を表す。

カレントスナップショットのバージョンは[package.json](../package.json)のversionフィールドを参照。

直近の実装タスクの詳細は [TODO.md](TODO.md) を参照。

## Phase 1: 基盤構築

- [x] Tauri v2 + React + TypeScript プロジェクト初期化
- [x] ESLint + Prettier + clippy + rustfmt 設定
- [x] アプリケーションウィンドウの表示
- [x] 基本レイアウト（Sidebar + MainPanel コンポーネント）
- [x] Vitest + cargo test によるテスト基盤
- [x] WebdriverIO + tauri-driver によるE2Eテスト基盤
- [ ] E2EテストのCI統合

## Phase 2: 全文検索

- [x] tantivy + lindera（IPAdic辞書）による日本語全文検索
- [x] フォルダ選択ダイアログ（Tauri plugin-dialog）
- [x] ファイル走査・インデックス構築
- [x] BM25スコアリング + スニペット生成
- [x] 検索UI（SearchBar, ResultList, Preview）
- [x] インデックス状態の表示（件数）
- [ ] ファイル監視による自動インデックス更新（notify クレート）
- [ ] 原文ファイルを外部エディタで開く機能

## Phase 3: ベクトル検索・ハイブリッド検索

- [x] ONNX Runtime + multilingual-e5-small によるembedding生成
- [x] テキストのチャンク分割（500文字、100文字オーバーラップ）
- [x] HNSWインデックスによるベクトル近似最近傍検索
- [x] RRF（Reciprocal Rank Fusion）によるハイブリッド検索
- [x] embeddingモデルのダウンロード・進捗表示
- [x] ベクトルインデックスの自動構築（フォルダ選択後/モデルロード後）
- [ ] ベクトルインデックスのディスク永続化
- [ ] embedding差分更新

## Phase 4: ローカルLLM統合

- [x] llama-cpp-2 によるGGUFモデル推論
- [x] ストリーミング推論（トークン単位送信）
- [x] RAGパイプライン（検索→コンテキスト構築→LLM推論）
- [x] Qwen2.5系モデル4種のプリセット
- [x] LLMモデルのダウンロード・切り替え機能
- [x] チャットUI（ストリーミングカーソル、参照元リンク）
- [x] 検索モード / チャットモード切り替え
- [ ] GPU VRAM検出・自動モデル推奨
- [ ] モデルストレージ管理（削除、サイズ表示）

## 将来の機能

- [ ] 初回ダウンロードウィザード
- [ ] ベクトルインデックスのフォルダごとのキャッシュ
- [ ] アプリ設定の永続化（JSON）
- [ ] 複数フォルダの登録対応
