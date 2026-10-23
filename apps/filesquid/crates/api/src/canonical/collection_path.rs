//! Collection logical paths (API-S-026) and the numbering of a path that is already taken
//! (API-S-011 **Same logical path**; [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)
//! Decision 12, Task DE-1.7).
//!
//! A file's **collection logical path** places it inside its home collection: `/`-separated
//! segments whose last one is the file's name. It is a JSON value of the file's snapshot
//! entity, not an OCFL logical path, so it may hold any character but `/`, `\`, NUL and
//! control characters: 1 to [`MAX_PATH_BYTES`] UTF-8 bytes in NFC, each segment 1 to
//! [`MAX_SEGMENT_BYTES`] bytes and never `.` or `..`.
//!
//! When an upload, import, copy, move, rename or folder creation would give a new item a path
//! that already exists in its collection, the new item gets a hyphen and the first free number
//! from 1 beside the existing one, never a refusal or an overwrite (owner decision
//! 2026-10-22). A folder takes the number at its end (`directoryY-1`); a file takes it after
//! the first part of its name, before the first dot (`photo-1.jpg`, `archive-1.tar.gz`), a
//! name without a dot at its end, and a name that starts with a dot keeps that dot in its
//! first part, so a numbered copy stays a dotfile (`.env-1`, `.config-1.json`). At most
//! [`MAX_SUFFIX`] numbers are tried ("Caps: 9999 for both").
//!
//! Every function here is pure, except that [`free_path`] calls the lookup it is given.

use crate::files::names::safe_name;
use icu_normalizer::ComposingNormalizerBorrowed;

/// Longest collection logical path, in UTF-8 bytes.
pub const MAX_PATH_BYTES: usize = 1_024;

/// Longest segment of a collection logical path, in UTF-8 bytes (a file or folder name).
pub const MAX_SEGMENT_BYTES: usize = 255;

/// Most numbers tried for a taken path: `-1` to `-9999` (owner decision 2026-10-22).
pub const MAX_SUFFIX: u32 = 9_999;

/// Folders no collection logical path may start with: the reserved folders of the file
/// routes (API-S-011).
pub const RESERVED_FOLDERS: [&str; 2] = ["provenance", ".filesquid"];

/// Why a path or name cannot be used. Each carries a fixed error code and never the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    /// Not a collection logical path (empty or `.`/`..` segment, a forbidden character, not
    /// NFC, a reserved folder or longer than [`MAX_PATH_BYTES`]).
    Invalid,
    /// A segment is longer than [`MAX_SEGMENT_BYTES`] bytes.
    NameTooLong,
    /// Every number from 1 to [`MAX_SUFFIX`] is taken.
    SuffixExhausted,
}

impl PathError {
    /// The error code a refusal or a quarantine carries.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Invalid => "invalid_path",
            Self::NameTooLong => "name_too_long",
            Self::SuffixExhausted => "path_suffix_exhausted",
        }
    }
}

/// Whether a numbered item is a file or a folder: they take the number in different places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// A file: the number goes before the first dot of its name.
    File,
    /// A folder: the number goes at the end of its name.
    Folder,
}

/// Check that `path` is a collection logical path.
///
/// # Errors
/// [`PathError::NameTooLong`] for a segment over [`MAX_SEGMENT_BYTES`] bytes;
/// [`PathError::Invalid`] for anything else outside the rule.
pub fn check_collection_path(path: &str) -> Result<(), PathError> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || !is_nfc(path) {
        return Err(PathError::Invalid);
    }
    path.split('/').try_for_each(check_segment)
}

/// Check that `folder` can be the destination folder of a write: a collection logical path
/// outside the [`RESERVED_FOLDERS`].
///
/// # Errors
/// As [`check_collection_path`]; [`PathError::Invalid`] for a reserved folder.
pub fn check_folder(folder: &str) -> Result<(), PathError> {
    check_collection_path(folder)?;
    let first = folder.split('/').next().unwrap_or_default();
    match RESERVED_FOLDERS.contains(&first) {
        true => Err(PathError::Invalid),
        false => Ok(()),
    }
}

/// The segment a file or folder named `name` gets: its name after the API-S-005 safe
/// transformation ([`safe_name`]), in NFC.
///
/// # Errors
/// [`PathError::Invalid`] when nothing is left or the name is `.` or `..`;
/// [`PathError::NameTooLong`] when it is still longer than [`MAX_SEGMENT_BYTES`] bytes.
pub fn safe_segment(name: &str) -> Result<String, PathError> {
    let safe = safe_name(name).ok_or(PathError::Invalid)?;
    let normalized = ComposingNormalizerBorrowed::new_nfc()
        .normalize(&safe)
        .into_owned();
    check_segment(&normalized)?;
    Ok(normalized)
}

/// `rest` beneath the destination `folder` (the collection's root when `None`).
pub fn join(folder: Option<&str>, rest: &str) -> String {
    match folder {
        Some(folder) => format!("{folder}/{rest}"),
        None => rest.to_string(),
    }
}

/// `segment` with the number `n` placed as [`ItemKind`] says.
pub fn numbered(segment: &str, n: u32, kind: ItemKind) -> String {
    let split = match kind {
        ItemKind::Folder => None,
        ItemKind::File => first_dot(segment),
    };
    match split {
        Some(at) => format!("{}-{n}{}", &segment[..at], &segment[at..]),
        None => format!("{segment}-{n}"),
    }
}

/// The path a new item at `path` gets: `path` itself when `taken` says it is free, else the
/// first free numbered path beside it, trying at most [`MAX_SUFFIX`] numbers. `taken` answers
/// whether a path is already used in the collection (a deleted file's path included). An item
/// named like a reserved folder at the collection's root counts as taken, so it is numbered
/// beside it (`provenance-1`) rather than placed inside the reserved folder.
///
/// # Errors
/// The outer error is `taken`'s own failure, which stops the search. The inner one is
/// [`PathError::SuffixExhausted`] when every number is taken, [`PathError::NameTooLong`] when a
/// numbered segment would exceed [`MAX_SEGMENT_BYTES`] bytes, or [`PathError::Invalid`] for a
/// `path` outside the rule, a `path` inside a reserved folder or a numbered path over
/// [`MAX_PATH_BYTES`] bytes.
pub fn free_path<E>(
    path: &str,
    kind: ItemKind,
    used: &mut dyn FnMut(&str) -> Result<bool, E>,
) -> Result<Result<String, PathError>, E> {
    if let Err(err) = check_collection_path(path) {
        return Ok(Err(err));
    }
    if path.contains('/') && check_folder(path).is_err() {
        return Ok(Err(PathError::Invalid));
    }
    let mut taken = |candidate: &str| match RESERVED_FOLDERS.contains(&candidate) {
        true => Ok(true),
        false => used(candidate),
    };
    if !taken(path)? {
        return Ok(Ok(path.to_string()));
    }
    let (parent, last) = match path.rsplit_once('/') {
        Some((parent, last)) => (Some(parent), last),
        None => (None, path),
    };
    for n in 1..=MAX_SUFFIX {
        let candidate = join(parent, &numbered(last, n, kind));
        if let Err(err) = check_collection_path(&candidate) {
            return Ok(Err(err));
        }
        if !taken(&candidate)? {
            return Ok(Ok(candidate));
        }
    }
    Ok(Err(PathError::SuffixExhausted))
}

/// Check one segment: 1 to [`MAX_SEGMENT_BYTES`] bytes, not `.` or `..`, without `\`, NUL or
/// control characters (`/` cannot occur in a split segment).
fn check_segment(segment: &str) -> Result<(), PathError> {
    let forbidden = |c: char| c == '\\' || c == '/' || c.is_control();
    if segment.is_empty() || segment == "." || segment == ".." || segment.contains(forbidden) {
        return Err(PathError::Invalid);
    }
    if segment.len() > MAX_SEGMENT_BYTES {
        return Err(PathError::NameTooLong);
    }
    Ok(())
}

/// The byte index of the first dot of a file name that ends its first part, skipping the
/// leading dots of a dotfile, so `.config.json` splits before `.json` and `.env` not at all.
fn first_dot(name: &str) -> Option<usize> {
    let lead = name.len() - name.trim_start_matches('.').len();
    name[lead..].find('.').map(|at| lead + at)
}

/// Whether `text` is in NFC.
fn is_nfc(text: &str) -> bool {
    ComposingNormalizerBorrowed::new_nfc().is_normalized(text)
}
