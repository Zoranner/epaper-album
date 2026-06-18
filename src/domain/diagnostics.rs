use crate::model::LocalDate;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub const LOGS_DIR: &str = "/sdcard/data/logs";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticEvent {
    pub time: u64,
    pub run: u64,
    pub level: DiagnosticLevel,
    pub event: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub data: Map<String, Value>,
}

impl DiagnosticEvent {
    pub fn new(
        time: u64,
        run: u64,
        level: DiagnosticLevel,
        event: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            time,
            run,
            level,
            event: event.into(),
            message: message.into(),
            data: Map::new(),
        }
    }

    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLogWrite {
    Written,
    SerializeError,
    WriteError,
}

pub fn daily_log_path(date: LocalDate) -> PathBuf {
    Path::new(LOGS_DIR).join(format!("{date}.jsonl"))
}

pub fn event_to_jsonl(event: &DiagnosticEvent) -> Result<String, serde_json::Error> {
    let mut line = serde_json::to_string(event)?;
    line.push('\n');
    Ok(line)
}

pub fn append_event_to_file(path: impl AsRef<Path>, event: &DiagnosticEvent) -> DiagnosticLogWrite {
    let line = match event_to_jsonl(event) {
        Ok(line) => line,
        Err(_) => return DiagnosticLogWrite::SerializeError,
    };

    append_line(path.as_ref(), &line)
}

fn append_line(path: &Path, line: &str) -> DiagnosticLogWrite {
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return DiagnosticLogWrite::WriteError;
        }
    }

    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| {
            use std::io::Write;

            file.write_all(line.as_bytes())
        }) {
        Ok(()) => DiagnosticLogWrite::Written,
        Err(_) => DiagnosticLogWrite::WriteError,
    }
}

pub fn remove_logs_older_than(
    logs_dir: impl AsRef<Path>,
    today: LocalDate,
    keep_days: u8,
) -> Result<(), std::io::Error> {
    let logs_dir = logs_dir.as_ref();
    if !logs_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(logs_dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(date_text) = file_name.strip_suffix(".jsonl") else {
            continue;
        };
        let Ok(date) = LocalDate::parse(date_text) else {
            continue;
        };
        if days_between(date, today).is_some_and(|days| days >= keep_days as u32) {
            std::fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn days_between(start: LocalDate, end: LocalDate) -> Option<u32> {
    let mut date = start;
    for days in 0..=3660 {
        if date == end {
            return Some(days);
        }
        if date > end {
            return None;
        }
        date = date.next_day();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> LocalDate {
        LocalDate::parse(value).unwrap()
    }

    #[test]
    fn serializes_event_as_single_json_line() {
        let event = DiagnosticEvent::new(
            1781373609,
            1781373600,
            DiagnosticLevel::Info,
            "sync",
            "sync succeeded",
        )
        .with_data("action", "Fetch")
        .with_data("date", "2026-06-14");

        let line = event_to_jsonl(&event).unwrap();

        assert!(line.ends_with('\n'));
        assert_eq!(line.lines().count(), 1);
        assert!(line.contains(r#""event":"sync""#));
        assert!(line.contains(r#""level":"info""#));
        assert!(line.contains(r#""action":"Fetch""#));
    }

    #[test]
    fn appends_events_to_daily_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("logs").join("2026-06-14.jsonl");
        let first = DiagnosticEvent::new(1, 1, DiagnosticLevel::Info, "wake", "timer wake");
        let second = DiagnosticEvent::new(2, 1, DiagnosticLevel::Warn, "sync", "sync failed");

        assert_eq!(
            append_event_to_file(&path, &first),
            DiagnosticLogWrite::Written
        );
        assert_eq!(
            append_event_to_file(&path, &second),
            DiagnosticLogWrite::Written
        );

        let content = std::fs::read_to_string(path).unwrap();
        let lines = content.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains(r#""event":"wake""#));
        assert!(lines[1].contains(r#""event":"sync""#));
    }

    #[test]
    fn removes_logs_outside_retention_window() {
        let temp = tempfile::tempdir().unwrap();
        let logs = temp.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("2026-06-01.jsonl"), "old\n").unwrap();
        std::fs::write(logs.join("2026-06-10.jsonl"), "kept\n").unwrap();
        std::fs::write(logs.join("not-a-log.txt"), "kept\n").unwrap();

        remove_logs_older_than(&logs, date("2026-06-14"), 7).unwrap();

        assert!(!logs.join("2026-06-01.jsonl").exists());
        assert!(logs.join("2026-06-10.jsonl").exists());
        assert!(logs.join("not-a-log.txt").exists());
    }
}
