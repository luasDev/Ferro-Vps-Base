//! Level/target filtering.

use super::level::LogLevel;
use super::target::LogTarget;

/// Decides whether a `(level, target)` pair should be recorded.
///
/// A global minimum level applies to every target, and per-target overrides can
/// raise or lower the threshold for a single subsystem (for example silencing
/// the very chatty CPU target while keeping a low global level). The
/// [`LogFilter::enabled`] check is a couple of integer comparisons plus an
/// array lookup, so it is cheap enough to call on hot paths.
#[derive(Debug, Clone)]
pub struct LogFilter {
    global: LogLevel,
    overrides: [Option<LogLevel>; LogTarget::COUNT],
}

impl LogFilter {
    /// Creates a filter with the given global minimum level and no overrides.
    #[must_use]
    pub fn new(global: LogLevel) -> Self {
        Self {
            global,
            overrides: [None; LogTarget::COUNT],
        }
    }

    /// Returns the current global minimum level.
    #[must_use]
    pub fn global(&self) -> LogLevel {
        self.global
    }

    /// Sets the global minimum level.
    pub fn set_global(&mut self, level: LogLevel) {
        self.global = level;
    }

    /// Sets a per-target minimum level override.
    pub fn set_target(&mut self, target: LogTarget, level: LogLevel) {
        self.overrides[target.index()] = Some(level);
    }

    /// Returns `true` if a record at `level` for `target` should be recorded.
    #[must_use]
    pub fn enabled(&self, level: LogLevel, target: LogTarget) -> bool {
        let threshold = self.overrides[target.index()].unwrap_or(self.global);
        level >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::LogFilter;
    use crate::log::{LogLevel, LogTarget};

    #[test]
    fn respects_global_level() {
        let filter = LogFilter::new(LogLevel::Info);
        assert!(filter.enabled(LogLevel::Warn, LogTarget::Cpu));
        assert!(filter.enabled(LogLevel::Info, LogTarget::Cpu));
        assert!(!filter.enabled(LogLevel::Debug, LogTarget::Cpu));
    }

    #[test]
    fn override_can_raise_threshold() {
        let mut filter = LogFilter::new(LogLevel::Debug);
        filter.set_target(LogTarget::Cpu, LogLevel::Warn);
        assert!(!filter.enabled(LogLevel::Info, LogTarget::Cpu));
        assert!(filter.enabled(LogLevel::Error, LogTarget::Cpu));
        assert!(filter.enabled(LogLevel::Debug, LogTarget::Gpu));
    }

    #[test]
    fn override_can_lower_threshold() {
        let mut filter = LogFilter::new(LogLevel::Info);
        filter.set_target(LogTarget::Gpu, LogLevel::Trace);
        assert!(filter.enabled(LogLevel::Trace, LogTarget::Gpu));
        assert!(!filter.enabled(LogLevel::Trace, LogTarget::Cpu));
    }
}
