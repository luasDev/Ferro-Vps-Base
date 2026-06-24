//! Ergonomic error macros for the `Ferro-VPS` workspace.
//!
//! These macros are exported at the crate root, so other crates call them as
//! `ferro_common::bail!`, `ferro_common::ensure!`, and `ferro_common::internal!`.
//!
//! Safety note: the format string passed to these macros is controlled by the
//! developer, never by guest input. Never pass an untrusted, guest-supplied
//! string as the first (format-string) argument; pass it as a value argument
//! instead, e.g. `internal!("bad token: {token}")`.

/// Builds a [`FerroError`](crate::error::FerroError) carrying an internal
/// invariant-violation message, formatted like [`format!`].
///
/// The error location is captured automatically via `#[track_caller]`.
#[macro_export]
macro_rules! internal {
    ($($arg:tt)*) => {
        $crate::error::FerroError::from($crate::error::InternalError::invariant(
            ::std::format!($($arg)*),
        ))
    };
}

/// Returns early from the current function with an internal
/// [`FerroError`](crate::error::FerroError), formatted like [`format!`].
///
/// Use inside a function that returns [`FerroResult`](crate::error::FerroResult)
/// (or any `Result<_, FerroError>`).
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return ::core::result::Result::Err($crate::internal!($($arg)*))
    };
}

/// Returns early with an internal [`FerroError`](crate::error::FerroError) when
/// `cond` is false, formatted like [`format!`].
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)*) => {
        if !($cond) {
            $crate::bail!($($arg)*);
        }
    };
}

/// Internal building block for the level-specific logging macros.
///
/// It checks the filter *before* formatting, so a disabled `(level, target)`
/// pair costs only an integer comparison and never allocates or runs the
/// format arguments. `file!`, `line!`, and `module_path!` are captured here.
///
/// Safety note: the format string is always a developer-controlled literal.
/// Never pass guest-supplied text as the format string; pass it as a value
/// argument instead.
#[macro_export]
macro_rules! log_event {
    ($level:expr, $target:expr, $($arg:tt)*) => {{
        let level = $level;
        let target = $target;
        if $crate::log::enabled(level, target) {
            $crate::log::record(&$crate::log::LogRecord {
                level,
                target,
                message: ::std::borrow::Cow::Owned(::std::format!($($arg)*)),
                timestamp: $crate::log::Timestamp::now(),
                module_path: ::core::module_path!(),
                file: ::core::file!(),
                line: ::core::line!(),
                thread_name: ::std::thread::current()
                    .name()
                    .map(::std::borrow::ToOwned::to_owned),
            });
        }
    }};
}

/// Emits a [`Trace`](crate::log::LogLevel::Trace) log record.
///
/// In release builds this compiles to nothing unless the `trace-logs` feature
/// is enabled, so the most verbose logs cost zero at runtime in production.
#[macro_export]
macro_rules! log_trace {
    ($target:expr, $($arg:tt)*) => {{
        #[cfg(any(debug_assertions, feature = "trace-logs"))]
        {
            $crate::log_event!($crate::log::LogLevel::Trace, $target, $($arg)*);
        }
    }};
}

/// Emits a [`Debug`](crate::log::LogLevel::Debug) log record.
#[macro_export]
macro_rules! log_debug {
    ($target:expr, $($arg:tt)*) => {
        $crate::log_event!($crate::log::LogLevel::Debug, $target, $($arg)*)
    };
}

/// Emits an [`Info`](crate::log::LogLevel::Info) log record.
#[macro_export]
macro_rules! log_info {
    ($target:expr, $($arg:tt)*) => {
        $crate::log_event!($crate::log::LogLevel::Info, $target, $($arg)*)
    };
}

/// Emits a [`Warn`](crate::log::LogLevel::Warn) log record.
#[macro_export]
macro_rules! log_warn {
    ($target:expr, $($arg:tt)*) => {
        $crate::log_event!($crate::log::LogLevel::Warn, $target, $($arg)*)
    };
}

/// Emits an [`Error`](crate::log::LogLevel::Error) log record.
#[macro_export]
macro_rules! log_error {
    ($target:expr, $($arg:tt)*) => {
        $crate::log_event!($crate::log::LogLevel::Error, $target, $($arg)*)
    };
}
