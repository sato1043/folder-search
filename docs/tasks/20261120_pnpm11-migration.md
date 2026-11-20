---
task: 20261120
status: frozen
features: []
requirements: []
user-reach: low
dev-reach: high
irreversible: low
hazard: on-release
---

# pnpm 11 以降へのパッケージマネージャ設定の移行

## 目的

pnpm 11 で `pnpm install`・`pnpm test`・`pnpm lint` が失敗する状態を解き、
手元とリリース CI が同じ pnpm の版で同じ結果を出すようにする。
次のリリースを出せる状態へ戻すのが動機である。

## 目標

- pnpm 11 で依存のビルド可否を新しい設定（`allowBuilds`）で宣言し、install を通す
- pnpm の版をリポジトリで 1 つに固定し、手元とリリース CI の食い違いを無くす
- 開発環境要件の記載（Node.js・pnpm の版）を実態へ揃える

## 非目標

- `pnpm/action-setup` の major 更新（v4 → v6）・`pnpm/setup` への置き換え
- pnpm 12 への移行（版の選択は概要設計と裁定記録で扱う）
- 依存パッケージの更新・`pnpm-lock.yaml` の再解決
- E2E テストの CI 統合（`docs/TODO.md` の既存項目）
- アプリのバージョン番号の変更

## 問題

- pnpm 11 は `package.json` の `"pnpm"` フィールドを読まない。`onlyBuiltDependencies` は
  v11 で削除され `allowBuilds` に置き換わった
  （<https://github.com/pnpm/pnpm/releases/tag/v11.0.0>・<https://pnpm.io/settings/build>）
- pnpm 11 は `strictDepBuilds` の既定が `true` で、可否を宣言していないビルドがあると
  `ERR_PNPM_IGNORED_BUILDS` で exit 1 になる。`pnpm test` / `pnpm lint` は実行前の依存
  チェックで install を走らせるので、同じ理由で落ちる
- リリース CI は `pnpm/action-setup@v4` に `version: latest` を渡しており、
  今は pnpm 12 系が入る。手元（11.25.0）と major が食い違う。CI の失敗は未確認（仮説）
- `pnpm/action-setup@v4` は `version` 入力と `package.json` の `packageManager` が
  食い違うと `Multiple versions of pnpm specified` で止まる
  （同 action の `src/install-pnpm/run.ts`、タグ v4）。
  `packageManager` で固定するなら `version` を外す必要がある
- pnpm 11 は Node.js 22 以上を要する（v11.0.0 リリースノート）。README と
  `.claude/CLAUDE.md` は `Node.js >= 20` のまま。`.node-version` は `22` で、
  実態は合っている

## 到達基準

- [x] pnpm 11.25.0（手元）で `pnpm install`・`pnpm test`・`pnpm lint` が exit 0 になる。
      警告・skip の件数を記録する
- [x] 固定した版（`packageManager`）で `pnpm install --frozen-lockfile`・`pnpm test`・
      `pnpm lint` が exit 0 になる
- [x] `pnpm-lock.yaml` の差分が 0 行である
- [x] `"pnpm" field in package.json is no longer read` の警告が出ない
- [x] esbuild の postinstall が走り、edgedriver / geckodriver の install スクリプトが
      走らない（従来の `onlyBuiltDependencies: [esbuild]` と同じ挙動）
- [x] `release.yml` の pnpm の版が `packageManager` から決まる。CI 上での実行は
      タグ起動でしか走らないため未検証として記録する
- [x] Node.js・pnpm の版の記載が README・`.claude/CLAUDE.md` で実態と一致する
- [x] CP6 第三者視点レビュー: **不要を提案する**（変更は設定 3 ファイルと記載 2 箇所に
      閉じ、下の実測で挙動を確かめ済みのため）。裁定はユーザーが下す
      → 2026-11-20 ユーザー裁定: 不要

## 概要設計

### 変更の一覧

| ファイル | 変更 |
|---|---|
| `pnpm-workspace.yaml` | 新規追跡。`allowBuilds` で 3 パッケージの可否を宣言する |
| `package.json` | `"pnpm"` フィールドを削除し、`"packageManager": "pnpm@11.28.13"` を足す |
| `.github/workflows/release.yml` | `pnpm/action-setup@v4` の `version: latest` を削除する |
| `README.md` | 開発時要件を `Node.js >= 22`・`pnpm 11` へ直す |
| `.claude/CLAUDE.md` | 開発環境要件を同じく直す |

### `pnpm-workspace.yaml`

```yaml
allowBuilds:
  esbuild: true
  # WebdriverIO が推移的に引くブラウザドライバ。install スクリプトはドライバの
  # バイナリを取得するだけで、E2E は tauri-driver を使うため要らない
  edgedriver: false
  geckodriver: false
```

- esbuild（0.25.12・0.27.4）: postinstall `node install.js` を許可する。
  従来の許可リストと同じ
- edgedriver（5.6.1）/ geckodriver（4.2.1）: `@wdio/*` 8.46.0 が推移的に引く。install
  スクリプトは msedgedriver / geckodriver のバイナリを OS の一時ディレクトリへ取得する。
  `wdio.conf.ts` は `localhost:4444` の tauri-driver へ接続し、
  ブラウザのドライバを起動しない。
  従来の許可リストにも無く、ビルドされていなかった。`false` は従来の挙動を保つ
- 単一パッケージのリポジトリでは `packages:` を書かなくてよい
  （<https://pnpm.io/pnpm-workspace_yaml>）

### 版の固定

`package.json` の `packageManager` に `pnpm@11.28.13` を書く
（11 系の最新、2026-11-17 公開）。

- 手元: pnpm 11 は `packageManager` と違う版で起動されると、既定（`pmOnFail: download`）で
  宣言の版を取得して切り替える（<https://pnpm.io/package_json>・
  <https://pnpm.io/settings/cli>）。開発者ごとの版の差は自動で吸収される
- CI: `pnpm/action-setup@v4` は `version` を省くと `packageManager` を読む。v4 は
  `devEngines.packageManager` を読まないので、`packageManager` を使う
- 11 と 12 の比較（2026-11-20 実測、下の「事前の実測」）

| 観点 | 11.28.13 | 12.22.0 |
|---|---|---|
| `pnpm-lock.yaml` の差分 | 0 行 | +167 行（先頭へ pnpm 本体の解決情報の文書が入る） |
| install / test / lint | exit 0 | exit 0 |
| 手元の版との関係 | 同じ major | major が上がる |
| 保守 | 11 系にも 11 月だけで 4 回の patch（11.28.13 は security fix） | 現行の major |
| 実装形態 | Node.js 上で動く | native 実行ファイル |

推奨は 11.28.13 である。lockfile を変えず、手元と同じ major で、影響範囲が最小になる。
pnpm 11 の保守期限は公式に見当たらない（pnpm.io・GitHub releases を検分）。12 への移行は
lockfile の書き換えを伴う別の変更として切り出せる。

### 事前の実測（2026-11-20）

リポジトリの `package.json`（`"pnpm"` を除いたもの）・`pnpm-lock.yaml`・上の
`pnpm-workspace.yaml` をスクラッチ領域へ写して測った。リポジトリは変えていない。

| 版 | install | lockfile 差分 | 警告 | test | lint |
|---|---|---|---|---|---|
| 11.25.0（固定なし） | exit 0 | 0 行 | 0 件 | 12 files / 66 tests 合格 | exit 0 |
| 11.28.13（`packageManager`） | exit 0 | 0 行 | 0 件 | 12 files / 66 tests 合格 | exit 0 |
| 12.22.0（`packageManager`） | exit 0 | +167 行 | 0 件 | 12 files / 66 tests 合格 | exit 0 |

- 11.25.0 の install ログでは esbuild 2 版の postinstall が `Done` で終わり、ドライバの
  install スクリプトは走っていない
- install は `--frozen-lockfile` で測った。12.22.0 は frozen でも lockfile へ書き込んだ

### 付随ドキュメント

- `docs/CONTRIBUTING.md`・`docs/RELEASING.md` は pnpm の版を書いておらず、コマンドも
  変わらないので直さない
- CHANGELOG は記載しない。プロジェクト規約は「ユーザー向けの変更」を対象とし、本変更は
  開発環境だけに効く

## 実装計画

1. `git switch -c task/20261120-pnpm11-migration`
2. `pnpm-workspace.yaml`・`package.json`・`release.yml` を直す
3. 手元で検証する（到達基準の上 5 項目）。`node_modules` は作り直す
4. README・`.claude/CLAUDE.md` を直す
5. 作業書の検証・作業記録を埋めて `frozen` を立てる
6. コミット（設定とドキュメントを分ける）、push、リーン本文の PR を作る（レビューアなし）

## 工数概算

| ステップ | 見積もり | 幅の主因 |
|---|---|---|
| 1–2 設定の変更 | 10–20 分 | なし |
| 3 検証 | 15–30 分 | `node_modules` の作り直しで起きる想定外 |
| 4–5 ドキュメント | 15–30 分 | なし |
| 6 コミット・PR | 10–20 分 | なし |
| 合計 | 50–100 分 | 参照クラス: 設定ファイル数本の移行 |

実績: 想定外の手戻りは 0 回。実働の時間はエージェントが経過時間を観測できないため
記録していない。

## 検証

### 概要

手元（Windows 11・Node v22.23.2）で `node_modules` を消して作り直し、固定版 11.28.13 と
手元の 11.25.0（`pnpm with current`）の 2 版で install・test・lint を回した。
リリース CI はタグ起動でしか走らないため未検証である。E2E は対象外で、実行していない。

### 検証項目（チェックリスト）

- [x] 11.28.13: `pnpm install --frozen-lockfile` exit 0、警告 0 行
- [x] 11.28.13: `pnpm test` exit 0、12 files / 66 tests 合格、skip 0 件
- [x] 11.28.13: `pnpm lint` exit 0、警告 0 行
- [x] 11.28.13: CI と同じ `pnpm install`（frozen なし）exit 0
- [x] 11.25.0: install・test・lint とも exit 0、警告 0 行、66 tests 合格
- [x] `pnpm-lock.yaml` の差分 0 行（`git diff --numstat`）
- [x] esbuild 0.25.12 / 0.27.4 の postinstall が `Done` で終わり、2 版とも
      `transformSync` で TS を変換できる
- [x] install ログに edgedriver / geckodriver の行が 0 件
- [ ] リリース CI での `pnpm install`（未検証。タグ起動でしか走らない）

### 検証手順

#### 自動

```bash
rm -rf node_modules
pnpm install --frozen-lockfile
pnpm test
pnpm lint
pnpm with current install --frozen-lockfile
git diff --numstat -- pnpm-lock.yaml
```

#### 実機

なし

## 作業記録

- 11.25.0 の 2 回目の install では esbuild の postinstall の行が出なかった。11.28.13 の
  install で作った build 結果を side effects cache から再利用したと解釈している（推論）。
  esbuild の動作は 2 版とも `transformSync` で確かめた
- `pnpm/action-setup@v4` で pnpm 11 が入ることは、action のソースで `packageManager` を
  読む分岐を確かめただけで、CI 上では動かしていない
- 2026-11-20 追記: Rust 側の健全性も確かめた（本変更は Rust に触れない）。LLVM と CMake を
  入れた後、`cargo test` は 185 件合格・失敗 0・ignored 0（ユーザーが実行）。
  `cargo clippy -- -W clippy::all` は exit 0・error 0・warning 5 件で、いずれも既存の
  コード（`commands/mod.rs` 3 件・`infra/hnsw/mod.rs` 1 件・`infra/llama/mod.rs` 1 件）

## 機能への反映

なし（機能を生まない開発環境の変更）

## 裁定記録

- **作業書の置き場所**（2026-11-20・ユーザー）: `docs/tasks/` を新設する
- **作業 ID**（2026-11-20・ユーザー）: 日付 `20261120` を使う
- **固定する pnpm の版**（2026-11-20・ユーザー）: 11.28.13
- **CP6 第三者視点レビュー**（2026-11-20・ユーザー）: 不要

## stakeholder 未裁定の残課題

- リリース CI での実行確認の方法（タグを打たない限り走らない。次のリリース時に確かめるか、
  `workflow_dispatch` の追加を別に起こすか）
- `pnpm/action-setup@v4` の更新（v6 が最新）。v4 で pnpm 11 が入ることは CI 上では未検証
