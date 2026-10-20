//! RFC 3339 UTC timestamp check (AGENTS.md section 10).

/// Whether `text` is an RFC 3339 UTC timestamp:
/// `YYYY-MM-DDTHH:MM:SS[.fraction]Z`, with a real calendar date, hours below
/// 24, minutes below 60 and seconds up to 60 (a leap second).
pub(crate) fn is_rfc3339_utc(text: &str) -> bool {
    let Some(body) = text.strip_suffix('Z') else {
        return false;
    };
    let Some((date, time)) = body.split_once('T') else {
        return false;
    };
    valid_date(date) && valid_time(time)
}

/// Parse `digits` as a number when it is exactly `len` ASCII digits.
fn number(digits: &str, len: usize) -> Option<u32> {
    if digits.len() != len || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Whether `date` is `YYYY-MM-DD` and exists in the Gregorian calendar.
fn valid_date(date: &str) -> bool {
    let parts: Vec<&str> = date.split('-').collect();
    let [year, month, day] = parts[..] else {
        return false;
    };
    let (Some(year), Some(month), Some(day)) = (number(year, 4), number(month, 2), number(day, 2))
    else {
        return false;
    };
    (1..=12).contains(&month) && day >= 1 && day <= days_in(year, month)
}

/// Days in `month` of `year`.
fn days_in(year: u32, month: u32) -> u32 {
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Whether `time` is `HH:MM:SS` with an optional fraction of 1 to 9 digits.
fn valid_time(time: &str) -> bool {
    let (clock, fraction) = match time.split_once('.') {
        Some((clock, fraction)) => (clock, Some(fraction)),
        None => (time, None),
    };
    let parts: Vec<&str> = clock.split(':').collect();
    let [hour, minute, second] = parts[..] else {
        return false;
    };
    let (Some(hour), Some(minute), Some(second)) =
        (number(hour, 2), number(minute, 2), number(second, 2))
    else {
        return false;
    };
    let fraction_ok = fraction
        .is_none_or(|f| (1..=9).contains(&f.len()) && f.bytes().all(|b| b.is_ascii_digit()));
    hour < 24 && minute < 60 && second <= 60 && fraction_ok
}
