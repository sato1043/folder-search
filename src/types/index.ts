/** 検索結果の1件を表す */
export type SearchResult = {
  path: string;
  title: string;
  snippet: string;
  score: number;
};

/** ハイブリッド検索結果 */
export type HybridSearchResult = {
  path: string;
  title: string;
  snippet: string;
  score: number;
  source: "fulltext" | "vector" | "hybrid" | "unknown";
};

/** インデックスの状態 */
export type IndexStatus = {
  file_count: number;
  index_path: string;
  is_ready: boolean;
};

/** ダウンロード進捗 */
export type DownloadProgress = {
  file_name: string;
  downloaded_bytes: number;
  total_bytes: number | null;
  is_complete: boolean;
};

/** ベクトルインデックス構築進捗 */
export type VectorIndexProgress = {
  current: number;
  total: number;
};

/** 検索モード */
export type SearchMode = "fulltext" | "hybrid";
