//! The read-only storage view of one source file (Task DE-1.9; DE-S-019, OCFL-S-026): what the
//! import hands `filesquid-api`'s import commit, so the original is staged through the streamed
//! `ChangeModel::stage_file` in bounded chunks rather than read into memory.
//!
//! The view holds exactly one object, at [`SOURCE_KEY`]. A ranged read opens the file, seeks
//! and reads at most the range (the stage asks for at most 8 MiB at a time), and closes it
//! again. The entity tag is made from the file's size, modification time and file identity,
//! never from its bytes, so asking for it reads nothing: the stage compares it before and
//! after its hashing read, and compares the digests of its two reads, to refuse a file that
//! changed meanwhile. Every write is refused. Errors name the key, never the path. A view may
//! count the bytes it reads ([`SourceFile::counting`]), which the import job reports as the
//! file's progress (Task DE-1.8).

use filesquid_ocfl::storage::{
    ByteRange, CompletedPart, ETag, ListPage, MaybeSend, ObjectMeta, Precondition, StorageBackend,
    StorageError, StorageKey, UploadId,
};
use std::future::Future;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The key of the one object a [`SourceFile`] holds.
pub const SOURCE_KEY: &str = "source";

/// One source file as a read-only storage backend.
#[derive(Clone)]
pub struct SourceFile {
    path: PathBuf,
    read: Option<Arc<AtomicU64>>,
}

impl std::fmt::Debug for SourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceFile").finish_non_exhaustive()
    }
}

impl SourceFile {
    /// The view of the file at `path`. Pure: nothing is opened until a read.
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            read: None,
        }
    }

    /// The same view, adding every byte it reads to `read`. Pure.
    pub fn counting(mut self, read: Option<Arc<AtomicU64>>) -> Self {
        self.read = read;
        self
    }

    /// Refuse every key but [`SOURCE_KEY`]. Pure.
    fn check(key: &StorageKey) -> Result<(), StorageError> {
        match key.as_str() == SOURCE_KEY {
            true => Ok(()),
            false => Err(not_found(key)),
        }
    }

    /// The size and entity tag of the file. Reads its metadata.
    fn meta(&self, key: &StorageKey) -> Result<ObjectMeta, StorageError> {
        Self::check(key)?;
        let meta = std::fs::metadata(&self.path).map_err(|err| io_error(key, &err))?;
        if !meta.is_file() {
            return Err(not_found(key));
        }
        Ok(ObjectMeta {
            size: meta.len(),
            etag: ETag::new(tag_of(&meta)),
        })
    }

    /// The bytes of `range`, at most its length (fewer at the end of the file). Opens, reads
    /// and closes the file.
    fn read(&self, key: &StorageKey, range: ByteRange) -> Result<Vec<u8>, StorageError> {
        Self::check(key)?;
        let mut file = std::fs::File::open(&self.path).map_err(|err| io_error(key, &err))?;
        file.seek(SeekFrom::Start(range.start()))
            .map_err(|err| io_error(key, &err))?;
        let capacity = usize::try_from(range.len()).unwrap_or(0);
        let mut bytes = Vec::with_capacity(capacity);
        file.take(range.len())
            .read_to_end(&mut bytes)
            .map_err(|err| io_error(key, &err))?;
        if let Some(read) = &self.read {
            read.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        }
        Ok(bytes)
    }
}

/// The entity tag of a file: its size, modification time (nanoseconds) and, on Unix, device
/// and inode, so a rewrite or a replacement changes it. Pure.
fn tag_of(meta: &std::fs::Metadata) -> String {
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos());
    format!("{}-{modified}-{}", meta.len(), identity_of(meta))
}

/// The file's identity on its volume (device and inode). Pure.
#[cfg(unix)]
fn identity_of(meta: &std::fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt;
    format!("{}:{}", meta.dev(), meta.ino())
}

/// The file's identity on its volume: not read on this platform. Pure.
#[cfg(not(unix))]
fn identity_of(_meta: &std::fs::Metadata) -> String {
    String::new()
}

/// The error of a missing object at `key`. Pure.
fn not_found(key: &StorageKey) -> StorageError {
    StorageError::NotFound {
        key: key.to_string(),
    }
}

/// The error of a failed read of `key`, naming its kind and never the path. Pure.
fn io_error(key: &StorageKey, err: &std::io::Error) -> StorageError {
    match err.kind() {
        std::io::ErrorKind::NotFound => not_found(key),
        kind => StorageError::Io {
            key: key.to_string(),
            kind,
            message: kind.to_string(),
        },
    }
}

/// The refusal of a write to the read-only view. Pure.
fn read_only() -> StorageError {
    StorageError::InvalidArgument {
        reason: "a source file is read-only".into(),
    }
}

impl StorageBackend for SourceFile {
    fn put(
        &self,
        _key: &StorageKey,
        _bytes: &[u8],
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend {
        std::future::ready(Err(read_only()))
    }

    fn put_if(
        &self,
        _key: &StorageKey,
        _bytes: &[u8],
        _precondition: &Precondition,
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend {
        std::future::ready(Err(read_only()))
    }

    fn get_range(
        &self,
        key: &StorageKey,
        range: ByteRange,
    ) -> impl Future<Output = Result<Vec<u8>, StorageError>> + MaybeSend {
        std::future::ready(self.read(key, range))
    }

    fn head(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend {
        std::future::ready(self.meta(key))
    }

    fn exists(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<bool, StorageError>> + MaybeSend {
        let found = match self.meta(key) {
            Ok(_) => Ok(true),
            Err(StorageError::NotFound { .. }) => Ok(false),
            Err(err) => Err(err),
        };
        std::future::ready(found)
    }

    fn list(
        &self,
        _prefix: &str,
        _start_after: Option<&StorageKey>,
        _limit: usize,
    ) -> impl Future<Output = Result<ListPage, StorageError>> + MaybeSend {
        std::future::ready(Err(StorageError::InvalidArgument {
            reason: "a source file is not listed".into(),
        }))
    }

    fn delete(
        &self,
        _key: &StorageKey,
    ) -> impl Future<Output = Result<(), StorageError>> + MaybeSend {
        std::future::ready(Err(read_only()))
    }

    fn create_multipart(
        &self,
        _key: &StorageKey,
    ) -> impl Future<Output = Result<UploadId, StorageError>> + MaybeSend {
        std::future::ready(Err(read_only()))
    }

    fn upload_part(
        &self,
        _upload: &UploadId,
        _part_number: u32,
        _bytes: &[u8],
    ) -> impl Future<Output = Result<CompletedPart, StorageError>> + MaybeSend {
        std::future::ready(Err(read_only()))
    }

    fn complete_multipart(
        &self,
        _upload: &UploadId,
        _parts: &[CompletedPart],
        _precondition: Option<&Precondition>,
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend {
        std::future::ready(Err(read_only()))
    }

    fn abort_multipart(
        &self,
        _upload: &UploadId,
    ) -> impl Future<Output = Result<(), StorageError>> + MaybeSend {
        std::future::ready(Err(read_only()))
    }
}
