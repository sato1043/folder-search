# Folder Search - プロジェクト開発ガイドライン

## プロジェクト概要

ローカルナレッジベースから高速・高精度に知識を引き出すデスクトップアプリケーション。
指定フォルダ内のプレーンテキスト・マークダウンファイルに対し、全文検索・ベクトル検索・ローカルLLMによるRAGを提供する。

## 技術スタック

- **フレームワーク**: Tauri v2
- **フロントエンド**: React + TypeScript
- **バックエンド（ネイティブレイヤー）**: Rust
- **全文検索**: tantivy + lindera（日本語形態素解析）
- **ベクトル検索**: ONNX Runtime（embedding生成）+ HNSW（近傍探索）
- **ローカルLLM**: llama.cpp（単一バイナリ統合）
- **対象プラットフォーム**: Windows / macOS

## 開発環境要件

- Node.js >= 20
- Rust >= 1.77
- pnpm（パッケージマネージャ）

## コマンド一覧

```bash
# 開発サーバー起動
pnpm tauri dev

# ビルド
pnpm tauri build

# フロントエンドテスト
pnpm test

# Rustテスト
cargo test --manifest-path src-tauri/Cargo.toml

# リント
pnpm lint

# フォーマット
pnpm format
```

## 開発ルール

### 全般

- CONTRIBUTING.md に記載されたTDD手法に従う
- テストコードを先に書き、失敗を確認してから実装する
- E2Eテストを必ず検討する（自動化不可の場合は手動テスト項目に追記）
- コミット前にlint・フォーマット・テストを実行する

### フロントエンド（TypeScript / React）

- 関数コンポーネント + hooks を使用する
- 型定義を先に書く
- 純粋関数を優先し、副作用を分離する
- Vitest でユニットテストを書く

### バックエンド（Rust）

- Tauriコマンドとして公開するAPIは `src-tauri/src/commands/` に配置する
- ドメインロジックは `src-tauri/src/domain/` に分離する
- `cargo test` でユニットテストを書く
- エラー型は `thiserror` で定義する

### ドキュメント

- 設計変更時は `docs/` 配下のドキュメントを更新する
- 手動E2Eテスト項目は `docs/manual-e2e-tests.md` に追記する
