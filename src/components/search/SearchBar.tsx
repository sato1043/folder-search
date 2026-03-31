import { useState, type ReactNode } from "react";

type SearchBarProps = {
  onSearch: (query: string) => void;
  disabled?: boolean;
  placeholder?: string;
  children?: ReactNode;
};

export function SearchBar({
  onSearch,
  disabled,
  placeholder = "検索...",
  children,
}: SearchBarProps) {
  const [query, setQuery] = useState("");

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    if (e.key === "Enter" && query.trim() !== "") {
      onSearch(query.trim());
    }
  };

  return (
    <div className="search-bar">
      <input
        type="text"
        placeholder={placeholder}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
      />
      {children}
    </div>
  );
}
