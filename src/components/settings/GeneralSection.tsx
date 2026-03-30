import type {
  AppSettings,
  LlmModelInfo,
  SystemInfo,
  ModelRecommendation,
  DownloadedModelInfo,
} from "../../types";

const GB = 1024 * 1024 * 1024;

type Props = {
  settings: AppSettings;
  diskFreeBytes: number;
  onChange: (settings: AppSettings) => void;
  onResetField: (field: keyof AppSettings) => void;
  systemInfo: SystemInfo | null;
  llmModels: LlmModelInfo[];
  loadedModelFilename: string | null;
  initialLoadedModel: string | null;
  recommendations: ModelRecommendation[];
  downloadedModels: DownloadedModelInfo[];
  modelReady: boolean;
  isDownloadingEmbedding: boolean;
  embeddingDownloadStatus: string;
  onDownloadEmbeddingModel: () => void;
  isLoadingLlm: boolean;
  switchingModelFilename: string | null;
  downloadStatus: string;
  onDownloadAndLoadLlm: (filename: string) => void;
};

export function GeneralSection({
  settings,
  diskFreeBytes,
  onChange,
  onResetField,
  systemInfo,
  llmModels,
  loadedModelFilename,
  initialLoadedModel,
  recommendations,
  downloadedModels,
  modelReady,
  isDownloadingEmbedding,
  embeddingDownloadStatus,
  onDownloadEmbeddingModel,
  isLoadingLlm,
  switchingModelFilename,
  downloadStatus,
  onDownloadAndLoadLlm,
}: Props) {
  const diskFreeGb = diskFreeBytes / GB;
  const minGb = Math.min(25, Math.floor(diskFreeGb));
  const maxGb = Math.max(minGb, Math.floor(diskFreeGb * 0.75));
  const currentGb = Math.round(settings.cache_limit_bytes / GB);

  const handleSliderChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const gb = Number(e.target.value);
    onChange({ ...settings, cache_limit_bytes: gb * GB });
  };

  return (
    <div className="settings-section">
      <h3>ベクトル検索モデル</h3>
      <div className="loaded-model-info">
        <span className="loaded-model-label">
          multilingual-e5-small:{" "}
          {modelReady
            ? "ダウンロード済み"
            : isDownloadingEmbedding
              ? "ダウンロード中..."
              : "未ダウンロード"}
        </span>
        {!modelReady && !isDownloadingEmbedding && (
          <button
            className="model-load-btn"
            onClick={onDownloadEmbeddingModel}
            title="Embeddingモデルをダウンロード"
          >
            &#8595;
          </button>
        )}
      </div>
      {isDownloadingEmbedding && embeddingDownloadStatus && (
        <p className="progress-text">{embeddingDownloadStatus}</p>
      )}

      <hr className="settings-divider" />

      <h3>LLMモデル選択</h3>
      <div className="loaded-model-info">
        <span className="loaded-model-label">
          現在:{" "}
          {loadedModelFilename
            ? (llmModels.find((m) => m.filename === loadedModelFilename)?.name ??
              loadedModelFilename)
            : "未ロード"}
        </span>
        {initialLoadedModel && loadedModelFilename !== initialLoadedModel && !isLoadingLlm && (
          <button
            className="reset-btn"
            onClick={() => onDownloadAndLoadLlm(initialLoadedModel)}
            title="ダイアログを開いた時点のモデルに戻す"
          >
            &#8635;
          </button>
        )}
      </div>
      {isLoadingLlm && switchingModelFilename && (
        <div className="switching-model-info">
          <p className="switching-model-label">
            切替中:{" "}
            {llmModels.find((m) => m.filename === switchingModelFilename)?.name ??
              switchingModelFilename}
          </p>
          {downloadStatus && <p className="progress-text">{downloadStatus}</p>}
        </div>
      )}
      {systemInfo && (
        <p className="system-info">
          RAM: {Math.round(systemInfo.total_ram_mb / 1024)} GB
          {systemInfo.gpus.length > 0 && (
            <>
              {" "}
              | GPU:{" "}
              {systemInfo.gpus
                .map((g) =>
                  g.vram_mb > 0 ? `${g.name} (${Math.round(g.vram_mb / 1024)} GB)` : g.name,
                )
                .join(", ")}
            </>
          )}
        </p>
      )}
      <div className="model-list">
        {llmModels
          .filter(
            (m) =>
              !settings.disabled_models.includes(m.filename) &&
              downloadedModels.some((d) => d.filename === m.filename),
          )
          .map((m) => {
            const rec = recommendations.find((r) => r.filename === m.filename);
            const dl = downloadedModels.find((d) => d.filename === m.filename);
            const isLoaded = m.filename === loadedModelFilename;
            return (
              <div key={m.filename} className="model-list-item">
                <span className="model-loaded-indicator">{isLoaded ? "選択中" : ""}</span>
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
                  {dl ? (
                    <span className="model-badge dl" title="ダウンロード済み">
                      DL済
                    </span>
                  ) : (
                    <span className="model-badge no-dl" title="未ダウンロード">
                      未DL
                    </span>
                  )}
                </div>
                {!isLoaded ? (
                  <button
                    className="model-load-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      onDownloadAndLoadLlm(m.filename);
                    }}
                    disabled={isLoadingLlm}
                    title="このモデルをロード"
                  >
                    &#9654;
                  </button>
                ) : (
                  <span className="model-load-btn-spacer" />
                )}
              </div>
            );
          })}
      </div>

      <hr className="settings-divider" />

      <h3>ダウンロードキャッシュサイズ</h3>
      <div className="cache-slider-container">
        <input
          type="range"
          min={minGb}
          max={maxGb}
          step={5}
          value={currentGb}
          onChange={handleSliderChange}
          className="cache-slider"
        />
        <span className="cache-slider-value">{currentGb} GB</span>
        <button
          className="reset-btn"
          onClick={() => onResetField("cache_limit_bytes")}
          title="元に戻す"
        >
          &#8635;
        </button>
      </div>
      <div className="cache-suggestions">
        <p className="suggestion-title">サジェスト:</p>
        <p className="suggestion-item">25 GB — 小型モデル (0.5B-1.5B) を数個保持可能</p>
        <p className="suggestion-item">50 GB — 中型モデル (4B-7B) を数個保持可能</p>
        <p className="suggestion-item">100 GB — 大型モデル (12B+) を含む複数保持可能</p>
      </div>
    </div>
  );
}
