//! Configuration loading and validation errors.

use std::num::{ParseFloatError, ParseIntError};

/// Errors that arise while loading or validating configuration values.
#[derive(Debug)]
#[non_exhaustive]
pub enum ConfigError {
    /// A required configuration field was absent.
    Missing {
        /// Name of the missing field.
        field: String,
    },
    /// A configuration field held an invalid value.
    Invalid {
        /// Name of the offending field.
        field: String,
        /// Human-readable explanation of why the value is invalid.
        reason: String,
    },
    /// A configuration value failed to parse.
    ParseFailed {
        /// Human-readable explanation of the parse failure.
        reason: String,
    },
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Missing { field } => write!(f, "missing required field `{field}`"),
            Self::Invalid { field, reason } => {
                write!(f, "invalid value for `{field}`: {reason}")
            }
            Self::ParseFailed { reason } => write!(f, "failed to parse value: {reason}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<ParseIntError> for ConfigError {
    fn from(value: ParseIntError) -> Self {
        Self::ParseFailed {
            reason: value.to_string(),
        }
    }
}

impl From<ParseFloatError> for ConfigError {
    fn from(value: ParseFloatError) -> Self {
        Self::ParseFailed {
            reason: value.to_string(),
        }
    }
}
