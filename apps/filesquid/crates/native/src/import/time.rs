//! RFC 3339 UTC timestamps at native precision for the import record (ADR 038): a file time
//! as the operating system reports it, to the nanosecond, with no time zone but `Z`.

use filesquid_ocfl::write::rfc3339_utc;
use std::time::{SystemTime, UNIX_EPOCH};

/// `time` as `YYYY-MM-DDTHH:MM:SS.nnnnnnnnnZ`, or `None` for a time before 1970 (which the
/// record cannot state as a valid fact). Pure.
pub fn rfc3339_nanos(time: SystemTime) -> Option<String> {
    let since = time.duration_since(UNIX_EPOCH).ok()?;
    Some(at_nanos(since.as_secs(), since.subsec_nanos()))
}

/// `seconds` and `nanos` after the Unix epoch as an RFC 3339 UTC timestamp with nine
/// fraction digits. Pure.
pub fn at_nanos(seconds: u64, nanos: u32) -> String {
    let whole = rfc3339_utc(seconds.saturating_mul(1_000));
    let stem = whole.strip_suffix('Z').unwrap_or(&whole);
    format!("{stem}.{nanos:09}Z")
}

/// The current time at native precision. Reads the system clock.
pub fn now() -> String {
    rfc3339_nanos(SystemTime::now()).unwrap_or_else(|| at_nanos(0, 0))
}
