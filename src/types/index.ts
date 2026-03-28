/** 検索結果の1件を表す */
export type SearchResult = {
  path: string;
  title: string;
  snippet: string;
  score: number;
};

/** インデックスの状態 */
export type IndexStatus = {
  file_count: number;
  index_path: string;
  is_ready: boolean;
};
