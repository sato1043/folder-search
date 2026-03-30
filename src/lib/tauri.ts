import { invoke } from "@tauri-apps/api/core";
import type { SearchResult, HybridSearchResult, IndexStatus, FolderScanResult } from "../types";
import type {
  AppSettings,
  LlmModelInfo,
  RagAnswer,
  SystemInfo,
  ModelRecommendation,
  LlmLoadResult,
  DownloadedModelInfo,
  StorageUsage,
} from "../types";

export async function scanFolder(folderPath: string): Promise<FolderScanResult> {
  return invoke<FolderScanResult>("scan_folder", { folderPath });
}

export async function cancelIndexing(): Promise<void> {
  return invoke<void>("cancel_indexing");
}

export async function buildIndex(
  folderPath: string,
  indexPath: string,
  totalFiles: number,
): Promise<number> {
  return invoke<number>("build_index", { folderPath, indexPath, totalFiles });
}

export async function search(query: string, limit: number = 20): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("search", { query, limit });
}

export async function hybridSearch(
  query: string,
  limit: number = 20,
): Promise<HybridSearchResult[]> {
  return invoke<HybridSearchResult[]>("hybrid_search", { query, limit });
}

export async function getIndexStatus(): Promise<IndexStatus> {
  return invoke<IndexStatus>("get_index_status");
}

export async function readFileContent(path: string): Promise<string> {
  return invoke<string>("read_file_content", { path });
}

export async function isEmbeddingModelReady(): Promise<boolean> {
  return invoke<boolean>("is_embedding_model_ready");
}

export async function downloadEmbeddingModel(): Promise<void> {
  return invoke<void>("download_embedding_model");
}

export async function buildVectorIndex(): Promise<number> {
  return invoke<number>("build_vector_index");
}

export async function listAvailableModels(): Promise<LlmModelInfo[]> {
  return invoke<LlmModelInfo[]>("list_available_models");
}

export async function downloadLlmModel(
  filename: string,
  url: string,
  sizeBytes: number,
): Promise<string[]> {
  return invoke<string[]>("download_llm_model", { filename, url, sizeBytes });
}

export async function loadLlmModel(
  filename: string,
  chatTemplate: string,
  contextLength: number,
): Promise<LlmLoadResult> {
  return invoke<LlmLoadResult>("load_llm_model", {
    filename,
    chatTemplate,
    contextLength,
  });
}

export async function isLlmReady(): Promise<boolean> {
  return invoke<boolean>("is_llm_ready");
}

export async function getLoadedModelFilename(): Promise<string | null> {
  return invoke<string | null>("get_loaded_model_filename");
}

export async function chat(question: string): Promise<RagAnswer> {
  return invoke<RagAnswer>("chat", { question });
}

export async function detectSystemInfo(): Promise<SystemInfo> {
  return invoke<SystemInfo>("detect_system_info");
}

export async function getModelRecommendations(): Promise<ModelRecommendation[]> {
  return invoke<ModelRecommendation[]>("get_model_recommendations");
}

export async function listDownloadedModels(): Promise<DownloadedModelInfo[]> {
  return invoke<DownloadedModelInfo[]>("list_downloaded_models");
}

export async function deleteModel(filename: string): Promise<void> {
  return invoke<void>("delete_model", { filename });
}

export async function getStorageUsage(): Promise<StorageUsage> {
  return invoke<StorageUsage>("get_storage_usage");
}

export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

export async function clearModelCache(): Promise<string[]> {
  return invoke<string[]>("clear_model_cache");
}

export async function registerCustomModel(model: LlmModelInfo): Promise<void> {
  return invoke<void>("register_custom_model", { model });
}

export async function unregisterCustomModel(filename: string): Promise<void> {
  return invoke<void>("unregister_custom_model", { filename });
}

export type IndexValidationResult = {
  fulltext_removed: boolean;
  vector_cache_removed: boolean;
};

export async function validateFolderIndexes(folderPath: string): Promise<IndexValidationResult> {
  return invoke<IndexValidationResult>("validate_folder_indexes", { folderPath });
}
