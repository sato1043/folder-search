import { useState, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { appDataDir } from "@tauri-apps/api/path";
import { Sidebar } from "./components/layout/Sidebar";
import { MainPanel } from "./components/layout/MainPanel";
import { SearchBar } from "./components/search/SearchBar";
import { ResultList } from "./components/search/ResultList";
import { Preview } from "./components/search/Preview";
import { buildIndex, search, readFileContent } from "./lib/tauri";
import type { SearchResult } from "./types";

function App() {
  const [folderPath, setFolderPath] = useState<string | null>(null);
  const [results, setResults] = useState<SearchResult[] | null>(null);
  const [previewTitle, setPreviewTitle] = useState<string | null>(null);
  const [previewContent, setPreviewContent] = useState<string | null>(null);
  const [indexCount, setIndexCount] = useState<number>(0);
  const [isIndexing, setIsIndexing] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  const handleSearch = useCallback(async (query: string) => {
    try {
      setError(null);
      const searchResults = await search(query);
      setResults(searchResults);
      setPreviewTitle(null);
      setPreviewContent(null);
    } catch (e) {
      setError(String(e));
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

  return (
    <div className="app">
      <Sidebar>
        <button onClick={handleSelectFolder} disabled={isIndexing}>
          {isIndexing ? "インデックス構築中..." : "フォルダを選択"}
        </button>
        {folderPath && <p className="folder-path">{folderPath}</p>}
        {indexCount > 0 && <p className="index-count">{indexCount} 件のファイル</p>}
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
