//! Strongly-typed unit helpers for the configuration system.
//!
//! [`ByteSize`] represents a size in bytes and [`ClockHz`] represents a
//! frequency in hertz. Both parse human-friendly strings, are case-insensitive
//! on the unit suffix, tolerate surrounding whitespace, and reject any value
//! that would overflow [`u64`] instead of wrapping silently.
//!
//! # Binary vs decimal units
//!
//! [`ByteSize`] understands both *binary* suffixes (powers of 1024) and
//! *decimal* suffixes (powers of 1000):
//!
//! - Binary: `KiB` = 1024, `MiB` = 1024², `GiB` = 1024³, `TiB` = 1024⁴.
//! - Decimal: `KB` = 1000, `MB` = 1000², `GB` = 1000³, `TB` = 1000⁴.
//! - A bare `B` or no suffix at all means raw bytes.
//!
//! [`ClockHz`] understands `Hz`, `kHz` (1000), `MHz` (`1_000_000`) and `GHz`
//! (`1_000_000_000`), plus a bare number meaning hertz.
//!
//! Fractional values such as `"3.5MHz"` or `"1.5KiB"` are supported using exact
//! integer arithmetic (no floating point), and any fractional remainder smaller
//! than one base unit is truncated toward zero.

use core::fmt;
use core::str::FromStr;

use crate::error::ConfigError;

/// Number of bytes in one binary kibibyte.
const KIB: u64 = 1024;
/// Number of bytes in one binary mebibyte.
const MIB: u64 = 1024 * 1024;
/// Number of bytes in one binary gibibyte.
const GIB: u64 = 1024 * 1024 * 1024;

/// One kilohertz in hertz.
const KHZ: u64 = 1_000;
/// One megahertz in hertz.
const MHZ: u64 = 1_000_000;
/// One gigahertz in hertz.
const GHZ: u64 = 1_000_000_000;

/// A size expressed in bytes.
///
/// Stored as a [`u64`] count of bytes. Construct it from friendly strings with
/// [`str::parse`] / [`FromStr`], or from explicit amounts with [`ByteSize::from_bytes`]
/// and friends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteSize(u64);

impl ByteSize {
    /// Builds a [`ByteSize`] from a raw byte count.
    #[must_use]
    pub const fn from_bytes(bytes: u64) -> Self {
        Self(bytes)
    }

    /// Builds a [`ByteSize`] from a number of binary kibibytes (saturating on
    /// overflow).
    #[must_use]
    pub const fn from_kib(kib: u64) -> Self {
        Self(kib.saturating_mul(KIB))
    }

    /// Builds a [`ByteSize`] from a number of binary mebibytes (saturating on
    /// overflow).
    #[must_use]
    pub const fn from_mib(mib: u64) -> Self {
        Self(mib.saturating_mul(MIB))
    }

    /// Builds a [`ByteSize`] from a number of binary gibibytes (saturating on
    /// overflow).
    #[must_use]
    pub const fn from_gib(gib: u64) -> Self {
        Self(gib.saturating_mul(GIB))
    }

    /// Returns the size as a raw byte count.
    #[must_use]
    pub const fn as_bytes(self) -> u64 {
        self.0
    }

    /// Returns the size in whole binary kibibytes, rounding down.
    #[must_use]
    pub const fn as_kib(self) -> u64 {
        self.0 / KIB
    }

    /// Returns the size in whole binary mebibytes, rounding down.
    #[must_use]
    pub const fn as_mib(self) -> u64 {
        self.0 / MIB
    }

    /// Returns `true` if the size is an exact power of two and greater than
    /// zero.
    #[must_use]
    pub const fn is_power_of_two(self) -> bool {
        self.0 != 0 && self.0.is_power_of_two()
    }

    /// Returns `true` if this size divides `other` evenly (and is non-zero).
    #[must_use]
    pub const fn divides(self, other: ByteSize) -> bool {
        self.0 != 0 && other.0 % self.0 == 0
    }

    /// Parses a friendly size string, returning a plain reason string on
    /// failure so callers can attach their own field context.
    pub(crate) fn parse_str(input: &str) -> Result<Self, String> {
        let (number, suffix) = split_number_and_suffix(input)?;
        let unit = match suffix.as_str() {
            "" | "b" => 1,
            "kib" => KIB,
            "mib" => MIB,
            "gib" => GIB,
            "tib" => GIB.saturating_mul(KIB),
            "kb" => 1_000,
            "mb" => 1_000_000,
            "gb" => 1_000_000_000,
            "tb" => 1_000_000_000_000,
            other => return Err(format!("unknown size unit `{other}`")),
        };
        Ok(Self(scale_decimal(&number, unit)?))
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.0;
        if bytes != 0 && bytes % GIB == 0 {
            write!(f, "{}GiB", bytes / GIB)
        } else if bytes != 0 && bytes % MIB == 0 {
            write!(f, "{}MiB", bytes / MIB)
        } else if bytes != 0 && bytes % KIB == 0 {
            write!(f, "{}KiB", bytes / KIB)
        } else {
            write!(f, "{bytes}")
        }
    }
}

impl FromStr for ByteSize {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_str(value).map_err(|reason| ConfigError::ParseFailed { reason })
    }
}

/// A frequency expressed in hertz.
///
/// Stored as a [`u64`] count of hertz. Parses friendly strings such as
/// `"8MHz"`, `"500kHz"`, `"3.5MHz"`, or a bare `"1000000"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClockHz(u64);

impl ClockHz {
    /// Builds a [`ClockHz`] from a raw hertz count.
    #[must_use]
    pub const fn from_hz(hz: u64) -> Self {
        Self(hz)
    }

    /// Builds a [`ClockHz`] from a number of kilohertz (saturating on
    /// overflow).
    #[must_use]
    pub const fn from_khz(khz: u64) -> Self {
        Self(khz.saturating_mul(KHZ))
    }

    /// Builds a [`ClockHz`] from a number of megahertz (saturating on
    /// overflow).
    #[must_use]
    pub const fn from_mhz(mhz: u64) -> Self {
        Self(mhz.saturating_mul(MHZ))
    }

    /// Returns the frequency as a raw hertz count.
    #[must_use]
    pub const fn as_hz(self) -> u64 {
        self.0
    }

    /// Parses a friendly frequency string, returning a plain reason string on
    /// failure so callers can attach their own field context.
    pub(crate) fn parse_str(input: &str) -> Result<Self, String> {
        let (number, suffix) = split_number_and_suffix(input)?;
        let unit = match suffix.as_str() {
            "" | "hz" => 1,
            "khz" => KHZ,
            "mhz" => MHZ,
            "ghz" => GHZ,
            other => return Err(format!("unknown frequency unit `{other}`")),
        };
        Ok(Self(scale_decimal(&number, unit)?))
    }
}

impl fmt::Display for ClockHz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hz = self.0;
        if hz != 0 && hz % GHZ == 0 {
            write!(f, "{}GHz", hz / GHZ)
        } else if hz != 0 && hz % MHZ == 0 {
            write!(f, "{}MHz", hz / MHZ)
        } else if hz != 0 && hz % KHZ == 0 {
            write!(f, "{}kHz", hz / KHZ)
        } else {
            write!(f, "{hz}Hz")
        }
    }
}

impl FromStr for ClockHz {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_str(value).map_err(|reason| ConfigError::ParseFailed { reason })
    }
}

/// Splits a friendly unit string into its numeric prefix and lowercased suffix.
///
/// The numeric prefix is the leading run of ASCII digits and decimal points;
/// everything after it (with surrounding whitespace trimmed) is the suffix.
fn split_number_and_suffix(input: &str) -> Result<(String, String), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty value".to_string());
    }
    let split_at = trimmed
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(trimmed.len());
    let (number, suffix) = trimmed.split_at(split_at);
    Ok((
        number.trim().to_string(),
        suffix.trim().to_ascii_lowercase(),
    ))
}

/// Multiplies a decimal number string by an integer `unit` using exact integer
/// arithmetic, rejecting overflow and malformed input.
///
/// Any fractional remainder smaller than one whole unit is truncated.
fn scale_decimal(number: &str, unit: u64) -> Result<u64, String> {
    if number.is_empty() {
        return Err("missing numeric value".to_string());
    }
    let (int_part, frac_part) = match number.split_once('.') {
        Some((int_part, frac_part)) => (int_part, frac_part),
        None => (number, ""),
    };

    let int_value = if int_part.is_empty() {
        0
    } else {
        int_part
            .parse::<u64>()
            .map_err(|error| format!("invalid integer `{int_part}`: {error}"))?
    };
    let mut total = int_value
        .checked_mul(unit)
        .ok_or_else(|| "value is too large and would overflow".to_string())?;

    if !frac_part.is_empty() {
        if !frac_part.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!("invalid fractional part `{frac_part}`"));
        }
        let exponent = u32::try_from(frac_part.len())
            .map_err(|_| "fractional part is too long".to_string())?;
        let scale = 10u64
            .checked_pow(exponent)
            .ok_or_else(|| "fractional part is too long".to_string())?;
        let frac_digits = frac_part
            .parse::<u64>()
            .map_err(|error| format!("invalid fractional part: {error}"))?;
        let contribution = unit
            .checked_mul(frac_digits)
            .ok_or_else(|| "value is too large and would overflow".to_string())?
            / scale;
        total = total
            .checked_add(contribution)
            .ok_or_else(|| "value is too large and would overflow".to_string())?;
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::{ByteSize, ClockHz};

    #[test]
    fn parses_binary_and_decimal_byte_units() {
        assert_eq!("1024".parse::<ByteSize>().unwrap().as_bytes(), 1024);
        assert_eq!("1KiB".parse::<ByteSize>().unwrap().as_bytes(), 1024);
        assert_eq!("64MiB".parse::<ByteSize>().unwrap().as_bytes(), 64 * 1024 * 1024);
        assert_eq!("1GiB".parse::<ByteSize>().unwrap().as_bytes(), 1024 * 1024 * 1024);
        assert_eq!("8 MB".parse::<ByteSize>().unwrap().as_bytes(), 8_000_000);
        assert_eq!("2KB".parse::<ByteSize>().unwrap().as_bytes(), 2000);
        assert_eq!("512B".parse::<ByteSize>().unwrap().as_bytes(), 512);
    }

    #[test]
    fn parses_case_insensitive_and_whitespace() {
        assert_eq!("64mib".parse::<ByteSize>().unwrap().as_bytes(), 64 * 1024 * 1024);
        assert_eq!("  4 KiB ".parse::<ByteSize>().unwrap().as_bytes(), 4096);
    }

    #[test]
    fn parses_fractional_sizes_exactly() {
        assert_eq!("1.5KiB".parse::<ByteSize>().unwrap().as_bytes(), 1536);
        assert_eq!("0.5GiB".parse::<ByteSize>().unwrap().as_bytes(), 512 * 1024 * 1024);
    }

    #[test]
    fn rejects_invalid_byte_values() {
        assert!("".parse::<ByteSize>().is_err());
        assert!("abc".parse::<ByteSize>().is_err());
        assert!("12ZB".parse::<ByteSize>().is_err());
        assert!("-5".parse::<ByteSize>().is_err());
        assert!("1.2.3KiB".parse::<ByteSize>().is_err());
    }

    #[test]
    fn rejects_byte_overflow() {
        assert!("99999999999GiB".parse::<ByteSize>().is_err());
        assert!("18446744073709551616".parse::<ByteSize>().is_err());
    }

    #[test]
    fn parses_frequencies() {
        assert_eq!("1MHz".parse::<ClockHz>().unwrap().as_hz(), 1_000_000);
        assert_eq!("500kHz".parse::<ClockHz>().unwrap().as_hz(), 500_000);
        assert_eq!("3.5MHz".parse::<ClockHz>().unwrap().as_hz(), 3_500_000);
        assert_eq!("1000000".parse::<ClockHz>().unwrap().as_hz(), 1_000_000);
        assert_eq!("2ghz".parse::<ClockHz>().unwrap().as_hz(), 2_000_000_000);
    }

    #[test]
    fn rejects_invalid_frequencies() {
        assert!("".parse::<ClockHz>().is_err());
        assert!("fast".parse::<ClockHz>().is_err());
        assert!("5XHz".parse::<ClockHz>().is_err());
        assert!("99999999999GHz".parse::<ClockHz>().is_err());
    }

    #[test]
    fn display_round_trips_through_parse() {
        for raw in ["1024", "64MiB", "1GiB", "4KiB", "1536", "0"] {
            let parsed = raw.parse::<ByteSize>().unwrap();
            let reparsed = parsed.to_string().parse::<ByteSize>().unwrap();
            assert_eq!(parsed, reparsed);
        }
        for raw in ["8MHz", "500kHz", "2GHz", "1234", "0"] {
            let parsed = raw.parse::<ClockHz>().unwrap();
            let reparsed = parsed.to_string().parse::<ClockHz>().unwrap();
            assert_eq!(parsed, reparsed);
        }
    }

    #[test]
    fn helpers_report_power_of_two_and_division() {
        assert!(ByteSize::from_kib(4).is_power_of_two());
        assert!(!ByteSize::from_bytes(3000).is_power_of_two());
        assert!(ByteSize::from_kib(4).divides(ByteSize::from_mib(64)));
        assert!(!ByteSize::from_bytes(3).divides(ByteSize::from_mib(64)));
    }
}
