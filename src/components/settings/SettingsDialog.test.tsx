import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { SettingsDialog } from "./SettingsDialog";

const GB = 1024 * 1024 * 1024;

// getSettings のモックレスポンスを設定
vi.mocked(invoke).mockImplementation(async (cmd: string) => {
  if (cmd === "get_settings") {
    return { cache_limit_bytes: 100 * GB, last_loaded_model: null };
  }
  if (cmd === "save_settings") {
    return undefined;
  }
  return undefined;
});

const defaultProps = {
  isOpen: true,
  diskFreeBytes: 200 * GB,
  systemInfo: { total_ram_mb: 16384, gpus: [], gpu_inference_available: false },
  llmModels: [],
  loadedModelFilename: null,
  recommendations: [],
  downloadedModels: [],
  storageUsage: { total_used_bytes: 0, disk_free_bytes: 200 * GB, cache_limit_bytes: 100 * GB },
  isLoadingLlm: false,
  switchingModelFilename: null,
  isDownloading: false,
  downloadStatus: "",
  onDownloadAndLoadLlm: vi.fn(),
  onDownloadModel: vi.fn(),
  onDeleteModel: vi.fn(),
  onRegisterCustomModel: vi.fn(),
  onUnregisterCustomModel: vi.fn(),
  onClose: vi.fn(),
};

describe("SettingsDialog", () => {
  it("isOpen=falseのとき何も表示しない", () => {
    render(<SettingsDialog {...defaultProps} isOpen={false} />);
    expect(screen.queryByText("設定")).not.toBeInTheDocument();
  });

  it("isOpen=trueのときダイアログを表示する", async () => {
    render(<SettingsDialog {...defaultProps} />);
    expect(await screen.findByText("設定")).toBeInTheDocument();
  });

  it("セクションナビゲーションを表示する", async () => {
    render(<SettingsDialog {...defaultProps} />);
    expect(await screen.findByText("一般")).toBeInTheDocument();
    expect(screen.getByText("モデル管理")).toBeInTheDocument();
  });

  it("モデル管理セクションに切り替えられる", async () => {
    render(<SettingsDialog {...defaultProps} />);
    await screen.findByText("設定");
    fireEvent.click(screen.getByText("モデル管理"));
    expect(screen.getByText("モデル一覧")).toBeInTheDocument();
  });

  it("閉じるボタンでonCloseが呼ばれる", async () => {
    const onClose = vi.fn();
    render(<SettingsDialog {...defaultProps} onClose={onClose} />);
    await screen.findByText("設定");
    fireEvent.click(screen.getByText("×"));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });
});
