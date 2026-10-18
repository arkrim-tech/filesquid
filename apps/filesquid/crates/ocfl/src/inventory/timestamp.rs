//! RFC 3339 internet date/time check for a version's `created` value (OCFL
//! 1.1 section 3.5.3.1, E049).
//!
//! The grammar is RFC 3339 section 5.6 `date-time`: `YYYY-MM-DD`, `T`, then
//! `hh:mm:ss` with optional fractional seconds, then `Z` or `+hh:mm` /
//! `-hh:mm`. `T` and `Z` may be lowercase (RFC 3339 section 5.6 note). The
//! date must exist (leap years included) and a second of 60 is allowed for a
//! leap second.

/// Whether `text` is an RFC 3339 date-time to at least the second, with a
/// time zone.
pub(crate) fn is_rfc3339(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() < 20 {
        return false;
    }
    let (date, rest) = bytes.split_at(10);
    let (time, zone) = rest.split_at(9);
    valid_date(date)
        && matches!(time[0], b'T' | b't')
        && valid_clock(&time[1..])
        && valid_tail(zone)
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

/// `hh:mm:ss`, with a second of 60 allowed for a leap second.
fn valid_clock(clock: &[u8]) -> bool {
    if clock[2] != b':' || clock[5] != b':' {
        return false;
    }
    match (
        number(&clock[..2]),
        number(&clock[3..5]),
        number(&clock[6..8]),
    ) {
        (Some(hour), Some(minute), Some(second)) => hour <= 23 && minute <= 59 && second <= 60,
        _ => false,
    }
}

/// Optional `.` and at least one digit, then the time zone.
fn valid_tail(tail: &[u8]) -> bool {
    let Some(fraction) = tail.strip_prefix(b".") else {
        return valid_zone(tail);
    };
    let digits = fraction.iter().take_while(|b| b.is_ascii_digit()).count();
    digits > 0 && valid_zone(&fraction[digits..])
}

/// `Z`, `z`, or `+hh:mm` / `-hh:mm`.
fn valid_zone(zone: &[u8]) -> bool {
    if matches!(zone, [b'Z'] | [b'z']) {
        return true;
    }
    let [sign, h1, h2, b':', m1, m2] = zone else {
        return false;
    };
    let in_range = |hi: u8, lo: u8, max: u32| number(&[hi, lo]).is_some_and(|n| n <= max);
    matches!(sign, b'+' | b'-') && in_range(*h1, *h2, 23) && in_range(*m1, *m2, 59)
}
