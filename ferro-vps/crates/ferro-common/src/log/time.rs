//! Manual UTC timestamp formatting.
//!
//! Timestamps are produced from [`SystemTime`] without pulling in `chrono` or
//! `time`, keeping the logger free of external runtime dependencies.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Formats a wall-clock time as an `ISO-8601`-like UTC string
/// (`YYYY-MM-DDTHH:MM:SS.mmmZ`).
pub(crate) fn format_wall_clock(time: SystemTime) -> String {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_secs = duration.as_secs();
    let millis = duration.subsec_millis();
    let days = i64::try_from(total_secs / 86_400).unwrap_or_default();
    let secs_of_day = total_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Formats an elapsed duration as `seconds.millis` since the logger started.
pub(crate) fn format_relative(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    let millis = elapsed.subsec_millis();
    format!("{secs}.{millis:03}")
}

/// Converts a count of days since the Unix epoch into a `(year, month, day)`
/// triple using Howard Hinnant's well-known `civil_from_days` algorithm.
///
/// The casts below are bounded by the algorithm itself (month in `1..=12`, day
/// in `1..=31`, and the era arithmetic stays within `i64`), so truncation,
/// sign loss, and wrapping cannot occur for any real timestamp.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u32;
    let month = (if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    }) as u32;
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::{civil_from_days, format_relative, format_wall_clock};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn epoch_formats_to_known_string() {
        assert_eq!(format_wall_clock(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn known_instant_formats_correctly() {
        let time = UNIX_EPOCH + Duration::from_millis(1_700_000_001_123);
        assert_eq!(format_wall_clock(time), "2023-11-14T22:13:21.123Z");
    }

    #[test]
    fn civil_from_days_matches_reference_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_675), (2023, 11, 14));
    }

    #[test]
    fn relative_format_includes_millis() {
        assert_eq!(format_relative(Duration::from_millis(2_007)), "2.007");
    }
}
