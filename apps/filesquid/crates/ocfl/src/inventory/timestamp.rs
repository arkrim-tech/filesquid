//! RFC 3339 internet date/time check for a version's `created` value (OCFL
//! 1.1 section 3.5.3.1, E049).
//!
//! The grammar is RFC 3339 section 5.6 `date-time`: `YYYY-MM-DD`, `T`, then
//! `hh:mm:ss` with optional fractional seconds, then `Z` or `+hh:mm` /
//! `-hh:mm`. `T` and `Z` may be lowercase (RFC 3339 section 5.6 note). The
//! date must exist (leap years included).
//!
//! Second 60 is a leap second, which RFC 3339 section 5.7 allows only at the
//! end of a UTC day: it is accepted only when the time, moved to UTC by its
//! offset, is 23:59. Whether a leap second was actually inserted on that
//! date is not checked; that needs a leap-second table that changes over
//! time, and OCFL does not ask for it.

/// Minutes in a day.
const DAY_MINUTES: i32 = 24 * 60;

/// The UTC minute of the day a leap second may follow: 23:59.
const LAST_MINUTE: i32 = 23 * 60 + 59;

/// A parsed `hh:mm:ss`.
#[derive(Clone, Copy)]
struct Clock {
    /// Minutes since midnight, local time.
    minute_of_day: i32,
    /// Seconds, 0 to 60.
    second: u32,
}

/// Whether `text` is an RFC 3339 date-time to at least the second, with a
/// time zone.
pub(crate) fn is_rfc3339(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() < 20 {
        return false;
    }
    let (date, rest) = bytes.split_at(10);
    let (time, tail) = rest.split_at(9);
    if !valid_date(date) || !matches!(time[0], b'T' | b't') {
        return false;
    }
    let (Some(clock), Some(offset)) = (read_clock(&time[1..]), read_tail(tail)) else {
        return false;
    };
    clock.second < 60 || ends_utc_day(clock, offset)
}

/// `n` ASCII digits read as a number, or `None`.
fn number(digits: &[u8]) -> Option<u32> {
    digits.iter().try_fold(0u32, |acc, &d| {
        d.is_ascii_digit().then(|| acc * 10 + u32::from(d - b'0'))
    })
}

/// `YYYY-MM-DD` naming a day that exists.
fn valid_date(date: &[u8]) -> bool {
    if date[4] != b'-' || date[7] != b'-' {
        return false;
    }
    let (Some(year), Some(month), Some(day)) = (
        number(&date[..4]),
        number(&date[5..7]),
        number(&date[8..10]),
    ) else {
        return false;
    };
    (1..=12).contains(&month) && (1..=days_in_month(year, month)).contains(&day)
}

/// The number of days in `month` of `year` (Gregorian).
fn days_in_month(year: u32, month: u32) -> u32 {
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `hh:mm:ss` with hour 0-23, minute 0-59 and second 0-60.
fn read_clock(clock: &[u8]) -> Option<Clock> {
    if clock[2] != b':' || clock[5] != b':' {
        return None;
    }
    let hour = number(&clock[..2]).filter(|h| *h <= 23)?;
    let minute = number(&clock[3..5]).filter(|m| *m <= 59)?;
    let second = number(&clock[6..8]).filter(|s| *s <= 60)?;
    Some(Clock {
        minute_of_day: i32::try_from(hour * 60 + minute).ok()?,
        second,
    })
}

/// Optional `.` and at least one digit, then the time zone. Returns the
/// zone's offset from UTC in minutes.
fn read_tail(tail: &[u8]) -> Option<i32> {
    let Some(fraction) = tail.strip_prefix(b".") else {
        return read_zone(tail);
    };
    let digits = fraction.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    read_zone(&fraction[digits..])
}

/// `Z`, `z`, or `+hh:mm` / `-hh:mm`, as an offset from UTC in minutes.
fn read_zone(zone: &[u8]) -> Option<i32> {
    if matches!(zone, [b'Z'] | [b'z']) {
        return Some(0);
    }
    let [sign, h1, h2, b':', m1, m2] = zone else {
        return None;
    };
    let hours = number(&[*h1, *h2]).filter(|h| *h <= 23)?;
    let minutes = number(&[*m1, *m2]).filter(|m| *m <= 59)?;
    let offset = i32::try_from(hours * 60 + minutes).ok()?;
    match sign {
        b'+' => Some(offset),
        b'-' => Some(-offset),
        _ => None,
    }
}

/// Whether `clock`, at `offset` minutes from UTC, is 23:59 UTC: the only
/// minute a leap second may end.
fn ends_utc_day(clock: Clock, offset: i32) -> bool {
    (clock.minute_of_day - offset).rem_euclid(DAY_MINUTES) == LAST_MINUTE
}
