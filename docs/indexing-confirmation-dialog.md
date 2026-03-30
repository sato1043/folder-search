# フォルダ選択時のインデックス作成確認ダイアログ

## 1. 背景・目的

フォルダ選択後、インデックス作成（全文検索 + ベクトル）が即座に開始される。大規模フォルダの場合、数分〜数十分かかる可能性があるが、現状はユーザーへの事前通知も中断手段もない。

### 目的

- インデックス作成に長時間要すると予想される場合、ユーザーに確認ダイアログを表示する
- ダイアログ上でインデックス作成の進捗を表示する
- 任意のタイミングでインデックス作成を中断できるようにする
- 中断時に中途半端な状態を残さない（クリーンアップ）
- 中断時にベクトルembeddingの途中結果を保存し、再開時に再利用する

### 現状の課題

| 項目 | 現状 | 課題 |
|---|---|---|
| 全文検索インデックス | 進捗通知なし、同期的に完了を待つ | UIフリーズに見える |
| ベクトルインデックス | `vector-index-progress`イベントでパーセント表示あり | 中断手段がない |
| ファイル数チェック | なし | 巨大フォルダでも無条件に開始される |
| 中断 | 手段なし | 数分〜数十分待つしかない |
| 中断時クリーンアップ | 考慮なし | 中途半端なインデックスが残るリスク |

## 2. 処理フロー

### 変更前

```
フォルダ選択 → buildIndex(同期・進捗なし) → buildVectorIndex(進捗あり)
```

### 変更後

```
フォルダ選択
  → scanFolder（軽量スキャン、メタデータのみ）
  → 閾値判定
      ├─ 閾値未満 → 従来通り即座にインデックス作成（進捗表示は改善）
      └─ 閾値以上 → 確認ダイアログ表示
                       ├─ [キャンセル] → 処理中止
                       └─ [開始] → ダイアログが進捗表示に遷移
                                    → Phase1: 全文検索インデックス構築
                                    → Phase2: ベクトルインデックス構築
                                    → 完了 → ダイアログ自動クローズ
                                    （どのフェーズでも [中断] ボタンで中断可能）
```

## 3. 軽量スキャン

### 新規Tauriコマンド: `scan_folder`

フォルダ内の対象ファイル（`.txt` + `.md`）のメタデータのみを走査する。ファイル内容は読まない。

#### 入力

| パラメータ | 型 | 説明 |
|---|---|---|
| `folder_path` | `String` | スキャン対象フォルダパス |

#### 出力: `FolderScanResult`

| フィールド | 型 | 説明 |
|---|---|---|
| `file_count` | `u64` | 対象ファイル数 |
| `total_size_bytes` | `u64` | 対象ファイルの合計サイズ |
| `max_file_size_bytes` | `u64` | 最大ファイルサイズ |
| `estimated_chunks` | `u64` | 推定チャンク数（`total_size / 400`） |
| `has_symlinks` | `bool` | シンボリックリンクの有無 |

#### タイムアウト

スキャン自体に5秒のタイムアウトを設ける。タイムアウト時はそれまでに得られた情報で `FolderScanResult` を返し、`timed_out: true` フラグを付与する。ネットワークドライブや超大規模フォルダへの対策。

#### 実装方針

`walkdir::WalkDir` でメタデータ走査する。既存の `VectorCache::scan_fingerprints` と同様の走査だが、集約統計のみを返す軽量版とする。

## 4. 確認ダイアログの表示条件

以下のいずれか1つでも該当すれば表示する。閾値は固定値とする。

| 条件 | 閾値 | 理由 |
|---|---|---|
| ファイル数が多い | >= 500 | 想定規模1,000の半分。体験的に数秒〜数十秒かかり始める |
| 合計サイズが大きい | >= 100MB | ベクトルインデックスのembedding生成が重い |
| 推定チャンク数がHNSW上限に近い | >= 40,000 | `nb_elem=50,000` の80%。精度劣化リスク |
| 巨大ファイルが含まれる | 1ファイル >= 10MB | `read_to_string`によるメモリ圧迫リスク |
| スキャンがタイムアウトした | — | フォルダが非常に大きい可能性がある |

## 5. ダイアログUI

### 状態遷移

```
[確認] → [Phase1進捗] → [Phase2進捗] → [完了]
                ↓              ↓
             [中断済み]     [中断済み]
```

### 確認状態

```
┌─────────────────────────────────────────────┐
│  インデックス作成                              │
│                                              │
│  /path/to/large-folder                       │
│  2,345 ファイル（256 MB）                     │
│                                              │
│  ⚠ ファイル数が多いため時間がかかる可能性がある  │
│                                              │
│             [キャンセル]  [開始]               │
└─────────────────────────────────────────────┘
```

警告メッセージのバリエーション:

| 条件 | メッセージ |
|---|---|
| ファイル数多 | ファイル数が多いため時間がかかる可能性がある |
| サイズ大 | 合計サイズが大きいため時間がかかる可能性がある |
| チャンク上限近 | 推定チャンク数がベクトルインデックスの上限（50,000）に近い。検索精度が低下する可能性がある |
| 巨大ファイル | 10MB以上のファイルが含まれている。メモリ使用量が増加する可能性がある |
| タイムアウト | フォルダの走査に時間がかかった。非常に大きなフォルダの可能性がある |

### Phase1進捗状態（全文検索インデックス構築）

```
┌─────────────────────────────────────────────┐
│  インデックス作成中                            │
│                                              │
│  全文検索インデックス: 834 / 2,345 ファイル     │
│  [████████░░░░░░░░░░░░] 35%                  │
│                                              │
│                    [中断]                     │
└─────────────────────────────────────────────┘
```

### Phase2進捗状態（ベクトルインデックス構築）

```
┌─────────────────────────────────────────────┐
│  インデックス作成中                            │
│                                              │
│  ✓ 全文検索インデックス: 2,345 ファイル完了     │
│                                              │
│  ベクトルインデックス: 4,200 / 12,800 チャンク  │
│  [██████░░░░░░░░░░░░░░] 32%                  │
│                                              │
│                    [中断]                     │
└─────────────────────────────────────────────┘
```

### 完了状態

```
┌─────────────────────────────────────────────┐
│  インデックス作成完了                          │
│                                              │
│  ✓ 全文検索インデックス: 2,345 ファイル         │
│  ✓ ベクトルインデックス: 12,800 チャンク        │
│                                              │
│                     [OK]                     │
└─────────────────────────────────────────────┘
```

### 中断済み状態

```
┌──────────────────────────────────────────────────┐
│  インデックス作成を中断した                          │
│                                                   │
│  ✓ 全文検索インデックス: 2,345 ファイル（完了済み）   │
│  ✗ ベクトルインデックス: 中断（4,200 / 12,800）     │
│                                                   │
│  全文検索のみで動作する。                            │
│  ベクトルインデックスは次回構築時に途中から再開する。  │
│                                                   │
│                      [OK]                         │
└──────────────────────────────────────────────────┘
```

### フロントエンド状態型

```typescript
type IndexingPhase =
  | { kind: "confirm"; scanResult: FolderScanResult }
  | { kind: "fulltext"; current: number; total: number }
  | { kind: "vector"; current: number; total: number }
  | { kind: "done"; fulltextCount: number; vectorChunks: number }
  | { kind: "cancelled"; fulltextCount: number; vectorChunks?: number };
```

## 6. 中断メカニズム

### キャンセルトークン

`Arc<AtomicBool>` を `AppState` に追加する。

```rust
pub struct AppState {
    // ... 既存フィールド
    pub cancel_token: Arc<AtomicBool>,
}
```

### 新規Tauriコマンド: `cancel_indexing`

```rust
#[tauri::command]
pub fn cancel_indexing(state: State<'_, AppState>) {
    state.cancel_token.store(true, Ordering::Relaxed);
}
```

### キャンセルチェックの挿入箇所

#### 全文検索インデックス（`index_folder`）

`infra/tantivy/mod.rs` の `index_folder` メソッドにキャンセルトークンと進捗コールバックを追加する。

```rust
pub fn index_folder_cancellable(
    &mut self,
    folder_path: &str,
    cancel_token: &AtomicBool,
    on_progress: impl Fn(u64, u64),  // (current, total)
) -> Result<u64, IndexError>
```

ファイル処理ループ内で毎回 `cancel_token.load(Ordering::Relaxed)` をチェックする。

#### ベクトルインデックス（`build_vector_index_full`）

`commands/mod.rs` のembedding生成ループ内で毎回チェックする。

```rust
for (i, chunk) in all_chunks.iter().enumerate() {
    if cancel_token.load(Ordering::Relaxed) {
        // 途中保存 → エラー返却
    }
    // ... embedding生成
}
```

### エラー型の追加

```rust
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    // ... 既存バリアント
    #[error("インデックス作成が中断された")]
    Cancelled,
}
```

### 全文検索の進捗イベント追加

現在、全文検索インデックス構築には進捗通知がない。新規イベント `fulltext-index-progress` を追加する。

```rust
app.emit("fulltext-index-progress", json!({
    "current": count,
    "total": total_files,  // scan_folderで取得済みの値をパラメータとして受け取る
}));
```

`build_index` コマンドに `total_files: u64` パラメータを追加し、フロントエンドからスキャン結果を渡す。

## 7. クリーンアップ戦略

### 原則

- 中断時に中途半端なインデックスをAppStateに格納しない
- ディスク上のゴミを残さない
- 再利用可能なデータは保存する

### Phase1（全文検索）中断時

| 対象 | 処理 |
|---|---|
| tantivy IndexWriter | `commit()` を呼ばない。writerがdropされると未commitデータは破棄される |
| インデックスディレクトリ | 中断時に `remove_dir_all` で削除する |
| AppState.engine | 格納しない（`None` のまま） |
| ファイル監視 | 開始しない |
| 前回のベクトルキャッシュ（ディスク） | そのまま保持（独立しているため影響なし） |

### Phase2（ベクトル）中断時

| 対象 | 処理 |
|---|---|
| 全文検索インデックス | Phase1完了済みのためそのまま有効。AppStateに格納済み |
| HNSWインデックス（メモリ） | AppStateに格納しない。不完全なグラフは検索品質が不安定なため使わない |
| 生成済みembedding | 処理済みファイル単位でキャッシュに途中保存する（後述） |
| ファイル監視 | Phase1で開始済み。全文検索の差分更新は継続する |

### ベクトルembeddingの途中保存

#### 目的

embedding生成は1チャンクあたり数十ms（CPU）かかる。大規模フォルダで中断した場合、途中までの生成結果を保存し、次回ビルド時に差分更新パスで未処理分のみを生成する。

#### 現状の `cache.save()` の問題

```rust
// vector_cache/mod.rs:222
file_fingerprints: Self::scan_fingerprints(folder_path),
```

`scan_fingerprints` はフォルダ内の全ファイルを列挙する。途中保存時にこれを使うと、未処理ファイルも「キャッシュ済み」と記録されてしまい、次回の `compute_diff` が「変更なし」と誤判定する。

#### 解決策: `save_with_fingerprints` メソッドの追加

```rust
pub fn save_with_fingerprints(
    &self,
    folder_path: &str,
    metas: &[ChunkMeta],
    embeddings: &[Embedding],
    fingerprints: HashMap<String, FileFingerprint>,
) -> Result<(), String>
```

中断時は処理済みファイルのフィンガープリントのみを渡す。次回 `compute_diff` は未処理ファイルを `added` として正しく検出し、差分更新パスで処理する。

#### 途中保存の実行タイミング

中断検知時（`cancel_token` がtrueになった時点）に1回だけ保存する。

```rust
if cancel_token.load(Ordering::Relaxed) {
    // 処理済みファイルのフィンガープリントを収集
    let processed_fingerprints = collect_fingerprints_for(&processed_files);
    let _ = cache.save_with_fingerprints(
        folder_path,
        &processed_metas,
        &processed_embeddings,
        processed_fingerprints,
    );
    return Err("ベクトルインデックス構築が中断された".into());
}
```

#### 処理済みファイルの追跡

ベクトルインデックスのフルビルドでは、ファイル→チャンク分割→embedding生成の順で処理する。途中保存のために「どのファイルまで処理済みか」を追跡する必要がある。

現在の実装はファイル走査→全チャンク収集→チャンクごとにembedding生成の2段階構造。チャンク単位のループ内でファイル境界を判定するために、`source_path` の変化を監視する。

```rust
let mut processed_files: HashSet<String> = HashSet::new();
let mut current_file = String::new();

for (i, chunk) in all_chunks.iter().enumerate() {
    if cancel_token.load(Ordering::Relaxed) {
        // current_fileは未完了なので processed_files に含めない
        // → 次回ビルド時に再処理される
        break;
    }

    if chunk.source_path != current_file {
        if !current_file.is_empty() {
            processed_files.insert(current_file.clone());
        }
        current_file = chunk.source_path.clone();
    }

    // embedding生成...
}
```

ファイルの最後のチャンクまで処理完了した時点でそのファイルを「処理済み」に追加する。ファイル途中で中断した場合、そのファイルは未処理として扱い、次回に再処理する。

#### キャッシュ保存のアトミック性

`embeddings.bin` → `manifest.json` の順に書き込む。`manifest.json` が存在しなければキャッシュ無効と判定されるため、`embeddings.bin` だけ書かれて中断しても安全にフォールバックする（現在の実装と同じ順序）。

## 8. 影響範囲

### Rust domain層

| ファイル | 変更 |
|---|---|
| `domain/indexer/mod.rs` | `FolderScanResult` 型追加、`IndexError::Cancelled` バリアント追加 |

### Rust infra層

| ファイル | 変更 |
|---|---|
| `infra/tantivy/mod.rs` | `index_folder_cancellable` メソッド追加（キャンセルトークン + 進捗コールバック） |
| `infra/vector_cache/mod.rs` | `save_with_fingerprints` メソッド追加 |

### Rust commands層

| ファイル | 変更 |
|---|---|
| `commands/mod.rs` | `scan_folder` コマンド追加 |
| | `cancel_indexing` コマンド追加 |
| | `AppState` に `cancel_token: Arc<AtomicBool>` 追加 |
| | `build_index` にキャンセルチェック・進捗通知・クリーンアップ追加 |
| | `build_index` に `total_files: u64` パラメータ追加 |
| | `build_vector_index_full` にキャンセルチェック・途中保存追加 |
| | `build_vector_index_incremental` にキャンセルチェック追加 |
| | `fulltext-index-progress` イベント発火追加 |
| `lib.rs` | 新コマンド登録、`AppState` 初期化に `cancel_token` 追加 |

### フロントエンド

| ファイル | 変更 |
|---|---|
| `src/types/index.ts` | `FolderScanResult`, `IndexingPhase` 型追加 |
| `src/lib/tauri.ts` | `scanFolder()`, `cancelIndexing()` 関数追加、`buildIndex` のシグネチャ変更 |
| `src/App.tsx` | `handleSelectFolder` のフロー変更（スキャン→判定→ダイアログ or 直接実行） |
| `src/components/dialog/IndexingDialog.tsx` | 新規。確認→進捗→完了→中断済みの状態遷移ダイアログ |

### テスト

| 対象 | 内容 |
|---|---|
| `infra/tantivy` | `index_folder_cancellable` のキャンセルテスト |
| `infra/vector_cache` | `save_with_fingerprints` のラウンドトリップテスト、途中保存→差分更新テスト |
| `commands` | `scan_folder` の正常系テスト |
| フロントエンド | `IndexingDialog` の状態遷移テスト（Vitest） |
| E2E | 手動テスト項目に追加（大規模フォルダでのダイアログ表示・中断・再開） |

### ドキュメント

| ファイル | 変更 |
|---|---|
| `docs/architecture.md` | IPCコマンド数の更新、新規イベントの追記 |
| `docs/manual-e2e-tests.md` | 確認ダイアログ・中断・再開の手動テスト項目追加 |

## 9. エッジケース

| ケース | 対処 |
|---|---|
| 対象ファイル0件 | 確認ダイアログではなく「対象ファイルが見つからない」旨を通知する |
| スキャンタイムアウト（5秒） | それまでに得た情報で `FolderScanResult` を返す。`timed_out: true` 付き |
| 深い階層やシンボリックリンク循環 | WalkDirの循環検出に依存する。スキャンタイムアウトでも捕捉可能 |
| ネットワークドライブ | メタデータ取得が遅延する。スキャンタイムアウトで対応 |
| 同一フォルダの再選択（インデックス既存） | 全文検索はフルリビルド（既存実装の動作）。ベクトルは差分更新パスが適用される |
| 中断後に同じフォルダを再選択 | 通常フローで再度スキャン→判定。ベクトルの途中保存があれば差分更新で続行 |
| 閾値未満のフォルダ | 確認ダイアログを表示せず従来通り即座に実行する。ただし進捗イベントは発火する |
| Phase1中断後のベクトルキャッシュ | 前回の成功時のキャッシュがディスクに残っている。再利用可能 |
