import { useState, useCallback, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { appDataDir } from "@tauri-apps/api/path";
import { listen } from "@tauri-apps/api/event";
import { Sidebar } from "./components/layout/Sidebar";
import { MainPanel } from "./components/layout/MainPanel";
import { SearchBar } from "./components/search/SearchBar";
import { ResultList } from "./components/search/ResultList";
import { Preview } from "./components/search/Preview";
import { ChatMessage } from "./components/chat/ChatMessage";
import {
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
} from "./lib/tauri";
import type {
  SearchResult,
  DownloadProgress,
  VectorIndexProgress,
  SearchMode,
  LlmModelInfo,
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
  const [searchMode, setSearchMode] = useState<SearchMode>("fulltext");
  const [modelReady, setModelReady] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadStatus, setDownloadStatus] = useState<string>("");
  const [isBuildingVector, setIsBuildingVector] = useState(false);
  const [vectorProgress, setVectorProgress] = useState<string>("");
  const [vectorChunkCount, setVectorChunkCount] = useState<number>(0);

  // LLM state
  const [appMode, setAppMode] = useState<AppMode>("search");
  const [llmReady, setLlmReady] = useState(false);
  const [llmModels, setLlmModels] = useState<LlmModelInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [isLoadingLlm, setIsLoadingLlm] = useState(false);
  const [chatAnswer, setChatAnswer] = useState<string | null>(null);
  const [chatSources, setChatSources] = useState<string[]>([]);
  const [isChatting, setIsChatting] = useState(false);
  const [streamingText, setStreamingText] = useState<string>("");

  // 初期化
  useEffect(() => {
    isEmbeddingModelReady()
      .then(setModelReady)
      .catch(() => setModelReady(false));
    isLlmReady()
      .then(setLlmReady)
      .catch(() => setLlmReady(false));
    listAvailableModels()
      .then((models) => {
        setLlmModels(models);
        if (models.length > 0) setSelectedModel(models[0].filename);
      })
      .catch(() => {});
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

  // ベクトルインデックス構築進捗のリスナー
  useEffect(() => {
    const unlisten = listen<VectorIndexProgress>("vector-index-progress", (event) => {
      const p = event.payload;
      const pct = p.total > 0 ? Math.round((p.current / p.total) * 100) : 0;
      setVectorProgress(`${pct}%`);
    });
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

  const handleSelectFolder = useCallback(async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected) {
        setFolderPath(selected as string);
        setIsIndexing(true);
        setError(null);

        const indexPath = (await appDataDir()) + "/index/fulltext";
        const count = await buildIndex(selected as string, indexPath);
        setIndexCount(count);
        setIsIndexing(false);

        if (modelReady && count > 0) {
          await triggerBuildVectorIndex();
        }
      }
    } catch (e) {
      setError(String(e));
      setIsIndexing(false);
    }
  }, [modelReady, triggerBuildVectorIndex]);

  const handleDownloadEmbeddingModel = useCallback(async () => {
    try {
      setIsDownloading(true);
      setError(null);
      setDownloadStatus("ダウンロード開始...");
      await downloadEmbeddingModel();
      setModelReady(true);
      setIsDownloading(false);
      setDownloadStatus("");

      if (indexCount > 0) {
        await triggerBuildVectorIndex();
      }
    } catch (e) {
      setError(String(e));
      setIsDownloading(false);
      setDownloadStatus("");
    }
  }, [indexCount, triggerBuildVectorIndex]);

  const handleDownloadAndLoadLlm = useCallback(async () => {
    const model = llmModels.find((m) => m.filename === selectedModel);
    if (!model) return;

    try {
      setIsLoadingLlm(true);
      setError(null);
      setDownloadStatus("LLMモデルダウンロード中...");
      await downloadLlmModel(model.filename, model.url);
      setDownloadStatus("モデルロード中...");
      await loadLlmModel(model.filename);
      setLlmReady(true);
      setIsLoadingLlm(false);
      setDownloadStatus("");
    } catch (e) {
      setError(String(e));
      setIsLoadingLlm(false);
      setDownloadStatus("");
    }
  }, [llmModels, selectedModel]);

  const handleSearch = useCallback(
    async (query: string) => {
      try {
        setError(null);
        let searchResults: SearchResult[];
        if (searchMode === "hybrid") {
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
    [searchMode],
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

  const canHybridSearch = modelReady && vectorChunkCount > 0;

  return (
    <div className="app">
      <Sidebar>
        <button onClick={handleSelectFolder} disabled={isIndexing || isBuildingVector}>
          フォルダを選択
        </button>
        {folderPath && <p className="folder-path">{folderPath}</p>}
        {indexCount > 0 && <p className="index-count">{indexCount} 件のファイル</p>}

        <hr className="sidebar-divider" />

        {!modelReady && indexCount > 0 && (
          <button onClick={handleDownloadEmbeddingModel} disabled={isDownloading}>
            {isDownloading ? "ダウンロード中..." : "Embeddingモデル取得"}
          </button>
        )}

        {isBuildingVector && <p className="status-ok">ベクトルインデックス: 構築中{vectorProgress && ` ${vectorProgress}`}</p>}
        {!isBuildingVector && vectorChunkCount > 0 && <p className="status-ok">ベクトルインデックス: {vectorChunkCount} チャンク登録済み</p>}

        <hr className="sidebar-divider" />

        <div className="llm-section">
          <select
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
            disabled={isLoadingLlm}
          >
            {llmModels.map((m) => (
              <option key={m.filename} value={m.filename}>
                {m.name}
              </option>
            ))}
          </select>
          <button onClick={handleDownloadAndLoadLlm} disabled={isLoadingLlm || !selectedModel}>
            {isLoadingLlm ? "準備中..." : llmReady ? "モデル切替" : "LLMモデル取得・ロード"}
          </button>
          {llmReady && <p className="status-ok">LLMモデル: 準備完了</p>}
        </div>

        {(isDownloading || isLoadingLlm) && <p className="progress-text">{downloadStatus}</p>}

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

        {appMode === "search" && (
          <div className="search-mode-selector">
            <label>
              <input
                type="radio"
                name="searchMode"
                value="fulltext"
                checked={searchMode === "fulltext"}
                onChange={() => setSearchMode("fulltext")}
              />
              全文検索
            </label>
            <label>
              <input
                type="radio"
                name="searchMode"
                value="hybrid"
                checked={searchMode === "hybrid"}
                onChange={() => setSearchMode("hybrid")}
                disabled={!canHybridSearch}
              />
              ハイブリッド検索
            </label>
          </div>
        )}
      </Sidebar>
      <MainPanel>
        {appMode === "search" ? (
          <>
            <SearchBar onSearch={handleSearch} disabled={indexCount === 0} />
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
            />
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
