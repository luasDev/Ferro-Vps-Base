//! Internal invariant-violation errors ("this should never happen").

use core::panic::Location;

/// Errors that represent a violated internal invariant. Reaching one indicates
/// a bug in the host itself, not invalid input or a guest fault.
#[derive(Debug)]
#[non_exhaustive]
pub enum InternalError {
    /// An internal invariant was violated. Carries a human-readable message and
    /// the source location where the error was created.
    Invariant {
        /// Description of the invariant that was violated.
        message: String,
        /// Source location where the error was constructed.
        location: &'static Location<'static>,
    },
}

impl InternalError {
    /// Creates an [`InternalError::Invariant`], capturing the caller's source
    /// location automatically.
    #[must_use]
    #[track_caller]
    pub fn invariant(message: impl Into<String>) -> Self {
        Self::Invariant {
            message: message.into(),
            location: Location::caller(),
        }
    }
}

impl core::fmt::Display for InternalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invariant { message, location } => {
                write!(f, "invariant violated at {location}: {message}")
            }
        }
    }
}

impl std::error::Error for InternalError {}
