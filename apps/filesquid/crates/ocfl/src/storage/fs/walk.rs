//! Bounded directory walks for filesystem listings and hierarchy audits.
//!
//! One walker visits the entries below a start directory without following
//! links, within [`MAX_WALK_DEPTH`] and [`MAX_WALK_ENTRIES`]. A [`Visitor`]
//! decides what each entry means, which names to skip and which entries to
//! prune ([`Visitor::prunes`]; a pruned directory's subtree is never read):
//! the listing skips the backend's reserved names, prunes every entry and
//! subtree its prefix cannot match and keeps regular files; the audit (see
//! `audit.rs`) sees every entry and refuses anything that is not a plain file.

use std::fs::{self, DirEntry, FileType};
use std::io::ErrorKind;
use std::path::PathBuf;

use super::super::error::StorageError;
use super::super::types::{MAX_WALK_DEPTH, MAX_WALK_ENTRIES};
use super::io_error;
use super::paths::RESERVED_PREFIX;

/// What a walk does with the entries it meets. Directories are queued by the
/// walker itself; every other entry, including links to directories, goes to
/// [`Visitor::entry`].
pub(super) trait Visitor {
    /// Whether to skip the entry called `name`. A skipped directory is not
    /// entered. Skipped entries still count toward the entry bound and make
    /// their parent non-empty. Skips nothing by default.
    fn skips(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    /// Whether to leave out the entry whose storage key is `key` (a
    /// directory when `is_dir`), after [`Visitor::skips`] kept its name. A
    /// pruned directory is not entered, so nothing in its subtree is read.
    /// Pruned entries still count toward the entry bound. Prunes nothing by
    /// default.
    fn prunes(&self, key: &str, is_dir: bool) -> bool {
        let _ = (key, is_dir);
        false
    }

    /// Handle one entry that is not a directory. `key` is its storage key and
    /// `file_type` is read without following links.
    fn entry(
        &mut self,
        entry: &DirEntry,
        key: String,
        file_type: FileType,
    ) -> Result<(), StorageError>;

    /// Handle a directory that holds no entries at all. `key` is its key
    /// without a trailing `/`, empty for the storage root. Ignored by default.
    fn empty_dir(&mut self, key: &str) -> Result<(), StorageError> {
        let _ = key;
        Ok(())
    }
}

/// One directory still to visit during a walk.
struct WalkDir {
    dir: PathBuf,
    key_prefix: String,
    depth: usize,
}

/// Collects the keys of regular files that can start with a listing
/// prefix: the listing's visitor.
struct KeyCollector {
    /// The keys found so far.
    keys: Vec<String>,
    /// The listing prefix; entries whose keys cannot start with it are
    /// pruned, so a listing never walks an unrelated subtree.
    prefix: String,
}

impl Visitor for KeyCollector {
    /// Skip the backend's reserved internal names (temporary files), which
    /// are never keys.
    fn skips(&self, name: &str) -> bool {
        name.starts_with(RESERVED_PREFIX)
    }

    /// Prune an entry when no key at or below it can start with the
    /// prefix. A file is kept only when its key starts with the prefix. A
    /// directory is entered only when the keys inside it (`<key>/...`) can:
    /// `<key>/` starts with the prefix, or the prefix continues below it
    /// (the prefix starts with `<key>/`). A directory whose name is only a
    /// leading part of a prefix segment (`0=oc` for `0=ocfl_`) is pruned.
    fn prunes(&self, key: &str, is_dir: bool) -> bool {
        if !is_dir {
            return !key.starts_with(&self.prefix);
        }
        let inside = format!("{key}/");
        !(inside.starts_with(&self.prefix) || self.prefix.starts_with(&inside))
    }

    /// Keep a regular file's key. Symbolic links, FIFOs, sockets and devices
    /// are not objects and are not listed.
    fn entry(
        &mut self,
        _: &DirEntry,
        key: String,
        file_type: FileType,
    ) -> Result<(), StorageError> {
        if file_type.is_file() {
            self.keys.push(key);
        }
        Ok(())
    }
}

/// Collect the keys of the files under `start` (whose key prefix is
/// `key_prefix`) that can start with `prefix`, skipping the backend's
/// reserved internal files, anything that is not a regular file and every
/// directory whose keys cannot start with `prefix`. The caller still
/// filters the keys by `prefix`. A missing start, or one that is a file,
/// yields no keys. Bounded as [`walk`].
pub(super) fn walk_keys(
    start: PathBuf,
    key_prefix: String,
    prefix: &str,
) -> Result<Vec<String>, StorageError> {
    let mut collector = KeyCollector {
        keys: Vec::new(),
        prefix: prefix.to_string(),
    };
    walk(start, key_prefix, &mut collector)?;
    Ok(collector.keys)
}

/// Visit the entries under `start` (whose key prefix is `key_prefix`) with
/// `visitor`, skipping the names [`Visitor::skips`] names and the entries,
/// with their whole subtrees, that [`Visitor::prunes`] prunes. A missing
/// start, or one that is a file, visits nothing. Bounded by
/// [`MAX_WALK_DEPTH`] and [`MAX_WALK_ENTRIES`]; links to directories are not
/// followed.
pub(super) fn walk(
    start: PathBuf,
    key_prefix: String,
    visitor: &mut impl Visitor,
) -> Result<(), StorageError> {
    let mut pending = vec![WalkDir {
        dir: start,
        key_prefix,
        depth: 0,
    }];
    let mut visited = 0usize;
    while let Some(next) = pending.pop() {
        visit_dir(&next, visitor, &mut pending, &mut visited)?;
    }
    Ok(())
}

/// Visit the entries of one directory, queueing subdirectories on `pending`
/// and counting every entry in `visited`. A directory with no entries goes to
/// [`Visitor::empty_dir`]; a directory that has vanished is skipped.
fn visit_dir(
    current: &WalkDir,
    visitor: &mut impl Visitor,
    pending: &mut Vec<WalkDir>,
    visited: &mut usize,
) -> Result<(), StorageError> {
    let what = current.dir.display().to_string();
    let entries = match fs::read_dir(&current.dir) {
        Ok(entries) => entries,
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            return Ok(())
        }
        Err(err) => return Err(io_error(&what, err)),
    };
    let mut seen = 0usize;
    for entry in entries {
        *visited += 1;
        seen += 1;
        check_walk_bounds(*visited, current.depth)?;
        let entry = entry.map_err(|e| io_error(&what, e))?;
        visit_entry(&entry, current, visitor, pending)?;
    }
    if seen == 0 {
        visitor.empty_dir(current.key_prefix.trim_end_matches('/'))?;
    }
    Ok(())
}

/// Fail once a walk passes its entry or depth bound.
fn check_walk_bounds(visited: usize, depth: usize) -> Result<(), StorageError> {
    if visited > MAX_WALK_ENTRIES {
        return Err(StorageError::LimitExceeded {
            what: "listing directory entries",
            limit: MAX_WALK_ENTRIES as u64,
        });
    }
    if depth > MAX_WALK_DEPTH {
        return Err(StorageError::LimitExceeded {
            what: "listing directory depth",
            limit: MAX_WALK_DEPTH as u64,
        });
    }
    Ok(())
}

/// Route one directory entry: skip a name the visitor skips or a key it
/// prunes, queue a directory
/// (failing when it would be deeper than [`MAX_WALK_DEPTH`]) and hand
/// anything else to the visitor.
fn visit_entry(
    entry: &DirEntry,
    parent: &WalkDir,
    visitor: &mut impl Visitor,
    pending: &mut Vec<WalkDir>,
) -> Result<(), StorageError> {
    let name = entry
        .file_name()
        .into_string()
        .map_err(|raw| StorageError::InvalidKey {
            key: raw.to_string_lossy().into_owned(),
            reason: "a key must be valid UTF-8",
        })?;
    if visitor.skips(&name) {
        return Ok(());
    }
    let key = format!("{}{name}", parent.key_prefix);
    let file_type = entry.file_type().map_err(|e| io_error(&key, e))?;
    if visitor.prunes(&key, file_type.is_dir()) {
        return Ok(());
    }
    if !file_type.is_dir() {
        return visitor.entry(entry, key, file_type);
    }
    let depth = parent.depth + 1;
    check_walk_bounds(0, depth)?;
    pending.push(WalkDir {
        dir: entry.path(),
        key_prefix: format!("{key}/"),
        depth,
    });
    Ok(())
}
