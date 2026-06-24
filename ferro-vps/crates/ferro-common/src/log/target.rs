//! Log subsystems (targets).
//!
//! Every crate logs using its own [`LogTarget`] so that records can be
//! filtered per subsystem. The convention is: each crate always passes its own
//! target (for example `ferro-cpu` always logs with [`LogTarget::Cpu`]).

use core::fmt;
use core::str::FromStr;

use crate::error::ConfigError;

/// The subsystem that emitted a log record.
///
/// The string form (see [`LogTarget::as_str`]) is short and stable so it can be
/// used both in formatted output and in the `FERRO_LOG` environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LogTarget {
    /// Shared `ferro-common` code.
    Common,
    /// Configuration loading and validation.
    Config,
    /// The virtual CPU.
    Cpu,
    /// The virtual memory subsystem.
    Memory,
    /// The device bus.
    Bus,
    /// The virtual GPU.
    Gpu,
    /// The storage subsystem.
    Storage,
    /// The audio subsystem.
    Audio,
    /// The network subsystem.
    Network,
    /// The guest kernel.
    Kernel,
    /// The virtual machine orchestration layer.
    Vm,
    /// The assembler.
    Asm,
    /// The guest SDK.
    Sdk,
    /// The host process.
    Host,
    /// The command-line tools.
    Cli,
    /// The developer task runner.
    Xtask,
}

impl LogTarget {
    /// Number of defined targets.
    pub const COUNT: usize = 16;

    /// Every target, in declaration order. Useful for iteration in tooling and
    /// tests.
    pub const ALL: [LogTarget; LogTarget::COUNT] = [
        Self::Common,
        Self::Config,
        Self::Cpu,
        Self::Memory,
        Self::Bus,
        Self::Gpu,
        Self::Storage,
        Self::Audio,
        Self::Network,
        Self::Kernel,
        Self::Vm,
        Self::Asm,
        Self::Sdk,
        Self::Host,
        Self::Cli,
        Self::Xtask,
    ];

    /// Returns the short, stable identifier for this target (for example
    /// `"cpu"` or `"net"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Config => "config",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Bus => "bus",
            Self::Gpu => "gpu",
            Self::Storage => "storage",
            Self::Audio => "audio",
            Self::Network => "net",
            Self::Kernel => "kernel",
            Self::Vm => "vm",
            Self::Asm => "asm",
            Self::Sdk => "sdk",
            Self::Host => "host",
            Self::Cli => "cli",
            Self::Xtask => "xtask",
        }
    }

    /// Returns the dense index of this target, used by the filter's override
    /// table.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Common => 0,
            Self::Config => 1,
            Self::Cpu => 2,
            Self::Memory => 3,
            Self::Bus => 4,
            Self::Gpu => 5,
            Self::Storage => 6,
            Self::Audio => 7,
            Self::Network => 8,
            Self::Kernel => 9,
            Self::Vm => 10,
            Self::Asm => 11,
            Self::Sdk => 12,
            Self::Host => 13,
            Self::Cli => 14,
            Self::Xtask => 15,
        }
    }
}

impl fmt::Display for LogTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LogTarget {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "common" => Ok(Self::Common),
            "config" => Ok(Self::Config),
            "cpu" => Ok(Self::Cpu),
            "memory" => Ok(Self::Memory),
            "bus" => Ok(Self::Bus),
            "gpu" => Ok(Self::Gpu),
            "storage" => Ok(Self::Storage),
            "audio" => Ok(Self::Audio),
            "net" => Ok(Self::Network),
            "kernel" => Ok(Self::Kernel),
            "vm" => Ok(Self::Vm),
            "asm" => Ok(Self::Asm),
            "sdk" => Ok(Self::Sdk),
            "host" => Ok(Self::Host),
            "cli" => Ok(Self::Cli),
            "xtask" => Ok(Self::Xtask),
            other => Err(ConfigError::Invalid {
                field: "log target".to_string(),
                reason: format!("unknown log target `{other}`"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LogTarget;

    #[test]
    fn as_str_and_display_agree() {
        assert_eq!(LogTarget::Cpu.as_str(), "cpu");
        assert_eq!(LogTarget::Network.as_str(), "net");
        assert_eq!(LogTarget::Network.to_string(), "net");
    }

    #[test]
    fn from_str_parses_known_targets() {
        assert_eq!("net".parse::<LogTarget>().unwrap(), LogTarget::Network);
        assert_eq!("GPU".parse::<LogTarget>().unwrap(), LogTarget::Gpu);
        assert!("bogus".parse::<LogTarget>().is_err());
    }

    #[test]
    fn string_round_trip_for_all_targets() {
        for target in LogTarget::ALL {
            assert_eq!(target.as_str().parse::<LogTarget>().unwrap(), target);
        }
    }

    #[test]
    fn indices_are_unique_and_in_range() {
        for (expected, target) in LogTarget::ALL.into_iter().enumerate() {
            assert_eq!(target.index(), expected);
        }
    }
}
