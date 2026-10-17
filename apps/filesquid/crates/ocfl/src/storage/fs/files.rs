//! File-level operations for the filesystem backend: staged atomic writes,
//! bounded hashing and copying, and ranged reads.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::super::error::StorageError;
use super::super::types::{ByteRange, CompletedPart, ETag, ObjectMeta, MAX_OBJECT_BYTES};
use super::io_error;
use super::paths::RESERVED_PREFIX;

/// Bytes read per step when hashing or copying a file.
const COPY_BUFFER: usize = 64 * 1024;

/// Whether a write may create the destination's parent directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Parent {
    /// Create missing parent directories (objects under the root).
    Create,
    /// The parent MUST already exist (a multipart upload directory): a
    /// closed or aborted upload is never recreated by a late part.
    MustExist,
}

/// How a staged temporary file becomes the object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Publish {
    /// Rename over any existing object.
    Replace,
    /// Publish only if no object exists: a hard link, which the operating
    /// system refuses when the target exists, so concurrent creators cannot
    /// both win even without a shared lock.
    NoReplace,
}

/// Remove `path`, logging (never hiding) a failure that cannot be returned
/// because another error or a completed write is already being reported.
pub(super) fn remove_file_logged(path: &Path) {
    if let Err(err) = fs::remove_file(path) {
        if err.kind() != ErrorKind::NotFound {
            tracing::warn!(path = %path.display(), error = %err, "could not remove temporary file");
        }
    }
}

/// Write a temporary file beside `dest` with `fill`, flushed to disk. The
/// temporary name starts with the reserved prefix, so it is never a key and
/// never listed. The file is removed on every error path.
fn stage_temp<R>(
    dest: &Path,
    suffix: &str,
    parent_mode: Parent,
    fill: impl FnOnce(&mut File) -> Result<R, StorageError>,
) -> Result<(PathBuf, R), StorageError> {
    let what = dest.display().to_string();
    let parent = dest.parent().ok_or_else(|| StorageError::Backend {
        reason: format!("{what} has no parent directory"),
    })?;
    if parent_mode == Parent::Create {
        fs::create_dir_all(parent).map_err(|e| io_error(&what, e))?;
    }
    let temp = parent.join(format!("{RESERVED_PREFIX}tmp-{suffix}"));
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
    match filled {
        Ok(value) => Ok((temp, value)),
        Err(err) => {
            remove_file_logged(&temp);
            Err(err)
        }
    }
}

/// Move a staged file into place at `dest`. The temporary file is gone
/// afterwards on every path.
fn publish_temp(temp: &Path, dest: &Path, key: &str, mode: Publish) -> Result<(), StorageError> {
    let published = match mode {
        Publish::Replace => fs::rename(temp, dest),
        Publish::NoReplace => fs::hard_link(temp, dest),
    };
    if mode == Publish::NoReplace || published.is_err() {
        remove_file_logged(temp);
    }
    match published {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            Err(StorageError::PreconditionFailed {
                key: key.to_string(),
            })
        }
        Err(err) => Err(io_error(key, err)),
    }
}

/// Write a new object at `dest` through a staged temporary file: `fill`
/// writes the content, then the file is published with `mode`. Readers see
/// the old object or the new one, never a partial file.
///
/// # Errors
///
/// [`StorageError::PreconditionFailed`] when `mode` is
/// [`Publish::NoReplace`] and an object already exists;
/// [`StorageError::NotFound`] when `parent` is [`Parent::MustExist`] and the
/// parent directory is gone; any error from `fill` or the filesystem.
pub(super) fn write_atomic<R>(
    dest: &Path,
    key: &str,
    suffix: &str,
    (parent, mode): (Parent, Publish),
    fill: impl FnOnce(&mut File) -> Result<R, StorageError>,
) -> Result<R, StorageError> {
    let (temp, value) = stage_temp(dest, suffix, parent, fill)?;
    publish_temp(&temp, dest, key, mode)?;
    Ok(value)
}

/// Hash and measure a file in bounded steps. `Ok(None)` when it is missing
/// or is not a regular file; a failure to read its metadata is an error.
pub(super) fn hash_file(path: &Path) -> Result<Option<ObjectMeta>, StorageError> {
    let what = path.display().to_string();
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            return Ok(None)
        }
        Err(err) => return Err(io_error(&what, err)),
    };
    if !file.metadata().map_err(|e| io_error(&what, e))?.is_file() {
        return Ok(None);
    }
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
pub(super) fn read_range(
    path: &Path,
    key: &str,
    range: ByteRange,
) -> Result<Vec<u8>, StorageError> {
    let mut file = File::open(path).map_err(|e| match e.kind() {
        ErrorKind::NotADirectory => StorageError::NotFound {
            key: key.to_string(),
        },
        _ => io_error(key, e),
    })?;
    if !file.metadata().map_err(|e| io_error(key, e))?.is_file() {
        return Err(StorageError::NotFound {
            key: key.to_string(),
        });
    }
    file.seek(SeekFrom::Start(range.start()))
        .map_err(|e| io_error(key, e))?;
    let mut bytes = Vec::with_capacity(range.len() as usize);
    file.take(range.len())
        .read_to_end(&mut bytes)
        .map_err(|e| io_error(key, e))?;
    Ok(bytes)
}

/// The staged file of part `number` in an upload directory.
pub(super) fn part_path(dir: &Path, number: u32) -> PathBuf {
    dir.join(format!("part-{number:05}"))
}

/// Append every listed part to `out`, verifying each part's tag. Returns the
/// published object's size and tag.
pub(super) fn append_parts(
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
        let path = part_path(dir, part.part_number);
        let what = path.display().to_string();
        let mut src = open_part(&path, &what, &mismatch)?;
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

/// Open a staged part without following links. A missing part is a
/// [`StorageError::PartMismatch`]; a link or other failure is reported as
/// such, never hidden as a mismatch.
fn open_part(path: &Path, what: &str, mismatch: &StorageError) -> Result<File, StorageError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(StorageError::SymlinkForbidden {
                key: what.to_string(),
            })
        }
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => return Err(mismatch.clone()),
        Err(err) => return Err(io_error(what, err)),
    }
    File::open(path).map_err(|e| match e.kind() {
        ErrorKind::NotFound => mismatch.clone(),
        _ => io_error(what, e),
    })
}
