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

/** 全文検索インデックス構築進捗 */
export type FulltextIndexProgress = {
  current: number;
  total: number;
};

/** フォルダスキャン結果 */
export type FolderScanResult = {
  file_count: number;
  total_size_bytes: number;
  max_file_size_bytes: number;
  estimated_chunks: number;
  has_symlinks: boolean;
  timed_out: boolean;
};

/** インデックス作成ダイアログの状態 */
export type IndexingPhase =
  | { kind: "confirm"; scanResult: FolderScanResult }
  | { kind: "fulltext"; current: number; total: number }
  | { kind: "vector"; current: number; total: number }
  | { kind: "done"; fulltextCount: number; vectorChunks: number }
  | { kind: "cancelled"; fulltextCount: number; vectorChunks?: number };

/** チャットテンプレート */
export type ChatTemplateType = "chatml" | "gemma" | "llama3";

/** LLMモデル情報 */
export type LlmModelInfo = {
  name: string;
  filename: string;
  url: string;
  size_bytes: number;
  min_vram_mb: number;
  params: string;
  quantization: string;
  chat_template: ChatTemplateType;
  context_length: number;
  is_preset: boolean;
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

/** LLMモデルロード結果 */
export type LlmLoadResult = {
  gpu_active: boolean;
  gpu_layers: number;
};

/** ダウンロード済みモデル情報 */
export type DownloadedModelInfo = {
  filename: string;
  size_bytes: number;
  is_embedding: boolean;
};

/** アプリ設定 */
export type AppSettings = {
  cache_limit_bytes: number;
  last_loaded_model: string | null;
  disabled_models: string[];
};

/** モデルストレージ使用状況 */
export type StorageUsage = {
  total_used_bytes: number;
  disk_free_bytes: number;
  cache_limit_bytes: number;
};
