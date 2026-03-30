import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  it("sidebarロールを持つ", () => {
    render(<Sidebar />);
    expect(screen.getByRole("complementary")).toBeInTheDocument();
  });
});
