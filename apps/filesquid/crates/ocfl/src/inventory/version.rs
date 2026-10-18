//! Version directory names (OCFL 1.1 section 3.3).
//!
//! A name is `v` followed by a positive base-ten integer: `v1`, `v2`, ... or,
//! zero-padded, `v001`, `v002`, ... The first version sets the convention
//! for the whole object; a padded convention has a fixed number of digits
//! and always a leading zero, so `v10000` is not allowed at five digits.

use std::fmt;

use super::error::FormatError;

/// Most digits a version number may have, so it fits a `u64`. No real object
/// comes near it.
const MAX_DIGITS: usize = 18;

/// A parsed version directory name.
///
/// Ordering is by version number; names that differ only in padding never
/// meet in one valid object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VersionNum {
    number: u64,
    /// Digit count of a zero-padded name, or 0 for an unpadded one.
    width: usize,
}

impl VersionNum {
    /// The first version, unpadded: `v1`.
    pub const FIRST: Self = Self {
        number: 1,
        width: 0,
    };

    /// Parse a version directory name.
    ///
    /// # Errors
    ///
    /// [`FormatError::Invalid`] with E104 when the name does not start with
    /// `v`, or E105 when the rest is not a positive base-ten integer of at
    /// most 18 ASCII digits.
    pub fn parse(name: &str) -> Result<Self, FormatError> {
        let Some(digits) = name.strip_prefix('v') else {
            return Err(FormatError::invalid(
                "E104",
                name,
                "a version name starts with v",
            ));
        };
        let well_formed = !digits.is_empty()
            && digits.len() <= MAX_DIGITS
            && digits.bytes().all(|b| b.is_ascii_digit());
        let number = digits.parse::<u64>().ok().filter(|n| well_formed && *n > 0);
        let Some(number) = number else {
            return Err(FormatError::invalid(
                "E105",
                name,
                "a version number is a positive base-ten integer",
            ));
        };
        let width = if digits.starts_with('0') {
            digits.len()
        } else {
            0
        };
        Ok(Self { number, width })
    }

    /// The version number.
    pub fn number(&self) -> u64 {
        self.number
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
        self.number.to_string().len()
    }

    /// The next version under the same naming convention, or `None` when it
    /// would not fit (a padded name always keeps a leading zero, so `v0999`
    /// is the last at four digits).
    pub fn next(&self) -> Option<Self> {
        let number = self.number.checked_add(1)?;
        let candidate = Self {
            number,
            width: self.width,
        };
        let fits = self.width == 0 || number.to_string().len() < self.width;
        fits.then_some(candidate)
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

impl fmt::Display for VersionNum {
    /// Write the directory name, padded to the convention's width.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{:0width$}", self.number, width = self.width)
    }
}
