import type { ReactNode } from "react";

type MainPanelProps = {
  children?: ReactNode;
};

export function MainPanel({ children }: MainPanelProps) {
  return <main className="main-panel">{children}</main>;
}
