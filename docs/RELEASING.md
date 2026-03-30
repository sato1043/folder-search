# リリース手順

## バージョン管理の方針

バージョン番号はリリース時にのみ更新する。通常の開発中はバージョンを変更しない。

バージョニングは [Semantic Versioning](https://semver.org/lang/ja/) に従う。

- **メジャー（X.0.0）**: 破壊的変更
- **マイナー（0.X.0）**: 機能追加（後方互換）
- **パッチ（0.0.X）**: バグ修正

上位バージョンをインクリメントしたとき、下位バージョンは0にリセットする（例: `0.1.3` → マイナー → `0.2.0`）。

### バンプレベルの判断方法

開発中の変更は `CHANGELOG.md` の `[Unreleased]` セクションに記録していく。リリース時に `[Unreleased]` の内容を確認し、最も高いレベルのバンプを適用する。

| [Unreleased] の内容 | バンプレベル | 例 |
|---|---|---|
| Fixed のみ | パッチ | `0.1.0` → `0.1.1` |
| Added を含む | マイナー | `0.1.1` → `0.2.0` |
| 破壊的変更を含む | メジャー | `0.2.0` → `1.0.0` |

バグ修正と機能追加が混在するリリースではマイナーバンプのみでよい。最も高いレベルが優先される。

## リリース前の確認事項

- [ ] `pnpm test` が通る
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` が通る
- [ ] `pnpm lint` でエラーがない
- [ ] `CHANGELOG.md` にリリース内容を記載した
- [ ] develop ブランチの内容が main にマージ済み

## 手順

### 1. CHANGELOG.md の更新

`[Unreleased]` セクションがある場合、バージョン番号と日付に置き換える。

```markdown
## [0.2.0] - 2026-04-15
```

### 2. バージョン番号の更新

以下の3ファイルのバージョンを揃えて更新する。

| ファイル | フィールド |
|---|---|
| `package.json` | `"version"` |
| `src-tauri/Cargo.toml` | `version` |
| `src-tauri/tauri.conf.json` | `"version"` |

```bash
# 例: 0.2.0 にバンプする場合
# package.json:       "version": "0.2.0"
# Cargo.toml:         version = "0.2.0"
# tauri.conf.json:    "version": "0.2.0"
```

### 3. コミット・タグ作成・プッシュ

```bash
# バージョン更新をコミット
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json CHANGELOG.md
git commit -m "chore: release v0.2.0"

# タグを作成
git tag v0.2.0

# プッシュ（コミットとタグを同時に）
git push origin main --tags
```

### 4. GitHub Actions による自動ビルド

タグのプッシュにより `.github/workflows/release.yml` が起動する。

以下の3プラットフォームで並行ビルドが実行される。

| プラットフォーム | ランナー | 成果物 |
|---|---|---|
| Windows | windows-latest | `.msi`, `.exe` |
| macOS (Intel) | macos-14 | `.dmg` |
| macOS (Apple Silicon) | macos-latest | `.dmg` |

ビルド完了後、**ドラフトリリース**としてGitHub Releasesに成果物がアップロードされる。

### 5. ドラフトリリースの確認・公開

1. GitHub の Releases ページを開く
2. ドラフトリリースの成果物を確認する
3. リリースノートを必要に応じて編集する
4. 「Publish release」で公開する

## トラブルシューティング

### ビルドがタイムアウトする

LTO有効のためビルドに時間がかかる。ワークフローのタイムアウトは120分に設定済み。それでも超過する場合は `release.yml` の `timeout-minutes` を延長する。

### 特定のプラットフォームだけ失敗する

`fail-fast: false` を設定しているため、1つのプラットフォームが失敗しても他のビルドは継続する。失敗したプラットフォームのログを確認し、修正後にタグを削除して再作成する。

```bash
# タグを削除して再作成する場合
git tag -d v0.2.0
git push origin :refs/tags/v0.2.0
git tag v0.2.0
git push origin v0.2.0
```
