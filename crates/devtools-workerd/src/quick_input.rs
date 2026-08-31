use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::platform;

const HISTORY_FILE: &str = "quick-input-history.jsonl";
const MAX_LOADED_HISTORY: usize = 500;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryRecord {
    timestamp_ms: u128,
    text: String,
}

/// 快速输入历史以一条记录一行的 JSONL 文件保存。
pub struct HistoryStore {
    path: PathBuf,
}

impl HistoryStore {
    pub fn from_environment() -> io::Result<Self> {
        Ok(Self {
            path: platform::data_root_from_environment()?
                .join("devtools")
                .join(HISTORY_FILE),
        })
    }

    #[cfg(test)]
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append(&self, text: &str) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let record = HistoryRecord {
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            text: text.to_owned(),
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, &record)?;
        writeln!(file)
    }

    pub fn load(&self) -> Vec<String> {
        let Ok(file) = File::open(&self.path) else {
            return Vec::new();
        };
        let mut values = BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str::<HistoryRecord>(&line).ok())
            .map(|record| record.text)
            .collect::<Vec<_>>();
        if values.len() > MAX_LOADED_HISTORY {
            values.drain(..values.len() - MAX_LOADED_HISTORY);
        }
        values
    }

    #[cfg(test)]
    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn appends_and_loads_jsonl_history_while_ignoring_invalid_lines() {
        let directory = tempdir().expect("应创建临时目录");
        let store = HistoryStore::new(directory.path().join(HISTORY_FILE));
        store.append("hello").expect("应写入第一条记录");
        store.append("世界").expect("应写入第二条记录");
        let mut file = OpenOptions::new()
            .append(true)
            .open(store.path())
            .expect("应打开历史文件");
        writeln!(file, "not-json").expect("应写入无效测试行");

        assert_eq!(store.load(), vec!["hello", "世界"]);
        let contents = fs::read_to_string(store.path()).expect("应读取历史文件");
        assert!(contents
            .lines()
            .next()
            .expect("应有首行")
            .contains("timestampMs"));
    }

    #[test]
    fn missing_history_is_empty() {
        let directory = tempdir().expect("应创建临时目录");
        assert!(HistoryStore::new(directory.path().join("missing.jsonl"))
            .load()
            .is_empty());
    }

    #[test]
    fn history_loader_keeps_the_most_recent_records() {
        let directory = tempdir().expect("应创建临时目录");
        let store = HistoryStore::new(directory.path().join(HISTORY_FILE));
        for index in 0..=MAX_LOADED_HISTORY {
            store.append(&index.to_string()).expect("应写入历史记录");
        }
        let values = store.load();
        assert_eq!(values.len(), MAX_LOADED_HISTORY);
        assert_eq!(values.first().map(String::as_str), Some("1"));
    }
}
