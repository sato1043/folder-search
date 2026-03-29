# コントリビューションガイド

## 1. 開発手法

### 1.1 テスト駆動開発（TDD）

本プロジェクトはTDD（テスト駆動開発）を採用する。全ての機能実装は以下のサイクルに従う。

```
1. Red:   テストコードを書く → 実行して失敗を確認する
2. Green: テストを通す最小限の実装を書く
3. Refactor: コードを改善する（テストが通り続けることを確認する）
```

#### 手順の詳細

1. **テストを先に書く**
   - 実装する機能の期待する入力と出力を明確にする
   - テストケースとして記述する
   - テストを実行し、失敗することを確認する（Red）

2. **最小限の実装を書く**
   - テストが通る最小限のコードを書く
   - この段階では完璧なコードを目指さない（Green）

3. **リファクタリングする**
   - テストが通ることを確認しながらコードを改善する
   - 重複の除去、命名の改善、構造の整理を行う（Refactor）

#### テストの種類

| テスト種別 | ツール | 対象 | 実行コマンド |
|---|---|---|---|
| フロントエンド ユニットテスト | Vitest | React コンポーネント、hooks、ユーティリティ | `pnpm test` |
| Rust ユニットテスト | cargo test | ドメインロジック、インフラ実装 | `cargo test --manifest-path src-tauri/Cargo.toml` |
| E2Eテスト（自動） | WebdriverIO | アプリ全体の統合テスト | `pnpm tauri build --debug && pnpm test:e2e` |
| E2Eテスト（手動） | — | 自動化困難なテスト | `docs/manual-e2e-tests.md` 参照 |

### 1.2 E2Eテスト方針

#### 自動E2Eテスト

- アプリケーション全体の統合テストを自動化する
- Tauri対応のE2Eテストフレームワーク（Playwright / WebdriverIO）を使用する
- Phase 1の基盤構築でE2Eテスト環境を整備する
- 新機能の追加時にE2Eテストを必ず検討する

#### 手動E2Eテスト

- 自動化が困難または非効率なテストケースは手動テスト項目とする
- `docs/manual-e2e-tests.md` に項目を追記する
- 手動テスト項目には以下を記載する:
  - テストID
  - テスト名
  - 前提条件
  - 手順
  - 期待結果
  - 対応Phase

#### E2Eテスト自動化の判断基準

自動化する:
- 繰り返し実行する回帰テスト
- データ入力→結果表示の基本フロー
- エラーケースの表示確認

手動に残す:
- プラットフォーム固有のUI挙動確認
- パフォーマンスの主観的評価
- 外部アプリ連携（ファイルを外部エディタで開く等）

#### MLモデルのキャッシュ方針

ベクトル検索機能はembeddingモデル（`intfloat/multilingual-e5-small`、約470MB）を必要とする。

**E2Eテスト時:**
- 初回実行時にHuggingFaceからモデルを自動ダウンロードする
- ダウンロード済みのモデルはローカルにキャッシュされ、2回目以降は再利用する
- キャッシュ場所: `{app_data}/models/`（開発時は `src-tauri/target/debug/models/` 付近）

**CI環境:**
- CIのキャッシュ機構（GitHub Actions の `actions/cache` 等）を使い、モデルファイルをキャッシュする
- キャッシュキー例: `embedding-model-multilingual-e5-small-v1`
- キャッシュ対象: `model.onnx` と `tokenizer.json`
- キャッシュミス時は初回ダウンロードが発生する（CI実行時間が数分延びる）

**ローカル開発時:**
- 一度ダウンロードしたモデルは `models/` ディレクトリにキャッシュされる
- `models/` ディレクトリは `.gitignore` に含まれる（リポジトリにコミットしない）

## 2. コーディング規約

### 2.1 フロントエンド（TypeScript / React）

#### 型定義

- 型定義を先に書く
- `any` の使用を禁止する
- APIレスポンスの型は `src/types/` に集約する

#### コンポーネント

- 関数コンポーネント + hooks を使用する
- 1ファイル1コンポーネントを基本とする
- props の型を明示的に定義する

#### 関数

- 純粋関数を優先する
- 副作用は hooks に集約する
- Tauri IPC呼び出しは `src/lib/tauri.ts` 経由で行う

#### スタイル

- ESLint + Prettier の設定に従う
- `pnpm lint` と `pnpm format` を実行してからコミットする

### 2.2 バックエンド（Rust）

#### モジュール構成

- `commands/`: Tauri IPC コマンド。フロントエンドとの境界
- `domain/`: ドメインロジック。外部ライブラリに直接依存しない
- `infra/`: 外部ライブラリの具体的な実装。domain層のトレイトを実装する
- `config/`: 設定管理

#### エラーハンドリング

- `thiserror` でエラー型を定義する
- `Result<T, E>` を返す。`unwrap()` や `expect()` は使用しない（テストコード内を除く）
- Tauriコマンドのエラーはフロントエンドが表示可能な形式にシリアライズする

#### スタイル

- `cargo clippy` と `cargo fmt` の設定に従う
- `clippy::all` と `clippy::pedantic` を有効にする

## 3. ドキュメント運用

### docs/features.md — 機能ロードマップ

機能セットの全容を示すスナップショット。`[x]` は実装済み、`[ ]` は計画中の未実装を表す。

- 新機能を追加したら `[x]` に更新する
- 新しい機能計画は `[ ]` として追記する
- 作業手順やベンチマーク目標などの詳細は書かない

### docs/TODO.md — 実装タスク

直近で着手予定の実装タスク。設計メモ・実装検討を含む。

- features.md の未実装項目のうち、具体化したものを記載する
- 設計方針、検討事項、実装上の注意点を詳しく書く
- 実装完了後は項目を削除し、features.md の対応項目を `[x]` に更新する

### docs/CHANGELOG.md

ユーザー向けの変更履歴。Keep a Changelog形式。

- 開発中の変更は `[Unreleased]` セクションに記録する
- リリース時にバージョン番号と日付に置き換える
- 詳細は [RELEASING.md](RELEASING.md) を参照

## 4. ブランチ戦略


```
main        ← リリース用。直接コミットしない
  └── develop  ← 開発用メインブランチ
        └── feature/*  ← 機能ブランチ
        └── fix/*      ← バグ修正ブランチ
```

- 機能ブランチは `feature/{phase}-{概要}` の命名とする（例: `feature/p2-fulltext-search`）
- develop へのマージはPR経由で行う
- main へのマージはリリース時に行う

## 5. コミットメッセージ

[Conventional Commits](https://www.conventionalcommits.org/) に従う。

```
<type>(<scope>): <description>

[optional body]
```

type:
- `feat`: 新機能
- `fix`: バグ修正
- `test`: テストの追加・修正
- `docs`: ドキュメント
- `refactor`: リファクタリング
- `chore`: ビルド、CI、依存関係の更新

scope:
- `frontend`: フロントエンド
- `rust`: Rustネイティブレイヤー
- `search`: 検索機能
- `llm`: LLM関連
- `e2e`: E2Eテスト

## 6. PRチェックリスト

PRを作成する前に以下を確認する:

- [ ] テストコードを書いた（TDD: Red → Green → Refactor）
- [ ] `pnpm test` が通る
- [ ] `cargo test` が通る
- [ ] `pnpm lint` でエラーがない
- [ ] `cargo clippy` で警告がない
- [ ] E2Eテストを検討した（自動化 or 手動テスト項目に追記）
- [ ] 必要に応じてドキュメントを更新した
- [ ] CHANGELOG.md にユーザー向けの変更内容を記載した
- [ ] バージョン番号を変更していない（バージョン更新はリリース時のみ。docs/RELEASING.md 参照）
