import type { ReactNode } from "react";

type SidebarProps = {
  children?: ReactNode;
};

export function Sidebar({ children }: SidebarProps) {
  return (
    <aside className="sidebar">
      <h2>Folder Search</h2>
      {children}
    </aside>
  );
}
