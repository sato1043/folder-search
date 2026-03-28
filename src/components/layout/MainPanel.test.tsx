import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { MainPanel } from "./MainPanel";

describe("MainPanel", () => {
  it("mainロールを持つ", () => {
    render(<MainPanel />);
    expect(screen.getByRole("main")).toBeInTheDocument();
  });

  it("子要素を表示する", () => {
    render(
      <MainPanel>
        <p>テストコンテンツ</p>
      </MainPanel>,
    );
    expect(screen.getByText("テストコンテンツ")).toBeInTheDocument();
  });
});
