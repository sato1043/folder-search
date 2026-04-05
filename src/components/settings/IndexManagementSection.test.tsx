import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { IndexManagementSection } from "./IndexManagementSection";
import type { IndexedFolderInfo } from "../../types";

const folders: IndexedFolderInfo[] = [
  { folder_path: "/home/user/docs", has_fulltext: true, vector_complete: true },
  { folder_path: "/home/user/notes", has_fulltext: true, vector_complete: false },
];

describe("IndexManagementSection", () => {
  it("インデックス済みフォルダを一覧表示する", () => {
    render(
      <IndexManagementSection
        indexedFolders={folders}
        currentFolder={null}
        onRebuild={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("/home/user/docs")).toBeInTheDocument();
    expect(screen.getByText("/home/user/notes")).toBeInTheDocument();
  });

  it("全文検索バッジを表示する", () => {
    render(
      <IndexManagementSection
        indexedFolders={folders}
        currentFolder={null}
        onRebuild={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    const badges = screen.getAllByText("全文検索");
    expect(badges).toHaveLength(2);
  });

  it("ベクトルバッジを該当フォルダのみ表示する", () => {
    render(
      <IndexManagementSection
        indexedFolders={folders}
        currentFolder={null}
        onRebuild={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    const badges = screen.getAllByText("ベクトル");
    expect(badges).toHaveLength(1);
  });

  it("選択中フォルダに選択中バッジを表示する", () => {
    render(
      <IndexManagementSection
        indexedFolders={folders}
        currentFolder="/home/user/docs"
        onRebuild={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("選択中")).toBeInTheDocument();
  });

  it("再構築ボタンのクリックでonRebuildが呼ばれる", () => {
    const onRebuild = vi.fn();
    render(
      <IndexManagementSection
        indexedFolders={folders}
        currentFolder={null}
        onRebuild={onRebuild}
        onDelete={vi.fn()}
      />,
    );
    const rebuildButtons = screen.getAllByTitle("インデックスを再構築");
    fireEvent.click(rebuildButtons[0]);
    expect(onRebuild).toHaveBeenCalledWith("/home/user/docs");
  });

  it("削除ボタンのクリックでonDeleteが呼ばれる", () => {
    const onDelete = vi.fn();
    render(
      <IndexManagementSection
        indexedFolders={folders}
        currentFolder={null}
        onRebuild={vi.fn()}
        onDelete={onDelete}
      />,
    );
    const deleteButtons = screen.getAllByTitle("インデックスを削除");
    fireEvent.click(deleteButtons[1]);
    expect(onDelete).toHaveBeenCalledWith("/home/user/notes");
  });

  it("フォルダ一覧が空のとき空状態メッセージを表示する", () => {
    render(
      <IndexManagementSection
        indexedFolders={[]}
        currentFolder={null}
        onRebuild={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText("インデックスなし")).toBeInTheDocument();
  });
});
