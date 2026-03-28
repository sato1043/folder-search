type PreviewProps = {
  title: string | null;
  content: string | null;
};

export function Preview({ title, content }: PreviewProps) {
  if (title === null || content === null) {
    return (
      <div className="preview">
        <p className="preview-placeholder">ファイルを選択してください</p>
      </div>
    );
  }

  return (
    <div className="preview">
      <div className="preview-header">{title}</div>
      <pre className="preview-content">{content}</pre>
    </div>
  );
}
