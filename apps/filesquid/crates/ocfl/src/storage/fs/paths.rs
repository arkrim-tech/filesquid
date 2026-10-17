//! Path confinement for the filesystem backend: resolve configured
//! directories, map keys to paths under the root, and refuse keys that name
//! internal files, break Windows naming rules, or pass through a symbolic
//! link below the root.

use std::ffi::OsString;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};

use super::super::error::StorageError;
use super::super::key::StorageKey;
use super::super::types::MAX_WALK_DEPTH;
use super::io_error;

/// Prefix of every name the backend creates for itself (temporary files).
/// Keys with a segment that starts with it are refused, so internal files
/// never collide with or hide stored objects.
pub(super) const RESERVED_PREFIX: &str = ".fsquid-";

/// Most path components [`resolve_path`] examines.
const MAX_RESOLVE_COMPONENTS: usize = 4 * MAX_WALK_DEPTH;

/// Windows device names that cannot be used as file names, with or without
/// an extension.
const WINDOWS_DEVICE_NAMES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

/// Return `path` as an absolute path with symbolic links and `.`/`..`
/// resolved, even when its last components do not exist yet: the deepest
/// existing ancestor is canonicalised and the rest is appended lexically.
///
/// # Errors
///
/// An I/O error other than a missing component, or a path deeper than
/// the component bound.
pub(super) fn resolve_path(path: &Path) -> io::Result<PathBuf> {
    let mut existing = std::path::absolute(path)?;
    let mut rest: Vec<OsString> = Vec::new();
    for _ in 0..MAX_RESOLVE_COMPONENTS {
        match existing.canonicalize() {
            Ok(base) => return Ok(append_lexically(base, &rest)),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        let last = existing
            .components()
            .next_back()
            .map(|c| c.as_os_str().to_owned());
        let Some(last) = last else {
            return Err(io::Error::new(ErrorKind::NotFound, "no existing ancestor"));
        };
        if !existing.pop() {
            return Err(io::Error::new(ErrorKind::NotFound, "no existing ancestor"));
        }
        rest.push(last);
    }
    Err(io::Error::other("path has too many components to resolve"))
}

/// Append components collected deepest-first to `base`, applying `.` and
/// `..` lexically (none of them exist on disk, so no link can intervene).
fn append_lexically(mut base: PathBuf, rest_deepest_first: &[OsString]) -> PathBuf {
    for part in rest_deepest_first.iter().rev() {
        match Path::new(part).components().next() {
            Some(Component::ParentDir) => {
                base.pop();
            }
            Some(Component::CurDir) | None => {}
            Some(_) => base.push(part),
        }
    }
    base
}

/// Return why `segment` cannot be a file name on Windows, or `None`.
///
/// Rejects drive and stream separators (`:`), the other reserved characters
/// and control characters, a trailing dot or space, and device names such as
/// `CON`, `NUL`, `COM1` or `lpt9.txt`. Applied by the filesystem backend on
/// Windows only: OCFL content paths may contain `:` and these names, and other
/// systems store them as ordinary names.
pub(super) fn windows_segment_violation(segment: &str) -> Option<&'static str> {
    let reserved_char = |c: char| matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*');
    if segment.chars().any(|c| reserved_char(c) || c.is_control()) {
        return Some("a key segment must not contain a character Windows reserves");
    }
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Some("a key segment must not end with a dot or space on Windows");
    }
    let stem = segment.split('.').next().unwrap_or(segment);
    let stem = stem.trim_end_matches(' ').to_ascii_uppercase();
    if WINDOWS_DEVICE_NAMES.contains(&stem.as_str()) || is_numbered_device(&stem) {
        return Some("a key segment must not be a Windows device name");
    }
    None
}

/// `COM1`-`COM9` and `LPT1`-`LPT9`, including the superscript digits Windows
/// also treats as device numbers.
fn is_numbered_device(stem: &str) -> bool {
    let Some(number) = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
    else {
        return false;
    };
    let mut chars = number.chars();
    let digit = chars.next();
    chars.next().is_none() && digit.is_some_and(|d| matches!(d, '1'..='9' | '¹' | '²' | '³'))
}

/// Return why `segment` is refused by the filesystem backend on this
/// platform, or `None`.
fn segment_violation(segment: &str, windows: bool) -> Option<&'static str> {
    if segment.starts_with(RESERVED_PREFIX) {
        return Some("names starting with .fsquid- are reserved for filesquid-ocfl");
    }
    if windows {
        return windows_segment_violation(segment);
    }
    None
}

/// Check every segment of `raw` (a key or a listing prefix without its
/// trailing `/`) against the platform naming rules.
pub(super) fn check_segments(raw: &str) -> Result<(), StorageError> {
    let violation = raw
        .split('/')
        .filter(|s| !s.is_empty())
        .find_map(|s| segment_violation(s, cfg!(windows)));
    match violation {
        Some(reason) => Err(StorageError::InvalidKey {
            key: raw.to_string(),
            reason,
        }),
        None => Ok(()),
    }
}

/// Join the segments of `raw` under `root`, refusing any existing component
/// that is a symbolic link. Components that do not exist yet are fine: they
/// are created as plain directories and files.
///
/// The check is made before the operation, so a link swapped in afterwards
/// by another process is not detected; the storage root MUST NOT be writable
/// by untrusted processes.
pub(super) fn confined_path(root: &Path, raw: &str) -> Result<PathBuf, StorageError> {
    check_segments(raw)?;
    let mut path = root.to_path_buf();
    let mut checking = true;
    for segment in raw.split('/').filter(|s| !s.is_empty()) {
        path.push(segment);
        if checking {
            checking = component_is_plain(&path, raw)?;
        }
    }
    Ok(path)
}

/// Return whether `path` exists (and so later components need checking),
/// failing when it is a symbolic link.
fn component_is_plain(path: &Path, raw: &str) -> Result<bool, StorageError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(StorageError::SymlinkForbidden {
            key: raw.to_string(),
        }),
        Ok(_) => Ok(true),
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            Ok(false)
        }
        Err(err) => Err(io_error(raw, err)),
    }
}

/// The confined path of `key`.
pub(super) fn key_path(root: &Path, key: &StorageKey) -> Result<PathBuf, StorageError> {
    confined_path(root, key.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_rejects_drive_prefixes_and_streams() {
        for segment in ["C:", "c:outside", "file:stream", "a?b", "a*b", "tab\there"] {
            assert!(windows_segment_violation(segment).is_some(), "{segment}");
        }
    }

    #[test]
    fn windows_rejects_device_names_and_trailing_dots() {
        for segment in [
            "CON",
            "con.txt",
            "Aux",
            "nul.tar.gz",
            "COM1",
            "lpt9.dat",
            "COM¹",
        ] {
            assert!(windows_segment_violation(segment).is_some(), "{segment}");
        }
        for segment in ["trailing.", "trailing "] {
            assert!(windows_segment_violation(segment).is_some(), "{segment}");
        }
    }

    #[test]
    fn windows_accepts_ordinary_names() {
        for segment in [
            "inventory.json",
            "v1",
            "COM10",
            "console.txt",
            "LPT",
            "0=ocfl_1.1",
        ] {
            assert_eq!(windows_segment_violation(segment), None, "{segment}");
        }
    }

    #[test]
    fn windows_rules_apply_only_on_windows() {
        assert!(segment_violation("C:", true).is_some());
        assert_eq!(segment_violation("C:", false), None);
        assert!(segment_violation(".fsquid-tmp-1", false).is_some());
        assert_eq!(segment_violation("x.fsquid-tmp-y", false), None);
    }

    #[test]
    fn a_drive_segment_never_replaces_the_root() {
        let root = Path::new("root");
        if cfg!(windows) {
            assert!(confined_path(root, "nested/C:/outside").is_err());
        } else {
            let path = confined_path(root, "nested/C:/outside").expect("plain name on unix");
            assert!(path.starts_with(root));
        }
    }

    #[test]
    fn missing_components_resolve_lexically() {
        let base = std::env::temp_dir().canonicalize().expect("temp dir");
        let path = base.join("fsquid-no-such-dir").join("..").join("leaf");
        assert_eq!(resolve_path(&path).expect("resolve"), base.join("leaf"));
    }
}
