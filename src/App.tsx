import { useState, useCallback, useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { appDataDir } from "@tauri-apps/api/path";
import { listen } from "@tauri-apps/api/event";
import { Sidebar } from "./components/layout/Sidebar";
import { MainPanel } from "./components/layout/MainPanel";
import { SearchBar } from "./components/search/SearchBar";
import { ResultList } from "./components/search/ResultList";
import { Preview } from "./components/search/Preview";
import { ChatMessage } from "./components/chat/ChatMessage";
import { SettingsDialog } from "./components/settings/SettingsDialog";
import { IndexingDialog } from "./components/indexing/IndexingDialog";
import {
  scanFolder,
  cancelIndexing,
  buildIndex,
  search,
  hybridSearch,
  readFileContent,
  isEmbeddingModelReady,
  downloadEmbeddingModel,
  buildVectorIndex,
  listAvailableModels,
  downloadLlmModel,
  loadLlmModel,
  isLlmReady,
  chat,
  detectSystemInfo,
  getModelRecommendations,
  getLoadedModelFilename,
  getSettings,
  saveSettings,
  listDownloadedModels,
  deleteModel,
  getStorageUsage,
  registerCustomModel,
  unregisterCustomModel,
} from "./lib/tauri";
import type {
  SearchResult,
  DownloadProgress,
  VectorIndexProgress,
  FulltextIndexProgress,
  IndexingPhase,
  LlmModelInfo,
  SystemInfo,
  ModelRecommendation,
  DownloadedModelInfo,
  StorageUsage,
} from "./types";

type AppMode = "search" | "chat";

function App() {
  const [folderPath, setFolderPath] = useState<string | null>(null);
  const [results, setResults] = useState<SearchResult[] | null>(null);
  const [previewTitle, setPreviewTitle] = useState<string | null>(null);
  const [previewContent, setPreviewContent] = useState<string | null>(null);
  const [indexCount, setIndexCount] = useState<number>(0);
  const [isIndexing, setIsIndexing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [modelReady, setModelReady] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);
  const [isDownloadingEmbedding, setIsDownloadingEmbedding] = useState(false);
  const [downloadStatus, setDownloadStatus] = useState<string>("");
  const [isBuildingVector, setIsBuildingVector] = useState(false);
  const [vectorProgress, setVectorProgress] = useState<string>("");
  const [vectorChunkCount, setVectorChunkCount] = useState<number>(0);

  // LLM state
  const [appMode, setAppMode] = useState<AppMode>("search");
  const [llmReady, setLlmReady] = useState(false);
  const [llmModels, setLlmModels] = useState<LlmModelInfo[]>([]);
  const [loadedModelFilename, setLoadedModelFilename] = useState<string | null>(null);
  const [isLoadingLlm, setIsLoadingLlm] = useState(false);
  const [switchingModelFilename, setSwitchingModelFilename] = useState<string | null>(null);
  const [chatAnswer, setChatAnswer] = useState<string | null>(null);
  const [chatSources, setChatSources] = useState<string[]>([]);
  const [isChatting, setIsChatting] = useState(false);
  const [streamingText, setStreamingText] = useState<string>("");

  // システム情報・モデル推奨・ストレージ
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [recommendations, setRecommendations] = useState<ModelRecommendation[]>([]);
  const [downloadedModels, setDownloadedModels] = useState<DownloadedModelInfo[]>([]);
  const [storageUsage, setStorageUsage] = useState<StorageUsage | null>(null);

  // 設定ダイアログ
  const [showSettings, setShowSettings] = useState(false);

  // インデックス作成ダイアログ
  const [indexingPhase, setIndexingPhase] = useState<IndexingPhase | null>(null);
  const pendingFolderRef = useRef<string | null>(null);

  // 初期化
  useEffect(() => {
    // Embeddingモデルの確認・自動ダウンロード
    (async () => {
      try {
        const ready = await isEmbeddingModelReady();
        if (ready) {
          setModelReady(true);
          return;
        }
        setIsDownloadingEmbedding(true);
        setIsDownloading(true);
        setDownloadStatus("Embeddingモデルをダウンロード中...");
        await downloadEmbeddingModel();
        setModelReady(true);
      } catch (e) {
        console.error("Embeddingモデル自動ダウンロード失敗:", e);
        setModelReady(false);
      } finally {
        setIsDownloadingEmbedding(false);
        setIsDownloading(false);
        setDownloadStatus("");
      }
    })();

    isLlmReady()
      .then(setLlmReady)
      .catch(() => setLlmReady(false));
    getLoadedModelFilename()
      .then(setLoadedModelFilename)
      .catch(() => {});
    listAvailableModels()
      .then(setLlmModels)
      .catch(() => {});
    detectSystemInfo()
      .then(setSystemInfo)
      .catch(() => {});
    getModelRecommendations()
      .then(setRecommendations)
      .catch(() => {});
    listDownloadedModels()
      .then(setDownloadedModels)
      .catch(() => {});
    getStorageUsage()
      .then(setStorageUsage)
      .catch(() => {});

    // 前回ロードしたモデルの自動ロード
    (async () => {
      try {
        const [settings, models] = await Promise.all([getSettings(), listAvailableModels()]);
        if (!settings.last_loaded_model) return;
        const alreadyLoaded = await isLlmReady();
        if (alreadyLoaded) return;
        const model = models.find((m) => m.filename === settings.last_loaded_model);
        if (!model) return;
        setIsLoadingLlm(true);
        setSwitchingModelFilename(model.filename);
        setDownloadStatus("前回のモデルをロード中...");
        const result = await loadLlmModel(
          model.filename,
          model.chat_template,
          model.context_length,
        );
        void result; // GPU情報は将来使用予定
        setLlmReady(true);
        setLoadedModelFilename(model.filename);
      } catch (e) {
        console.error("LLM自動ロード失敗:", e);
      } finally {
        setIsLoadingLlm(false);
        setSwitchingModelFilename(null);
        setDownloadStatus("");
      }
    })();
  }, []);

  // ダウンロード進捗のリスナー
  useEffect(() => {
    const unlisten = listen<DownloadProgress>("download-progress", (event) => {
      const p = event.payload;
      if (p.total_bytes) {
        const pct = Math.round((p.downloaded_bytes / p.total_bytes) * 100);
        setDownloadStatus(`${p.file_name}: ${pct}%`);
      } else {
        const mb = (p.downloaded_bytes / 1024 / 1024).toFixed(1);
        setDownloadStatus(`${p.file_name}: ${mb}MB`);
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // 全文検索インデックス構築進捗のリスナー
  useEffect(() => {
    const unlisten = listen<FulltextIndexProgress>("fulltext-index-progress", (event) => {
      const p = event.payload;
      setIndexingPhase((prev) => {
        if (prev && (prev.kind === "fulltext" || prev.kind === "confirm")) {
          return { kind: "fulltext", current: p.current, total: p.total };
        }
        return prev;
      });
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // ベクトルインデックス構築進捗のリスナー
  useEffect(() => {
    const unlisten = listen<VectorIndexProgress>("vector-index-progress", (event) => {
      const p = event.payload;
      const pct = p.total > 0 ? Math.round((p.current / p.total) * 100) : 0;
      setVectorProgress(`${pct}%`);
      setIndexingPhase((prev) => {
        if (prev && (prev.kind === "vector" || prev.kind === "fulltext")) {
          return { kind: "vector", current: p.current, total: p.total };
        }
        return prev;
      });
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // インデックス自動更新のリスナー
  useEffect(() => {
    const unlisten = listen<{ fulltext_count: number; vector_chunk_count: number }>(
      "index-updated",
      (event) => {
        const p = event.payload;
        setIndexCount(p.fulltext_count);
        setVectorChunkCount(p.vector_chunk_count);
      },
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // チャットトークンのリスナー
  useEffect(() => {
    const unlisten = listen<string>("chat-token", (event) => {
      setStreamingText((prev) => prev + event.payload);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const refreshModelStorage = useCallback(async () => {
    const [models, usage] = await Promise.all([listDownloadedModels(), getStorageUsage()]);
    setDownloadedModels(models);
    setStorageUsage(usage);
  }, []);

  const refreshModelList = useCallback(async () => {
    const [models, recs] = await Promise.all([listAvailableModels(), getModelRecommendations()]);
    setLlmModels(models);
    setRecommendations(recs);
  }, []);

  const triggerBuildVectorIndex = useCallback(async () => {
    try {
      setIsBuildingVector(true);
      setError(null);
      setVectorProgress("");
      await new Promise((r) => requestAnimationFrame(r));
      const count = await buildVectorIndex();
      setVectorChunkCount(count);
      setIsBuildingVector(false);
      setVectorProgress("");
    } catch (e) {
      setError(String(e));
      setIsBuildingVector(false);
      setVectorProgress("");
    }
  }, []);

  /** 確認ダイアログが必要か判定する */
  const needsConfirmation = useCallback(
    (scan: {
      file_count: number;
      total_size_bytes: number;
      estimated_chunks: number;
      max_file_size_bytes: number;
      timed_out: boolean;
    }) => {
      return (
        scan.file_count >= 500 ||
        scan.total_size_bytes >= 100 * 1024 * 1024 ||
        scan.estimated_chunks >= 40000 ||
        scan.max_file_size_bytes >= 10 * 1024 * 1024 ||
        scan.timed_out
      );
    },
    [],
  );

  /** インデックス構築を実行する（確認ダイアログ経由 or 直接） */
  const executeIndexBuild = useCallback(
    async (selected: string, fileCount: number) => {
      try {
        setFolderPath(selected);
        setIsIndexing(true);
        setError(null);

        if (indexingPhase) {
          setIndexingPhase({ kind: "fulltext", current: 0, total: fileCount });
        }

        const indexPath = (await appDataDir()) + "/index/fulltext";
        const count = await buildIndex(selected, indexPath, fileCount);
        setIndexCount(count);
        setIsIndexing(false);

        if (modelReady && count > 0) {
          if (indexingPhase) {
            setIndexingPhase({ kind: "vector", current: 0, total: 0 });
          }
          setIsBuildingVector(true);
          setVectorProgress("");
          await new Promise((r) => requestAnimationFrame(r));
          const chunkCount = await buildVectorIndex();
          setVectorChunkCount(chunkCount);
          setIsBuildingVector(false);
          setVectorProgress("");

          if (indexingPhase) {
            setIndexingPhase({ kind: "done", fulltextCount: count, vectorChunks: chunkCount });
          }
        } else {
          if (indexingPhase) {
            setIndexingPhase({ kind: "done", fulltextCount: count, vectorChunks: 0 });
          }
        }
      } catch (e) {
        const msg = String(e);
        if (msg.includes("中断")) {
          // 中断された場合
          setIsIndexing(false);
          setIsBuildingVector(false);
          setVectorProgress("");
          setIndexingPhase({
            kind: "cancelled",
            fulltextCount: indexCount,
            vectorChunks: undefined,
          });
        } else {
          setError(msg);
          setIsIndexing(false);
          setIsBuildingVector(false);
          setIndexingPhase(null);
        }
      }
    },
    [modelReady, indexingPhase, indexCount],
  );

  const handleSelectFolder = useCallback(async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return;

      const scan = await scanFolder(selected as string);

      if (scan.file_count === 0) {
        setError("対象ファイル（.txt / .md）が見つからない");
        return;
      }

      if (needsConfirmation(scan)) {
        // 確認ダイアログを表示
        pendingFolderRef.current = selected as string;
        setIndexingPhase({ kind: "confirm", scanResult: scan });
      } else {
        // 閾値未満 → 即座に実行
        await executeIndexBuild(selected as string, scan.file_count);
      }
    } catch (e) {
      setError(String(e));
      setIsIndexing(false);
    }
  }, [needsConfirmation, executeIndexBuild]);

  /** ダイアログの[開始]ボタン */
  const handleIndexingStart = useCallback(async () => {
    const folder = pendingFolderRef.current;
    if (!folder || !indexingPhase || indexingPhase.kind !== "confirm") return;
    const fileCount = indexingPhase.scanResult.file_count;
    await executeIndexBuild(folder, fileCount);
  }, [indexingPhase, executeIndexBuild]);

  /** ダイアログの[中断]ボタン */
  const handleIndexingCancel = useCallback(async () => {
    if (indexingPhase?.kind === "confirm") {
      // 確認画面でのキャンセル → ダイアログを閉じるだけ
      setIndexingPhase(null);
      pendingFolderRef.current = null;
      return;
    }
    // 実行中の中断
    await cancelIndexing();
  }, [indexingPhase]);

  /** ダイアログの[OK]/クローズ */
  const handleIndexingClose = useCallback(() => {
    setIndexingPhase(null);
    pendingFolderRef.current = null;
  }, []);

  const handleDownloadEmbeddingModel = useCallback(async () => {
    try {
      setIsDownloadingEmbedding(true);
      setIsDownloading(true);
      setError(null);
      setDownloadStatus("ダウンロード開始...");
      await downloadEmbeddingModel();
      setModelReady(true);

      if (indexCount > 0) {
        await triggerBuildVectorIndex();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsDownloadingEmbedding(false);
      setIsDownloading(false);
      setDownloadStatus("");
      await refreshModelStorage();
    }
  }, [indexCount, triggerBuildVectorIndex, refreshModelStorage]);

  const handleDownloadAndLoadLlm = useCallback(
    async (filename: string) => {
      const model = llmModels.find((m) => m.filename === filename);
      if (!model) return;

      try {
        setIsLoadingLlm(true);
        setSwitchingModelFilename(model.filename);
        setError(null);
        setDownloadStatus("LLMモデルダウンロード中...");
        // 状態変更をブラウザに描画させてから処理を続行
        await new Promise((r) => requestAnimationFrame(r));
        const evicted = await downloadLlmModel(model.filename, model.url, model.size_bytes);
        if (evicted.length > 0) {
          setError(`キャッシュ上限超過のため自動削除: ${evicted.join(", ")}`);
        }
        setDownloadStatus("モデルロード中...");
        const result = await loadLlmModel(
          model.filename,
          model.chat_template,
          model.context_length,
        );
        void result; // GPU情報は将来使用予定
        setLlmReady(true);
        setLoadedModelFilename(model.filename);
        // 設定に前回ロードしたモデルを保存
        const currentSettings = await getSettings();
        await saveSettings({ ...currentSettings, last_loaded_model: model.filename });
      } catch (e) {
        setError(String(e));
      } finally {
        setIsLoadingLlm(false);
        setSwitchingModelFilename(null);
        setDownloadStatus("");
        await refreshModelStorage();
      }
    },
    [llmModels, refreshModelStorage],
  );

  const handleDownloadModel = useCallback(
    async (filename: string) => {
      const model = llmModels.find((m) => m.filename === filename);
      if (!model) return;
      try {
        setIsDownloading(true);
        setError(null);
        setDownloadStatus("ダウンロード中...");
        await downloadLlmModel(model.filename, model.url, model.size_bytes);
        await refreshModelStorage();
      } catch (e) {
        setError(String(e));
      } finally {
        setIsDownloading(false);
        setDownloadStatus("");
      }
    },
    [llmModels, refreshModelStorage],
  );

  const handleDeleteModel = useCallback(
    async (filename: string) => {
      try {
        setError(null);
        await deleteModel(filename);
        await refreshModelStorage();
      } catch (e) {
        setError(String(e));
      }
    },
    [refreshModelStorage],
  );

  const handleSearch = useCallback(
    async (query: string) => {
      try {
        setError(null);
        let searchResults: SearchResult[];
        if (modelReady && vectorChunkCount > 0) {
          searchResults = await hybridSearch(query);
        } else {
          searchResults = await search(query);
        }
        setResults(searchResults);
        setPreviewTitle(null);
        setPreviewContent(null);
      } catch (e) {
        setError(String(e));
      }
    },
    [modelReady, vectorChunkCount],
  );

  const handleChat = useCallback(async (question: string) => {
    try {
      setError(null);
      setIsChatting(true);
      setStreamingText("");
      setChatAnswer(null);
      setChatSources([]);

      const result = await chat(question);
      setChatAnswer(result.answer);
      setChatSources(result.sources);
      setIsChatting(false);
    } catch (e) {
      setError(String(e));
      setIsChatting(false);
    }
  }, []);

  const handleSelectResult = useCallback(async (result: SearchResult) => {
    try {
      const content = await readFileContent(result.path);
      setPreviewTitle(result.title);
      setPreviewContent(content);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const handleSourceClick = useCallback(async (path: string) => {
    try {
      const content = await readFileContent(path);
      const fileName = path.split("/").pop() || path;
      setPreviewTitle(fileName);
      setPreviewContent(content);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  return (
    <div className="app">
      {indexingPhase && (
        <IndexingDialog
          phase={indexingPhase}
          onStart={handleIndexingStart}
          onCancel={handleIndexingCancel}
          onClose={handleIndexingClose}
        />
      )}
      <SettingsDialog
        isOpen={showSettings}
        diskFreeBytes={storageUsage?.disk_free_bytes ?? 0}
        systemInfo={systemInfo}
        llmModels={llmModels}
        loadedModelFilename={loadedModelFilename}
        recommendations={recommendations}
        downloadedModels={downloadedModels}
        storageUsage={storageUsage}
        modelReady={modelReady}
        isLoadingLlm={isLoadingLlm}
        switchingModelFilename={switchingModelFilename}
        isDownloading={isDownloading}
        isDownloadingEmbedding={isDownloadingEmbedding}
        downloadStatus={downloadStatus}
        onDownloadAndLoadLlm={handleDownloadAndLoadLlm}
        onDownloadEmbeddingModel={handleDownloadEmbeddingModel}
        onDownloadModel={handleDownloadModel}
        onDeleteModel={handleDeleteModel}
        onRegisterCustomModel={async (model) => {
          try {
            setError(null);
            await registerCustomModel(model);
            await refreshModelList();
          } catch (e) {
            setError(String(e));
          }
        }}
        onUnregisterCustomModel={async (filename) => {
          try {
            setError(null);
            await unregisterCustomModel(filename);
            await refreshModelList();
          } catch (e) {
            setError(String(e));
          }
        }}
        onClose={() => {
          setShowSettings(false);
          refreshModelStorage();
        }}
      />
      {(isDownloadingEmbedding || isLoadingLlm) && !showSettings && (
        <div className="loading-overlay">
          <div className="loading-spinner" />
          {isDownloadingEmbedding && (
            <>
              <p className="loading-text">Embeddingモデルをダウンロード中...</p>
              {downloadStatus && <p className="loading-text">{downloadStatus}</p>}
            </>
          )}
          {isLoadingLlm && <p className="loading-text">LLMモデルをロード中...</p>}
        </div>
      )}
      <Sidebar>
        {folderPath && <p className="folder-path">{folderPath}</p>}
        {indexCount > 0 && <p className="index-count">{indexCount} 件のファイル</p>}

        <hr className="sidebar-divider" />

        {isBuildingVector && (
          <p className="status-ok">
            ベクトルインデックス: 構築中{vectorProgress && ` ${vectorProgress}`}
          </p>
        )}
        {!isBuildingVector && vectorChunkCount > 0 && (
          <p className="status-ok">ベクトルインデックス: {vectorChunkCount} チャンク登録済み</p>
        )}

        <hr className="sidebar-divider" />

        {isDownloading && <p className="progress-text">{downloadStatus}</p>}

        <hr className="sidebar-divider" />

        <div className="mode-selector">
          <label>
            <input
              type="radio"
              name="appMode"
              value="search"
              checked={appMode === "search"}
              onChange={() => setAppMode("search")}
            />
            検索モード
          </label>
          <label>
            <input
              type="radio"
              name="appMode"
              value="chat"
              checked={appMode === "chat"}
              onChange={() => setAppMode("chat")}
              disabled={!llmReady}
            />
            チャットモード
          </label>
        </div>
      </Sidebar>
      <MainPanel>
        {appMode === "search" ? (
          <>
            <SearchBar onSearch={handleSearch} disabled={indexCount === 0 || isLoadingLlm}>
              <button
                className="search-bar-icon-btn"
                onClick={handleSelectFolder}
                disabled={isIndexing || isBuildingVector}
                title="フォルダを選択"
              >
                <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h3.172a1.5 1.5 0 0 1 1.06.44l.829.828a.5.5 0 0 0 .353.146H13.5A1.5 1.5 0 0 1 15 4.914V12.5a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z" />
                </svg>
              </button>
              <button
                className="search-bar-icon-btn"
                onClick={() => setShowSettings(true)}
                title="設定"
              >
                &#9881;
              </button>
            </SearchBar>
            {error && <p className="error-message">{error}</p>}
            <div className="content-area">
              <ResultList results={results} onSelect={handleSelectResult} />
              <Preview title={previewTitle} content={previewContent} />
            </div>
          </>
        ) : (
          <>
            <SearchBar
              onSearch={handleChat}
              disabled={!llmReady || isChatting}
              placeholder="質問を入力..."
            >
              <button
                className="search-bar-icon-btn"
                onClick={handleSelectFolder}
                disabled={isIndexing || isBuildingVector}
                title="フォルダを選択"
              >
                <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h3.172a1.5 1.5 0 0 1 1.06.44l.829.828a.5.5 0 0 0 .353.146H13.5A1.5 1.5 0 0 1 15 4.914V12.5a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z" />
                </svg>
              </button>
              <button
                className="search-bar-icon-btn"
                onClick={() => setShowSettings(true)}
                title="設定"
              >
                &#9881;
              </button>
            </SearchBar>
            {error && <p className="error-message">{error}</p>}
            <div className="content-area">
              <div className="chat-panel">
                <ChatMessage
                  answer={isChatting ? streamingText : chatAnswer}
                  sources={chatSources}
                  onSourceClick={handleSourceClick}
                  isStreaming={isChatting}
                />
              </div>
              <Preview title={previewTitle} content={previewContent} />
            </div>
          </>
        )}
      </MainPanel>
    </div>
  );
}

export default App;
