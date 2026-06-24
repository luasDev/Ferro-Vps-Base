//! Severity levels for log records.

use core::fmt;
use core::str::FromStr;

use crate::error::ConfigError;

/// Severity of a log record, ordered from least to most severe.
///
/// The ordering is meaningful: `Trace < Debug < Info < Warn < Error`, which is
/// exactly what the filter relies on. Each level also has a stable numeric
/// representation (see [`LogLevel::as_u8`]) so it can be stored in an atomic
/// for lock-free filtering on the hot path.
///
/// Level semantics:
/// - `Trace`: extremely detailed step-by-step tracing (for example every
///   executed CPU instruction).
/// - `Debug`: diagnostic information useful during development.
/// - `Info`: normal, relevant events (VM boot, program load).
/// - `Warn`: something unexpected but recoverable happened.
/// - `Error`: a failure that prevents an operation from completing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    /// Extremely detailed step-by-step tracing.
    Trace = 0,
    /// Diagnostic information useful during development.
    Debug = 1,
    /// Normal, relevant events.
    Info = 2,
    /// Something unexpected but recoverable.
    Warn = 3,
    /// A failure that prevents an operation from completing.
    Error = 4,
}

impl LogLevel {
    /// Returns the stable numeric value for this level (`Trace` = 0 .. `Error`
    /// = 4). Used for fast integer comparisons and atomic storage.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Reconstructs a level from its [`LogLevel::as_u8`] value. Out-of-range
    /// values saturate to [`LogLevel::Error`]; this never happens in practice
    /// because the value always comes from `as_u8`.
    pub(crate) const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Trace,
            1 => Self::Debug,
            2 => Self::Info,
            3 => Self::Warn,
            _ => Self::Error,
        }
    }

    /// Returns the uppercase name of the level (for example `"INFO"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

impl FromStr for LogLevel {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            other => Err(ConfigError::Invalid {
                field: "log level".to_string(),
                reason: format!("unknown log level `{other}`"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LogLevel;

    #[test]
    fn ordering_is_by_severity() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn display_uses_uppercase() {
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
    }

    #[test]
    fn from_str_is_case_insensitive() {
        assert_eq!("info".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("INFO".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("Warn".parse::<LogLevel>().unwrap(), LogLevel::Warn);
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert!("loud".parse::<LogLevel>().is_err());
    }

    #[test]
    fn numeric_representation_is_stable() {
        assert_eq!(LogLevel::Trace.as_u8(), 0);
        assert_eq!(LogLevel::Error.as_u8(), 4);
        assert_eq!(LogLevel::from_u8(2), LogLevel::Info);
        assert_eq!(LogLevel::from_u8(250), LogLevel::Error);
    }
}
