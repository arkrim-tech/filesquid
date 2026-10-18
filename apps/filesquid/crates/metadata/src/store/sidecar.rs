//! `sidecar` store: `<file>.metadata.md` beside a plain file (native only).
//!
//! Browsers have no ordinary filesystem, so this store is compiled out for
//! `wasm32`; there the caller selects the `ocfl` store or supplies its own
//! [`MetadataStore`]. The file I/O is named at the call sites below
//! ([`std::fs::read`], [`std::fs::write`], [`std::fs::rename`]).

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::file_ref::FileRef;
use crate::markdown::{from_markdown, to_markdown};
use crate::record::Record;
use crate::store::{MetadataStore, StoreError};

/// Suffix appended to the described file's name to name its sidecar.
pub const SIDECAR_SUFFIX: &str = ".metadata.md";

/// Stores records as sidecar files under a root directory.
#[derive(Clone, Debug)]
pub struct SidecarStore {
    /// Directory that logical paths resolve under.
    root: PathBuf,
}

impl SidecarStore {
    /// Create a store whose logical paths resolve under `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Path of the sidecar of `file`: its logical path plus [`SIDECAR_SUFFIX`].
    pub fn sidecar_path(&self, file: &FileRef) -> PathBuf {
        let mut name = file.logical_path().to_string();
        name.push_str(SIDECAR_SUFFIX);
        self.root.join(name)
    }

    /// Refuse a root that is a Git working tree, including a dangling `.git`
    /// symlink; only a missing `.git` passes (file I/O).
    fn reject_git(&self) -> Result<(), StoreError> {
        match std::fs::symlink_metadata(self.root.join(".git")) {
            Ok(_) => Err(StoreError::Git),
            Err(err) if is_absent(&err) => Ok(()),
            Err(err) => Err(backend(err)),
        }
    }

    /// Refuse a sidecar path that reaches outside the root through a symlink:
    /// every component under the root, and the sidecar itself, must not be one
    /// (file I/O). Components that do not exist yet are fine.
    fn reject_symlinks(&self, file: &FileRef) -> Result<(), StoreError> {
        let mut current = self.root.clone();
        let sidecar = self.sidecar_path(file);
        let relative = sidecar
            .strip_prefix(&self.root)
            .map_err(|_| StoreError::InvalidPath)?;
        for part in relative.components() {
            current.push(part);
            match std::fs::symlink_metadata(&current) {
                Ok(meta) if meta.file_type().is_symlink() => return Err(StoreError::InvalidPath),
                Ok(_) => {}
                Err(err) if is_absent(&err) => return Ok(()),
                Err(err) => return Err(backend(err)),
            }
        }
        Ok(())
    }

    /// Run the Git and symlink guards for `file`.
    fn guard(&self, file: &FileRef) -> Result<(), StoreError> {
        self.reject_git()?;
        self.reject_symlinks(file)
    }
}

/// Return whether `err` only means the path (or a parent) does not exist.
fn is_absent(err: &std::io::Error) -> bool {
    matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory)
}

/// Wrap an I/O failure as a backend error.
fn backend(err: std::io::Error) -> StoreError {
    StoreError::Backend(Box::new(err))
}

/// Parse sidecar bytes; invalid UTF-8 is an error so a later save never
/// overwrites the owner's file with a lossy replacement.
fn parse_record(bytes: Vec<u8>) -> Result<Record, StoreError> {
    let text = String::from_utf8(bytes)
        .map_err(|e| backend(std::io::Error::new(ErrorKind::InvalidData, e)))?;
    Ok(from_markdown(&text))
}

/// Write `text` to `path` through a sibling temporary file, then rename it
/// into place so a reader never sees a partial record.
fn write_replacing(path: &Path, text: &str) -> Result<(), std::io::Error> {
    let mut staging = path.as_os_str().to_owned();
    staging.push(".tmp");
    let staging = PathBuf::from(staging);
    if let Err(err) = std::fs::write(&staging, text) {
        // Best effort: the write error is the one the caller needs.
        let _ = std::fs::remove_file(&staging);
        return Err(err);
    }
    if let Err(err) = std::fs::rename(&staging, path) {
        // Best effort: the rename error is the one the caller needs.
        let _ = std::fs::remove_file(&staging);
        return Err(err);
    }
    Ok(())
}

impl MetadataStore for SidecarStore {
    async fn load(&self, file: &FileRef) -> Result<Record, StoreError> {
        self.guard(file)?;
        match std::fs::read(self.sidecar_path(file)) {
            Ok(bytes) => parse_record(bytes),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Record::default()),
            Err(err) => Err(backend(err)),
        }
    }

    async fn save(&self, file: &FileRef, record: &Record) -> Result<(), StoreError> {
        self.guard(file)?;
        write_replacing(&self.sidecar_path(file), &to_markdown(record)).map_err(backend)
    }
}
