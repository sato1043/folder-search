import { invoke } from "@tauri-apps/api/core";
import type { SearchResult, HybridSearchResult, IndexStatus } from "../types";
import type {
  LlmModelInfo,
  RagAnswer,
  SystemInfo,
  ModelRecommendation,
  LlmLoadResult,
  DownloadedModelInfo,
  StorageUsage,
} from "../types";

export async function buildIndex(folderPath: string, indexPath: string): Promise<number> {
  return invoke<number>("build_index", { folderPath, indexPath });
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

export async function downloadLlmModel(filename: string, url: string): Promise<void> {
  return invoke<void>("download_llm_model", { filename, url });
}

export async function loadLlmModel(filename: string): Promise<LlmLoadResult> {
  return invoke<LlmLoadResult>("load_llm_model", { filename });
}

export async function isLlmReady(): Promise<boolean> {
  return invoke<boolean>("is_llm_ready");
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
