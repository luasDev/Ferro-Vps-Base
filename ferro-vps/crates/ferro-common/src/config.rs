//! The `Ferro-VPS` configuration system.
//!
//! This module defines [`VpsConfig`], the root description of a virtual
//! machine's specifications: CPU, memory, display, storage, audio, network,
//! resource limits, and document metadata. Every field has a sensible default,
//! so [`VpsConfig::default`] alone yields a valid, bootable machine.
//!
//! Configuration can be loaded from a small `INI`/`TOML`-lite text format with
//! [`VpsConfig::from_str`], [`VpsConfig::from_file`], or
//! [`VpsConfig::load_or_default`], and serialised back with
//! [`VpsConfig::to_config_string`]. Loading always validates the result, and
//! [`VpsConfig::validate`] accumulates every problem it finds into a single
//! error rather than failing on the first one.
//!
//! Parsing is deliberately defensive: documents are size- and line-capped,
//! unknown keys are ignored (or rejected in strict mode), strings may not
//! contain control characters, and every numeric value is range-checked
//! against the published ceilings. See `docs/CONVENTIONS.md` for the format
//! reference.

#![allow(clippy::module_name_repetitions)]

mod parser;
mod sections;
mod units;

use std::fs;
use std::path::Path;

use crate::error::{ConfigError, FerroResult, ResultContextExt};
use crate::log::LogTarget;

pub use sections::{
    AudioConfig, CpuConfig, DisplayConfig, LimitsConfig, MemoryConfig, MetaConfig, NetworkConfig,
    PixelFormat, StorageConfig,
};
pub use units::{ByteSize, ClockHz};

use sections::{
    CORE_COUNT_MAX, CORE_COUNT_MIN, DIMENSION_MAX, DIMENSION_MIN, DISK_MAX, DISK_MIN, FPS_MAX,
    FPS_MIN, MAX_SOCKETS_MAX, MAX_SOCKETS_MIN, MTU_MAX, MTU_MIN, PIXELS_MAX, RAM_MAX, RAM_MIN,
    SAMPLE_RATE_MAX, SAMPLE_RATE_MIN, SCALE_MAX, SCALE_MIN,
};

/// The complete specification of a virtual machine.
///
/// Aggregates every per-domain section. Derives [`Default`] so an
/// unconfigured machine is fully usable, and [`PartialEq`] so round-trips
/// through [`VpsConfig::to_config_string`] can be asserted exactly.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VpsConfig {
    /// Virtual CPU specification.
    pub cpu: CpuConfig,
    /// Virtual memory specification.
    pub memory: MemoryConfig,
    /// Virtual display / framebuffer specification.
    pub display: DisplayConfig,
    /// Virtual storage specification.
    pub storage: StorageConfig,
    /// Virtual audio specification.
    pub audio: AudioConfig,
    /// Virtual network specification.
    pub network: NetworkConfig,
    /// Guest resource ceilings.
    pub limits: LimitsConfig,
    /// Document metadata.
    pub meta: MetaConfig,
}

impl VpsConfig {
    /// Parses a configuration document and validates the result.
    ///
    /// When `strict` is `true`, unknown sections and keys are rejected;
    /// otherwise they are logged and ignored.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] (wrapped in [`crate::error::FerroError`]) if
    /// the document is malformed, exceeds the size or line caps, or fails
    /// validation.
    pub fn from_str(text: &str, strict: bool) -> FerroResult<Self> {
        let config = parser::parse_document(text, strict)?;
        config.validate()?;
        Ok(config)
    }

    /// Reads and parses a configuration file (non-strict), then validates it.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, fails to parse, or fails
    /// validation.
    pub fn from_file(path: &Path) -> FerroResult<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading configuration file `{}`", path.display()))?;
        Self::from_str(&text, false)
    }

    /// Loads configuration from an optional path, falling back to defaults.
    ///
    /// If `path` is `None` or points at a file that does not exist, the
    /// defaults are used (and the decision is logged). The returned
    /// configuration is always validated.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read, fails to parse,
    /// or fails validation.
    pub fn load_or_default(path: Option<&Path>) -> FerroResult<Self> {
        match path {
            Some(path) if path.exists() => Self::from_file(path),
            Some(path) => {
                crate::log_info!(
                    LogTarget::Config,
                    "configuration file `{}` not found; using defaults",
                    path.display()
                );
                Self::default_validated()
            }
            None => {
                crate::log_info!(LogTarget::Config, "no configuration file provided; using defaults");
                Self::default_validated()
            }
        }
    }

    fn default_validated() -> FerroResult<Self> {
        let config = Self::default();
        config.validate()?;
        Ok(config)
    }

    /// Validates every section and cross-section invariant.
    ///
    /// All problems are accumulated and reported together, so a single call
    /// surfaces every misconfiguration at once.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError::Invalid`] (wrapped in
    /// [`crate::error::FerroError`]) listing every invariant that was
    /// violated.
    pub fn validate(&self) -> FerroResult<()> {
        let mut problems = Vec::new();
        validate_cpu(&self.cpu, &mut problems);
        validate_memory(&self.memory, &mut problems);
        validate_display(&self.display, &mut problems);
        validate_storage(&self.storage, &mut problems);
        validate_audio(&self.audio, &mut problems);
        validate_network(&self.network, &mut problems);
        validate_limits(self, &mut problems);

        if self.cpu.instruction_budget_per_frame == 0 {
            crate::log_info!(
                LogTarget::Config,
                "cpu.instruction_budget_per_frame is auto; derived {} instructions/frame",
                self.instruction_budget_per_frame()
            );
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid {
                field: "vps".to_string(),
                reason: problems.join("; "),
            }
            .into())
        }
    }

    /// Serialises this configuration back into the text format.
    ///
    /// The output round-trips: `VpsConfig::from_str(&config.to_config_string(),
    /// true)` reproduces an equal value.
    #[must_use]
    pub fn to_config_string(&self) -> String {
        use core::fmt::Write as _;
        let mut out = String::with_capacity(640);

        let _ = writeln!(out, "config_version = {}", self.meta.config_version);
        let _ = writeln!(out, "vps_name = \"{}\"", self.meta.vps_name);
        if let Some(description) = &self.meta.description {
            let _ = writeln!(out, "description = \"{description}\"");
        }

        let _ = writeln!(out, "\n[cpu]");
        let _ = writeln!(out, "target_clock_hz = {}", self.cpu.target_clock_hz);
        let _ = writeln!(out, "core_count = {}", self.cpu.core_count);
        let _ = writeln!(out, "enable_throttle = {}", self.cpu.enable_throttle);
        let _ = writeln!(
            out,
            "instruction_budget_per_frame = {}",
            self.cpu.instruction_budget_per_frame
        );

        let _ = writeln!(out, "\n[memory]");
        let _ = writeln!(out, "ram_size = {}", self.memory.ram_size);
        let _ = writeln!(out, "page_size = {}", self.memory.page_size);
        let _ = writeln!(out, "enable_mmu = {}", self.memory.enable_mmu);
        let _ = writeln!(out, "stack_size_default = {}", self.memory.stack_size_default);

        let _ = writeln!(out, "\n[display]");
        let _ = writeln!(out, "width = {}", self.display.width);
        let _ = writeln!(out, "height = {}", self.display.height);
        let _ = writeln!(out, "pixel_format = {}", self.display.pixel_format);
        let _ = writeln!(out, "target_fps = {}", self.display.target_fps);
        let _ = writeln!(out, "scale_factor = {}", self.display.scale_factor);
        let _ = writeln!(out, "vsync = {}", self.display.vsync);

        let _ = writeln!(out, "\n[storage]");
        let _ = writeln!(out, "disk_size = {}", self.storage.disk_size);
        if let Some(path) = &self.storage.disk_image_path {
            let _ = writeln!(out, "disk_image_path = \"{path}\"");
        }
        let _ = writeln!(out, "block_size = {}", self.storage.block_size);
        let _ = writeln!(out, "read_only = {}", self.storage.read_only);

        let _ = writeln!(out, "\n[audio]");
        let _ = writeln!(out, "enabled = {}", self.audio.enabled);
        let _ = writeln!(out, "sample_rate_hz = {}", self.audio.sample_rate_hz);
        let _ = writeln!(out, "channels = {}", self.audio.channels);
        let _ = writeln!(out, "buffer_frames = {}", self.audio.buffer_frames);

        let _ = writeln!(out, "\n[network]");
        let _ = writeln!(out, "enabled = {}", self.network.enabled);
        let _ = writeln!(out, "max_sockets = {}", self.network.max_sockets);
        let _ = writeln!(out, "loopback_only = {}", self.network.loopback_only);
        let _ = writeln!(out, "mtu = {}", self.network.mtu);

        let _ = writeln!(out, "\n[limits]");
        let _ = writeln!(out, "max_processes = {}", self.limits.max_processes);
        let _ = writeln!(
            out,
            "max_threads_per_process = {}",
            self.limits.max_threads_per_process
        );
        let _ = writeln!(out, "max_open_files = {}", self.limits.max_open_files);
        let _ = writeln!(
            out,
            "max_cpu_time_ms_per_tick = {}",
            self.limits.max_cpu_time_ms_per_tick
        );
        let _ = writeln!(out, "max_total_memory = {}", self.limits.max_total_memory);

        out
    }

    /// Returns the total number of pixels in one framebuffer.
    #[must_use]
    pub fn frame_pixels(&self) -> u64 {
        u64::from(self.display.width).saturating_mul(u64::from(self.display.height))
    }

    /// Returns the number of virtual memory pages (`ram_size / page_size`).
    ///
    /// Returns `0` if the page size is zero.
    #[must_use]
    pub fn page_count(&self) -> u64 {
        self.memory
            .ram_size
            .as_bytes()
            .checked_div(self.memory.page_size.as_bytes())
            .unwrap_or(0)
    }

    /// Returns the effective per-frame instruction budget.
    ///
    /// If the configured budget is non-zero it is returned verbatim; otherwise
    /// a budget is derived from the clock rate and target frame rate (at least
    /// one instruction per frame).
    #[must_use]
    pub fn instruction_budget_per_frame(&self) -> u64 {
        if self.cpu.instruction_budget_per_frame != 0 {
            return self.cpu.instruction_budget_per_frame;
        }
        let fps = u64::from(self.display.target_fps.max(1));
        (self.cpu.target_clock_hz.as_hz() / fps).max(1)
    }

    /// Returns the host window dimensions after applying the scale factor.
    #[must_use]
    pub fn window_dimensions(&self) -> (u32, u32) {
        (
            self.display.width.saturating_mul(self.display.scale_factor),
            self.display.height.saturating_mul(self.display.scale_factor),
        )
    }
}

fn out_of_range_u32(problems: &mut Vec<String>, field: &str, value: u32, min: u32, max: u32) {
    if value < min || value > max {
        problems.push(format!("{field}: must be between {min} and {max} (got {value})"));
    }
}

fn out_of_range_bytes(
    problems: &mut Vec<String>,
    field: &str,
    value: ByteSize,
    min: ByteSize,
    max: ByteSize,
) {
    if value < min || value > max {
        problems.push(format!("{field}: must be between {min} and {max} (got {value})"));
    }
}

fn at_least_one(problems: &mut Vec<String>, field: &str, value: u32) {
    if value < 1 {
        problems.push(format!("{field}: must be at least 1"));
    }
}

fn validate_cpu(cpu: &CpuConfig, problems: &mut Vec<String>) {
    out_of_range_u32(problems, "cpu.core_count", cpu.core_count, CORE_COUNT_MIN, CORE_COUNT_MAX);
    if cpu.target_clock_hz.as_hz() == 0 {
        problems.push("cpu.target_clock_hz: must be greater than zero".to_string());
    }
}

fn validate_memory(memory: &MemoryConfig, problems: &mut Vec<String>) {
    out_of_range_bytes(problems, "memory.ram_size", memory.ram_size, RAM_MIN, RAM_MAX);
    if !memory.page_size.is_power_of_two() {
        problems.push(format!(
            "memory.page_size: must be a power of two (got {})",
            memory.page_size
        ));
    } else if !memory.page_size.divides(memory.ram_size) {
        problems.push(format!(
            "memory.page_size: must evenly divide ram_size ({} does not divide {})",
            memory.page_size, memory.ram_size
        ));
    }
    if memory.stack_size_default.as_bytes() == 0
        || memory.stack_size_default > memory.ram_size
    {
        problems.push(format!(
            "memory.stack_size_default: must be between 1 byte and ram_size ({})",
            memory.ram_size
        ));
    }
}

fn validate_display(display: &DisplayConfig, problems: &mut Vec<String>) {
    out_of_range_u32(problems, "display.width", display.width, DIMENSION_MIN, DIMENSION_MAX);
    out_of_range_u32(problems, "display.height", display.height, DIMENSION_MIN, DIMENSION_MAX);
    out_of_range_u32(problems, "display.target_fps", display.target_fps, FPS_MIN, FPS_MAX);
    out_of_range_u32(problems, "display.scale_factor", display.scale_factor, SCALE_MIN, SCALE_MAX);

    let pixels = u64::from(display.width).saturating_mul(u64::from(display.height));
    if pixels > PIXELS_MAX {
        problems.push(format!(
            "display: width×height ({pixels} px) exceeds the maximum of {PIXELS_MAX} px"
        ));
    }
    if display.width.checked_mul(display.scale_factor).is_none()
        || display.height.checked_mul(display.scale_factor).is_none()
    {
        problems.push("display: scaled window dimensions overflow".to_string());
    }
}

fn validate_storage(storage: &StorageConfig, problems: &mut Vec<String>) {
    out_of_range_bytes(problems, "storage.disk_size", storage.disk_size, DISK_MIN, DISK_MAX);
    if !storage.block_size.is_power_of_two() {
        problems.push(format!(
            "storage.block_size: must be a power of two (got {})",
            storage.block_size
        ));
    } else if !storage.block_size.divides(storage.disk_size) {
        problems.push(format!(
            "storage.block_size: must evenly divide disk_size ({} does not divide {})",
            storage.block_size, storage.disk_size
        ));
    }
    if let Some(path) = &storage.disk_image_path {
        if path.trim().is_empty() {
            problems.push("storage.disk_image_path: must not be empty".to_string());
        }
    }
}

fn validate_audio(audio: &AudioConfig, problems: &mut Vec<String>) {
    out_of_range_u32(
        problems,
        "audio.sample_rate_hz",
        audio.sample_rate_hz,
        SAMPLE_RATE_MIN,
        SAMPLE_RATE_MAX,
    );
    if audio.channels != 1 && audio.channels != 2 {
        problems.push(format!(
            "audio.channels: must be 1 (mono) or 2 (stereo) (got {})",
            audio.channels
        ));
    }
    if audio.buffer_frames == 0 {
        problems.push("audio.buffer_frames: must be greater than zero".to_string());
    }
}

fn validate_network(network: &NetworkConfig, problems: &mut Vec<String>) {
    out_of_range_u32(
        problems,
        "network.max_sockets",
        network.max_sockets,
        MAX_SOCKETS_MIN,
        MAX_SOCKETS_MAX,
    );
    out_of_range_u32(problems, "network.mtu", network.mtu, MTU_MIN, MTU_MAX);
}

fn validate_limits(config: &VpsConfig, problems: &mut Vec<String>) {
    let limits = &config.limits;
    at_least_one(problems, "limits.max_processes", limits.max_processes);
    at_least_one(problems, "limits.max_threads_per_process", limits.max_threads_per_process);
    at_least_one(problems, "limits.max_open_files", limits.max_open_files);
    at_least_one(problems, "limits.max_cpu_time_ms_per_tick", limits.max_cpu_time_ms_per_tick);
    if limits.max_total_memory < config.memory.ram_size {
        problems.push(format!(
            "limits.max_total_memory: must be at least ram_size ({}) (got {})",
            config.memory.ram_size, limits.max_total_memory
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{ByteSize, PixelFormat, VpsConfig};

    #[test]
    fn default_is_valid() {
        assert!(VpsConfig::default().validate().is_ok());
    }

    #[test]
    fn load_or_default_none_returns_default() {
        let config = VpsConfig::load_or_default(None).unwrap();
        assert_eq!(config, VpsConfig::default());
    }

    #[test]
    fn round_trips_through_config_string() {
        let original = VpsConfig::default();
        let text = original.to_config_string();
        let reparsed = VpsConfig::from_str(&text, true).unwrap();
        assert_eq!(original, reparsed);
    }

    #[test]
    fn round_trips_customised_config() {
        let mut original = VpsConfig::default();
        original.cpu.core_count = 4;
        original.cpu.instruction_budget_per_frame = 500;
        original.memory.ram_size = ByteSize::from_mib(128);
        original.display.width = 640;
        original.display.height = 480;
        original.display.pixel_format = PixelFormat::Rgb565;
        original.storage.disk_image_path = Some("disk.img".to_string());
        original.meta.description = Some("my machine".to_string());
        original.limits.max_total_memory = ByteSize::from_mib(256);

        let text = original.to_config_string();
        let reparsed = VpsConfig::from_str(&text, true).unwrap();
        assert_eq!(original, reparsed);
    }

    #[test]
    fn rejects_out_of_range_values() {
        let mut config = VpsConfig::default();
        config.cpu.core_count = 0;
        config.display.target_fps = 1000;
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("core_count"), "error was: {error}");
        assert!(error.contains("target_fps"), "error was: {error}");
    }

    #[test]
    fn rejects_non_power_of_two_page_size() {
        let mut config = VpsConfig::default();
        config.memory.page_size = ByteSize::from_bytes(3000);
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_total_memory_below_ram() {
        let mut config = VpsConfig::default();
        config.limits.max_total_memory = ByteSize::from_mib(1);
        assert!(config.validate().is_err());
    }

    #[test]
    fn derived_values_are_consistent() {
        let config = VpsConfig::default();
        assert_eq!(config.frame_pixels(), 320 * 240);
        assert_eq!(config.page_count(), 64 * 1024 * 1024 / 4096);
        assert_eq!(config.window_dimensions(), (640, 480));
        assert_eq!(config.instruction_budget_per_frame(), 8_000_000 / 60);
    }
}
