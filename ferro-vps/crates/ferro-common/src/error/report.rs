//! Process-level error reporting and stable exit-code mapping for binaries.

use std::process::ExitCode;

use super::{FerroError, FerroResult};

/// Exit code for a successful run.
pub const EXIT_SUCCESS: u8 = 0;
/// Exit code for a generic, unclassified error.
pub const EXIT_GENERAL: u8 = 1;
/// Exit code for configuration errors.
pub const EXIT_CONFIG: u8 = 2;
/// Exit code for host I/O errors.
pub const EXIT_IO: u8 = 3;
/// Exit code for virtual-machine errors.
pub const EXIT_VM: u8 = 4;
/// Exit code for internal invariant violations.
pub const EXIT_INTERNAL: u8 = 70;

/// Maps an error to its stable, domain-specific process exit code.
///
/// Contextualized errors are unwrapped to the domain of their underlying cause.
#[must_use]
pub fn exit_code_for(error: &FerroError) -> u8 {
    match error {
        FerroError::Config(_) => EXIT_CONFIG,
        FerroError::Io(_) => EXIT_IO,
        FerroError::Vm(_) => EXIT_VM,
        FerroError::Internal(_) => EXIT_INTERNAL,
        FerroError::Contextualized(inner) => exit_code_for(inner.inner_error()),
        _ => EXIT_GENERAL,
    }
}

/// Prints the full error chain to stderr without panicking.
///
/// The top-level error is printed on an `error:` line, and every nested
/// [`std::error::Error::source`] is printed on its own indented `caused by:`
/// line.
pub fn report_error(error: &FerroError) {
    eprintln!("error: {error}");
    let mut current = std::error::Error::source(error);
    while let Some(cause) = current {
        eprintln!("  caused by: {cause}");
        current = cause.source();
    }
}

/// Runs a binary entry point, reporting any error to stderr and converting it
/// into a stable [`ExitCode`]. Return its result directly from `main`.
///
/// This never panics: errors are formatted and turned into an exit code.
pub fn run<F>(entry: F) -> ExitCode
where
    F: FnOnce() -> FerroResult<()>,
{
    match entry() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report_error(&error);
            ExitCode::from(exit_code_for(&error))
        }
    }
}
