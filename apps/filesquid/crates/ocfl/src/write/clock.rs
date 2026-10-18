//! Time for the write path: lease expiry and version `created` values.
//!
//! The engine never reads the system clock itself, because
//! `wasm32-unknown-unknown` has none: the caller hands it a [`Clock`].
//! [`SystemClock`] is the native one. Times are Unix milliseconds in UTC,
//! and [`rfc3339_utc`] writes them as RFC 3339 UTC text, as OCFL `created`
//! values and every FileSquid timestamp require.

use crate::storage::{MaybeSend, MaybeSync};

/// A source of the current time.
pub trait Clock: MaybeSend + MaybeSync {
    /// Milliseconds since 1970-01-01T00:00:00Z.
    fn now_millis(&self) -> u64;
}

/// The operating system's clock (native targets only).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

#[cfg(not(target_arch = "wasm32"))]
impl Clock for SystemClock {
    /// The system time; a clock set before 1970 reads as 0 and is logged,
    /// which makes every lease look expired rather than everlasting.
    fn now_millis(&self) -> u64 {
        let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        match since {
            Ok(duration) => u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            Err(err) => {
                tracing::warn!(error = %err, "the system clock is set before 1970");
                0
            }
        }
    }
}

/// Seconds in a day.
const DAY_SECONDS: u64 = 86_400;

/// `millis` as RFC 3339 UTC to the second, such as `2026-10-18T12:00:00Z`.
/// Milliseconds are dropped. Valid for years up to 9999; later times give
/// text that is not RFC 3339, which an inventory then refuses (E049).
pub fn rfc3339_utc(millis: u64) -> String {
    let seconds = millis / 1_000;
    let (year, month, day) = civil_from_days(seconds / DAY_SECONDS);
    let time = seconds % DAY_SECONDS;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3_600,
        time % 3_600 / 60,
        time % 60
    )
}

/// The proleptic Gregorian date `days` after 1970-01-01, as
/// (year, month, day). Howard Hinnant's `civil_from_days`, for days on or
/// after the epoch.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}
