//! Logger configuration and `FERRO_LOG` parsing.

use std::env;

use crate::error::FerroResult;

use super::level::LogLevel;
use super::target::LogTarget;

/// How the timestamp field is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimestampFormat {
    /// Absolute `ISO-8601`-like UTC timestamp.
    #[default]
    Absolute,
    /// Seconds.millis elapsed since the logger started.
    Relative,
}

/// Formatting options shared by the formatter.
#[derive(Debug, Clone, Copy)]
pub struct FormatOptions {
    /// How the timestamp is rendered.
    pub timestamp_format: TimestampFormat,
    /// Whether to include the source `file:line`.
    pub include_location: bool,
    /// Whether to include the emitting thread name.
    pub include_thread: bool,
}

/// Declarative description of an output sink, resolved into a real sink during
/// [`init`](super::logger::init).
#[derive(Debug, Clone)]
pub enum SinkSpec {
    /// Standard error, with colors decided by [`LogConfig::use_colors`].
    Stderr,
    /// A log file opened in append mode.
    File {
        /// Path to the log file.
        path: String,
    },
    /// An in-memory ring buffer.
    Memory {
        /// Maximum number of buffered lines.
        capacity: usize,
    },
}

/// Configuration for the global logger.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Global minimum level (default [`LogLevel::Info`]).
    pub global_level: LogLevel,
    /// Per-target minimum-level overrides.
    pub target_overrides: Vec<(LogTarget, LogLevel)>,
    /// Color preference; `None` auto-detects whether stderr is a terminal.
    pub use_colors: Option<bool>,
    /// Timestamp rendering mode.
    pub timestamp_format: TimestampFormat,
    /// Whether to include `file:line` (defaults to on in debug builds).
    pub include_location: bool,
    /// Whether to include the emitting thread name.
    pub include_thread: bool,
    /// The output sinks to install.
    pub sinks: Vec<SinkSpec>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            global_level: LogLevel::Info,
            target_overrides: Vec::new(),
            use_colors: None,
            timestamp_format: TimestampFormat::Absolute,
            include_location: cfg!(debug_assertions),
            include_thread: false,
            sinks: vec![SinkSpec::Stderr],
        }
    }
}

impl LogConfig {
    /// Builds a configuration from the defaults, applying `FERRO_LOG` when it is
    /// set and non-empty.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`](crate::error::ConfigError) (as a `FerroError`)
    /// when `FERRO_LOG` is set but cannot be parsed.
    pub fn from_env() -> FerroResult<Self> {
        let mut config = Self::default();
        if let Ok(value) = env::var("FERRO_LOG") {
            if !value.trim().is_empty() {
                let (global, overrides) = parse_filter_spec(&value)?;
                if let Some(level) = global {
                    config.global_level = level;
                }
                config.target_overrides = overrides;
            }
        }
        Ok(config)
    }
}

/// Parsed result of a `FERRO_LOG` filter string: an optional global level plus
/// the per-target overrides in declaration order.
type ParsedFilter = (Option<LogLevel>, Vec<(LogTarget, LogLevel)>);

/// Parses a `FERRO_LOG`-style filter string such as `"info,cpu=warn,gpu=debug"`.
///
/// A bare token sets the global level; a `target=level` token sets a per-target
/// override. Whitespace around entries is ignored and empty entries are
/// skipped.
///
/// # Errors
///
/// Returns a [`ConfigError`](crate::error::ConfigError) when any token names an
/// unknown level or target.
pub fn parse_filter_spec(
    spec: &str,
) -> FerroResult<ParsedFilter> {
    let mut global = None;
    let mut overrides = Vec::new();
    for raw in spec.split(',') {
        let entry = raw.trim();
        if entry.is_empty() {
            continue;
        }
        if let Some((target, level)) = entry.split_once('=') {
            let target = target.trim().parse::<LogTarget>()?;
            let level = level.trim().parse::<LogLevel>()?;
            overrides.push((target, level));
        } else {
            global = Some(entry.parse::<LogLevel>()?);
        }
    }
    Ok((global, overrides))
}

#[cfg(test)]
mod tests {
    use super::{parse_filter_spec, LogConfig, TimestampFormat};
    use crate::log::{LogLevel, LogTarget};

    #[test]
    fn default_is_info_with_stderr() {
        let config = LogConfig::default();
        assert_eq!(config.global_level, LogLevel::Info);
        assert_eq!(config.sinks.len(), 1);
        assert_eq!(config.timestamp_format, TimestampFormat::Absolute);
    }

    #[test]
    fn parses_global_and_overrides() {
        let (global, overrides) = parse_filter_spec("info,cpu=warn,gpu=debug").unwrap();
        assert_eq!(global, Some(LogLevel::Info));
        assert_eq!(
            overrides,
            vec![
                (LogTarget::Cpu, LogLevel::Warn),
                (LogTarget::Gpu, LogLevel::Debug),
            ]
        );
    }

    #[test]
    fn skips_empty_entries() {
        let (global, overrides) = parse_filter_spec(" , warn , ").unwrap();
        assert_eq!(global, Some(LogLevel::Warn));
        assert!(overrides.is_empty());
    }

    #[test]
    fn rejects_invalid_level() {
        assert!(parse_filter_spec("cpu=nope").is_err());
    }

    #[test]
    fn rejects_invalid_target() {
        assert!(parse_filter_spec("bogus=warn").is_err());
    }
}
