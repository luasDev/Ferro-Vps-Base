//! Line formatting and message sanitization.
//!
//! The formatter produces a plain (no `ANSI`) line. Color is the job of the
//! [`StderrSink`](super::sink::StderrSink), so file and memory sinks always
//! receive uncolored text.

use core::fmt::Write as _;
use std::time::Instant;

use super::config::{FormatOptions, TimestampFormat};
use super::record::LogRecord;
use super::time;

/// Maximum number of bytes kept from a single message before truncation.
const MAX_MESSAGE_BYTES: usize = 16 * 1024;

/// Formats a record into a single plain-text line, without a trailing newline
/// and without any `ANSI` escape codes.
pub(crate) fn format_line(record: &LogRecord, options: FormatOptions, start: Instant) -> String {
    let timestamp = match options.timestamp_format {
        TimestampFormat::Absolute => time::format_wall_clock(record.timestamp.wall),
        TimestampFormat::Relative => {
            time::format_relative(record.timestamp.instant.saturating_duration_since(start))
        }
    };

    let mut out = String::with_capacity(64 + record.message.len());
    let level = record.level;
    let target = record.target.as_str();
    let _ = write!(out, "{timestamp} {level:<5} [{target}]");

    if options.include_location {
        let file = record.file;
        let line = record.line;
        let _ = write!(out, " ({file}:{line})");
    }

    if options.include_thread {
        match &record.thread_name {
            Some(name) => {
                let _ = write!(out, " {name}");
            }
            None => out.push_str(" -"),
        }
    }

    out.push_str(": ");
    append_sanitized(&mut out, &record.message);
    out
}

/// Appends `message` to `out`, neutralizing control characters (which could be
/// used for `ANSI` escape injection or to forge extra log lines) and limiting
/// the total length.
fn append_sanitized(out: &mut String, message: &str) {
    let mut written = 0usize;
    for ch in message.chars() {
        match ch {
            '\t' => out.push('\t'),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            control if control.is_control() => {
                let _ = write!(out, "\\x{:02x}", control as u32);
            }
            other => out.push(other),
        }
        written += ch.len_utf8();
        if written >= MAX_MESSAGE_BYTES {
            out.push_str("…[truncated]");
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_line;
    use crate::log::{FormatOptions, LogLevel, LogRecord, LogTarget, Timestamp, TimestampFormat};
    use std::borrow::Cow;
    use std::time::Instant;

    fn record(message: &'static str) -> LogRecord {
        LogRecord {
            level: LogLevel::Warn,
            target: LogTarget::Network,
            message: Cow::Borrowed(message),
            timestamp: Timestamp::now(),
            module_path: "module",
            file: "ferro-net/src/lib.rs",
            line: 42,
            thread_name: None,
        }
    }

    fn options() -> FormatOptions {
        FormatOptions {
            timestamp_format: TimestampFormat::Absolute,
            include_location: false,
            include_thread: false,
        }
    }

    #[test]
    fn line_contains_expected_fields() {
        let line = format_line(&record("packet dropped"), options(), Instant::now());
        assert!(line.contains("WARN"));
        assert!(line.contains("[net]"));
        assert!(line.contains("packet dropped"));
        assert!(line.contains('T') && line.ends_with("packet dropped"));
    }

    #[test]
    fn level_field_is_width_five() {
        let line = format_line(&record("x"), options(), Instant::now());
        assert!(line.contains("WARN  [net]"));
    }

    #[test]
    fn output_never_contains_ansi_or_newlines() {
        let line = format_line(
            &record("evil\u{1b}[2Jline\nbreak"),
            options(),
            Instant::now(),
        );
        assert!(!line.contains('\u{1b}'));
        assert!(!line.contains('\n'));
        assert!(line.contains("\\n"));
    }

    #[test]
    fn location_included_when_requested() {
        let mut options = options();
        options.include_location = true;
        let line = format_line(&record("boot"), options, Instant::now());
        assert!(line.contains("(ferro-net/src/lib.rs:42)"));
    }
}
