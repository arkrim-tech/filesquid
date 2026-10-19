//! Validated scalar values and the character rules for identifiers
//! (PROV-S-002, PROV-S-003).
//!
//! [`Timestamp`] is an RFC 3339 UTC date-time, [`Sha256Digest`] a SHA-256
//! digest in lower-case hexadecimal and [`InstallationId`] a UUID version 4
//! (ADR 021). The `check_*` functions are the profile's rules for the other
//! strings a record holds. Everything here is pure computation.

use std::cmp::Ordering;
use std::fmt;

use crate::error::InvalidValue;

/// Longest identifier (object, operation, journal entry) in bytes.
pub(crate) const MAX_IDENTIFIER_BYTES: usize = 1024;

/// Longest logical path in bytes.
pub(crate) const MAX_PATH_BYTES: usize = 4096;

/// Longest operation kind in bytes.
pub(crate) const MAX_KIND_BYTES: usize = 128;

/// Longest software name, distribution or version in bytes.
pub(crate) const MAX_LABEL_BYTES: usize = 256;

/// Longest version message in bytes.
pub(crate) const MAX_MESSAGE_BYTES: usize = 65_536;

/// Instant of a [`Timestamp`]: year, month, day, hour, minute, second, nanosecond.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Instant(u16, u8, u8, u8, u8, u8, u32);

/// An RFC 3339 date-time in UTC: `YYYY-MM-DDTHH:MM:SS[.fraction]Z`.
///
/// Upper-case `T` and `Z` are required, the fraction has 1 to 9 digits, and a
/// leap second (`:60`) is accepted. The original text is kept verbatim, so a
/// record round-trips byte for byte. Ordering is by instant first and by text
/// only between two spellings of one instant (`.5Z` and `.50Z`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Timestamp {
    text: String,
    instant: Instant,
}

impl Timestamp {
    /// Parse `text` as an RFC 3339 UTC date-time.
    ///
    /// # Errors
    /// [`InvalidValue`] when the text is not in the form above, names an
    /// offset other than `Z`, or names a day or time that does not exist.
    pub fn parse(text: &str) -> Result<Self, InvalidValue> {
        let instant = parse_instant(text.as_bytes()).ok_or(InvalidValue {
            what: "RFC 3339 UTC timestamp",
        })?;
        Ok(Self {
            text: text.to_string(),
            instant,
        })
    }

    /// The timestamp exactly as it was given.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The instant, for ordering by time regardless of spelling.
    pub(crate) fn instant(&self) -> Instant {
        self.instant
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.instant, &self.text).cmp(&(other.instant, &other.text))
    }
}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// Read `len` ASCII digits of `bytes` from `start` as a number.
fn digits(bytes: &[u8], start: usize, len: usize) -> Option<u32> {
    let slice = bytes.get(start..start + len)?;
    slice.iter().try_fold(0u32, |acc, b| {
        b.is_ascii_digit().then(|| acc * 10 + u32::from(b - b'0'))
    })
}

/// Number of days in `month` of `year` (Gregorian calendar).
fn days_in_month(year: u32, month: u32) -> u32 {
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Parse the fraction and the closing `Z` that follow the seconds at `at`.
fn parse_fraction(bytes: &[u8], at: usize) -> Option<u32> {
    if bytes.len() == at + 1 && bytes[at] == b'Z' {
        return Some(0);
    }
    let digits_end = bytes.len().checked_sub(1)?;
    let count = digits_end.checked_sub(at + 1)?;
    let valid = bytes[at] == b'.' && bytes[digits_end] == b'Z' && (1..=9).contains(&count);
    let value = digits(bytes, at + 1, count).filter(|_| valid)?;
    Some(value * 10u32.pow((9 - count) as u32))
}

/// Parse `YYYY-MM-DDTHH:MM:SS[.f]Z` into an [`Instant`], checking ranges.
fn parse_instant(bytes: &[u8]) -> Option<Instant> {
    let separators = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];
    if bytes.len() < 20 || separators.iter().any(|(i, c)| bytes[*i] != *c) {
        return None;
    }
    let (year, month, day) = (
        digits(bytes, 0, 4)?,
        digits(bytes, 5, 2)?,
        digits(bytes, 8, 2)?,
    );
    let (hour, minute, second) = (
        digits(bytes, 11, 2)?,
        digits(bytes, 14, 2)?,
        digits(bytes, 17, 2)?,
    );
    let nanos = parse_fraction(bytes, 19)?;
    let date_ok = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
    let time_ok = hour < 24 && minute < 60 && second <= 60;
    // Every field fits its type: at most four digits for the year and two
    // for the others, so the casts never truncate.
    (date_ok && time_ok).then_some(Instant(
        year as u16,
        month as u8,
        day as u8,
        hour as u8,
        minute as u8,
        second as u8,
        nanos,
    ))
}

/// A SHA-256 digest: 64 hexadecimal characters, kept in lower case.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Parse a 64-character hexadecimal digest; upper case is lowered.
    ///
    /// # Errors
    /// [`InvalidValue`] when the text is not 64 hexadecimal characters.
    pub fn parse(text: &str) -> Result<Self, InvalidValue> {
        let valid = text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit());
        valid
            .then(|| Self(text.to_ascii_lowercase()))
            .ok_or(InvalidValue {
                what: "SHA-256 digest",
            })
    }

    /// The digest in lower-case hexadecimal.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The installation UUID that identifies the owner's FileSquid installation:
/// a UUID version 4 (ADR 021), kept in canonical lower case.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstallationId(String);

impl InstallationId {
    /// Parse a hyphenated UUID version 4 (RFC 9562 variant); upper case is lowered.
    ///
    /// # Errors
    /// [`InvalidValue`] when the text is not a hyphenated UUID, its version
    /// nibble is not `4` or its variant is not RFC 9562.
    pub fn parse(text: &str) -> Result<Self, InvalidValue> {
        let bytes = text.as_bytes();
        let shape = bytes.len() == 36
            && bytes.iter().enumerate().all(|(i, b)| match i {
                8 | 13 | 18 | 23 => *b == b'-',
                _ => b.is_ascii_hexdigit(),
            });
        let v4 = shape && bytes[14] == b'4' && b"89abAB".contains(&bytes[19]);
        v4.then(|| Self(text.to_ascii_lowercase()))
            .ok_or(InvalidValue { what: "UUID v4" })
    }

    /// The UUID in canonical lower case, without the `urn:uuid:` prefix.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The agent identifier `urn:uuid:<uuid>`.
    pub fn urn(&self) -> String {
        format!("urn:uuid:{}", self.0)
    }
}

/// Whether `b` may appear in an identifier: RFC 3986 `pchar` without `%`.
fn identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/".contains(&b)
}

/// Check an object, operation or journal entry identifier: 1 to 1024 bytes of
/// RFC 3986 path characters (unreserved, sub-delims, `:`, `@`, `/`), no `%`.
pub(crate) fn check_identifier(text: &str) -> bool {
    (1..=MAX_IDENTIFIER_BYTES).contains(&text.len()) && text.bytes().all(identifier_byte)
}

/// Check an OCFL version name: `v` followed by 1 to 31 digits, not all zero.
pub(crate) fn check_version(text: &str) -> bool {
    let Some(number) = text.strip_prefix('v') else {
        return false;
    };
    (1..=31).contains(&number.len())
        && number.bytes().all(|b| b.is_ascii_digit())
        && number.bytes().any(|b| b != b'0')
}

/// Check an operation kind: 1 to 128 bytes of ASCII letters, digits, `.`, `_`, `-`.
pub(crate) fn check_kind(text: &str) -> bool {
    (1..=MAX_KIND_BYTES).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// Check a software label (product, distribution, version, tool name):
/// 1 to 256 bytes with no control character.
pub(crate) fn check_label(text: &str) -> bool {
    (1..=MAX_LABEL_BYTES).contains(&text.len()) && !text.chars().any(char::is_control)
}

/// Check an OCFL logical path: 1 to 4096 bytes, no control character, `/`
/// separated segments that are neither empty nor `.` nor `..`.
pub(crate) fn check_path(text: &str) -> bool {
    (1..=MAX_PATH_BYTES).contains(&text.len())
        && !text.chars().any(char::is_control)
        && text
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
}
