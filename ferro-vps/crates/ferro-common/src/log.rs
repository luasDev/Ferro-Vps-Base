//! Logging/tracing subsystem for the `Ferro-VPS` project.
//!
//! This is a lightweight, dependency-free logger. It provides severity
//! [`LogLevel`]s, per-subsystem [`LogTarget`]s, configurable formatting,
//! multiple [`LogSink`] outputs (stderr, file, in-memory ring buffer), and
//! runtime [`LogFilter`]ing by level and target.
//!
//! # Usage
//!
//! Initialize the global logger once at startup (binaries read `FERRO_LOG`):
//!
//! ```no_run
//! use ferro_common::log::{self, LogConfig};
//! log::init(&LogConfig::from_env()?)?;
//! # Ok::<(), ferro_common::error::FerroError>(())
//! ```
//!
//! Then log using the crate-root macros, always passing this crate's own
//! [`LogTarget`]:
//!
//! ```
//! use ferro_common::{log_info, log::LogTarget};
//! log_info!(LogTarget::Common, "loaded {} bytes", 42);
//! ```
//!
//! # `FERRO_LOG` syntax
//!
//! A comma-separated list. A bare token sets the global level; `target=level`
//! sets a per-target override. For example `FERRO_LOG="info,cpu=warn,gpu=debug"`.
//!
//! See `docs/CONVENTIONS.md` for the full conventions, including the anti-spam
//! logging guideline and the relationship with the error subsystem.

#![allow(clippy::module_name_repetitions)]

mod config;
mod filter;
mod format;
mod level;
mod logger;
mod record;
mod sink;
mod target;
mod time;

pub use config::{parse_filter_spec, FormatOptions, LogConfig, SinkSpec, TimestampFormat};
pub use filter::LogFilter;
pub use level::LogLevel;
pub use logger::{add_sink, enabled, flush, init, record, set_global_level, set_target_level};
pub use record::{LogRecord, Timestamp};
pub use sink::{FileSink, LogSink, MemorySink, MultiSink, StderrSink};
pub use target::LogTarget;
