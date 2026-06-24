//! Per-domain configuration sections and their sensible defaults.
//!
//! Every sub-config derives `Debug`, `Clone`, `PartialEq`, and implements
//! [`Default`] so that an unconfigured `Ferro-VPS` boots with a valid, working
//! machine. The numeric bounds enforced by validation are declared here as
//! public constants so tooling and tests can reference the exact limits.
//!
//! No hardware behaviour lives here — these are purely the declarative “specs”
//! that the future component crates will consume.

use core::fmt;
use core::str::FromStr;

use crate::error::ConfigError;

use super::units::{ByteSize, ClockHz};

/// Minimum number of virtual CPU cores.
pub const CORE_COUNT_MIN: u32 = 1;
/// Maximum number of virtual CPU cores.
pub const CORE_COUNT_MAX: u32 = 64;

/// Minimum virtual RAM size (1 `MiB`).
pub const RAM_MIN: ByteSize = ByteSize::from_mib(1);
/// Maximum virtual RAM size (4 `GiB`).
pub const RAM_MAX: ByteSize = ByteSize::from_gib(4);
/// Maximum disk image size (64 `GiB`).
pub const DISK_MAX: ByteSize = ByteSize::from_gib(64);
/// Minimum disk image size (1 `MiB`).
pub const DISK_MIN: ByteSize = ByteSize::from_mib(1);

/// Minimum width or height of the virtual framebuffer, in pixels.
pub const DIMENSION_MIN: u32 = 1;
/// Maximum width or height of the virtual framebuffer, in pixels.
pub const DIMENSION_MAX: u32 = 7680;
/// Maximum total framebuffer area, in pixels (≈ 4K).
pub const PIXELS_MAX: u64 = 3840 * 2160;
/// Minimum target frame rate, in frames per second.
pub const FPS_MIN: u32 = 1;
/// Maximum target frame rate, in frames per second.
pub const FPS_MAX: u32 = 240;
/// Minimum host display scale factor.
pub const SCALE_MIN: u32 = 1;
/// Maximum host display scale factor.
pub const SCALE_MAX: u32 = 16;

/// Minimum audio sample rate, in hertz.
pub const SAMPLE_RATE_MIN: u32 = 8_000;
/// Maximum audio sample rate, in hertz.
pub const SAMPLE_RATE_MAX: u32 = 192_000;

/// Minimum number of simultaneous virtual sockets.
pub const MAX_SOCKETS_MIN: u32 = 1;
/// Maximum number of simultaneous virtual sockets.
pub const MAX_SOCKETS_MAX: u32 = 4096;
/// Minimum network MTU, in bytes.
pub const MTU_MIN: u32 = 576;
/// Maximum network MTU, in bytes.
pub const MTU_MAX: u32 = 9000;

/// Maximum length, in bytes, of the `vps_name` field.
pub const MAX_NAME_LEN: usize = 128;
/// Maximum length, in bytes, of the `description` field.
pub const MAX_DESCRIPTION_LEN: usize = 1024;
/// Maximum length, in bytes, of the `disk_image_path` field.
pub const MAX_PATH_LEN: usize = 4096;

/// The pixel layout of the virtual framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelFormat {
    /// 32-bit RGBA, 8 bits per channel.
    #[default]
    Rgba8888,
    /// 16-bit RGB, 5/6/5 bits per channel.
    Rgb565,
    /// 8-bit palette index.
    Indexed8,
}

impl PixelFormat {
    /// Returns the short, stable identifier for this format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba8888 => "rgba8888",
            Self::Rgb565 => "rgb565",
            Self::Indexed8 => "indexed8",
        }
    }

    /// Returns the number of bytes each pixel occupies in this format.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Rgba8888 => 4,
            Self::Rgb565 => 2,
            Self::Indexed8 => 1,
        }
    }
}

impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PixelFormat {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "rgba8888" => Ok(Self::Rgba8888),
            "rgb565" => Ok(Self::Rgb565),
            "indexed8" => Ok(Self::Indexed8),
            other => Err(ConfigError::Invalid {
                field: "display.pixel_format".to_string(),
                reason: format!("unknown pixel format `{other}`"),
            }),
        }
    }
}

/// Configuration for the virtual CPU.
///
/// Describes only the *specs*; the instruction set and execution semantics are
/// defined by the future CPU crates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuConfig {
    /// Target execution rate of the virtual CPU.
    pub target_clock_hz: ClockHz,
    /// Number of virtual cores (`1..=64`).
    pub core_count: u32,
    /// Whether the executor throttles to `target_clock_hz`.
    pub enable_throttle: bool,
    /// Upper bound of instructions executed per frame/tick. A value of `0`
    /// means “derive automatically from the clock and frame rate”.
    pub instruction_budget_per_frame: u64,
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            target_clock_hz: ClockHz::from_mhz(8),
            core_count: 1,
            enable_throttle: true,
            instruction_budget_per_frame: 0,
        }
    }
}

/// Configuration for the virtual memory subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryConfig {
    /// Total virtual RAM size (`1 MiB ..= 4 GiB`).
    pub ram_size: ByteSize,
    /// Virtual MMU page size; must be a power of two that divides `ram_size`.
    pub page_size: ByteSize,
    /// Whether address translation and protection are enabled.
    pub enable_mmu: bool,
    /// Default per-process guest stack size.
    pub stack_size_default: ByteSize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            ram_size: ByteSize::from_mib(64),
            page_size: ByteSize::from_kib(4),
            enable_mmu: true,
            stack_size_default: ByteSize::from_mib(1),
        }
    }
}

/// Configuration for the virtual display / framebuffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayConfig {
    /// Framebuffer width in pixels (`1..=7680`).
    pub width: u32,
    /// Framebuffer height in pixels (`1..=7680`).
    pub height: u32,
    /// Pixel layout of the framebuffer.
    pub pixel_format: PixelFormat,
    /// Target frame rate in frames per second (`1..=240`).
    pub target_fps: u32,
    /// Integer magnification used when showing the framebuffer on the host
    /// (`1..=16`).
    pub scale_factor: u32,
    /// Whether the host should synchronise presentation to its display refresh.
    pub vsync: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            width: 320,
            height: 240,
            pixel_format: PixelFormat::Rgba8888,
            target_fps: 60,
            scale_factor: 2,
            vsync: true,
        }
    }
}

/// Configuration for the virtual storage subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    /// Virtual disk size (`1 MiB ..= 64 GiB`).
    pub disk_size: ByteSize,
    /// Host path of the backing disk image. When `None`, the disk is held in
    /// memory and is ephemeral (discarded on shutdown).
    pub disk_image_path: Option<String>,
    /// Virtual disk block size; must be a power of two that divides
    /// `disk_size`.
    pub block_size: ByteSize,
    /// Whether the disk is mounted read-only.
    pub read_only: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            disk_size: ByteSize::from_mib(256),
            disk_image_path: None,
            block_size: ByteSize::from_bytes(512),
            read_only: false,
        }
    }
}

/// Configuration for the virtual audio subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioConfig {
    /// Whether audio is enabled.
    pub enabled: bool,
    /// Sample rate in hertz (`8000..=192000`).
    pub sample_rate_hz: u32,
    /// Channel count (`1` for mono, `2` for stereo).
    pub channels: u8,
    /// Mixer buffer size in frames (a power of two is recommended).
    pub buffer_frames: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_rate_hz: 44_100,
            channels: 2,
            buffer_frames: 1024,
        }
    }
}

/// Configuration for the virtual network subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkConfig {
    /// Whether networking is enabled.
    pub enabled: bool,
    /// Maximum number of simultaneous sockets (`1..=4096`).
    pub max_sockets: u32,
    /// When `true`, the virtual network is isolated to loopback and has no
    /// external access. External access is strictly opt-in.
    pub loopback_only: bool,
    /// Maximum transmission unit in bytes (`576..=9000`).
    pub mtu: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_sockets: 64,
            loopback_only: true,
            mtu: 1500,
        }
    }
}

/// Resource ceilings for the guest sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitsConfig {
    /// Maximum number of guest processes.
    pub max_processes: u32,
    /// Maximum number of threads per guest process.
    pub max_threads_per_process: u32,
    /// Maximum number of open files across the guest.
    pub max_open_files: u32,
    /// Host CPU-time budget per tick, in milliseconds.
    pub max_cpu_time_ms_per_tick: u32,
    /// Aggregate guest memory ceiling; must be greater than or equal to
    /// `MemoryConfig::ram_size`.
    pub max_total_memory: ByteSize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_processes: 32,
            max_threads_per_process: 16,
            max_open_files: 256,
            max_cpu_time_ms_per_tick: 8,
            max_total_memory: ByteSize::from_mib(64),
        }
    }
}

/// Metadata about the configuration document itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaConfig {
    /// Schema version of the configuration format, reserved for future
    /// evolution.
    pub config_version: u32,
    /// Human-readable name of this virtual machine.
    pub vps_name: String,
    /// Optional free-form description.
    pub description: Option<String>,
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            config_version: 1,
            vps_name: "ferro-vps".to_string(),
            description: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AudioConfig, CpuConfig, DisplayConfig, LimitsConfig, MemoryConfig, MetaConfig,
        NetworkConfig, PixelFormat, StorageConfig,
    };

    #[test]
    fn defaults_match_spec() {
        assert_eq!(CpuConfig::default().target_clock_hz.as_hz(), 8_000_000);
        assert_eq!(CpuConfig::default().core_count, 1);
        assert_eq!(MemoryConfig::default().ram_size.as_mib(), 64);
        assert_eq!(MemoryConfig::default().page_size.as_bytes(), 4096);
        assert_eq!(DisplayConfig::default().width, 320);
        assert_eq!(DisplayConfig::default().height, 240);
        assert_eq!(DisplayConfig::default().target_fps, 60);
        assert_eq!(StorageConfig::default().disk_size.as_mib(), 256);
        assert_eq!(StorageConfig::default().block_size.as_bytes(), 512);
        assert_eq!(AudioConfig::default().sample_rate_hz, 44_100);
        assert_eq!(AudioConfig::default().channels, 2);
        assert!(NetworkConfig::default().loopback_only);
        assert_eq!(LimitsConfig::default().max_processes, 32);
        assert_eq!(MetaConfig::default().config_version, 1);
        assert_eq!(MetaConfig::default().vps_name, "ferro-vps");
    }

    #[test]
    fn pixel_format_round_trips() {
        for format in [
            PixelFormat::Rgba8888,
            PixelFormat::Rgb565,
            PixelFormat::Indexed8,
        ] {
            assert_eq!(format.as_str().parse::<PixelFormat>().unwrap(), format);
        }
        assert!("bogus".parse::<PixelFormat>().is_err());
        assert_eq!(PixelFormat::default(), PixelFormat::Rgba8888);
    }
}
