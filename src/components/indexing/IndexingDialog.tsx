import { useCallback } from "react";
import type { IndexingPhase, FolderScanResult } from "../../types";

type Props = {
  phase: IndexingPhase;
  onStart: () => void;
  onCancel: () => void;
  onClose: () => void;
};

/** ファイルサイズを人間が読みやすい形式に変換する */
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** スキャン結果から警告メッセージを生成する */
function getWarnings(scan: FolderScanResult): string[] {
  const warnings: string[] = [];
  if (scan.timed_out) {
    warnings.push("フォルダの走査に時間がかかった。非常に大きなフォルダの可能性がある");
  }
  if (scan.file_count >= 500) {
    warnings.push("ファイル数が多いため時間がかかる可能性がある");
  }
  if (scan.total_size_bytes >= 100 * 1024 * 1024) {
    warnings.push("合計サイズが大きいため時間がかかる可能性がある");
  }
  if (scan.estimated_chunks >= 40000) {
    warnings.push(
      "推定チャンク数がベクトルインデックスの上限（50,000）に近い。検索精度が低下する可能性がある",
    );
  }
  if (scan.max_file_size_bytes >= 10 * 1024 * 1024) {
    warnings.push("10MB以上のファイルが含まれている。メモリ使用量が増加する可能性がある");
  }
  return warnings;
}

function ProgressBar({ current, total }: { current: number; total: number }) {
  const pct = total > 0 ? Math.round((current / total) * 100) : 0;
  return (
    <div className="indexing-progress-bar">
      <div className="indexing-progress-fill" style={{ width: `${pct}%` }} />
      <span className="indexing-progress-text">{pct}%</span>
    </div>
  );
}

export function IndexingDialog({ phase, onStart, onCancel, onClose }: Props) {
  const handleOverlayClick = useCallback(() => {
    if (phase.kind === "done" || phase.kind === "cancelled") {
      onClose();
    }
  }, [phase.kind, onClose]);

  return (
    <div className="settings-overlay" onClick={handleOverlayClick}>
      <div className="indexing-dialog" onClick={(e) => e.stopPropagation()}>
        {phase.kind === "confirm" && (
          <ConfirmView scan={phase.scanResult} onStart={onStart} onCancel={onCancel} />
        )}
        {phase.kind === "fulltext" && (
          <FulltextProgressView current={phase.current} total={phase.total} onCancel={onCancel} />
        )}
        {phase.kind === "vector" && (
          <VectorProgressView current={phase.current} total={phase.total} onCancel={onCancel} />
        )}
        {phase.kind === "done" && (
          <DoneView
            fulltextCount={phase.fulltextCount}
            vectorChunks={phase.vectorChunks}
            onClose={onClose}
          />
        )}
        {phase.kind === "cancelled" && (
          <CancelledView
            fulltextCount={phase.fulltextCount}
            vectorChunks={phase.vectorChunks}
            onClose={onClose}
          />
        )}
      </div>
    </div>
  );
}

function ConfirmView({
  scan,
  onStart,
  onCancel,
}: {
  scan: FolderScanResult;
  onStart: () => void;
  onCancel: () => void;
}) {
  const warnings = getWarnings(scan);
  return (
    <>
      <h3>インデックス作成</h3>
      <div className="indexing-info">
        <p>
          {scan.file_count.toLocaleString()} ファイル（{formatBytes(scan.total_size_bytes)}）
        </p>
      </div>
      {warnings.length > 0 && (
        <div className="indexing-warnings">
          {warnings.map((w, i) => (
            <p key={i} className="indexing-warning">
              ⚠ {w}
            </p>
          ))}
        </div>
      )}
      <div className="indexing-actions">
        <button onClick={onCancel}>キャンセル</button>
        <button className="primary" onClick={onStart}>
          開始
        </button>
      </div>
    </>
  );
}

function FulltextProgressView({
  current,
  total,
  onCancel,
}: {
  current: number;
  total: number;
  onCancel: () => void;
}) {
  return (
    <>
      <h3>インデックス作成中</h3>
      <div className="indexing-info">
        <p>
          全文検索インデックス: {current.toLocaleString()} / {total.toLocaleString()} ファイル
        </p>
        <ProgressBar current={current} total={total} />
      </div>
      <div className="indexing-actions">
        <button onClick={onCancel}>中断</button>
      </div>
    </>
  );
}

function VectorProgressView({
  current,
  total,
  onCancel,
}: {
  current: number;
  total: number;
  onCancel: () => void;
}) {
  return (
    <>
      <h3>インデックス作成中</h3>
      <div className="indexing-info">
        <p className="indexing-phase-done">✓ 全文検索インデックス完了</p>
        <p>
          ベクトルインデックス: {current.toLocaleString()} / {total.toLocaleString()} チャンク
        </p>
        <ProgressBar current={current} total={total} />
      </div>
      <div className="indexing-actions">
        <button onClick={onCancel}>中断</button>
      </div>
    </>
  );
}

function DoneView({
  fulltextCount,
  vectorChunks,
  onClose,
}: {
  fulltextCount: number;
  vectorChunks: number;
  onClose: () => void;
}) {
  return (
    <>
      <h3>インデックス作成完了</h3>
      <div className="indexing-info">
        <p className="indexing-phase-done">
          ✓ 全文検索インデックス: {fulltextCount.toLocaleString()} ファイル
        </p>
        <p className="indexing-phase-done">
          ✓ ベクトルインデックス: {vectorChunks.toLocaleString()} チャンク
        </p>
      </div>
      <div className="indexing-actions">
        <button className="primary" onClick={onClose}>
          OK
        </button>
      </div>
    </>
  );
}

function CancelledView({
  fulltextCount,
  vectorChunks,
  onClose,
}: {
  fulltextCount: number;
  vectorChunks?: number;
  onClose: () => void;
}) {
  return (
    <>
      <h3>インデックス作成を中断した</h3>
      <div className="indexing-info">
        {fulltextCount > 0 ? (
          <p className="indexing-phase-done">
            ✓ 全文検索インデックス: {fulltextCount.toLocaleString()} ファイル（完了済み）
          </p>
        ) : (
          <p className="indexing-phase-cancelled">✗ 全文検索インデックス: 中断</p>
        )}
        <p className="indexing-phase-cancelled">
          ✗ ベクトルインデックス: 中断
          {vectorChunks !== undefined &&
            vectorChunks > 0 &&
            `（${vectorChunks.toLocaleString()} チャンク保存済み）`}
        </p>
        <p className="indexing-note">
          {fulltextCount > 0
            ? "全文検索のみで動作する。ベクトルインデックスは次回構築時に途中から再開する。"
            : "インデックスが構築されていない。フォルダを再選択して構築する。"}
        </p>
      </div>
      <div className="indexing-actions">
        <button className="primary" onClick={onClose}>
          OK
        </button>
      </div>
    </>
  );
}
