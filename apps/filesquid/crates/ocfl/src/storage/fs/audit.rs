//! Storage-hierarchy audit for the filesystem backend (OCFL-S-001, ADR 016
//! layout constraints): hard links, soft links and empty directories MUST
//! NOT be used under the storage hierarchy.
//!
//! The audit reuses the listing's bounded walker and the backend's
//! no-follow, regular-file-only rules from `internal.rs`; it adds only what a
//! listing does not refuse: links, extra hard links, empty directories and
//! other non-regular entries are errors instead of being skipped.

use std::fs::{self, DirEntry, FileType};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::super::error::StorageError;
use super::internal::{link_count, not_regular_entry};
use super::io_error;
use super::walk::{walk, Visitor};

/// The audit's visitor: every entry MUST be a regular file with one link,
/// and every directory below the start MUST hold something.
struct Audit;

impl Visitor for Audit {
    /// Refuse a soft link, a non-regular entry or a hard-linked file.
    fn entry(
        &mut self,
        entry: &DirEntry,
        key: String,
        file_type: FileType,
    ) -> Result<(), StorageError> {
        if file_type.is_symlink() {
            return Err(StorageError::SymlinkForbidden { key });
        }
        if !file_type.is_file() {
            return Err(not_regular_entry(&key));
        }
        let meta = entry.metadata().map_err(|e| io_error(&key, e))?;
        match link_count(&meta) {
            Some(links) if links > 1 => Err(StorageError::HardLinkForbidden { key }),
            _ => Ok(()),
        }
    }

    /// Refuse an empty directory below the storage root. The root itself may
    /// be empty: a storage root that holds nothing yet is valid.
    fn empty_dir(&mut self, key: &str) -> Result<(), StorageError> {
        if key.is_empty() {
            return Ok(());
        }
        Err(StorageError::EmptyDirectoryForbidden {
            key: key.to_string(),
        })
    }
}

/// Audit the hierarchy under `start`, whose key prefix is `key_prefix` (empty
/// for the root, otherwise ending in `/`). A missing start passes; an
/// existing start that is not a directory (a regular file, a FIFO) is
/// refused, because it cannot hold a storage hierarchy. Stops at the first
/// violation.
///
/// Hard links are detected where the standard library reports a link count
/// (Unix, including macOS); elsewhere they cannot be seen and pass.
///
/// # Errors
///
/// [`StorageError::SymlinkForbidden`], [`StorageError::HardLinkForbidden`],
/// [`StorageError::EmptyDirectoryForbidden`], [`StorageError::Io`] for
/// another non-regular entry or a start that is not a directory, or a walk
/// bound or I/O error.
pub(super) fn audit_tree(start: PathBuf, key_prefix: String) -> Result<(), StorageError> {
    if !start_is_directory(&start)? {
        return Ok(());
    }
    walk(start, key_prefix, &mut Audit)
}

/// Whether the audit's start exists as a directory, read without following
/// links. `Ok(false)` when it is missing.
///
/// # Errors
///
/// [`StorageError::Io`] with [`ErrorKind::NotADirectory`] when something
/// other than a directory is there, or the inspection error.
fn start_is_directory(start: &Path) -> Result<bool, StorageError> {
    let what = start.display().to_string();
    match fs::symlink_metadata(start) {
        Ok(meta) if meta.is_dir() => Ok(true),
        Ok(_) => Err(StorageError::Io {
            key: what,
            kind: ErrorKind::NotADirectory,
            message: "the storage hierarchy must be a directory".to_string(),
        }),
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            Ok(false)
        }
        Err(err) => Err(io_error(&what, err)),
    }
}
