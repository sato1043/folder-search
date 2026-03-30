import type { ReactNode } from "react";

type SidebarProps = {
  children?: ReactNode;
};

export function Sidebar({ children }: SidebarProps) {
  return <aside className="sidebar">{children}</aside>;
}
