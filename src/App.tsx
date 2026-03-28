import { useState, useCallback, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { appDataDir } from "@tauri-apps/api/path";
import { listen } from "@tauri-apps/api/event";
import { Sidebar } from "./components/layout/Sidebar";
import { MainPanel } from "./components/layout/MainPanel";
import { SearchBar } from "./components/search/SearchBar";
import { ResultList } from "./components/search/ResultList";
import { Preview } from "./components/search/Preview";
import {
  buildIndex,
  search,
  hybridSearch,
  readFileContent,
  isEmbeddingModelReady,
  downloadEmbeddingModel,
  buildVectorIndex,
} from "./lib/tauri";
import type { SearchResult, DownloadProgress, VectorIndexProgress, SearchMode } from "./types";

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

  // モデル状態の確認
  useEffect(() => {
    isEmbeddingModelReady()
      .then(setModelReady)
      .catch(() => setModelReady(false));
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
      setVectorProgress(`${p.current} / ${p.total} チャンク`);
    });
    return () => {
      unlisten.then((f) => f());
    };
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
      }
    } catch (e) {
      setError(String(e));
      setIsIndexing(false);
    }
  }, []);

  const handleDownloadModel = useCallback(async () => {
    try {
      setIsDownloading(true);
      setError(null);
      setDownloadStatus("ダウンロード開始...");
      await downloadEmbeddingModel();
      setModelReady(true);
      setIsDownloading(false);
      setDownloadStatus("");
    } catch (e) {
      setError(String(e));
      setIsDownloading(false);
      setDownloadStatus("");
    }
  }, []);

  const handleBuildVectorIndex = useCallback(async () => {
    try {
      setIsBuildingVector(true);
      setError(null);
      setVectorProgress("準備中...");
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

  const handleSelectResult = useCallback(async (result: SearchResult) => {
    try {
      const content = await readFileContent(result.path);
      setPreviewTitle(result.title);
      setPreviewContent(content);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const canHybridSearch = modelReady && vectorChunkCount > 0;

  return (
    <div className="app">
      <Sidebar>
        <button onClick={handleSelectFolder} disabled={isIndexing}>
          {isIndexing ? "インデックス構築中..." : "フォルダを選択"}
        </button>
        {folderPath && <p className="folder-path">{folderPath}</p>}
        {indexCount > 0 && <p className="index-count">{indexCount} 件のファイル</p>}

        <hr className="sidebar-divider" />

        {!modelReady && (
          <button onClick={handleDownloadModel} disabled={isDownloading}>
            {isDownloading ? "ダウンロード中..." : "Embeddingモデル取得"}
          </button>
        )}
        {isDownloading && <p className="progress-text">{downloadStatus}</p>}
        {modelReady && <p className="status-ok">Embeddingモデル: 準備完了</p>}

        {modelReady && indexCount > 0 && (
          <button onClick={handleBuildVectorIndex} disabled={isBuildingVector}>
            {isBuildingVector ? "構築中..." : "ベクトルインデックス構築"}
          </button>
        )}
        {isBuildingVector && <p className="progress-text">{vectorProgress}</p>}
        {vectorChunkCount > 0 && <p className="status-ok">{vectorChunkCount} チャンク登録済み</p>}

        <hr className="sidebar-divider" />

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
      </Sidebar>
      <MainPanel>
        <SearchBar onSearch={handleSearch} disabled={indexCount === 0} />
        {error && <p className="error-message">{error}</p>}
        <div className="content-area">
          <ResultList results={results} onSelect={handleSelectResult} />
          <Preview title={previewTitle} content={previewContent} />
        </div>
      </MainPanel>
    </div>
  );
}

export default App;
