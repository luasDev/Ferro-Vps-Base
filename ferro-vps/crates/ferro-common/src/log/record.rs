//! The in-memory representation of a single log entry.

use std::borrow::Cow;
use std::time::{Instant, SystemTime};

use super::level::LogLevel;
use super::target::LogTarget;

/// A pair of timestamps captured when a record is emitted.
///
/// The wall-clock time produces a human-readable timestamp, while the
/// monotonic instant is used for ordering and for relative timestamps that are
/// immune to wall-clock adjustments.
#[derive(Debug, Clone, Copy)]
pub struct Timestamp {
    /// Wall-clock time, used for the human-readable timestamp.
    pub wall: SystemTime,
    /// Monotonic instant, used for ordering and relative timestamps.
    pub instant: Instant,
}

impl Timestamp {
    /// Captures the current wall-clock and monotonic time.
    #[must_use]
    pub fn now() -> Self {
        Self {
            wall: SystemTime::now(),
            instant: Instant::now(),
        }
    }
}

/// A log entry at the moment of emission, before formatting or writing.
///
/// Records are only built after the filter has accepted the `(level, target)`
/// pair, so constructing one is already on the cold path. The message uses a
/// [`Cow`] so static literals avoid an allocation.
#[derive(Debug, Clone)]
pub struct LogRecord {
    /// Severity of the entry.
    pub level: LogLevel,
    /// Subsystem that produced the entry.
    pub target: LogTarget,
    /// The (already formatted) message text.
    pub message: Cow<'static, str>,
    /// When the entry was emitted.
    pub timestamp: Timestamp,
    /// Source module path, captured via `module_path!`.
    pub module_path: &'static str,
    /// Source file, captured via `file!`.
    pub file: &'static str,
    /// Source line, captured via `line!`.
    pub line: u32,
    /// Name of the host thread that emitted the entry, if it has one.
    pub thread_name: Option<String>,
}
