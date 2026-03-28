import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { Preview } from "./Preview";

describe("Preview", () => {
  it("ファイル内容を表示する", () => {
    render(<Preview title="test.md" content="# テスト内容" />);
    expect(screen.getByText("test.md")).toBeInTheDocument();
    expect(screen.getByText("# テスト内容")).toBeInTheDocument();
  });

  it("contentが空の場合はプレースホルダーを表示する", () => {
    render(<Preview title={null} content={null} />);
    expect(screen.getByText("ファイルを選択してください")).toBeInTheDocument();
  });
});
