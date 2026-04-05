import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { FolderSelector } from "./FolderSelector";
import type { IndexedFolderInfo } from "../../types";

const folders: IndexedFolderInfo[] = [
  { folder_path: "/home/user/docs", has_fulltext: true, vector_complete: true },
  { folder_path: "/home/user/notes", has_fulltext: true, vector_complete: false },
];

describe("FolderSelector", () => {
  it("フォルダ一覧をドロップダウンに表示する", () => {
    render(
      <FolderSelector
        folders={folders}
        currentFolder={null}
        onSelectExisting={vi.fn()}
        onSelectNew={vi.fn()}
      />,
    );
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(3); // 2 folders + "新しいフォルダを選択..."
    expect(options[0]).toHaveTextContent("/home/user/docs");
    expect(options[1]).toHaveTextContent("/home/user/notes");
    expect(options[2]).toHaveTextContent("新しいフォルダを選択...");
  });

  it("未選択時にプレースホルダーを表示する", () => {
    render(
      <FolderSelector
        folders={folders}
        currentFolder={null}
        onSelectExisting={vi.fn()}
        onSelectNew={vi.fn()}
      />,
    );
    expect(screen.getByText("フォルダを選択...")).toBeInTheDocument();
  });

  it("フォルダ選択時にプレースホルダーを表示しない", () => {
    render(
      <FolderSelector
        folders={folders}
        currentFolder="/home/user/docs"
        onSelectExisting={vi.fn()}
        onSelectNew={vi.fn()}
      />,
    );
    expect(screen.queryByText("フォルダを選択...")).not.toBeInTheDocument();
  });

  it("既存フォルダを選択するとonSelectExistingが呼ばれる", () => {
    const onSelectExisting = vi.fn();
    render(
      <FolderSelector
        folders={folders}
        currentFolder={null}
        onSelectExisting={onSelectExisting}
        onSelectNew={vi.fn()}
      />,
    );
    const select = screen.getByRole("combobox");
    fireEvent.change(select, { target: { value: "/home/user/docs" } });
    expect(onSelectExisting).toHaveBeenCalledWith("/home/user/docs");
  });

  it("「新しいフォルダを選択...」を選ぶとonSelectNewが呼ばれる", () => {
    const onSelectNew = vi.fn();
    render(
      <FolderSelector
        folders={folders}
        currentFolder={null}
        onSelectExisting={vi.fn()}
        onSelectNew={onSelectNew}
      />,
    );
    const select = screen.getByRole("combobox");
    fireEvent.change(select, { target: { value: "__new__" } });
    expect(onSelectNew).toHaveBeenCalled();
  });

  it("disabled時にselectが無効化される", () => {
    render(
      <FolderSelector
        folders={folders}
        currentFolder={null}
        disabled
        onSelectExisting={vi.fn()}
        onSelectNew={vi.fn()}
      />,
    );
    expect(screen.getByRole("combobox")).toBeDisabled();
  });

  it("フォルダ一覧が空でも「新しいフォルダを選択...」が表示される", () => {
    render(
      <FolderSelector
        folders={[]}
        currentFolder={null}
        onSelectExisting={vi.fn()}
        onSelectNew={vi.fn()}
      />,
    );
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(1);
    expect(options[0]).toHaveTextContent("新しいフォルダを選択...");
  });
});
