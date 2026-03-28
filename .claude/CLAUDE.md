# Folder Search - プロジェクト開発ガイドライン

## プロジェクト概要

ローカルナレッジベースから高速・高精度に知識を引き出すデスクトップアプリケーション。
指定フォルダ内のプレーンテキスト・マークダウンファイルに対し、全文検索・ベクトル検索・ローカルLLMによるRAGを提供する。

## 技術スタック

- **フレームワーク**: Tauri v2 (2.10)
- **フロントエンド**: React 19 + TypeScript 5.8 + Vite 6
- **バックエンド（ネイティブレイヤー）**: Rust 1.94
- **全文検索**: tantivy 0.25 + lindera 2（IPAdic辞書、日本語形態素解析）
- **ベクトル検索**: ort 2.0 (ONNX Runtime) + hnsw_rs 0.3 + intfloat/multilingual-e5-small
- **ローカルLLM**: llama-cpp-2 0.1.140（GGUF形式、Qwen2.5系モデル）
- **テスト**: Vitest 4 + cargo test + WebdriverIO 8 + tauri-driver
- **対象プラットフォーム**: Windows / macOS

## 開発環境要件

- Node.js >= 20
- Rust >= 1.77
- pnpm（パッケージマネージャ）
- CMake（llama-cpp-2のビルドに必要）
- libclang-dev（llama-cpp-2のbindgenに必要）
- libwebkit2gtk-4.1-dev（Linux/WSL2のTauri WebView）
- webkit2gtk-driver（E2Eテスト用、Linux）

## コマンド一覧

```bash
# 開発サーバー起動
pnpm tauri dev

# ビルド
pnpm tauri build

# デバッグビルド（E2Eテスト用）
pnpm tauri build --debug

# フロントエンドテスト
pnpm test

# Rustテスト
cargo test --manifest-path src-tauri/Cargo.toml

# E2Eテスト（要：デバッグビルド済み）
pnpm test:e2e

# リント
pnpm lint

# Rustリント
cargo clippy --manifest-path src-tauri/Cargo.toml -- -W clippy::all

# フォーマット
pnpm format

# Rustフォーマットチェック
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

## 開発ルール

### 全般

- CONTRIBUTING.md に記載されたTDD手法に従う
- テストコードを先に書き、失敗を確認してから実装する
- E2Eテストを必ず検討する（自動化不可の場合は手動テスト項目に追記）
- コミット前にlint・フォーマット・テストを実行する
- MLモデル（ONNX, GGUF）はリポジトリにコミットしない（実行時にダウンロード）
- バージョン番号（package.json, Cargo.toml, tauri.conf.json）はリリース時にのみ更新する。通常の開発中は変更しない。手順は docs/RELEASING.md を参照

### フロントエンド（TypeScript / React）

- 関数コンポーネント + hooks を使用する
- 型定義を先に書く（src/types/index.ts）
- 純粋関数を優先し、副作用を分離する
- Tauri IPC呼び出しは src/lib/tauri.ts 経由で行う
- テスト環境ではsrc/test/tauri-mock.tsでTauri APIをモックする
- Vitest でユニットテストを書く

### バックエンド（Rust）

- Tauriコマンドは `src-tauri/src/commands/mod.rs` に集約する
- ドメインロジックは `src-tauri/src/domain/` にトレイト・型として定義する
- 外部ライブラリの実装は `src-tauri/src/infra/` に配置し、domainのトレイトを実装する
- `cargo test` でユニットテストを書く
- エラー型は `thiserror` で定義する

### ドキュメント

- 設計変更時は `docs/` 配下のドキュメントを更新する
- 手動E2Eテスト項目は `docs/manual-e2e-tests.md` に追記する
- 機能の実装状況は `docs/features.md` で管理する
- ユーザー向けの変更（機能追加・変更・削除・バグ修正）は `CHANGELOG.md` に記載する
