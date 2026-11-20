---
task: 20261120-2
status: frozen
features: []
requirements: []
user-reach: low
dev-reach: low
irreversible: low
hazard: none
---

# clippy 警告 5 件と Cargo.lock の版ずれの解消

## 目的

`cargo clippy -- -W clippy::all` を警告 0 件に戻し、新しい警告が埋もれない状態にする。
リリースのたびに `Cargo.lock` の本体の版が取り残される手順の抜けを塞ぐ。

## 目標

- clippy の警告 5 件を、振る舞いを変えずに解消する
- `Cargo.lock` の `folder-search` の版を `Cargo.toml`（0.4.0）へ追随させる
- `docs/RELEASING.md` の版の更新手順に `Cargo.lock` を加える

## 非目標

- アプリのバージョン番号の変更
- `RELEASING.md` の他の古い記述（`develop` ブランチ・プラットフォーム表）の見直し
- `src-tauri/gen/schemas/windows-schema.json`（Windows のビルドで生成された未追跡
  ファイル）の扱い
- 依存クレートの更新

## 問題

- 2026-11-20 の実測（rustc 1.97.0）で clippy の警告は 5 件で、すべて既存のコードにある
  - `commands/mod.rs:34` `LoadedLlmConfig.context_length` が書かれるだけで読まれない。
    推論に使う値は `LlamaEngine` が自分のフィールドとして持っている
    （`infra/llama/mod.rs` の `self.context_length`）
  - `commands/mod.rs:411` 空の `else {}`（729fcca で入った）
  - `commands/mod.rs:526`・`infra/llama/mod.rs:140` `zip` の引数への明示的な
    `.into_iter()`
  - `infra/hnsw/mod.rs:26` 引数なしの `HnswVectorIndex::new()` に対する `Default` の欠如
- `Cargo.lock` の本体の版は 0.3.3 のまま。v0.4.0 のリリース（c4faf45）は
  `RELEASING.md` の手順どおり 3 ファイルだけを更新し、lock を含めなかった。v0.3.1〜0.3.3
  のリリースコミットは lock を含んでおり、dd7282d（2026-03-30）でも同じ版ずれを後追いで
  直している。手順の記載が無いため、2 回起きている

## 到達基準

- [x] `cargo clippy --manifest-path src-tauri/Cargo.toml -- -W clippy::all` が
      exit 0・warning 0 件
- [x] `cargo test --manifest-path src-tauri/Cargo.toml` が全件合格（変更前 185 件）・
      失敗 0・ignored 0 で、件数が変わらない
      （着手後に改める: `Default` のテストを 1 件足したので、185 + 1 = 186 件）
- [x] `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` が exit 0
- [x] `cargo build --locked` 相当（`cargo check --locked`）が lock を書き換えずに通る
- [x] `Cargo.lock` の差分が `folder-search` の版の 1 行だけである
- [x] `RELEASING.md` の手順 2・3 が `Cargo.lock` を含む
- [x] CP6 第三者視点レビュー: **不要を提案する**（機械的な修正 5 件と記載 1 箇所で、
      clippy・test で確かめられるため）。裁定はユーザーが下す
      → 2026-11-20 ユーザー裁定: 不要

## 概要設計

| 箇所 | 修正 | 振る舞い |
|---|---|---|
| `LoadedLlmConfig.context_length` | フィールドと代入を削除する | 変わらない（読み手が 0 件） |
| `commands/mod.rs:411` | 空の `else {}` を削除する | 変わらない |
| `commands/mod.rs:526` | `.zip(cached.embeddings)` | 変わらない |
| `infra/llama/mod.rs:140` | `.zip(tokens)` | 変わらない |
| `HnswVectorIndex` | `impl Default`（`Self::new()` へ委譲）を足す | 変わらない（追加のみ） |
| `Cargo.lock` | ビルドが書き換えた 1 行（0.3.3 → 0.4.0）をそのまま採る | ― |
| `RELEASING.md` | 手順 2 の表と手順 3 の `git add` に `Cargo.lock` を足し、更新コマンド `cargo update --workspace --manifest-path src-tauri/Cargo.toml` を書く | ― |

- `context_length` は `#[allow(dead_code)]` で黙らせず削除する。同じ値を `LlamaEngine` が
  持っており、二重に持つ理由が無い。将来読む必要が出たら、エンジンから取得する
- `Default` は clippy の `new_without_default` の処方どおり足す。`#[allow]` で黙らせる
  より、慣用の形に揃う
- `cargo update --workspace` はワークスペースのパッケージだけを更新し、依存は動かさない
  （`cargo update --help`: "Only update the workspace packages"）

## 実装計画

1. Rust の 5 箇所を直す（1 コミット）
2. `Cargo.lock` と `RELEASING.md` を直す（コードとは別のコミット。lock とドキュメントを
   分けるかは差分の量で決める）
3. clippy・test・fmt・`check --locked` で検証する
4. 作業書の検証の節を埋め、`frozen` を立てる
5. main へのローカル squash と push のコマンドを提示する（実行はユーザー）

## 工数概算

| ステップ | 見積もり | 幅の主因 |
|---|---|---|
| 1 Rust の修正 | 10–20 分 | なし |
| 2 lock・手順 | 10–15 分 | なし |
| 3 検証 | 15–30 分 | clippy と test のビルド時間（初回 3〜4 分） |
| 4–5 記録・PR | 10–20 分 | なし |
| 合計 | 45–85 分 | 参照クラス: lint 警告の機械的な解消 |

実績: 手戻りは 1 回（fmt の崩れが想定外に 3 ファイルへ広がっていたため、整形を別の
コミットへ分け直した）。実働の時間は、エージェントが経過時間を観測できないため記録して
いない。

## 検証

### 概要

Windows 11・rustc 1.97.0 で、最終コミットの状態に対して clippy・test・fmt・
`check --locked` を回した。手順書に書いた lock の更新コマンドは、lock を HEAD の版
（0.3.3）へ戻してから実行し、差分が 1 行になることを確かめた。macOS・Linux では
確かめていない。

### 検証項目（チェックリスト）

- [x] clippy: exit 0、warning 0 件、error 0 件
- [x] test: 186 件合格、失敗 0・ignored 0
- [x] 追加したテスト `test_default_builds_searchable_index`: `Default` の実装が無い状態で
      `E0599` によりコンパイルが失敗し、実装を足すと合格する
- [x] fmt `--check`: exit 0
- [x] `check --locked`: exit 0、lock の差分は 0 行
- [x] `cargo update --workspace`: 変更は `folder-search` の 0.3.3 → 0.4.0 の 1 行だけ。
      依存 216 件は動かない
- [x] lint 修正のコミットの差分は、概要設計の 5 件だけである

### 検証手順

#### 自動

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml -- -W clippy::all
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo update --workspace --manifest-path src-tauri/Cargo.toml
```

#### 実機

なし

## 作業記録

- `cargo fmt --check` は着手前から失敗していた。対象は `commands/mod.rs`・
  `vector_cache/mod.rs`・`lib.rs` の 3 ファイルで、到達基準に fmt の合格を入れていたため
  今回直した。整形だけのコミット（`style`）を先に置き、その上に lint の修正を載せて、
  2 種の差分を分けた
- 最初の確認では fmt の出力を先頭 20 行で切っており、崩れを 2 件と数え違えた。
  `cargo fmt` を実際に当てて、3 ファイルに広がっていることが分かった
- `cargo build` の際に `src-tauri/gen/schemas/windows-schema.json` が生成される。
  非目標なので追跡していない

## 機能への反映

なし（振る舞いを変えない保守）

## 裁定記録

- **作業 ID**（2026-11-20・ユーザー）: `20261120-2`。日付の作業 ID が同じ日の作業書
  `20261120_pnpm11-migration` と重なるため、枝番を付ける
- **CP6 第三者視点レビュー**（2026-11-20・ユーザー）: 不要
- **main への取り込み方**（2026-11-20・ユーザー）: PR を作らず、ローカルで main へ
  squash する

## stakeholder 未裁定の残課題

- `docs/CONTRIBUTING.md` §4 のブランチ戦略（develop・`feature/*`・develop へは PR）が、
  v0.3.1 以降の運用（main へ直接積む・ローカル squash）と食い違っている
- `src-tauri/gen/schemas/windows-schema.json` を追跡するか（`linux-schema.json` 等は
  追跡下にある）
