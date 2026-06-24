//! Log sinks: the destinations records are written to.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, IsTerminal, Write as _};
use std::sync::{Arc, Mutex};

use crate::error::{FerroResult, IoError};

use super::level::LogLevel;
use super::record::LogRecord;

/// A destination for log records.
///
/// Sinks are stored in the global logger and shared across threads, so the
/// trait requires `Send + Sync`. Implementations that write to a shared
/// resource serialize their writes internally so that lines never interleave.
pub trait LogSink: Send + Sync {
    /// Writes a single record. `formatted` is the ready-to-print line; `record`
    /// is provided so a sink can re-format if it wants to.
    fn write_record(&self, formatted: &str, record: &LogRecord);

    /// Flushes any buffered output. The default does nothing.
    fn flush(&self) {}
}

/// Returns the `ANSI` color code for a level (used only by [`StderrSink`]).
fn level_color(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "90",
        LogLevel::Debug => "34",
        LogLevel::Info => "32",
        LogLevel::Warn => "33",
        LogLevel::Error => "31",
    }
}

/// Writes records to standard error, optionally colored per level.
///
/// Colors are only emitted when enabled (explicitly, or auto-detected when
/// stderr is a terminal). Writes are serialized through the standard error
/// lock so concurrent lines do not interleave.
pub struct StderrSink {
    colors: bool,
}

impl StderrSink {
    /// Creates a sink. `use_colors` of `None` auto-detects whether stderr is a
    /// terminal; `Some(value)` forces the choice.
    #[must_use]
    pub fn new(use_colors: Option<bool>) -> Self {
        let colors = match use_colors {
            Some(value) => value,
            None => std::io::stderr().is_terminal(),
        };
        Self { colors }
    }
}

impl LogSink for StderrSink {
    fn write_record(&self, formatted: &str, record: &LogRecord) {
        let mut handle = std::io::stderr().lock();
        let result = if self.colors {
            let color = level_color(record.level);
            writeln!(handle, "\u{1b}[{color}m{formatted}\u{1b}[0m")
        } else {
            writeln!(handle, "{formatted}")
        };
        let _ = result;
    }

    fn flush(&self) {
        let _ = std::io::stderr().lock().flush();
    }
}

/// Appends records to a log file, creating it if needed.
///
/// Writes go through a buffered, mutex-guarded writer. Write failures fall back
/// to a short message on stderr instead of crashing the process.
pub struct FileSink {
    path: String,
    writer: Mutex<BufWriter<File>>,
}

impl FileSink {
    /// Opens (or creates) `path` for appending.
    ///
    /// # Errors
    ///
    /// Returns a [`FerroError`](crate::error::FerroError) if the file cannot be
    /// opened, preserving the underlying I/O error kind and the path.
    pub fn open(path: impl Into<String>) -> FerroResult<Self> {
        let path = path.into();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| {
                IoError::new("failed to open log file", error).with_resource(path.clone())
            })?;
        Ok(Self {
            path,
            writer: Mutex::new(BufWriter::new(file)),
        })
    }
}

impl LogSink for FileSink {
    fn write_record(&self, formatted: &str, _record: &LogRecord) {
        if let Ok(mut guard) = self.writer.lock() {
            if writeln!(guard, "{formatted}").is_err() {
                let path = &self.path;
                eprintln!("ferro-log: failed to write to log file `{path}`");
            }
        } else {
            let path = &self.path;
            eprintln!("ferro-log: log file lock poisoned for `{path}`");
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.writer.lock() {
            let _ = guard.flush();
        }
    }
}

/// An in-memory ring buffer of the most recent formatted lines.
///
/// Useful for the CLI/debugger to inspect recent logs and for tests. When full,
/// the oldest entries are dropped.
pub struct MemorySink {
    capacity: usize,
    buffer: Mutex<VecDeque<String>>,
}

impl MemorySink {
    /// Creates a ring buffer holding at most `capacity` lines (minimum 1).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            buffer: Mutex::new(VecDeque::with_capacity(capacity.min(1_024))),
        }
    }

    /// Returns a snapshot of the buffered lines, oldest first.
    #[must_use]
    pub fn records(&self) -> Vec<String> {
        match self.buffer.lock() {
            Ok(guard) => guard.iter().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Returns the number of buffered lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.lock().map_or(0, |guard| guard.len())
    }

    /// Returns `true` if no lines are buffered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl LogSink for MemorySink {
    fn write_record(&self, formatted: &str, _record: &LogRecord) {
        if let Ok(mut guard) = self.buffer.lock() {
            if guard.len() == self.capacity {
                guard.pop_front();
            }
            guard.push_back(formatted.to_string());
        }
    }
}

/// Forwards every record to a list of sinks.
pub struct MultiSink {
    sinks: Vec<Arc<dyn LogSink>>,
}

impl MultiSink {
    /// Creates a composite sink over `sinks`.
    #[must_use]
    pub fn new(sinks: Vec<Arc<dyn LogSink>>) -> Self {
        Self { sinks }
    }
}

impl LogSink for MultiSink {
    fn write_record(&self, formatted: &str, record: &LogRecord) {
        for sink in &self.sinks {
            sink.write_record(formatted, record);
        }
    }

    fn flush(&self) {
        for sink in &self.sinks {
            sink.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FileSink, LogSink, MemorySink, MultiSink};
    use crate::log::{LogLevel, LogRecord, LogTarget, Timestamp};
    use std::borrow::Cow;
    use std::sync::Arc;

    fn sample() -> LogRecord {
        LogRecord {
            level: LogLevel::Info,
            target: LogTarget::Common,
            message: Cow::Borrowed("msg"),
            timestamp: Timestamp::now(),
            module_path: "module",
            file: "file.rs",
            line: 1,
            thread_name: None,
        }
    }

    #[test]
    fn memory_sink_evicts_oldest_when_full() {
        let sink = MemorySink::new(2);
        let record = sample();
        sink.write_record("first", &record);
        sink.write_record("second", &record);
        sink.write_record("third", &record);
        let lines = sink.records();
        assert_eq!(lines, vec!["second".to_string(), "third".to_string()]);
        assert_eq!(sink.len(), 2);
        assert!(!sink.is_empty());
    }

    #[test]
    fn multi_sink_forwards_to_every_sink() {
        let first = Arc::new(MemorySink::new(4));
        let second = Arc::new(MemorySink::new(4));
        let multi = MultiSink::new(vec![first.clone(), second.clone()]);
        multi.write_record("line", &sample());
        assert_eq!(first.records(), vec!["line".to_string()]);
        assert_eq!(second.records(), vec!["line".to_string()]);
    }

    #[test]
    fn file_sink_open_failure_returns_error() {
        let result = FileSink::open("/nonexistent-dir-ferro/should/not/exist.log");
        assert!(result.is_err());
    }

    #[test]
    fn file_sink_appends_lines() {
        let mut path = std::env::temp_dir();
        path.push(format!("ferro-log-test-{}.log", std::process::id()));
        let path_str = path.to_string_lossy().into_owned();
        let sink = FileSink::open(path_str.clone()).unwrap();
        sink.write_record("hello world", &sample());
        sink.flush();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("hello world"));
        let _ = std::fs::remove_file(&path);
    }
}
