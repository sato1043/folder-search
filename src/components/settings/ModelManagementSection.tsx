import { useState } from "react";
import type {
  AppSettings,
  ChatTemplateType,
  LlmModelInfo,
  ModelRecommendation,
  DownloadedModelInfo,
  StorageUsage,
} from "../../types";

const GB = 1024 * 1024 * 1024;

type Props = {
  settings: AppSettings;
  onChangeSettings: (settings: AppSettings) => void;
  downloadedModels: DownloadedModelInfo[];
  storageUsage: StorageUsage | null;
  llmModels: LlmModelInfo[];
  recommendations: ModelRecommendation[];
  loadedModelFilename: string | null;
  isLoadingLlm: boolean;
  isDownloading: boolean;
  downloadStatus: string;
  onDownloadModel: (filename: string) => void;
  onDeleteModel: (filename: string) => void;
  onRegisterCustomModel: (model: LlmModelInfo) => void;
  onUnregisterCustomModel: (filename: string) => void;
};

export function ModelManagementSection({
  settings,
  onChangeSettings,
  downloadedModels,
  storageUsage,
  llmModels,
  recommendations,
  loadedModelFilename,
  isLoadingLlm,
  isDownloading,
  downloadStatus,
  onDownloadModel,
  onDeleteModel,
  onRegisterCustomModel,
  onUnregisterCustomModel,
}: Props) {
  const [showCustomForm, setShowCustomForm] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<{ filename: string; name: string } | null>(
    null,
  );
  const [customName, setCustomName] = useState("");
  const [customFilename, setCustomFilename] = useState("");
  const [customUrl, setCustomUrl] = useState("");
  const [customTemplate, setCustomTemplate] = useState<ChatTemplateType>("chatml");
  const [customContextLength, setCustomContextLength] = useState(4096);

  const handleRegister = () => {
    if (!customName || !customFilename) return;
    onRegisterCustomModel({
      name: customName,
      filename: customFilename,
      url: customUrl,
      size_bytes: 0,
      min_vram_mb: 0,
      params: "",
      quantization: "",
      chat_template: customTemplate,
      context_length: customContextLength,
      is_preset: false,
    });
    setShowCustomForm(false);
    setCustomName("");
    setCustomFilename("");
    setCustomUrl("");
    setCustomTemplate("chatml");
    setCustomContextLength(4096);
  };

  const llmFiles = downloadedModels.filter((d) => !d.is_embedding);
  const customModels = llmModels.filter((m) => !m.is_preset);

  const toggleModelEnabled = (filename: string) => {
    const disabled = settings.disabled_models;
    const isDisabled = disabled.includes(filename);
    const updated = isDisabled ? disabled.filter((f) => f !== filename) : [...disabled, filename];
    onChangeSettings({ ...settings, disabled_models: updated });
  };

  return (
    <div className="settings-section">
      <h3>モデル一覧</h3>
      <div className="model-list">
        {[...llmModels]
          .sort((a, b) => a.name.localeCompare(b.name))
          .map((m) => {
            const dl = llmFiles.find((d) => d.filename === m.filename);
            const rec = recommendations.find((r) => r.filename === m.filename);
            const isDownloaded = !!dl;
            const isLoaded = m.filename === loadedModelFilename;
            const enabled = isDownloaded && !settings.disabled_models.includes(m.filename);
            const checkboxDisabled = !isDownloaded || (isLoaded && enabled);
            return (
              <div
                key={m.filename}
                className={`model-list-item ${!isDownloaded ? "not-downloaded" : !enabled ? "disabled" : ""}`}
              >
                <input
                  type="checkbox"
                  checked={enabled}
                  onChange={() => toggleModelEnabled(m.filename)}
                  disabled={checkboxDisabled}
                  className="model-list-checkbox"
                  title={
                    !isDownloaded
                      ? "未ダウンロード"
                      : isLoaded && enabled
                        ? "ロード中のため無効にできない"
                        : enabled
                          ? "モデル選択で非表示にする"
                          : "モデル選択に表示する"
                  }
                />
                <div className="model-list-main">
                  <span className="model-list-name">{m.name}</span>
                  <span className="model-list-meta">
                    {(m.size_bytes / GB).toFixed(1)} GB
                    {m.params && ` / ${m.params}`}
                  </span>
                </div>
                <div className="model-list-badges">
                  {rec?.is_best_fit && <span className="model-badge best">最適</span>}
                  {rec?.status === "Warning" && <span className="model-badge warn">注意</span>}
                  {rec?.status === "TooLarge" && (
                    <span className="model-badge too-large">非推奨</span>
                  )}
                  {isDownloaded ? (
                    <span className="model-badge dl" title="ダウンロード済み">
                      DL済
                    </span>
                  ) : (
                    <span className="model-badge no-dl" title="未ダウンロード">
                      未DL
                    </span>
                  )}
                </div>
                {!isDownloaded ? (
                  <button
                    className="model-load-btn"
                    onClick={() => onDownloadModel(m.filename)}
                    disabled={isDownloading || isLoadingLlm}
                    title="ダウンロード"
                  >
                    &#11015;
                  </button>
                ) : (
                  <button
                    className="delete-btn"
                    onClick={() => setConfirmDelete({ filename: m.filename, name: m.name })}
                    disabled={isLoadingLlm || m.filename === loadedModelFilename}
                    title={m.filename === loadedModelFilename ? "ロード中のため削除不可" : "削除"}
                  >
                    ×
                  </button>
                )}
              </div>
            );
          })}
      </div>
      {isDownloading && downloadStatus && <p className="progress-text">{downloadStatus}</p>}

      {storageUsage && (
        <p className="storage-usage">
          モデル合計: {(storageUsage.total_used_bytes / GB).toFixed(1)} GB / 上限:{" "}
          {(storageUsage.cache_limit_bytes / GB).toFixed(0)} GB
          {storageUsage.disk_free_bytes > 0 && (
            <> / 空き: {(storageUsage.disk_free_bytes / GB).toFixed(0)} GB</>
          )}
        </p>
      )}

      <hr className="settings-divider" />

      <h3>カスタムモデル</h3>
      {customModels.length > 0 && (
        <div className="model-list">
          {customModels.map((m) => (
            <div key={m.filename} className="model-list-item">
              <div className="model-list-main">
                <span className="model-list-name">{m.name}</span>
                <span className="model-list-meta">{m.filename}</span>
              </div>
              <button
                className="delete-btn"
                onClick={() => onUnregisterCustomModel(m.filename)}
                title="登録解除"
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}

      <button className="custom-model-toggle" onClick={() => setShowCustomForm(!showCustomForm)}>
        {showCustomForm ? "フォームを閉じる" : "+ カスタムモデル登録"}
      </button>

      {showCustomForm && (
        <div className="custom-model-form">
          <input
            type="text"
            placeholder="モデル名"
            value={customName}
            onChange={(e) => setCustomName(e.target.value)}
          />
          <input
            type="text"
            placeholder="ファイル名 (例: model.gguf)"
            value={customFilename}
            onChange={(e) => setCustomFilename(e.target.value)}
          />
          <input
            type="text"
            placeholder="ダウンロードURL (任意)"
            value={customUrl}
            onChange={(e) => setCustomUrl(e.target.value)}
          />
          <label>
            テンプレート:
            <select
              value={customTemplate}
              onChange={(e) => setCustomTemplate(e.target.value as ChatTemplateType)}
            >
              <option value="chatml">ChatML</option>
              <option value="gemma">Gemma</option>
              <option value="llama3">Llama3</option>
            </select>
          </label>
          <label>
            コンテキスト長:
            <input
              type="number"
              value={customContextLength}
              onChange={(e) => setCustomContextLength(Number(e.target.value))}
              min={512}
              step={512}
            />
          </label>
          <button onClick={handleRegister} disabled={!customName || !customFilename}>
            登録
          </button>
        </div>
      )}
      {confirmDelete && (
        <div className="confirm-overlay">
          <div className="confirm-dialog">
            <p className="confirm-message">{confirmDelete.name} を削除しますか？</p>
            <div className="confirm-actions">
              <button className="confirm-cancel" onClick={() => setConfirmDelete(null)}>
                キャンセル
              </button>
              <button
                className="confirm-ok"
                onClick={() => {
                  onDeleteModel(confirmDelete.filename);
                  setConfirmDelete(null);
                }}
              >
                削除
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
