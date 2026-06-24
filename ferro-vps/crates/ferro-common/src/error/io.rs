//! Host I/O errors, enriched with context while preserving the original kind.

use std::io;

/// An I/O error originating in the host, enriched with a context message and an
/// optional resource name. The underlying [`std::io::Error`] is preserved and
/// exposed through [`std::error::Error::source`], so the original
/// [`std::io::ErrorKind`] is never lost.
#[derive(Debug)]
#[non_exhaustive]
pub struct IoError {
    context: String,
    resource: Option<String>,
    source: io::Error,
}

impl IoError {
    /// Creates an I/O error from a host [`std::io::Error`] and a context
    /// message describing the operation that failed.
    #[must_use]
    pub fn new(context: impl Into<String>, source: io::Error) -> Self {
        Self {
            context: context.into(),
            resource: None,
            source,
        }
    }

    /// Attaches the name of the resource (path, socket, device, ...) involved.
    #[must_use]
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    /// Returns the underlying I/O error kind, preserved from the host error.
    #[must_use]
    pub fn kind(&self) -> io::ErrorKind {
        self.source.kind()
    }
}

impl core::fmt::Display for IoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.resource {
            Some(resource) => write!(f, "{} ({resource})", self.context),
            None => f.write_str(&self.context),
        }
    }
}

impl std::error::Error for IoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl From<io::Error> for IoError {
    fn from(source: io::Error) -> Self {
        Self::new("host i/o operation failed", source)
    }
}
