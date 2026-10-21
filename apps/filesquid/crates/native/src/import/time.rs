//! RFC 3339 UTC timestamps at native precision for the import record (ADR 038): a file time
//! as the operating system reports it, to the nanosecond, with no time zone but `Z`.

use filesquid_ocfl::write::rfc3339_utc;
use std::time::{SystemTime, UNIX_EPOCH};

/// The last second RFC 3339 can state with a four-digit year (9999-12-31T23:59:59Z).
const MAX_SECONDS: u64 = 253_402_300_799;

/// `time` as `YYYY-MM-DDTHH:MM:SS.nnnnnnnnnZ`, or `None` for a time before 1970 or after
/// 9999 (which the record cannot state as a valid fact). Pure.
pub fn rfc3339_nanos(time: SystemTime) -> Option<String> {
    let since = time.duration_since(UNIX_EPOCH).ok()?;
    at_nanos(since.as_secs(), since.subsec_nanos())
}

/// `seconds` and `nanos` after the Unix epoch as an RFC 3339 UTC timestamp with nine
/// fraction digits, or `None` when `nanos` is not below one second or the time is after
/// the year 9999. Pure.
pub fn at_nanos(seconds: u64, nanos: u32) -> Option<String> {
    if nanos >= 1_000_000_000 || seconds > MAX_SECONDS {
        return None;
    }
    let whole = rfc3339_utc(seconds * 1_000);
    let stem = whole.strip_suffix('Z')?;
    Some(format!("{stem}.{nanos:09}Z"))
}

/// The current time at native precision, or `None` when the system clock is before 1970 or
/// after 9999, so no import records a time the clock did not give. Reads the system clock.
pub fn now() -> Option<String> {
    rfc3339_nanos(SystemTime::now())
}
