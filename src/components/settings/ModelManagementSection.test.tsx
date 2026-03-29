import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ModelManagementSection } from "./ModelManagementSection";
import type { LlmModelInfo, DownloadedModelInfo, StorageUsage } from "../../types";

const GB = 1024 * 1024 * 1024;

const testDownloaded: DownloadedModelInfo[] = [
  { filename: "preset.gguf", size_bytes: 1 * GB, is_embedding: false },
  { filename: "model.onnx", size_bytes: 0.5 * GB, is_embedding: true },
];

const testStorageUsage: StorageUsage = {
  total_used_bytes: 1.5 * GB,
  disk_free_bytes: 100 * GB,
  cache_limit_bytes: 100 * GB,
};

const testModels: LlmModelInfo[] = [
  {
    name: "Preset Model",
    filename: "preset.gguf",
    url: "",
    size_bytes: 1 * GB,
    min_vram_mb: 0,
    params: "1B",
    quantization: "Q4",
    chat_template: "chatml",
    context_length: 4096,
    is_preset: true,
  },
  {
    name: "Custom Model",
    filename: "custom.gguf",
    url: "",
    size_bytes: 2 * GB,
    min_vram_mb: 0,
    params: "2B",
    quantization: "Q4",
    chat_template: "gemma",
    context_length: 8192,
    is_preset: false,
  },
];

const defaultSettings = {
  cache_limit_bytes: 100 * GB,
  last_loaded_model: null,
  disabled_models: [] as string[],
};

const defaultProps = {
  settings: defaultSettings,
  onChangeSettings: vi.fn(),
  downloadedModels: testDownloaded,
  storageUsage: testStorageUsage,
  llmModels: testModels,
  recommendations: [],
  loadedModelFilename: null,
  isLoadingLlm: false,
  isDownloading: false,
  downloadStatus: "",
  onDownloadModel: vi.fn(),
  onDeleteModel: vi.fn(),
  onRegisterCustomModel: vi.fn(),
  onUnregisterCustomModel: vi.fn(),
};

describe("ModelManagementSection", () => {
  it("全モデルを表示する", () => {
    render(<ModelManagementSection {...defaultProps} />);
    expect(screen.getByText("Preset Model")).toBeInTheDocument();
  });

  it("DL済みモデルは削除ボタン、未DLモデルはダウンロードボタンを表示", () => {
    render(<ModelManagementSection {...defaultProps} />);
    // preset.gguf はDL済み → 削除ボタン
    expect(screen.getAllByTitle("削除").length).toBe(1);
    // custom.gguf は未DL → ダウンロードボタン
    expect(screen.getAllByTitle("ダウンロード").length).toBe(1);
  });

  it("DL済みモデルのチェックボックスは有効、未DLは無効", () => {
    render(<ModelManagementSection {...defaultProps} />);
    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes.length).toBe(2);
    // ソート順: Custom Model(C) → Preset Model(P)
    // custom.gguf(未DL) → disabled
    expect(checkboxes[0]).toBeDisabled();
    // preset.gguf(DL済み) → enabled
    expect(checkboxes[1]).not.toBeDisabled();
  });

  it("ストレージ使用量を表示する", () => {
    render(<ModelManagementSection {...defaultProps} />);
    expect(screen.getByText(/モデル合計:.*1\.5 GB/)).toBeInTheDocument();
  });

  it("カスタムモデルの登録解除ボタンを表示する", () => {
    render(<ModelManagementSection {...defaultProps} />);
    const unregisterBtn = screen.getByTitle("登録解除");
    expect(unregisterBtn).toBeInTheDocument();
  });

  it("カスタムモデル登録フォームのトグル", () => {
    render(<ModelManagementSection {...defaultProps} />);
    expect(screen.queryByPlaceholderText("モデル名")).not.toBeInTheDocument();
    fireEvent.click(screen.getByText("+ カスタムモデル登録"));
    expect(screen.getByPlaceholderText("モデル名")).toBeInTheDocument();
    fireEvent.click(screen.getByText("フォームを閉じる"));
    expect(screen.queryByPlaceholderText("モデル名")).not.toBeInTheDocument();
  });
});
