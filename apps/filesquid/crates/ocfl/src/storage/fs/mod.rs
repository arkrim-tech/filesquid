//! Local filesystem storage backend (native only, behind the default-on `fs`
//! feature). Keys map to plain files under the storage root, so a library
//! stays readable and copyable with ordinary file tools.
//!
//! The operations block the polling thread on `std::fs` calls and complete on
//! their first poll; the backend needs no executor (see
//! [`complete_immediately`](super::complete_immediately)). A host that must
//! not block its reactor drives these futures on a blocking pool.
//!
//! Guarantees:
//!
//! - **Atomic objects.** Writes go to a flushed temporary file beside the
//!   target, then are renamed into place, so readers never see a partial
//!   object. Temporary files use the reserved `.fsquid-` name prefix, which
//!   keys may not use and listings skip.
//! - **Serialised writes.** Every mutation of published objects (`put`,
//!   `put_if`, `delete`, `complete_multipart`) holds an operating-system lock
//!   on a file in the staging directory, named after the resolved root. All
//!   writers to one root, in any process, MUST use the same staging directory
//!   to share it. Multipart staging uses per-root, per-upload directories
//!   instead (see `uploads.rs`).
//! - **Create-only without a shared lock.** `Precondition::IfAbsent`
//!   publishes with a hard link, which the operating system refuses when the
//!   target exists, so two creators can never both win.
//! - **Confinement.** A key is refused when an existing component of its path
//!   below the root is a symbolic link, when a segment starts with `.fsquid-`,
//!   and on Windows when a segment is a drive or stream name, a device name or
//!   ends with a dot or space. Object files and the backend's own files are
//!   opened as regular files only, and the open handle is checked against the
//!   path, so a link swapped in after the check is refused (see
//!   `internal.rs`). Directory checks still run before each operation, so the
//!   root and staging MUST NOT be writable by untrusted processes.
//! - **Staging outside the root.** Root and staging are resolved (links,
//!   `.`, `..`) before the containment check.
//!
//! Exclusive writers across hosts come from the per-object OCFL lease (ADR
//! 016, ADR 007), not from this backend.

mod files;
mod internal;
mod lock;
mod paths;
mod uploads;
mod walk;

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::backend::StorageBackend;
use super::error::StorageError;
use super::key::StorageKey;
use super::types::{
    check_list_request, check_precondition, check_put_size, paginate, ByteRange, CompletedPart,
    ETag, ListPage, ObjectMeta, Precondition, UploadId,
};
use files::{hash_file, read_range, write_atomic, Parent, Publish};
use lock::{lock_path, RootLock};
use paths::{confined_path, key_path, resolve_path};
use uploads::uploads_path;
use walk::walk_keys;

/// A [`StorageBackend`] over a directory on the local filesystem.
#[derive(Debug)]
pub struct FsBackend {
    root: PathBuf,
    staging: PathBuf,
    uploads: PathBuf,
    lock_file: PathBuf,
    next_id: AtomicU64,
}

/// Map an I/O error on `what` to a storage error; `NotFound` stays not found.
fn io_error(what: &str, err: std::io::Error) -> StorageError {
    if err.kind() == ErrorKind::NotFound {
        return StorageError::NotFound {
            key: what.to_string(),
        };
    }
    StorageError::Io {
        key: what.to_string(),
        kind: err.kind(),
        message: err.to_string(),
    }
}

/// The staging directory [`FsBackend::with_default_staging`] uses for `root`:
/// `filesquid-ocfl-staging` in the system temporary directory, or, when that
/// lies inside `root` (for example a root of `/tmp`), the sibling
/// `<root>.fsquid-staging`. The choice depends only on `root` and the
/// temporary directory, so writers to one root agree on it and share its
/// write lock. Nothing is created.
///
/// # Errors
///
/// [`StorageError::InvalidConfig`] when no candidate is outside the root.
pub fn default_staging_dir(root: &Path) -> Result<PathBuf, StorageError> {
    let resolved = resolve_path(root).map_err(|e| config_error(root, e))?;
    let mut candidates = vec![std::env::temp_dir().join("filesquid-ocfl-staging")];
    if let (Some(parent), Some(name)) = (resolved.parent(), resolved.file_name()) {
        let mut sibling = name.to_os_string();
        sibling.push(".fsquid-staging");
        candidates.push(parent.join(sibling));
    }
    for candidate in candidates {
        let candidate = resolve_path(&candidate).map_err(|e| config_error(&candidate, e))?;
        if !candidate.starts_with(&resolved) {
            return Ok(candidate);
        }
    }
    Err(StorageError::InvalidConfig {
        reason: format!(
            "no default staging directory outside {}; pass one to FsBackend::new",
            resolved.display()
        ),
    })
}

/// Map a configuration-time I/O error on `path`.
fn config_error(path: &Path, err: std::io::Error) -> StorageError {
    StorageError::InvalidConfig {
        reason: format!("{}: {err}", path.display()),
    }
}

impl FsBackend {
    /// Use `root` as the storage root and `staging` for multipart parts and
    /// the root's write lock. Both are resolved (symbolic links, `.` and `..`)
    /// first. The staging directory is created; the root is created on the
    /// first write.
    ///
    /// # Errors
    ///
    /// [`StorageError::InvalidConfig`] when `staging` resolves to `root` or
    /// inside it (staging data MUST NOT appear in the storage hierarchy), or
    /// when a directory cannot be resolved or created.
    pub fn new(
        root: impl Into<PathBuf>,
        staging: impl Into<PathBuf>,
    ) -> Result<Self, StorageError> {
        let (root, staging) = (root.into(), staging.into());
        let root = resolve_path(&root).map_err(|e| config_error(&root, e))?;
        let staging = resolve_path(&staging).map_err(|e| config_error(&staging, e))?;
        if staging.starts_with(&root) {
            return Err(StorageError::InvalidConfig {
                reason: format!(
                    "multipart staging {} must be outside the storage root {}",
                    staging.display(),
                    root.display()
                ),
            });
        }
        let uploads = uploads_path(&staging, &root);
        let lock_file = lock_path(&staging, &root);
        for dir in [&uploads, &staging.join("locks")] {
            fs::create_dir_all(dir).map_err(|e| config_error(dir, e))?;
        }
        Ok(Self {
            root,
            staging,
            uploads,
            lock_file,
            next_id: AtomicU64::new(0),
        })
    }

    /// Use `root` with the default staging directory from
    /// [`default_staging_dir`].
    ///
    /// # Errors
    ///
    /// As [`FsBackend::new`] and [`default_staging_dir`].
    pub fn with_default_staging(root: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let root = root.into();
        let staging = default_staging_dir(&root)?;
        Self::new(root, staging)
    }

    /// The resolved storage root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A process-unique suffix for temporary names and upload identifiers.
    fn unique_suffix(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("{}-{nanos}-{n}", std::process::id())
    }

    /// Take the root's write lock.
    fn lock(&self) -> Result<RootLock, StorageError> {
        RootLock::acquire(&self.lock_file)
    }

    /// Write `bytes` at `key` with `mode`. The caller holds the root lock.
    fn write_locked(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        mode: Publish,
    ) -> Result<ObjectMeta, StorageError> {
        let what = key.as_str();
        let dest = key_path(&self.root, key)?;
        write_atomic(
            &dest,
            what,
            &self.unique_suffix(),
            (Parent::Create, mode),
            |file| file.write_all(bytes).map_err(|e| io_error(what, e)),
        )?;
        Ok(ObjectMeta {
            size: bytes.len() as u64,
            etag: ETag::sha256_of(bytes),
        })
    }

    /// Check `precondition` against the object at `key` and choose how to
    /// publish. The caller holds the root lock.
    fn admit(
        &self,
        key: &StorageKey,
        precondition: &Precondition,
    ) -> Result<Publish, StorageError> {
        let current = hash_file(&key_path(&self.root, key)?)?.map(|m| m.etag);
        check_precondition(key, current.as_ref(), precondition)?;
        Ok(match precondition {
            Precondition::IfAbsent => Publish::NoReplace,
            Precondition::IfMatch(_) => Publish::Replace,
        })
    }
}

impl FsBackend {
    /// Blocking body of [`StorageBackend::put`].
    fn put_blocking(&self, key: &StorageKey, bytes: &[u8]) -> Result<ObjectMeta, StorageError> {
        check_put_size(bytes)?;
        let _lock = self.lock()?;
        self.write_locked(key, bytes, Publish::Replace)
    }

    /// Blocking body of [`StorageBackend::put_if`].
    fn put_if_blocking(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        check_put_size(bytes)?;
        let _lock = self.lock()?;
        let mode = self.admit(key, precondition)?;
        self.write_locked(key, bytes, mode)
    }

    /// Blocking body of [`StorageBackend::get_range`].
    fn get_range_blocking(
        &self,
        key: &StorageKey,
        range: ByteRange,
    ) -> Result<Vec<u8>, StorageError> {
        read_range(&key_path(&self.root, key)?, key.as_str(), range)
    }

    /// Blocking body of [`StorageBackend::head`].
    fn head_blocking(&self, key: &StorageKey) -> Result<ObjectMeta, StorageError> {
        hash_file(&key_path(&self.root, key)?)?.ok_or_else(|| StorageError::NotFound {
            key: key.as_str().to_string(),
        })
    }

    /// Blocking body of [`StorageBackend::exists`].
    fn exists_blocking(&self, key: &StorageKey) -> Result<bool, StorageError> {
        match fs::metadata(key_path(&self.root, key)?) {
            Ok(meta) => Ok(meta.is_file()),
            Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
                Ok(false)
            }
            Err(err) => Err(io_error(key.as_str(), err)),
        }
    }

    /// Blocking body of [`StorageBackend::list`]: walk only the directory the
    /// prefix names, then page through the sorted keys.
    fn list_blocking(
        &self,
        prefix: &str,
        start_after: Option<&StorageKey>,
        limit: usize,
    ) -> Result<ListPage, StorageError> {
        check_list_request(prefix, limit)?;
        let dir_prefix = prefix.rfind('/').map_or("", |i| &prefix[..=i]);
        let start = confined_path(&self.root, dir_prefix)?;
        let mut keys = walk_keys(start, dir_prefix.to_string())?;
        keys.sort_unstable();
        paginate(keys.iter().map(String::as_str), prefix, start_after, limit)
    }

    /// Blocking body of [`StorageBackend::delete`]: remove the file, then
    /// prune directories it leaves empty, stopping at the root.
    fn delete_blocking(&self, key: &StorageKey) -> Result<(), StorageError> {
        let path = key_path(&self.root, key)?;
        let _lock = self.lock()?;
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
                return Ok(())
            }
            Err(err) => return Err(io_error(key.as_str(), err)),
        }
        self.prune_empty_parents(&path, key.segments().count())
    }

    /// Remove empty directories above `path`, at most `depth - 1` levels and
    /// never the root itself.
    fn prune_empty_parents(&self, path: &Path, depth: usize) -> Result<(), StorageError> {
        let mut dir = path.parent();
        for _ in 1..depth {
            let Some(current) = dir.filter(|d| *d != self.root) else {
                break;
            };
            match fs::remove_dir(current) {
                Ok(()) => dir = current.parent(),
                Err(err)
                    if matches!(
                        err.kind(),
                        ErrorKind::DirectoryNotEmpty | ErrorKind::NotFound
                    ) =>
                {
                    break
                }
                Err(err) => return Err(io_error(&current.display().to_string(), err)),
            }
        }
        Ok(())
    }
}

impl StorageBackend for FsBackend {
    /// The SHA-256 of the stored bytes is the tag.
    fn content_addressed_tags(&self) -> bool {
        true
    }

    async fn put(&self, key: &StorageKey, bytes: &[u8]) -> Result<ObjectMeta, StorageError> {
        self.put_blocking(key, bytes)
    }

    async fn put_if(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        self.put_if_blocking(key, bytes, precondition)
    }

    async fn get_range(&self, key: &StorageKey, range: ByteRange) -> Result<Vec<u8>, StorageError> {
        self.get_range_blocking(key, range)
    }

    async fn head(&self, key: &StorageKey) -> Result<ObjectMeta, StorageError> {
        self.head_blocking(key)
    }

    async fn exists(&self, key: &StorageKey) -> Result<bool, StorageError> {
        self.exists_blocking(key)
    }

    async fn list(
        &self,
        prefix: &str,
        start_after: Option<&StorageKey>,
        limit: usize,
    ) -> Result<ListPage, StorageError> {
        self.list_blocking(prefix, start_after, limit)
    }

    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        self.delete_blocking(key)
    }

    async fn create_multipart(&self, key: &StorageKey) -> Result<UploadId, StorageError> {
        self.create_multipart_blocking(key)
    }

    async fn upload_part(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        self.upload_part_blocking(upload, part_number, bytes)
    }

    async fn complete_multipart(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        self.complete_multipart_blocking(upload, parts, precondition)
    }

    async fn abort_multipart(&self, upload: &UploadId) -> Result<(), StorageError> {
        self.abort_multipart_blocking(upload)
    }
}
