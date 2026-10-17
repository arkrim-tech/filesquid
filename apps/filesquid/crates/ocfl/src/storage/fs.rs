//! Local filesystem storage backend (native only, behind the default-on `fs`
//! feature). Keys map to plain files under the storage root, so a library
//! stays readable and copyable with ordinary file tools.
//!
//! The operations block the polling thread on `std::fs` calls and complete on
//! their first poll; the backend needs no executor. A host that must not block
//! its reactor drives these futures on a blocking pool.
//!
//! Replacing writes go to a temporary file beside the target, are flushed to
//! disk and then renamed into place, so readers never see a partial object.
//! Multipart parts are staged in a separate directory that MUST be outside the
//! storage root. Conditional writes are serialised inside one process; across
//! processes, exclusive writers come from the per-object OCFL lease (ADR 016,
//! ADR 007), not from this backend.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::backend::StorageBackend;
use super::error::StorageError;
use super::key::StorageKey;
use super::types::{
    check_list_request, check_part, check_part_order, check_precondition, check_put_size, paginate,
    ByteRange, CompletedPart, ETag, ListPage, ObjectMeta, Precondition, UploadId, MAX_OBJECT_BYTES,
    MAX_WALK_DEPTH, MAX_WALK_ENTRIES,
};

/// Bytes read per step when hashing or copying a file.
const COPY_BUFFER: usize = 64 * 1024;

/// Marker in the names of temporary files; listings skip such files.
const TEMP_MARKER: &str = ".fsquid-tmp-";

/// File in a multipart staging directory that records the destination key.
const UPLOAD_KEY_FILE: &str = "key";

/// A [`StorageBackend`] over a directory on the local filesystem.
#[derive(Debug)]
pub struct FsBackend {
    root: PathBuf,
    staging: PathBuf,
    cas_lock: Mutex<()>,
    next_id: AtomicU64,
}

impl FsBackend {
    /// Use `root` as the storage root and `staging` for multipart parts.
    /// Neither directory is created until something is written.
    ///
    /// # Errors
    ///
    /// [`StorageError::InvalidConfig`] when `staging` is `root` or inside it:
    /// staging data MUST NOT appear in the storage hierarchy.
    pub fn new(
        root: impl Into<PathBuf>,
        staging: impl Into<PathBuf>,
    ) -> Result<Self, StorageError> {
        let (root, staging) = (root.into(), staging.into());
        if staging.starts_with(&root) {
            return Err(StorageError::InvalidConfig {
                reason: format!(
                    "multipart staging {} must be outside the storage root {}",
                    staging.display(),
                    root.display()
                ),
            });
        }
        Ok(Self {
            root,
            staging,
            cas_lock: Mutex::new(()),
            next_id: AtomicU64::new(0),
        })
    }

    /// The storage root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The file path of `key` under the root.
    fn path_of(&self, key: &StorageKey) -> PathBuf {
        key.segments()
            .fold(self.root.clone(), |path, s| path.join(s))
    }

    /// A process-unique suffix for temporary names and upload identifiers.
    fn unique_suffix(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("{}-{nanos}-{n}", std::process::id())
    }

    /// Serialise conditional writes within this process.
    fn lock_cas(&self) -> Result<MutexGuard<'_, ()>, StorageError> {
        self.cas_lock.lock().map_err(|_| StorageError::Backend {
            reason: "filesystem backend compare-and-swap lock is poisoned".to_string(),
        })
    }

    /// The staging directory of `upload`, after checking its identifier
    /// cannot name anything outside the staging directory.
    fn upload_dir(&self, upload: &UploadId) -> Result<PathBuf, StorageError> {
        let id = upload.as_str();
        let safe = !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit() || b == b'-');
        let dir = self.staging.join(id);
        if safe && dir.is_dir() {
            return Ok(dir);
        }
        Err(StorageError::UnknownUpload {
            upload_id: id.to_string(),
        })
    }
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

/// Remove `path`, logging (never hiding) a failure that cannot be returned
/// because another error or a completed write is already being reported.
fn remove_file_logged(path: &Path) {
    if let Err(err) = fs::remove_file(path) {
        if err.kind() != ErrorKind::NotFound {
            tracing::warn!(path = %path.display(), error = %err, "could not remove temporary file");
        }
    }
}

/// Write a new file at `dest` through a temporary sibling: `fill` writes the
/// content, then the file is flushed and renamed into place. The temporary
/// file is removed on every error path.
fn write_atomic<R>(
    dest: &Path,
    suffix: &str,
    fill: impl FnOnce(&mut File) -> Result<R, StorageError>,
) -> Result<R, StorageError> {
    let what = dest.display().to_string();
    let parent = dest.parent().ok_or_else(|| StorageError::Backend {
        reason: format!("{what} has no parent directory"),
    })?;
    fs::create_dir_all(parent).map_err(|e| io_error(&what, e))?;
    let name = dest
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let temp = parent.join(format!(".{name}{TEMP_MARKER}{suffix}"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| io_error(&what, e))?;
    let filled = fill(&mut file).and_then(|value| {
        file.sync_all().map_err(|e| io_error(&what, e))?;
        Ok(value)
    });
    drop(file);
    let published = filled.and_then(|value| {
        fs::rename(&temp, dest).map_err(|e| io_error(&what, e))?;
        Ok(value)
    });
    if published.is_err() {
        remove_file_logged(&temp);
    }
    published
}

/// Hash and measure a file in bounded steps. `Ok(None)` when it is missing
/// or is not a regular file.
fn hash_file(path: &Path) -> Result<Option<ObjectMeta>, StorageError> {
    let what = path.display().to_string();
    let mut file = match File::open(path) {
        Ok(file) if file.metadata().is_ok_and(|m| m.is_file()) => file,
        Ok(_) => return Ok(None),
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(io_error(&what, err)),
    };
    let mut hasher = Sha256::new();
    let size = copy_bounded(&mut file, &mut hasher, None, &what)?;
    Ok(Some(ObjectMeta {
        size,
        etag: ETag::from_hasher(hasher),
    }))
}

/// Copy `src` into `hasher` (and `sink`, when given) in [`COPY_BUFFER`]
/// steps, stopping at end of file. Returns the byte count. The loop is bounded
/// by [`MAX_OBJECT_BYTES`].
fn copy_bounded(
    src: &mut File,
    hasher: &mut Sha256,
    mut sink: Option<&mut File>,
    what: &str,
) -> Result<u64, StorageError> {
    let mut buffer = vec![0u8; COPY_BUFFER];
    let mut total: u64 = 0;
    let max_steps = MAX_OBJECT_BYTES / COPY_BUFFER as u64 + 1;
    for _ in 0..max_steps {
        let read = src.read(&mut buffer).map_err(|e| io_error(what, e))?;
        if read == 0 {
            return Ok(total);
        }
        hasher.update(&buffer[..read]);
        if let Some(out) = sink.as_deref_mut() {
            out.write_all(&buffer[..read])
                .map_err(|e| io_error(what, e))?;
        }
        total += read as u64;
    }
    Err(StorageError::LimitExceeded {
        what: "object size",
        limit: MAX_OBJECT_BYTES,
    })
}

/// Read up to `range.len()` bytes from `path` at `range.start()`.
fn read_range(path: &Path, key: &StorageKey, range: ByteRange) -> Result<Vec<u8>, StorageError> {
    let what = key.as_str();
    let not_found = || StorageError::NotFound {
        key: what.to_string(),
    };
    let file = File::open(path).map_err(|e| io_error(what, e))?;
    if !file.metadata().map_err(|e| io_error(what, e))?.is_file() {
        return Err(not_found());
    }
    let mut file = file;
    file.seek(SeekFrom::Start(range.start()))
        .map_err(|e| io_error(what, e))?;
    let mut bytes = Vec::with_capacity(range.len() as usize);
    file.take(range.len())
        .read_to_end(&mut bytes)
        .map_err(|e| io_error(what, e))?;
    Ok(bytes)
}

/// One directory still to visit during a listing walk.
struct WalkDir {
    dir: PathBuf,
    key_prefix: String,
    depth: usize,
}

/// Collect the keys of every file under `start` (whose key prefix is
/// `key_prefix`), skipping temporary files. Bounded by [`MAX_WALK_DEPTH`] and
/// [`MAX_WALK_ENTRIES`]; symbolic links to directories are not followed.
fn walk_keys(start: PathBuf, key_prefix: String) -> Result<Vec<String>, StorageError> {
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
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
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
    let key = format!("{}{name}", parent.key_prefix);
    let file_type = entry.file_type().map_err(|e| io_error(&key, e))?;
    if file_type.is_dir() {
        pending.push(WalkDir {
            dir: entry.path(),
            key_prefix: format!("{key}/"),
            depth: parent.depth + 1,
        });
    } else if !name.contains(TEMP_MARKER) {
        keys.push(key);
    }
    Ok(())
}

impl FsBackend {
    /// Blocking body of [`StorageBackend::put`].
    fn put_blocking(&self, key: &StorageKey, bytes: &[u8]) -> Result<ObjectMeta, StorageError> {
        check_put_size(bytes)?;
        let what = key.as_str();
        write_atomic(&self.path_of(key), &self.unique_suffix(), |file| {
            file.write_all(bytes).map_err(|e| io_error(what, e))
        })?;
        Ok(ObjectMeta {
            size: bytes.len() as u64,
            etag: ETag::sha256_of(bytes),
        })
    }

    /// Blocking body of [`StorageBackend::put_if`].
    fn put_if_blocking(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        check_put_size(bytes)?;
        let _guard = self.lock_cas()?;
        let current = hash_file(&self.path_of(key))?.map(|m| m.etag);
        check_precondition(key, current.as_ref(), precondition)?;
        self.put_blocking(key, bytes)
    }

    /// Blocking body of [`StorageBackend::head`].
    fn head_blocking(&self, key: &StorageKey) -> Result<ObjectMeta, StorageError> {
        hash_file(&self.path_of(key))?.ok_or_else(|| StorageError::NotFound {
            key: key.as_str().to_string(),
        })
    }

    /// Blocking body of [`StorageBackend::exists`].
    fn exists_blocking(&self, key: &StorageKey) -> Result<bool, StorageError> {
        match fs::metadata(self.path_of(key)) {
            Ok(meta) => Ok(meta.is_file()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
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
        let start = dir_prefix
            .split('/')
            .filter(|s| !s.is_empty())
            .fold(self.root.clone(), |path, s| path.join(s));
        let mut keys = walk_keys(start, dir_prefix.to_string())?;
        keys.sort_unstable();
        paginate(keys.iter().map(String::as_str), prefix, start_after, limit)
    }

    /// Blocking body of [`StorageBackend::delete`]: remove the file, then
    /// prune directories it leaves empty, stopping at the root.
    fn delete_blocking(&self, key: &StorageKey) -> Result<(), StorageError> {
        let path = self.path_of(key);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(io_error(key.as_str(), err)),
        }
        let mut dir = path.parent();
        for _ in 1..key.segments().count() {
            let Some(current) = dir else { break };
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
        let id = self.unique_suffix();
        let dir = self.staging.join(&id);
        fs::create_dir_all(&dir).map_err(|e| io_error(&dir.display().to_string(), e))?;
        let record = dir.join(UPLOAD_KEY_FILE);
        if let Err(err) = fs::write(&record, key.as_str()) {
            self.remove_upload_dir_logged(&dir);
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
        let dir = self.upload_dir(upload)?;
        let path = dir.join(format!("part-{part_number:05}"));
        let what = path.display().to_string();
        write_atomic(&path, &self.unique_suffix(), |file| {
            file.write_all(bytes).map_err(|e| io_error(&what, e))
        })?;
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
        let _guard = self.lock_cas()?;
        if let Some(condition) = precondition {
            let current = hash_file(&self.path_of(&key))?.map(|m| m.etag);
            check_precondition(&key, current.as_ref(), condition)?;
        }
        let meta = write_atomic(&self.path_of(&key), &self.unique_suffix(), |file| {
            append_parts(&dir, parts, file)
        })?;
        self.remove_upload_dir_logged(&dir);
        Ok(meta)
    }

    /// Blocking body of [`StorageBackend::abort_multipart`].
    fn abort_multipart_blocking(&self, upload: &UploadId) -> Result<(), StorageError> {
        let dir = self.upload_dir(upload)?;
        fs::remove_dir_all(&dir).map_err(|e| io_error(&dir.display().to_string(), e))
    }

    /// Remove an upload's staging directory after its object was published or
    /// its creation failed; a failure is logged because the caller's outcome
    /// is already decided.
    fn remove_upload_dir_logged(&self, dir: &Path) {
        if let Err(err) = fs::remove_dir_all(dir) {
            tracing::warn!(path = %dir.display(), error = %err, "could not remove multipart staging");
        }
    }
}

/// Append every listed part to `out`, verifying each part's tag. Returns the
/// published object's size and tag.
fn append_parts(
    dir: &Path,
    parts: &[CompletedPart],
    out: &mut File,
) -> Result<ObjectMeta, StorageError> {
    let mut whole = Sha256::new();
    let mut size: u64 = 0;
    for part in parts {
        let mismatch = StorageError::PartMismatch {
            part_number: part.part_number,
        };
        let path = dir.join(format!("part-{:05}", part.part_number));
        let mut src = File::open(&path).map_err(|_| mismatch.clone())?;
        let what = path.display().to_string();
        let mut part_hash = Sha256::new();
        let read = copy_bounded(&mut src, &mut part_hash, None, &what)?;
        if ETag::from_hasher(part_hash) != part.etag {
            return Err(mismatch);
        }
        src.rewind().map_err(|e| io_error(&what, e))?;
        copy_bounded(&mut src, &mut whole, Some(out), &what)?;
        size = size.saturating_add(read);
        if size > MAX_OBJECT_BYTES {
            return Err(StorageError::LimitExceeded {
                what: "object size",
                limit: MAX_OBJECT_BYTES,
            });
        }
    }
    Ok(ObjectMeta {
        size,
        etag: ETag::from_hasher(whole),
    })
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
        read_range(&self.path_of(key), key, range)
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
