import type { SearchResult } from "../../types";

type ResultListProps = {
  results: SearchResult[] | null;
  onSelect: (result: SearchResult) => void;
};

export function ResultList({ results, onSelect }: ResultListProps) {
  return (
    <div className="result-list">
      {results !== null && results.length === 0 && (
        <p className="result-empty">検索結果がありません</p>
      )}
      {results?.map((result) => (
        <div
          key={result.path}
          className="result-item"
          onClick={() => onSelect(result)}
          role="button"
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "Enter") onSelect(result);
          }}
        >
          <div className="result-title">{result.title}</div>
          <div className="result-snippet" dangerouslySetInnerHTML={{ __html: result.snippet }} />
          <div className="result-path">{result.path}</div>
        </div>
      ))}
    </div>
  );
}
