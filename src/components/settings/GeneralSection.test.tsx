import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { GeneralSection } from "./GeneralSection";
import type {
  AppSettings,
  LlmModelInfo,
  ModelRecommendation,
  DownloadedModelInfo,
} from "../../types";

const GB = 1024 * 1024 * 1024;

const defaultSettings: AppSettings = {
  cache_limit_bytes: 100 * GB,
  last_loaded_model: null,
  disabled_models: [],
};

const testModels: LlmModelInfo[] = [
  {
    name: "Model A",
    filename: "model-a.gguf",
    url: "https://example.com/a",
    size_bytes: 1 * GB,
    min_vram_mb: 0,
    params: "1B",
    quantization: "Q4_K_M",
    chat_template: "chatml",
    context_length: 4096,
    is_preset: true,
  },
  {
    name: "Model B",
    filename: "model-b.gguf",
    url: "https://example.com/b",
    size_bytes: 5 * GB,
    min_vram_mb: 6144,
    params: "7B",
    quantization: "Q4_K_M",
    chat_template: "gemma",
    context_length: 32768,
    is_preset: true,
  },
];

const testRecommendations: ModelRecommendation[] = [
  { filename: "model-a.gguf", status: "Recommended", is_best_fit: true, reason: "最適" },
  { filename: "model-b.gguf", status: "Warning", is_best_fit: false, reason: "メモリ不足の可能性" },
];

const testDownloaded: DownloadedModelInfo[] = [
  { filename: "model-a.gguf", size_bytes: 1 * GB, is_embedding: false },
  { filename: "model-b.gguf", size_bytes: 5 * GB, is_embedding: false },
];

const defaultProps = {
  settings: defaultSettings,
  diskFreeBytes: 200 * GB,
  onChange: vi.fn(),
  onResetField: vi.fn(),
  systemInfo: { total_ram_mb: 16384, gpus: [], gpu_inference_available: false },
  llmModels: testModels,
  loadedModelFilename: null,
  initialLoadedModel: null,
  recommendations: testRecommendations,
  downloadedModels: testDownloaded,
  modelReady: true,
  isDownloadingEmbedding: false,
  embeddingDownloadStatus: "",
  onDownloadEmbeddingModel: vi.fn(),
  isLoadingLlm: false,
  switchingModelFilename: null,
  downloadStatus: "",
  onDownloadAndLoadLlm: vi.fn(),
};

describe("GeneralSection", () => {
  it("ベクトル検索モデルセクションを表示する", () => {
    render(<GeneralSection {...defaultProps} />);
    expect(screen.getByText("ベクトル検索モデル")).toBeInTheDocument();
  });

  it("Embeddingモデルがダウンロード済みの場合「ダウンロード済み」を表示する", () => {
    render(<GeneralSection {...defaultProps} modelReady={true} />);
    expect(screen.getByText(/ダウンロード済み/)).toBeInTheDocument();
  });

  it("Embeddingモデルが未DLの場合「未ダウンロード」とダウンロードボタンを表示する", () => {
    render(<GeneralSection {...defaultProps} modelReady={false} />);
    expect(screen.getByText(/未ダウンロード/)).toBeInTheDocument();
    expect(screen.getByTitle("Embeddingモデルをダウンロード")).toBeInTheDocument();
  });

  it("ダウンロードボタン押下でonDownloadEmbeddingModelが呼ばれる", () => {
    const onDownloadEmbeddingModel = vi.fn();
    render(
      <GeneralSection
        {...defaultProps}
        modelReady={false}
        onDownloadEmbeddingModel={onDownloadEmbeddingModel}
      />,
    );
    fireEvent.click(screen.getByTitle("Embeddingモデルをダウンロード"));
    expect(onDownloadEmbeddingModel).toHaveBeenCalled();
  });

  it("Embeddingダウンロード中は「ダウンロード中...」を表示しボタンを非表示にする", () => {
    render(
      <GeneralSection
        {...defaultProps}
        modelReady={false}
        isDownloadingEmbedding={true}
        embeddingDownloadStatus="model.onnx: 45%"
      />,
    );
    expect(screen.getByText(/ダウンロード中\.\.\./)).toBeInTheDocument();
    expect(screen.queryByTitle("Embeddingモデルをダウンロード")).not.toBeInTheDocument();
    expect(screen.getByText("model.onnx: 45%")).toBeInTheDocument();
  });

  it("モデルリストを表示する", () => {
    render(<GeneralSection {...defaultProps} />);
    expect(screen.getByText("Model A")).toBeInTheDocument();
    expect(screen.getByText("Model B")).toBeInTheDocument();
  });

  it("DL済みバッジを表示する", () => {
    render(<GeneralSection {...defaultProps} />);
    const badges = screen.getAllByText("DL済");
    expect(badges.length).toBe(2);
  });

  it("推奨バッジを表示する", () => {
    render(<GeneralSection {...defaultProps} />);
    expect(screen.getByText("最適")).toBeInTheDocument();
    expect(screen.getByText("注意")).toBeInTheDocument();
  });

  it("ロード済みモデルに「選択中」が表示される", () => {
    render(<GeneralSection {...defaultProps} loadedModelFilename="model-a.gguf" />);
    const indicators = screen.getAllByText("選択中");
    expect(indicators.length).toBe(1);
  });

  it("未ロード時は「未ロード」と表示する", () => {
    render(<GeneralSection {...defaultProps} />);
    expect(screen.getByText(/未ロード/)).toBeInTheDocument();
  });

  it("ロード済みモデル名を表示する", () => {
    render(<GeneralSection {...defaultProps} loadedModelFilename="model-a.gguf" />);
    expect(screen.getByText(/現在:.*Model A/)).toBeInTheDocument();
  });

  it("未ロードモデルの行にロードボタンがある", () => {
    render(<GeneralSection {...defaultProps} />);
    const loadBtns = screen.getAllByTitle("このモデルをロード");
    expect(loadBtns.length).toBe(2);
  });

  it("ロード済みモデルの行にはロードボタンがない", () => {
    render(<GeneralSection {...defaultProps} loadedModelFilename="model-a.gguf" />);
    const loadBtns = screen.getAllByTitle("このモデルをロード");
    expect(loadBtns.length).toBe(1);
  });

  it("ロードボタン押下でonDownloadAndLoadLlmが呼ばれる", () => {
    const onDownloadAndLoadLlm = vi.fn();
    render(<GeneralSection {...defaultProps} onDownloadAndLoadLlm={onDownloadAndLoadLlm} />);
    const loadBtns = screen.getAllByTitle("このモデルをロード");
    fireEvent.click(loadBtns[0]);
    expect(onDownloadAndLoadLlm).toHaveBeenCalledWith("model-a.gguf");
  });

  it("スライダー値をGB表示する", () => {
    render(<GeneralSection {...defaultProps} />);
    expect(screen.getByText("100 GB")).toBeInTheDocument();
  });

  it("切替中の表示がある", () => {
    render(
      <GeneralSection
        {...defaultProps}
        isLoadingLlm={true}
        switchingModelFilename="model-b.gguf"
        downloadStatus="model-b.gguf: 45%"
      />,
    );
    expect(screen.getByText(/切替中:.*Model B/)).toBeInTheDocument();
    expect(screen.getByText("model-b.gguf: 45%")).toBeInTheDocument();
  });

  it("切替中でなければ切替中表示がない", () => {
    render(<GeneralSection {...defaultProps} />);
    expect(screen.queryByText(/切替中/)).not.toBeInTheDocument();
  });

  it("システム情報を表示する", () => {
    render(<GeneralSection {...defaultProps} />);
    expect(screen.getByText(/RAM: 16 GB/)).toBeInTheDocument();
  });

  it("モデル変更後にリセットボタンが表示される", () => {
    render(
      <GeneralSection
        {...defaultProps}
        loadedModelFilename="model-b.gguf"
        initialLoadedModel="model-a.gguf"
      />,
    );
    expect(screen.getByTitle("ダイアログを開いた時点のモデルに戻す")).toBeInTheDocument();
  });

  it("モデル未変更ならリセットボタンが表示されない", () => {
    render(
      <GeneralSection
        {...defaultProps}
        loadedModelFilename="model-a.gguf"
        initialLoadedModel="model-a.gguf"
      />,
    );
    expect(screen.queryByTitle("ダイアログを開いた時点のモデルに戻す")).not.toBeInTheDocument();
  });

  it("無効化モデルはリストに表示されない", () => {
    const settingsWithDisabled = {
      ...defaultSettings,
      disabled_models: ["model-a.gguf"],
    };
    render(
      <GeneralSection
        {...defaultProps}
        settings={settingsWithDisabled}
        downloadedModels={[
          { filename: "model-a.gguf", size_bytes: 1 * GB, is_embedding: false },
          { filename: "model-b.gguf", size_bytes: 5 * GB, is_embedding: false },
        ]}
      />,
    );
    expect(screen.queryByText("Model A")).not.toBeInTheDocument();
    expect(screen.getByText("Model B")).toBeInTheDocument();
  });

  it("未DLモデルはリストに表示されない", () => {
    render(
      <GeneralSection
        {...defaultProps}
        downloadedModels={[{ filename: "model-a.gguf", size_bytes: 1 * GB, is_embedding: false }]}
      />,
    );
    expect(screen.getByText("Model A")).toBeInTheDocument();
    expect(screen.queryByText("Model B")).not.toBeInTheDocument();
  });

  it("リセットボタン押下でonDownloadAndLoadLlmが呼ばれる", () => {
    const onDownloadAndLoadLlm = vi.fn();
    render(
      <GeneralSection
        {...defaultProps}
        loadedModelFilename="model-b.gguf"
        initialLoadedModel="model-a.gguf"
        onDownloadAndLoadLlm={onDownloadAndLoadLlm}
      />,
    );
    fireEvent.click(screen.getByTitle("ダイアログを開いた時点のモデルに戻す"));
    expect(onDownloadAndLoadLlm).toHaveBeenCalledWith("model-a.gguf");
  });
});
