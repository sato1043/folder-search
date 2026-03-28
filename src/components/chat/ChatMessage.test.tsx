import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ChatMessage } from "./ChatMessage";

describe("ChatMessage", () => {
  it("回答テキストを表示する", () => {
    render(<ChatMessage answer="Rustは安全な言語です" sources={[]} onSourceClick={vi.fn()} />);
    expect(screen.getByText("Rustは安全な言語です")).toBeInTheDocument();
  });

  it("参照元ファイルリンクを表示する", () => {
    render(
      <ChatMessage
        answer="回答"
        sources={["/docs/rust.md", "/docs/tauri.md"]}
        onSourceClick={vi.fn()}
      />,
    );
    expect(screen.getByText("/docs/rust.md")).toBeInTheDocument();
    expect(screen.getByText("/docs/tauri.md")).toBeInTheDocument();
  });

  it("参照元クリックでonSourceClickが呼ばれる", () => {
    const onSourceClick = vi.fn();
    render(<ChatMessage answer="回答" sources={["/docs/rust.md"]} onSourceClick={onSourceClick} />);
    fireEvent.click(screen.getByText("/docs/rust.md"));
    expect(onSourceClick).toHaveBeenCalledWith("/docs/rust.md");
  });

  it("回答がnullの場合はプレースホルダーを表示する", () => {
    render(<ChatMessage answer={null} sources={[]} onSourceClick={vi.fn()} />);
    expect(screen.getByText("質問を入力してください")).toBeInTheDocument();
  });
});
