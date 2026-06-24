//! The hand-written parser for the `Ferro-VPS` configuration format.
//!
//! The format is a small `INI`/`TOML`-lite dialect: blank lines and `#`
//! comments are ignored, `[section]` headers select the active section, and
//! `key = value` lines set fields. Values may be integers, booleans,
//! double-quoted strings, or unquoted unit strings such as `64MiB` or `8MHz`.
//!
//! The parser is purely declarative: it never includes other files, expands
//! environment variables, or runs commands. Errors are rich — they always carry
//! the offending line number and field — and the parser never panics.

use crate::error::{ConfigError, FerroResult};
use crate::log::LogTarget;

use super::sections::{
    AudioConfig, CpuConfig, DisplayConfig, LimitsConfig, MemoryConfig, NetworkConfig, PixelFormat,
    StorageConfig, MAX_DESCRIPTION_LEN, MAX_NAME_LEN, MAX_PATH_LEN,
};
use super::units::{ByteSize, ClockHz};
use super::VpsConfig;

/// Maximum accepted size of a configuration document, in bytes (1 `MiB`).
pub(crate) const MAX_CONFIG_BYTES: usize = 1024 * 1024;
/// Maximum accepted number of lines in a configuration document.
pub(crate) const MAX_CONFIG_LINES: usize = 100_000;

/// The active section while parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    /// Top-level keys (the `meta` section).
    Top,
    Cpu,
    Memory,
    Display,
    Storage,
    Audio,
    Network,
    Limits,
}

impl Section {
    fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "cpu" => Some(Self::Cpu),
            "memory" => Some(Self::Memory),
            "display" => Some(Self::Display),
            "storage" => Some(Self::Storage),
            "audio" => Some(Self::Audio),
            "network" => Some(Self::Network),
            "limits" => Some(Self::Limits),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Top => "meta",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Display => "display",
            Self::Storage => "storage",
            Self::Audio => "audio",
            Self::Network => "network",
            Self::Limits => "limits",
        }
    }
}

/// Parses a configuration document into a [`VpsConfig`], applying defaults for
/// any field that is not present.
///
/// When `strict` is `true`, unknown sections and keys are errors; otherwise
/// they are logged at warning level and ignored.
pub(crate) fn parse_document(text: &str, strict: bool) -> FerroResult<VpsConfig> {
    if text.len() > MAX_CONFIG_BYTES {
        return Err(ConfigError::ParseFailed {
            reason: format!("configuration exceeds the maximum size of {MAX_CONFIG_BYTES} bytes"),
        }
        .into());
    }

    let mut config = VpsConfig::default();
    let mut section = Section::Top;
    let mut skip_section = false;

    for (index, raw_line) in text.lines().enumerate() {
        let line_no = index + 1;
        if line_no > MAX_CONFIG_LINES {
            return Err(ConfigError::ParseFailed {
                reason: format!("configuration exceeds the maximum of {MAX_CONFIG_LINES} lines"),
            }
            .into());
        }

        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(header) = section_header(line) {
            match Section::parse(header) {
                Some(parsed) => {
                    section = parsed;
                    skip_section = false;
                }
                None if strict => {
                    return Err(ConfigError::Invalid {
                        field: "section".to_string(),
                        reason: format!("line {line_no}: unknown section `[{header}]`"),
                    }
                    .into());
                }
                None => {
                    crate::log_warn!(
                        LogTarget::Config,
                        "line {line_no}: ignoring unknown section `[{header}]`"
                    );
                    section = Section::Top;
                    skip_section = true;
                }
            }
            continue;
        }

        if skip_section {
            continue;
        }

        let (key, value) = split_key_value(line, line_no)?;
        if !set_field(&mut config, section, key, value, line_no)? {
            if strict {
                return Err(ConfigError::Invalid {
                    field: format!("{}.{key}", section.as_str()),
                    reason: format!("line {line_no}: unknown key `{key}`"),
                }
                .into());
            }
            crate::log_warn!(
                LogTarget::Config,
                "line {line_no}: ignoring unknown key `{key}` in section [{}]",
                section.as_str()
            );
        }
    }

    Ok(config)
}

/// Returns the inner name of a `[section]` header, or `None` if the line is not
/// a header.
fn section_header(line: &str) -> Option<&str> {
    if line.len() >= 2 && line.starts_with('[') && line.ends_with(']') {
        Some(line[1..line.len() - 1].trim())
    } else {
        None
    }
}

/// Splits a `key = value` line, trimming both sides.
fn split_key_value(line: &str, line_no: usize) -> Result<(&str, &str), ConfigError> {
    match line.split_once('=') {
        Some((key, value)) => {
            let key = key.trim();
            if key.is_empty() {
                return Err(ConfigError::ParseFailed {
                    reason: format!("line {line_no}: empty key before `=`"),
                });
            }
            Ok((key, value.trim()))
        }
        None => Err(ConfigError::ParseFailed {
            reason: format!("line {line_no}: expected `key = value`"),
        }),
    }
}

/// Routes a key/value pair to the right section. Returns `Ok(false)` when the
/// key is unknown for the active section.
fn set_field(
    config: &mut VpsConfig,
    section: Section,
    key: &str,
    value: &str,
    line: usize,
) -> Result<bool, ConfigError> {
    match section {
        Section::Top => set_top(config, key, value, line),
        Section::Cpu => set_cpu(&mut config.cpu, key, value, line),
        Section::Memory => set_memory(&mut config.memory, key, value, line),
        Section::Display => set_display(&mut config.display, key, value, line),
        Section::Storage => set_storage(&mut config.storage, key, value, line),
        Section::Audio => set_audio(&mut config.audio, key, value, line),
        Section::Network => set_network(&mut config.network, key, value, line),
        Section::Limits => set_limits(&mut config.limits, key, value, line),
    }
}

fn set_top(
    config: &mut VpsConfig,
    key: &str,
    value: &str,
    line: usize,
) -> Result<bool, ConfigError> {
    match key {
        "config_version" => config.meta.config_version = u32_value(value, "config_version", line)?,
        "vps_name" => config.meta.vps_name = string_value(value, "vps_name", line, MAX_NAME_LEN)?,
        "description" => {
            config.meta.description =
                Some(string_value(value, "description", line, MAX_DESCRIPTION_LEN)?);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn set_cpu(cpu: &mut CpuConfig, key: &str, value: &str, line: usize) -> Result<bool, ConfigError> {
    match key {
        "target_clock_hz" => {
            cpu.target_clock_hz = clockhz_value(value, "cpu.target_clock_hz", line)?;
        }
        "core_count" => cpu.core_count = u32_value(value, "cpu.core_count", line)?,
        "enable_throttle" => cpu.enable_throttle = bool_value(value, "cpu.enable_throttle", line)?,
        "instruction_budget_per_frame" => {
            cpu.instruction_budget_per_frame =
                u64_value(value, "cpu.instruction_budget_per_frame", line)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn set_memory(
    memory: &mut MemoryConfig,
    key: &str,
    value: &str,
    line: usize,
) -> Result<bool, ConfigError> {
    match key {
        "ram_size" => memory.ram_size = bytesize_value(value, "memory.ram_size", line)?,
        "page_size" => memory.page_size = bytesize_value(value, "memory.page_size", line)?,
        "enable_mmu" => memory.enable_mmu = bool_value(value, "memory.enable_mmu", line)?,
        "stack_size_default" => {
            memory.stack_size_default = bytesize_value(value, "memory.stack_size_default", line)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn set_display(
    display: &mut DisplayConfig,
    key: &str,
    value: &str,
    line: usize,
) -> Result<bool, ConfigError> {
    match key {
        "width" => display.width = u32_value(value, "display.width", line)?,
        "height" => display.height = u32_value(value, "display.height", line)?,
        "pixel_format" => display.pixel_format = pixel_format_value(value, line)?,
        "target_fps" => display.target_fps = u32_value(value, "display.target_fps", line)?,
        "scale_factor" => display.scale_factor = u32_value(value, "display.scale_factor", line)?,
        "vsync" => display.vsync = bool_value(value, "display.vsync", line)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn set_storage(
    storage: &mut StorageConfig,
    key: &str,
    value: &str,
    line: usize,
) -> Result<bool, ConfigError> {
    match key {
        "disk_size" => storage.disk_size = bytesize_value(value, "storage.disk_size", line)?,
        "disk_image_path" => {
            storage.disk_image_path = Some(path_value(value, "storage.disk_image_path", line)?);
        }
        "block_size" => storage.block_size = bytesize_value(value, "storage.block_size", line)?,
        "read_only" => storage.read_only = bool_value(value, "storage.read_only", line)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn set_audio(
    audio: &mut AudioConfig,
    key: &str,
    value: &str,
    line: usize,
) -> Result<bool, ConfigError> {
    match key {
        "enabled" => audio.enabled = bool_value(value, "audio.enabled", line)?,
        "sample_rate_hz" => audio.sample_rate_hz = u32_value(value, "audio.sample_rate_hz", line)?,
        "channels" => audio.channels = u8_value(value, "audio.channels", line)?,
        "buffer_frames" => audio.buffer_frames = u32_value(value, "audio.buffer_frames", line)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn set_network(
    network: &mut NetworkConfig,
    key: &str,
    value: &str,
    line: usize,
) -> Result<bool, ConfigError> {
    match key {
        "enabled" => network.enabled = bool_value(value, "network.enabled", line)?,
        "max_sockets" => network.max_sockets = u32_value(value, "network.max_sockets", line)?,
        "loopback_only" => {
            network.loopback_only = bool_value(value, "network.loopback_only", line)?;
        }
        "mtu" => network.mtu = u32_value(value, "network.mtu", line)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn set_limits(
    limits: &mut LimitsConfig,
    key: &str,
    value: &str,
    line: usize,
) -> Result<bool, ConfigError> {
    match key {
        "max_processes" => limits.max_processes = u32_value(value, "limits.max_processes", line)?,
        "max_threads_per_process" => {
            limits.max_threads_per_process =
                u32_value(value, "limits.max_threads_per_process", line)?;
        }
        "max_open_files" => {
            limits.max_open_files = u32_value(value, "limits.max_open_files", line)?;
        }
        "max_cpu_time_ms_per_tick" => {
            limits.max_cpu_time_ms_per_tick =
                u32_value(value, "limits.max_cpu_time_ms_per_tick", line)?;
        }
        "max_total_memory" => {
            limits.max_total_memory = bytesize_value(value, "limits.max_total_memory", line)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// Strips one pair of surrounding double quotes, if present.
fn unquote(raw: &str) -> &str {
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        &raw[1..raw.len() - 1]
    } else {
        raw
    }
}

fn invalid(field: &str, line: usize, reason: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        field: field.to_string(),
        reason: format!("line {line}: {}", reason.into()),
    }
}

fn bool_value(raw: &str, field: &str, line: usize) -> Result<bool, ConfigError> {
    match unquote(raw).to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(invalid(
            field,
            line,
            format!("expected `true` or `false`, found `{other}`"),
        )),
    }
}

fn u32_value(raw: &str, field: &str, line: usize) -> Result<u32, ConfigError> {
    unquote(raw)
        .parse::<u32>()
        .map_err(|error| invalid(field, line, format!("expected an integer: {error}")))
}

fn u64_value(raw: &str, field: &str, line: usize) -> Result<u64, ConfigError> {
    unquote(raw)
        .parse::<u64>()
        .map_err(|error| invalid(field, line, format!("expected an integer: {error}")))
}

fn u8_value(raw: &str, field: &str, line: usize) -> Result<u8, ConfigError> {
    unquote(raw)
        .parse::<u8>()
        .map_err(|error| invalid(field, line, format!("expected a small integer: {error}")))
}

fn bytesize_value(raw: &str, field: &str, line: usize) -> Result<ByteSize, ConfigError> {
    ByteSize::parse_str(unquote(raw)).map_err(|reason| invalid(field, line, reason))
}

fn clockhz_value(raw: &str, field: &str, line: usize) -> Result<ClockHz, ConfigError> {
    ClockHz::parse_str(unquote(raw)).map_err(|reason| invalid(field, line, reason))
}

fn pixel_format_value(raw: &str, line: usize) -> Result<PixelFormat, ConfigError> {
    let unquoted = unquote(raw);
    unquoted.parse::<PixelFormat>().map_err(|_| {
        invalid(
            "display.pixel_format",
            line,
            format!("unknown pixel format `{unquoted}`"),
        )
    })
}

fn string_value(
    raw: &str,
    field: &str,
    line: usize,
    max_len: usize,
) -> Result<String, ConfigError> {
    let unquoted = unquote(raw);
    if unquoted.len() > max_len {
        return Err(invalid(
            field,
            line,
            format!("string exceeds the maximum length of {max_len} bytes"),
        ));
    }
    if unquoted.chars().any(char::is_control) {
        return Err(invalid(field, line, "string contains control characters"));
    }
    Ok(unquoted.to_string())
}

fn path_value(raw: &str, field: &str, line: usize) -> Result<String, ConfigError> {
    let value = string_value(raw, field, line, MAX_PATH_LEN)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(invalid(field, line, "path must not be empty"));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_document, PixelFormat};

    const EXAMPLE: &str = "# comentario\nconfig_version = 1\nvps_name = \"minha-vps\"\n\n[cpu]\ntarget_clock_hz = 8MHz\ncore_count = 1\nenable_throttle = true\n\n[memory]\nram_size = 64MiB\npage_size = 4KiB\n\n[display]\nwidth = 320\nheight = 240\npixel_format = \"rgba8888\"\ntarget_fps = 60\nscale_factor = 2\n\n[storage]\ndisk_size = 256MiB\n\n[audio]\nsample_rate_hz = 44100\nchannels = 2\n\n[network]\nenabled = true\nloopback_only = true\n";

    #[test]
    fn parses_full_example() {
        let config = parse_document(EXAMPLE, true).unwrap();
        assert_eq!(config.meta.config_version, 1);
        assert_eq!(config.meta.vps_name, "minha-vps");
        assert_eq!(config.cpu.target_clock_hz.as_hz(), 8_000_000);
        assert_eq!(config.cpu.core_count, 1);
        assert!(config.cpu.enable_throttle);
        assert_eq!(config.memory.ram_size.as_mib(), 64);
        assert_eq!(config.memory.page_size.as_bytes(), 4096);
        assert_eq!(config.display.width, 320);
        assert_eq!(config.display.height, 240);
        assert_eq!(config.display.pixel_format, PixelFormat::Rgba8888);
        assert_eq!(config.display.target_fps, 60);
        assert_eq!(config.storage.disk_size.as_mib(), 256);
        assert_eq!(config.audio.sample_rate_hz, 44_100);
        assert_eq!(config.audio.channels, 2);
        assert!(config.network.loopback_only);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let config = parse_document("[cpu]\ncore_count = 4\n", false).unwrap();
        assert_eq!(config.cpu.core_count, 4);
        assert_eq!(config.memory.ram_size.as_mib(), 64);
        assert_eq!(config.display.width, 320);
    }

    #[test]
    fn unknown_key_is_ignored_unless_strict() {
        assert!(parse_document("[cpu]\nbogus = 1\n", false).is_ok());
        assert!(parse_document("[cpu]\nbogus = 1\n", true).is_err());
    }

    #[test]
    fn unknown_section_is_ignored_unless_strict() {
        assert!(parse_document("[bogus]\nkey = 1\n", false).is_ok());
        assert!(parse_document("[bogus]\nkey = 1\n", true).is_err());
    }

    #[test]
    fn wrong_value_type_reports_line_and_field() {
        let error = parse_document("# header comment\n\n[cpu]\ncore_count = oops\n", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 4"), "error was: {error}");
        assert!(error.contains("core_count"), "error was: {error}");
    }

    #[test]
    fn missing_equals_is_a_parse_error() {
        assert!(parse_document("[cpu]\ncore_count 1\n", false).is_err());
    }

    #[test]
    fn rejects_control_characters_in_strings() {
        assert!(parse_document("vps_name = \"bad\tname\"\n", false).is_err());
    }
}
