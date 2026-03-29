use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::RecommendedWatcher;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};

/// ファイル監視を行うウォッチャー
pub struct FileWatcher {
    _debouncer: Debouncer<RecommendedWatcher>,
}

impl FileWatcher {
    /// フォルダの監視を開始する
    ///
    /// 対象ファイル（.txt, .md）の変更を検知し、デバウンス後にコールバックを呼び出す。
    /// コールバックには変更されたファイルパスのリストが渡される。
    pub fn start<F>(folder_path: &str, callback: F) -> Result<Self, String>
    where
        F: Fn(Vec<String>) + Send + 'static,
    {
        let callback = Arc::new(Mutex::new(callback));

        let mut debouncer = new_debouncer(
            Duration::from_secs(2),
            move |result: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                let events = match result {
                    Ok(events) => events,
                    Err(e) => {
                        eprintln!("ファイル監視エラー: {:?}", e);
                        return;
                    }
                };

                // 対象ファイル（.txt, .md）のパスを収集
                let mut changed_files: Vec<String> = events
                    .iter()
                    .filter(|e| e.kind == DebouncedEventKind::Any)
                    .filter_map(|e| {
                        let path = &e.path;
                        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        if (ext == "txt" || ext == "md") && (path.is_file() || !path.exists()) {
                            Some(path.to_string_lossy().to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                changed_files.sort();
                changed_files.dedup();

                if !changed_files.is_empty() {
                    if let Ok(cb) = callback.lock() {
                        cb(changed_files);
                    }
                }
            },
        )
        .map_err(|e| format!("デバウンサー作成失敗: {}", e))?;

        debouncer
            .watcher()
            .watch(Path::new(folder_path), notify::RecursiveMode::Recursive)
            .map_err(|e| format!("ファイル監視開始失敗: {}", e))?;

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}
