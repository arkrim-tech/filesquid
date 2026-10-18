//! Version directory names (OCFL 1.1 section 3.3).
//!
//! A name is `v` followed by a positive base-ten integer: `v1`, `v2`, ... or,
//! zero-padded, `v001`, `v002`, ... The first version sets the convention
//! for the whole object; a padded convention has a fixed number of digits
//! and always a leading zero, so `v10000` is not allowed at five digits.
//!
//! OCFL sets no limit on the number of digits, so the number is kept as a
//! validated decimal string, not a machine integer: any valid name parses,
//! and [`VersionNum::next`] never produces a name [`VersionNum::parse`]
//! refuses. Names are bounded only by the input they come from.

use std::cmp::Ordering;
use std::fmt;

use super::error::FormatError;

/// A parsed version directory name.
///
/// Ordering is by version number; names that differ only in padding never
/// meet in one valid object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionNum {
    /// The number in decimal without leading zeros (never empty, never `0`).
    number: String,
    /// Digit count of a zero-padded name, or 0 for an unpadded one.
    width: usize,
}

impl VersionNum {
    /// The first version, unpadded: `v1`.
    pub fn first() -> Self {
        Self {
            number: "1".to_string(),
            width: 0,
        }
    }

    /// Parse a version directory name.
    ///
    /// # Errors
    ///
    /// [`FormatError::Invalid`] with E104 when the name does not start with
    /// `v`, or E105 when the rest is not a positive base-ten integer written
    /// in ASCII digits.
    pub fn parse(name: &str) -> Result<Self, FormatError> {
        let Some(digits) = name.strip_prefix('v') else {
            return Err(FormatError::invalid(
                "E104",
                name,
                "a version name starts with v",
            ));
        };
        let number = digits.trim_start_matches('0');
        let all_digits = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
        if !all_digits || number.is_empty() {
            return Err(FormatError::invalid(
                "E105",
                name,
                "a version number is a positive base-ten integer",
            ));
        }
        let width = if digits.starts_with('0') {
            digits.len()
        } else {
            0
        };
        Ok(Self {
            number: number.to_string(),
            width,
        })
    }

    /// The version number in decimal, without padding or leading zeros.
    pub fn number(&self) -> &str {
        &self.number
    }

    /// The version number as a `u64`, or `None` when it is larger.
    pub fn as_u64(&self) -> Option<u64> {
        self.number.parse().ok()
    }

    /// Whether the name is zero-padded, such as `v001`.
    pub fn is_zero_padded(&self) -> bool {
        self.width > 0
    }

    /// The number of digits in the name.
    pub fn digits(&self) -> usize {
        if self.width > 0 {
            return self.width;
        }
        self.number.len()
    }

    /// The next version under the same naming convention, or `None` when a
    /// padded name would lose its leading zero (`v0999` is the last at four
    /// digits). Unpadded names always have a next one.
    pub fn next(&self) -> Option<Self> {
        let number = increment(&self.number);
        if self.width > 0 && number.len() >= self.width {
            return None;
        }
        Some(Self {
            number,
            width: self.width,
        })
    }

    /// Whether this version's number is `n`.
    pub(crate) fn is_number(&self, n: u64) -> bool {
        self.as_u64() == Some(n)
    }

    /// Whether `other` follows the naming convention this version sets, as
    /// the first version does for its object.
    ///
    /// # Errors
    ///
    /// [`FormatError::Invalid`] with E011 when `other` has the padded width
    /// but no leading zero (`v10` after `v01`), or E012 when it uses another
    /// convention.
    pub fn check_convention(&self, other: &Self) -> Result<(), FormatError> {
        if self.width == other.width {
            return Ok(());
        }
        let at = other.to_string();
        if self.width > 0 && other.width == 0 && other.digits() == self.width {
            return Err(FormatError::invalid(
                "E011",
                at,
                "zero-padded version names keep a leading zero",
            ));
        }
        Err(FormatError::invalid(
            "E012",
            at,
            "all version names use the convention of the first version",
        ))
    }
}

/// `number` (ASCII decimal digits) plus one. The loop visits each digit at
/// most once.
fn increment(number: &str) -> String {
    let mut digits = number.as_bytes().to_vec();
    let mut carry = true;
    for digit in digits.iter_mut().rev() {
        if *digit < b'9' {
            *digit += 1;
            carry = false;
            break;
        }
        *digit = b'0';
    }
    if carry {
        digits.insert(0, b'1');
    }
    digits.into_iter().map(char::from).collect()
}

impl Ord for VersionNum {
    /// Numeric order: a shorter number (no leading zeros) is smaller; equal
    /// lengths compare digit by digit; padding breaks a tie.
    fn cmp(&self, other: &Self) -> Ordering {
        self.number
            .len()
            .cmp(&other.number.len())
            .then_with(|| self.number.cmp(&other.number))
            .then_with(|| self.width.cmp(&other.width))
    }
}

impl PartialOrd for VersionNum {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for VersionNum {
    /// Write the directory name, padded to the convention's width.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{:0>width$}", self.number, width = self.width)
    }
}
