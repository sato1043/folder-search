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

/** LLMモデル情報 */
export type LlmModelInfo = {
  name: string;
  filename: string;
  url: string;
  size_bytes: number;
  min_vram_mb: number;
  params: string;
  quantization: string;
};

/** RAG回答結果 */
export type RagAnswer = {
  answer: string;
  sources: string[];
};

/** GPU情報 */
export type GpuInfo = {
  name: string;
  vram_mb: number;
};

/** システム情報 */
export type SystemInfo = {
  total_ram_mb: number;
  gpus: GpuInfo[];
  gpu_inference_available: boolean;
};

/** モデル推奨ステータス */
export type RecommendationStatus = "Recommended" | "Warning" | "TooLarge";

/** モデル推奨情報 */
export type ModelRecommendation = {
  filename: string;
  status: RecommendationStatus;
  is_best_fit: boolean;
  reason: string;
};
