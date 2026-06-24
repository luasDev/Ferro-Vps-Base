//! The process-global logger and its public API.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

use crate::error::{FerroError, FerroResult, InternalError};

use super::config::{FormatOptions, LogConfig, SinkSpec};
use super::filter::LogFilter;
use super::format::format_line;
use super::level::LogLevel;
use super::record::LogRecord;
use super::sink::{FileSink, LogSink, MemorySink, StderrSink};
use super::target::LogTarget;

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// The global logger state.
///
/// The global level is mirrored into an [`AtomicU8`] so the common hot-path
/// check (`enabled`) is lock-free when no per-target overrides are configured.
struct Logger {
    global_level: AtomicU8,
    has_overrides: AtomicBool,
    filter: RwLock<LogFilter>,
    sinks: RwLock<Vec<Arc<dyn LogSink>>>,
    options: FormatOptions,
    start: Instant,
}

impl Logger {
    fn enabled(&self, level: LogLevel, target: LogTarget) -> bool {
        let global = LogLevel::from_u8(self.global_level.load(Ordering::Relaxed));
        if !self.has_overrides.load(Ordering::Relaxed) {
            return level >= global;
        }
        match self.filter.read() {
            Ok(filter) => filter.enabled(level, target),
            Err(_) => level >= global,
        }
    }

    fn dispatch(&self, record: &LogRecord) {
        let line = format_line(record, self.options, self.start);
        if let Ok(sinks) = self.sinks.read() {
            for sink in &*sinks {
                sink.write_record(&line, record);
            }
        }
    }
}

fn build_sinks(config: &LogConfig) -> FerroResult<Vec<Arc<dyn LogSink>>> {
    let mut sinks: Vec<Arc<dyn LogSink>> = Vec::with_capacity(config.sinks.len());
    for spec in &config.sinks {
        match spec {
            SinkSpec::Stderr => sinks.push(Arc::new(StderrSink::new(config.use_colors))),
            SinkSpec::File { path } => sinks.push(Arc::new(FileSink::open(path.clone())?)),
            SinkSpec::Memory { capacity } => sinks.push(Arc::new(MemorySink::new(*capacity))),
        }
    }
    Ok(sinks)
}

fn already_initialized() -> FerroError {
    FerroError::from(InternalError::invariant(
        "global logger already initialized",
    ))
}

/// Initializes the global logger from `config`.
///
/// This must be called once, early in `main`. Calling it again returns an error
/// (the logger is install-once and never silently replaced).
///
/// # Errors
///
/// Returns an error if a configured log file cannot be opened, or if the logger
/// has already been initialized.
pub fn init(config: &LogConfig) -> FerroResult<()> {
    let sinks = build_sinks(config)?;

    let mut filter = LogFilter::new(config.global_level);
    for (target, level) in &config.target_overrides {
        filter.set_target(*target, *level);
    }
    let has_overrides = !config.target_overrides.is_empty();

    let options = FormatOptions {
        timestamp_format: config.timestamp_format,
        include_location: config.include_location,
        include_thread: config.include_thread,
    };

    let logger = Logger {
        global_level: AtomicU8::new(config.global_level.as_u8()),
        has_overrides: AtomicBool::new(has_overrides),
        filter: RwLock::new(filter),
        sinks: RwLock::new(sinks),
        options,
        start: Instant::now(),
    };

    LOGGER.set(logger).map_err(|_| already_initialized())
}

/// Sets the global minimum level at runtime. No-op if the logger is not yet
/// initialized.
pub fn set_global_level(level: LogLevel) {
    if let Some(logger) = LOGGER.get() {
        logger.global_level.store(level.as_u8(), Ordering::Relaxed);
        if let Ok(mut filter) = logger.filter.write() {
            filter.set_global(level);
        }
    }
}

/// Sets a per-target minimum level at runtime. No-op if the logger is not yet
/// initialized.
pub fn set_target_level(target: LogTarget, level: LogLevel) {
    if let Some(logger) = LOGGER.get() {
        if let Ok(mut filter) = logger.filter.write() {
            filter.set_target(target, level);
        }
        logger.has_overrides.store(true, Ordering::Relaxed);
    }
}

/// Adds an extra sink at runtime. No-op if the logger is not yet initialized.
pub fn add_sink(sink: Arc<dyn LogSink>) {
    if let Some(logger) = LOGGER.get() {
        if let Ok(mut sinks) = logger.sinks.write() {
            sinks.push(sink);
        }
    }
}

/// Flushes every sink. No-op if the logger is not yet initialized.
pub fn flush() {
    if let Some(logger) = LOGGER.get() {
        if let Ok(sinks) = logger.sinks.read() {
            for sink in &*sinks {
                sink.flush();
            }
        }
    }
}

/// Returns whether a record at `level` for `target` would be recorded.
///
/// Used by the logging macros to skip formatting entirely when disabled. If the
/// logger is not initialized, logging is disabled and this returns `false`.
#[must_use]
pub fn enabled(level: LogLevel, target: LogTarget) -> bool {
    match LOGGER.get() {
        Some(logger) => logger.enabled(level, target),
        None => false,
    }
}

/// Submits a fully built record to the global logger. Intended for use by the
/// logging macros after they have confirmed the record is [`enabled`].
pub fn record(record: &LogRecord) {
    if let Some(logger) = LOGGER.get() {
        logger.dispatch(record);
    }
}

#[cfg(test)]
mod tests {
    use super::{add_sink, enabled, init};
    use crate::log::{LogConfig, LogLevel, LogTarget, MemorySink};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct Counter<'a>(&'a AtomicUsize);

    impl core::fmt::Display for Counter<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            self.0.fetch_add(1, Ordering::Relaxed);
            f.write_str("touched")
        }
    }

    // The global logger can only be installed once per process, so all behavior
    // that depends on the singleton is exercised in a single test.
    #[test]
    fn global_logger_end_to_end() {
        let config = LogConfig {
            global_level: LogLevel::Info,
            sinks: Vec::new(),
            ..LogConfig::default()
        };
        assert!(init(&config).is_ok());
        assert!(init(&LogConfig::default()).is_err());

        let memory = Arc::new(MemorySink::new(8));
        add_sink(memory.clone());

        let counter = AtomicUsize::new(0);
        crate::log_trace!(LogTarget::Cpu, "trace {}", Counter(&counter));
        assert_eq!(
            counter.load(Ordering::Relaxed),
            0,
            "a disabled level must not evaluate its arguments"
        );

        crate::log_info!(LogTarget::Vm, "vm booted");
        crate::log_error!(LogTarget::Network, "oops {}", Counter(&counter));
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "an enabled level must evaluate its arguments"
        );

        assert!(enabled(LogLevel::Error, LogTarget::Cpu));
        assert!(!enabled(LogLevel::Trace, LogTarget::Cpu));

        let lines = memory.records();
        assert!(lines
            .iter()
            .any(|line| line.contains("[vm]") && line.contains("vm booted")));
        assert!(lines
            .iter()
            .any(|line| line.contains("ERROR") && line.contains("[net]")));
    }
}
