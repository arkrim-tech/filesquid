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
//! - **Serialised writes.** Every mutation (`put`, `put_if`, `delete`,
//!   `complete_multipart`) holds an operating-system lock on a file in the
//!   staging directory, named after the resolved root. All writers to one
//!   root, in any process, MUST use the same staging directory to share it.
//! - **Create-only without a shared lock.** `Precondition::IfAbsent`
//!   publishes with a hard link, which the operating system refuses when the
//!   target exists, so two creators can never both win.
//! - **Confinement.** A key is refused when an existing component of its path
//!   below the root is a symbolic link, when a segment starts with `.fsquid-`,
//!   and on Windows when a segment is a drive or stream name, a device name or
//!   ends with a dot or space. The checks run before each operation; the root
//!   MUST NOT be writable by untrusted processes, which could swap a link in
//!   afterwards.
//! - **Staging outside the root.** Root and staging are resolved (links,
//!   `.`, `..`) before the containment check.
//!
//! Exclusive writers across hosts come from the per-object OCFL lease (ADR
//! 016, ADR 007), not from this backend.

mod files;
mod lock;
mod paths;
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
    check_list_request, check_part, check_part_order, check_precondition, check_put_size, paginate,
    ByteRange, CompletedPart, ETag, ListPage, ObjectMeta, Precondition, UploadId,
};
use files::{append_parts, hash_file, part_path, read_range, write_atomic, Publish};
use lock::{lock_path, RootLock};
use paths::{confined_path, key_path, resolve_path};
use walk::walk_keys;

/// File in a multipart staging directory that records the destination key.
const UPLOAD_KEY_FILE: &str = "key";

/// A [`StorageBackend`] over a directory on the local filesystem.
#[derive(Debug)]
pub struct FsBackend {
    root: PathBuf,
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
        let uploads = staging.join("uploads");
        let lock_file = lock_path(&staging, &root);
        for dir in [&uploads, &staging.join("locks")] {
            fs::create_dir_all(dir).map_err(|e| config_error(dir, e))?;
        }
        Ok(Self {
            root,
            uploads,
            lock_file,
            next_id: AtomicU64::new(0),
        })
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

    /// The staging directory of `upload`, after checking its identifier
    /// cannot name anything outside the uploads directory.
    fn upload_dir(&self, upload: &UploadId) -> Result<PathBuf, StorageError> {
        let id = upload.as_str();
        let safe = !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit() || b == b'-');
        let dir = self.uploads.join(id);
        if safe && dir.is_dir() {
            return Ok(dir);
        }
        Err(StorageError::UnknownUpload {
            upload_id: id.to_string(),
        })
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
        write_atomic(&dest, what, &self.unique_suffix(), mode, |file| {
            file.write_all(bytes).map_err(|e| io_error(what, e))
        })?;
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

impl FsBackend {
    /// Blocking body of [`StorageBackend::create_multipart`].
    fn create_multipart_blocking(&self, key: &StorageKey) -> Result<UploadId, StorageError> {
        key_path(&self.root, key)?;
        let id = self.unique_suffix();
        let dir = self.uploads.join(&id);
        fs::create_dir_all(&dir).map_err(|e| io_error(&dir.display().to_string(), e))?;
        let record = dir.join(UPLOAD_KEY_FILE);
        if let Err(err) = fs::write(&record, key.as_str()) {
            remove_dir_logged(&dir);
            return Err(io_error(&record.display().to_string(), err));
        }
        Ok(UploadId::new(id))
    }

    /// Blocking body of [`StorageBackend::upload_part`].
    fn upload_part_blocking(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        check_part(part_number, bytes)?;
        let path = part_path(&self.upload_dir(upload)?, part_number);
        let what = path.display().to_string();
        write_atomic(
            &path,
            &what,
            &self.unique_suffix(),
            Publish::Replace,
            |file| file.write_all(bytes).map_err(|e| io_error(&what, e)),
        )?;
        Ok(CompletedPart {
            part_number,
            etag: ETag::sha256_of(bytes),
        })
    }

    /// Blocking body of [`StorageBackend::complete_multipart`].
    fn complete_multipart_blocking(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        check_part_order(parts)?;
        let dir = self.upload_dir(upload)?;
        let record = dir.join(UPLOAD_KEY_FILE);
        let raw_key =
            fs::read_to_string(&record).map_err(|e| io_error(&record.display().to_string(), e))?;
        let key = StorageKey::new(&raw_key)?;
        let dest = key_path(&self.root, &key)?;
        let _lock = self.lock()?;
        let mode = match precondition {
            Some(condition) => self.admit(&key, condition)?,
            None => Publish::Replace,
        };
        let meta = write_atomic(&dest, key.as_str(), &self.unique_suffix(), mode, |file| {
            append_parts(&dir, parts, file)
        })?;
        remove_dir_logged(&dir);
        Ok(meta)
    }

    /// Blocking body of [`StorageBackend::abort_multipart`].
    fn abort_multipart_blocking(&self, upload: &UploadId) -> Result<(), StorageError> {
        let dir = self.upload_dir(upload)?;
        fs::remove_dir_all(&dir).map_err(|e| io_error(&dir.display().to_string(), e))
    }
}

/// Remove an upload's staging directory after its object was published or
/// its creation failed; a failure is logged because the caller's outcome is
/// already decided.
fn remove_dir_logged(dir: &Path) {
    if let Err(err) = fs::remove_dir_all(dir) {
        tracing::warn!(path = %dir.display(), error = %err, "could not remove multipart staging");
    }
}

impl StorageBackend for FsBackend {
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
