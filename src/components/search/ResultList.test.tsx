import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ResultList } from "./ResultList";
import type { SearchResult } from "../../types";

const mockResults: SearchResult[] = [
  {
    path: "/docs/rust.md",
    title: "rust.md",
    snippet: "<b>Rust</b>は安全な言語",
    score: 1.5,
  },
  {
    path: "/docs/tauri.md",
    title: "tauri.md",
    snippet: "<b>Rust</b>で書かれたフレームワーク",
    score: 1.2,
  },
];

describe("ResultList", () => {
  it("検索結果を表示する", () => {
    render(<ResultList results={mockResults} onSelect={vi.fn()} />);
    expect(screen.getByText("rust.md")).toBeInTheDocument();
    expect(screen.getByText("tauri.md")).toBeInTheDocument();
  });

  it("結果クリックでonSelectが呼ばれる", () => {
    const onSelect = vi.fn();
    render(<ResultList results={mockResults} onSelect={onSelect} />);
    fireEvent.click(screen.getByText("rust.md"));
    expect(onSelect).toHaveBeenCalledWith(mockResults[0]);
  });

  it("結果が空の場合はメッセージを表示する", () => {
    render(<ResultList results={[]} onSelect={vi.fn()} />);
    expect(screen.getByText("検索結果がありません")).toBeInTheDocument();
  });

  it("結果がnullの場合は何も表示しない", () => {
    const { container } = render(<ResultList results={null} onSelect={vi.fn()} />);
    expect(container.querySelector(".result-list")).toBeInTheDocument();
    expect(container.querySelector(".result-item")).not.toBeInTheDocument();
  });
});
