import { useRef, useEffect } from "react";
import type { IndexedFolderInfo } from "../../types";

type FolderSelectorProps = {
  folders: IndexedFolderInfo[];
  currentFolder: string | null;
  disabled?: boolean;
  onSelectExisting: (folderPath: string) => void;
  onSelectNew: () => void;
};

export function FolderSelector({
  folders,
  currentFolder,
  disabled,
  onSelectExisting,
  onSelectNew,
}: FolderSelectorProps) {
  const selectRef = useRef<HTMLSelectElement>(null);

  useEffect(() => {
    if (!selectRef.current) return;
    if (!currentFolder) {
      selectRef.current.selectedIndex = -1;
    } else {
      const options = selectRef.current.options;
      for (let i = 0; i < options.length; i++) {
        if (options[i].value === currentFolder) {
          selectRef.current.selectedIndex = i;
          return;
        }
      }
      selectRef.current.selectedIndex = -1;
    }
  }, [currentFolder, folders]);

  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const value = e.target.value;
    if (value === "__new__") {
      onSelectNew();
      // 選択状態を元に戻す
      if (selectRef.current) {
        selectRef.current.selectedIndex = currentFolder
          ? Array.from(selectRef.current.options).findIndex(
              (o) => o.value === currentFolder,
            )
          : -1;
      }
    } else if (value) {
      onSelectExisting(value);
    }
  };

  return (
    <div className="folder-selector">
      {!currentFolder && (
        <span className="folder-selector-placeholder">フォルダを選択...</span>
      )}
      <select
        ref={selectRef}
        onChange={handleChange}
        disabled={disabled}
        className={`folder-selector-select${!currentFolder ? " folder-selector-empty" : ""}`}
      >
        {folders.map((f) => (
          <option key={f.folder_path} value={f.folder_path}>
            {f.folder_path}
          </option>
        ))}
        <option value="__new__">新しいフォルダを選択...</option>
      </select>
    </div>
  );
}
