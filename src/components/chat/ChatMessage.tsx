type ChatMessageProps = {
  answer: string | null;
  sources: string[];
  onSourceClick: (path: string) => void;
  isStreaming?: boolean;
};

export function ChatMessage({ answer, sources, onSourceClick, isStreaming }: ChatMessageProps) {
  if (answer === null) {
    return (
      <div className="chat-message">
        <p className="chat-placeholder">質問を入力してください</p>
      </div>
    );
  }

  return (
    <div className="chat-message">
      <div className="chat-answer">
        {answer}
        {isStreaming && <span className="chat-cursor">|</span>}
      </div>
      {sources.length > 0 && (
        <div className="chat-sources">
          <div className="chat-sources-label">参照元:</div>
          {sources.map((source) => (
            <button key={source} className="chat-source-link" onClick={() => onSourceClick(source)}>
              {source}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
