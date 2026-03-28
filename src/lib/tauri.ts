import { invoke } from "@tauri-apps/api/core";
import type { SearchResult, IndexStatus } from "../types";

export async function buildIndex(folderPath: string, indexPath: string): Promise<number> {
  return invoke<number>("build_index", {
    folderPath,
    indexPath,
  });
}

export async function search(query: string, limit: number = 20): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("search", { query, limit });
}

export async function getIndexStatus(): Promise<IndexStatus> {
  return invoke<IndexStatus>("get_index_status");
}

export async function readFileContent(path: string): Promise<string> {
  return invoke<string>("read_file_content", { path });
}
