import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import App from "./App";

describe("App", () => {
  it("サイドバーを表示する", () => {
    render(<App />);
    expect(screen.getByRole("complementary")).toBeInTheDocument();
  });
});
