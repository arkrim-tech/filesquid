//! Bounded directory walks for filesystem listings.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use super::super::error::StorageError;
use super::super::types::{MAX_WALK_DEPTH, MAX_WALK_ENTRIES};
use super::io_error;
use super::paths::RESERVED_PREFIX;

/// One directory still to visit during a listing walk.
struct WalkDir {
    dir: PathBuf,
    key_prefix: String,
    depth: usize,
}

/// Collect the keys of every file under `start` (whose key prefix is
/// `key_prefix`), skipping the backend's reserved internal files. A missing
/// start, or one that is a file, yields no keys. Bounded by
/// [`MAX_WALK_DEPTH`] and [`MAX_WALK_ENTRIES`]; links to directories are not
/// followed.
pub(super) fn walk_keys(start: PathBuf, key_prefix: String) -> Result<Vec<String>, StorageError> {
    let mut keys = Vec::new();
    let mut pending = vec![WalkDir {
        dir: start,
        key_prefix,
        depth: 0,
    }];
    let mut visited = 0usize;
    while let Some(next) = pending.pop() {
        let entries = match fs::read_dir(&next.dir) {
            Ok(entries) => entries,
            Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
                continue
            }
            Err(err) => return Err(io_error(&next.dir.display().to_string(), err)),
        };
        for entry in entries {
            visited += 1;
            check_walk_bounds(visited, next.depth)?;
            let entry = entry.map_err(|e| io_error(&next.dir.display().to_string(), e))?;
            visit_entry(&entry, &next, &mut keys, &mut pending)?;
        }
    }
    Ok(keys)
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

/// Record one directory entry: queue a directory, keep a file's key.
fn visit_entry(
    entry: &fs::DirEntry,
    parent: &WalkDir,
    keys: &mut Vec<String>,
    pending: &mut Vec<WalkDir>,
) -> Result<(), StorageError> {
    let name = entry
        .file_name()
        .into_string()
        .map_err(|raw| StorageError::InvalidKey {
            key: raw.to_string_lossy().into_owned(),
            reason: "a key must be valid UTF-8",
        })?;
    if name.starts_with(RESERVED_PREFIX) {
        return Ok(());
    }
    let key = format!("{}{name}", parent.key_prefix);
    let file_type = entry.file_type().map_err(|e| io_error(&key, e))?;
    if file_type.is_dir() {
        pending.push(WalkDir {
            dir: entry.path(),
            key_prefix: format!("{key}/"),
            depth: parent.depth + 1,
        });
    } else {
        keys.push(key);
    }
    Ok(())
}
