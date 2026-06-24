//! The `Ferro-VPS` error subsystem.
//!
//! [`FerroError`] is the root host error type. Each domain has its own
//! sub-error (see [`ConfigError`], [`IoError`], [`InternalError`], and the
//! per-component stubs). Guest-caused failures use the separate [`GuestFault`]
//! type and never convert into a [`FerroError`] automatically. See
//! `docs/CONVENTIONS.md` for the full philosophy, exit-code map, and panic
//! strategy.

// The error types intentionally share an `Error`/`Fault` suffix and live in
// per-domain modules (for example `ConfigError` in `config`). That naming is a
// deliberate, documented project convention, so the module-name-repetition
// lint is allowed throughout this module tree.
#![allow(clippy::module_name_repetitions)]

mod config;
mod domains;
mod guest;
mod internal;
mod io;
mod report;

use std::error::Error;
use std::fmt;
use std::io as std_io;

pub use config::ConfigError;
pub use domains::{
    AsmError, AudioError, BusError, CpuError, GpuError, HostError, KernelError, MemoryError,
    NetworkError, StorageError, VmError,
};
pub use guest::GuestFault;
pub use internal::InternalError;
pub use io::IoError;
pub use report::{
    exit_code_for, report_error, run, EXIT_CONFIG, EXIT_GENERAL, EXIT_INTERNAL, EXIT_IO,
    EXIT_SUCCESS, EXIT_VM,
};

/// Convenient result alias used throughout the workspace.
pub type FerroResult<T> = core::result::Result<T, FerroError>;

/// The root error type for the `Ferro-VPS` host.
///
/// Every variant corresponds to a domain. Heavy sub-errors are boxed so the
/// enum stays small and `Result<T, FerroError>` is cheap to move on the happy
/// path.
#[derive(Debug)]
#[non_exhaustive]
pub enum FerroError {
    /// Configuration loading or validation failed.
    Config(Box<ConfigError>),
    /// The virtual CPU reported an error.
    Cpu(CpuError),
    /// The virtual memory subsystem reported an error.
    Memory(MemoryError),
    /// The device bus reported an error.
    Bus(BusError),
    /// The virtual GPU reported an error.
    Gpu(GpuError),
    /// The storage subsystem reported an error.
    Storage(StorageError),
    /// The audio subsystem reported an error.
    Audio(AudioError),
    /// The network subsystem reported an error.
    Network(NetworkError),
    /// The guest kernel reported an error.
    Kernel(KernelError),
    /// Assembling or running the virtual machine reported an error.
    Vm(VmError),
    /// The assembler reported an error.
    Asm(AsmError),
    /// The host integration layer reported an error.
    Host(HostError),
    /// A host I/O operation failed.
    Io(Box<IoError>),
    /// An internal invariant was violated (a bug in the host).
    Internal(Box<InternalError>),
    /// Another error annotated with additional context.
    Contextualized(Box<Contextualized>),
}

/// An error wrapped with a human-readable context message.
///
/// The original cause is preserved and reachable through
/// [`std::error::Error::source`]; it is never discarded.
#[derive(Debug)]
pub struct Contextualized {
    context: String,
    source: Box<FerroError>,
}

impl Contextualized {
    fn new(context: String, source: FerroError) -> Self {
        Self {
            context: truncate_context(context),
            source: Box::new(source),
        }
    }

    pub(crate) fn inner_error(&self) -> &FerroError {
        &self.source
    }
}

impl fmt::Display for Contextualized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.context)
    }
}

impl Error for Contextualized {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source)
    }
}

impl FerroError {
    /// Wraps this error with additional human-readable context.
    ///
    /// The original error remains reachable through
    /// [`std::error::Error::source`].
    #[must_use]
    pub fn context<C: Into<String>>(self, context: C) -> Self {
        Self::Contextualized(Box::new(Contextualized::new(context.into(), self)))
    }
}

impl fmt::Display for FerroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => write!(f, "config error: {error}"),
            Self::Cpu(error) => write!(f, "cpu error: {error}"),
            Self::Memory(error) => write!(f, "memory error: {error}"),
            Self::Bus(error) => write!(f, "bus error: {error}"),
            Self::Gpu(error) => write!(f, "gpu error: {error}"),
            Self::Storage(error) => write!(f, "storage error: {error}"),
            Self::Audio(error) => write!(f, "audio error: {error}"),
            Self::Network(error) => write!(f, "network error: {error}"),
            Self::Kernel(error) => write!(f, "kernel error: {error}"),
            Self::Vm(error) => write!(f, "vm error: {error}"),
            Self::Asm(error) => write!(f, "asm error: {error}"),
            Self::Host(error) => write!(f, "host error: {error}"),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Internal(error) => write!(f, "internal error: {error}"),
            Self::Contextualized(error) => write!(f, "{error}"),
        }
    }
}

impl Error for FerroError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => error.source(),
            Self::Contextualized(error) => error.source(),
            _ => None,
        }
    }
}

impl From<ConfigError> for FerroError {
    fn from(value: ConfigError) -> Self {
        Self::Config(Box::new(value))
    }
}

impl From<IoError> for FerroError {
    fn from(value: IoError) -> Self {
        Self::Io(Box::new(value))
    }
}

impl From<InternalError> for FerroError {
    fn from(value: InternalError) -> Self {
        Self::Internal(Box::new(value))
    }
}

impl From<std_io::Error> for FerroError {
    fn from(value: std_io::Error) -> Self {
        Self::Io(Box::new(IoError::from(value)))
    }
}

macro_rules! impl_domain_from {
    ($($variant:ident => $error:ty),+ $(,)?) => {
        $(
            impl From<$error> for FerroError {
                fn from(value: $error) -> Self {
                    Self::$variant(value)
                }
            }
        )+
    };
}

impl_domain_from! {
    Cpu => CpuError,
    Memory => MemoryError,
    Bus => BusError,
    Gpu => GpuError,
    Storage => StorageError,
    Audio => AudioError,
    Network => NetworkError,
    Kernel => KernelError,
    Vm => VmError,
    Asm => AsmError,
    Host => HostError,
}

/// Extension trait that attaches context to any `Result` whose error converts
/// into a [`FerroError`].
pub trait ResultContextExt<T> {
    /// Attaches a fixed context message to the error, preserving the original
    /// cause.
    ///
    /// # Errors
    ///
    /// Returns the original error wrapped in [`FerroError::Contextualized`]
    /// when `self` is `Err`.
    fn context<C: Into<String>>(self, context: C) -> FerroResult<T>;

    /// Attaches a lazily-computed context message to the error, preserving the
    /// original cause. The closure runs only on the error path.
    ///
    /// # Errors
    ///
    /// Returns the original error wrapped in [`FerroError::Contextualized`]
    /// when `self` is `Err`.
    fn with_context<C, F>(self, context_fn: F) -> FerroResult<T>
    where
        C: Into<String>,
        F: FnOnce() -> C;
}

impl<T, E> ResultContextExt<T> for core::result::Result<T, E>
where
    E: Into<FerroError>,
{
    fn context<C: Into<String>>(self, context: C) -> FerroResult<T> {
        self.map_err(|error| error.into().context(context))
    }

    fn with_context<C, F>(self, context_fn: F) -> FerroResult<T>
    where
        C: Into<String>,
        F: FnOnce() -> C,
    {
        self.map_err(|error| error.into().context(context_fn()))
    }
}

const MAX_CONTEXT_BYTES: usize = 4096;

fn truncate_context(mut context: String) -> String {
    if context.len() <= MAX_CONTEXT_BYTES {
        return context;
    }
    let boundary = floor_char_boundary(&context, MAX_CONTEXT_BYTES);
    context.truncate(boundary);
    context.push_str("...");
    context
}

fn floor_char_boundary(text: &str, max: usize) -> usize {
    if max >= text.len() {
        return text.len();
    }
    let mut index = max;
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::{
        exit_code_for, ConfigError, FerroError, FerroResult, GuestFault, InternalError, IoError,
        ResultContextExt, VmError, EXIT_CONFIG, EXIT_INTERNAL, EXIT_IO, EXIT_VM,
    };
    use std::error::Error;

    #[test]
    fn display_has_domain_prefix() {
        let error = FerroError::from(ConfigError::Missing {
            field: "name".to_owned(),
        });
        assert!(error.to_string().starts_with("config error:"));
    }

    #[test]
    fn io_source_chain_is_preserved() {
        let raw = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let error = FerroError::from(raw);
        assert!(error.source().is_some());
    }

    #[test]
    fn question_mark_converts_io_error() {
        fn inner() -> FerroResult<()> {
            let result: Result<(), std::io::Error> =
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
            result?;
            Ok(())
        }
        assert!(inner().is_err());
    }

    #[test]
    fn macros_build_expected_errors() {
        fn bail_now() -> FerroResult<()> {
            crate::bail!("boom {}", 1);
        }
        fn ensure_now(flag: bool) -> FerroResult<()> {
            crate::ensure!(flag, "must hold");
            Ok(())
        }
        let direct = crate::internal!("nope");
        assert!(matches!(direct, FerroError::Internal(_)));
        assert!(bail_now().is_err());
        assert!(ensure_now(false).is_err());
        assert!(ensure_now(true).is_ok());
    }

    #[test]
    fn context_preserves_original_cause() {
        let base = FerroError::from(ConfigError::ParseFailed {
            reason: "bad".to_owned(),
        });
        let wrapped: FerroResult<()> = Err(base).context("while loading config");
        let error = wrapped.unwrap_err();
        assert!(error.to_string().contains("while loading config"));
        assert!(error.source().is_some());
    }

    #[test]
    fn with_context_runs_only_on_success_path() {
        let start: FerroResult<u8> = Ok(1);
        let outcome = start.with_context(|| "unused");
        assert_eq!(outcome.unwrap(), 1);
    }

    #[test]
    fn exit_codes_map_per_domain() {
        let config = FerroError::from(ConfigError::Missing {
            field: "x".to_owned(),
        });
        let raw = FerroError::from(std::io::Error::from(std::io::ErrorKind::NotFound));
        let vm = FerroError::from(VmError::Unimplemented);
        let internal = FerroError::from(InternalError::invariant("bug"));
        assert_eq!(exit_code_for(&config), EXIT_CONFIG);
        assert_eq!(exit_code_for(&raw), EXIT_IO);
        assert_eq!(exit_code_for(&vm), EXIT_VM);
        assert_eq!(exit_code_for(&internal), EXIT_INTERNAL);
    }

    #[test]
    fn context_exit_code_follows_inner_domain() {
        let base = FerroError::from(std::io::Error::from(std::io::ErrorKind::NotFound));
        let wrapped = base.context("reading file");
        assert_eq!(exit_code_for(&wrapped), EXIT_IO);
    }

    #[test]
    fn guest_fault_is_distinct_and_displays() {
        let fault = GuestFault::DivideByZero;
        assert_eq!(fault.to_string(), "divide by zero");
        let escalated = FerroError::from(VmError::GuestFaulted(GuestFault::IllegalInstruction));
        assert!(escalated.to_string().contains("guest faulted"));
    }

    #[test]
    fn ferro_error_size_is_bounded() {
        assert!(std::mem::size_of::<FerroError>() <= 32);
    }

    #[test]
    fn io_error_preserves_kind() {
        let raw = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let wrapped = IoError::from(raw);
        assert_eq!(wrapped.kind(), std::io::ErrorKind::PermissionDenied);
    }
}
