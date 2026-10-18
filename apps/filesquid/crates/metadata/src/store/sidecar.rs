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

    /// Refuse a root that is a Git working tree.
    fn reject_git(&self) -> Result<(), StoreError> {
        if self.root.join(".git").exists() {
            return Err(StoreError::Git);
        }
        Ok(())
    }
}

/// Wrap an I/O failure as a backend error.
fn backend(err: std::io::Error) -> StoreError {
    StoreError::Backend(Box::new(err))
}

/// Write `text` to `path` through a sibling temporary file, then rename it
/// into place so a reader never sees a partial record.
fn write_replacing(path: &Path, text: &str) -> Result<(), std::io::Error> {
    let mut staging = path.as_os_str().to_owned();
    staging.push(".tmp");
    let staging = PathBuf::from(staging);
    std::fs::write(&staging, text)?;
    if let Err(err) = std::fs::rename(&staging, path) {
        // Best effort: the rename error is the one the caller needs.
        let _ = std::fs::remove_file(&staging);
        return Err(err);
    }
    Ok(())
}

impl MetadataStore for SidecarStore {
    async fn load(&self, file: &FileRef) -> Result<Record, StoreError> {
        self.reject_git()?;
        match std::fs::read(self.sidecar_path(file)) {
            Ok(bytes) => Ok(from_markdown(&String::from_utf8_lossy(&bytes))),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Record::default()),
            Err(err) => Err(backend(err)),
        }
    }

    async fn save(&self, file: &FileRef, record: &Record) -> Result<(), StoreError> {
        self.reject_git()?;
        write_replacing(&self.sidecar_path(file), &to_markdown(record)).map_err(backend)
    }
}
