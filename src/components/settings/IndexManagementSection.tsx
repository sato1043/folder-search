import type { IndexedFolderInfo } from "../../types";

type Props = {
  indexedFolders: IndexedFolderInfo[];
  currentFolder: string | null;
  onRebuild: (folderPath: string) => void;
  onDelete: (folderPath: string) => void;
};

export function IndexManagementSection({
  indexedFolders,
  currentFolder,
  onRebuild,
  onDelete,
}: Props) {
  if (indexedFolders.length === 0) {
    return (
      <div className="settings-section">
        <h3>インデックス管理</h3>
        <p className="index-empty-message">インデックスなし</p>
      </div>
    );
  }

  return (
    <div className="settings-section">
      <h3>インデックス管理</h3>
      <div className="index-mgmt-list">
        {indexedFolders.map((f) => {
          const isCurrent = f.folder_path === currentFolder;
          return (
            <div key={f.folder_path} className="index-mgmt-item">
              <div className="index-mgmt-main">
                <span className="index-mgmt-path" title={f.folder_path}>
                  {f.folder_path}
                </span>
                <span className="index-mgmt-status">
                  {f.has_fulltext && (
                    <span className="index-badge fulltext">全文検索</span>
                  )}
                  {f.vector_complete && (
                    <span className="index-badge vector">ベクトル</span>
                  )}
                  {isCurrent && (
                    <span className="index-badge current">選択中</span>
                  )}
                </span>
              </div>
              <div className="index-mgmt-actions">
                <button
                  className="index-action-btn"
                  onClick={() => onRebuild(f.folder_path)}
                  title="インデックスを再構築"
                >
                  <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
                    <path d="M8 3a5 5 0 1 0 4.546 2.914.5.5 0 1 1 .908-.418A6 6 0 1 1 8 2v1z" />
                    <path d="M8 4.466V.534a.25.25 0 0 1 .41-.192l2.36 1.966c.12.1.12.284 0 .384L8.41 4.658A.25.25 0 0 1 8 4.466z" />
                  </svg>
                </button>
                <button
                  className="index-action-btn delete"
                  onClick={() => onDelete(f.folder_path)}
                  title="インデックスを削除"
                >
                  <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
                    <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0V6z" />
                    <path fillRule="evenodd" d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1v1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4H4.118zM2.5 3V2h11v1h-11z" />
                  </svg>
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
