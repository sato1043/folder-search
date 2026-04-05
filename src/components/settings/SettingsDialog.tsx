import { useState, useEffect, useCallback } from "react";
import { GeneralSection } from "./GeneralSection";
import { ModelManagementSection } from "./ModelManagementSection";
import { IndexManagementSection } from "./IndexManagementSection";
import { getSettings, saveSettings } from "../../lib/tauri";
import type {
  AppSettings,
  LlmModelInfo,
  SystemInfo,
  ModelRecommendation,
  DownloadedModelInfo,
  StorageUsage,
  IndexedFolderInfo,
} from "../../types";

type SettingsSection = "general" | "models" | "indexes";

type Props = {
  isOpen: boolean;
  diskFreeBytes: number;
  systemInfo: SystemInfo | null;
  llmModels: LlmModelInfo[];
  loadedModelFilename: string | null;
  recommendations: ModelRecommendation[];
  downloadedModels: DownloadedModelInfo[];
  storageUsage: StorageUsage | null;
  modelReady: boolean;
  isLoadingLlm: boolean;
  switchingModelFilename: string | null;
  isDownloading: boolean;
  isDownloadingEmbedding: boolean;
  downloadStatus: string;
  onDownloadAndLoadLlm: (filename: string) => void;
  onDownloadEmbeddingModel: () => void;
  onDownloadModel: (filename: string) => void;
  onDeleteModel: (filename: string) => void;
  onRegisterCustomModel: (model: LlmModelInfo) => void;
  onUnregisterCustomModel: (filename: string) => void;
  indexedFolders: IndexedFolderInfo[];
  currentFolder: string | null;
  onRebuildIndex: (folderPath: string) => void;
  onDeleteIndex: (folderPath: string) => void;
  onClose: () => void;
};

export function SettingsDialog({
  isOpen,
  diskFreeBytes,
  systemInfo,
  llmModels,
  loadedModelFilename,
  recommendations,
  downloadedModels,
  storageUsage,
  modelReady,
  isLoadingLlm,
  switchingModelFilename,
  isDownloading,
  isDownloadingEmbedding,
  downloadStatus,
  onDownloadAndLoadLlm,
  onDownloadEmbeddingModel,
  onDownloadModel,
  onDeleteModel,
  onRegisterCustomModel,
  onUnregisterCustomModel,
  indexedFolders,
  currentFolder,
  onRebuildIndex,
  onDeleteIndex,
  onClose,
}: Props) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [initialSettings, setInitialSettings] = useState<AppSettings | null>(null);
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const [initialLoadedModel, setInitialLoadedModel] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      getSettings()
        .then((s) => {
          setSettings(s);
          setInitialSettings(s);
          setActiveSection("general");
        })
        .catch(() => {});
      setInitialLoadedModel(loadedModelFilename);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- loadedModelFilenameはダイアログを開いた時点のみキャプチャ
  }, [isOpen]);

  const handleChange = useCallback((newSettings: AppSettings) => {
    setSettings(newSettings);
  }, []);

  const handleResetField = useCallback(
    (field: keyof AppSettings) => {
      if (initialSettings && settings) {
        setSettings({ ...settings, [field]: initialSettings[field] });
      }
    },
    [initialSettings, settings],
  );

  const handleClose = useCallback(async () => {
    if (settings) {
      try {
        // 最新の設定を読み込み、ダイアログで変更したフィールドのみ上書きして保存
        const latest = await getSettings();
        await saveSettings({ ...latest, cache_limit_bytes: settings.cache_limit_bytes });
      } catch (e) {
        console.error("設定保存失敗:", e);
      }
    }
    onClose();
  }, [settings, onClose]);

  if (!isOpen || !settings) return null;

  return (
    <div
      className="settings-overlay"
      onClick={isLoadingLlm || isDownloading ? undefined : handleClose}
    >
      <div className="settings-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>設定</h2>
          <button
            className="settings-close-btn"
            onClick={handleClose}
            disabled={isLoadingLlm || isDownloading}
          >
            ×
          </button>
        </div>
        <div className="settings-body">
          <div className="settings-nav">
            <div
              className={`settings-nav-item ${activeSection === "general" ? "active" : ""}`}
              onClick={() => setActiveSection("general")}
            >
              一般
            </div>
            <div
              className={`settings-nav-item ${activeSection === "models" ? "active" : ""}`}
              onClick={() => setActiveSection("models")}
            >
              モデル管理
            </div>
            <div
              className={`settings-nav-item ${activeSection === "indexes" ? "active" : ""}`}
              onClick={() => setActiveSection("indexes")}
            >
              インデックス管理
            </div>
          </div>
          <div className="settings-content">
            {activeSection === "general" && (
              <GeneralSection
                settings={settings}
                diskFreeBytes={diskFreeBytes}
                onChange={handleChange}
                onResetField={handleResetField}
                systemInfo={systemInfo}
                llmModels={llmModels}
                loadedModelFilename={loadedModelFilename}
                initialLoadedModel={initialLoadedModel}
                recommendations={recommendations}
                downloadedModels={downloadedModels}
                modelReady={modelReady}
                isDownloadingEmbedding={isDownloadingEmbedding}
                embeddingDownloadStatus={isDownloadingEmbedding ? downloadStatus : ""}
                onDownloadEmbeddingModel={onDownloadEmbeddingModel}
                isLoadingLlm={isLoadingLlm}
                switchingModelFilename={switchingModelFilename}
                downloadStatus={isLoadingLlm ? downloadStatus : ""}
                onDownloadAndLoadLlm={onDownloadAndLoadLlm}
              />
            )}
            {activeSection === "indexes" && (
              <IndexManagementSection
                indexedFolders={indexedFolders}
                currentFolder={currentFolder}
                onRebuild={onRebuildIndex}
                onDelete={onDeleteIndex}
              />
            )}
            {activeSection === "models" && (
              <ModelManagementSection
                settings={settings}
                onChangeSettings={handleChange}
                downloadedModels={downloadedModels}
                storageUsage={storageUsage}
                llmModels={llmModels}
                recommendations={recommendations}
                loadedModelFilename={loadedModelFilename}
                isLoadingLlm={isLoadingLlm}
                isDownloading={isDownloading}
                downloadStatus={isDownloading ? downloadStatus : ""}
                onDownloadModel={onDownloadModel}
                onDeleteModel={onDeleteModel}
                onRegisterCustomModel={onRegisterCustomModel}
                onUnregisterCustomModel={onUnregisterCustomModel}
              />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
