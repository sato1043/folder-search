import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { SearchBar } from "./SearchBar";

describe("SearchBar", () => {
  it("検索入力フィールドを表示する", () => {
    render(<SearchBar onSearch={vi.fn()} />);
    expect(screen.getByPlaceholderText("検索...")).toBeInTheDocument();
  });

  it("Enterキーで検索を実行する", () => {
    const onSearch = vi.fn();
    render(<SearchBar onSearch={onSearch} />);
    const input = screen.getByPlaceholderText("検索...");
    fireEvent.change(input, { target: { value: "Rust" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSearch).toHaveBeenCalledWith("Rust");
  });

  it("空文字では検索を実行しない", () => {
    const onSearch = vi.fn();
    render(<SearchBar onSearch={onSearch} />);
    const input = screen.getByPlaceholderText("検索...");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSearch).not.toHaveBeenCalled();
  });
});
