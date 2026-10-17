//! Safe opening of the backend's own files and directories.
//!
//! Every internal path (staging directories, upload directories, key
//! records, parts, lock files) and every object file is opened the same way:
//! the path is checked without following links, the file is opened, and the
//! open handle is checked again. On Unix the handle must be the same inode as
//! the path, so a link or a different file swapped in between the check and
//! the open is refused. Only regular files are accepted, and reads of small
//! records are size-capped.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{ErrorKind, Read};
use std::path::Path;

use super::super::error::StorageError;
use super::io_error;

/// Largest internal record the backend reads into memory (a multipart key
/// record): 64 KiB, far above any valid storage key.
pub(super) const MAX_RECORD_BYTES: u64 = 64 * 1024;

/// The error for a path that is a link where none is allowed.
fn link_error(path: &Path) -> StorageError {
    StorageError::SymlinkForbidden {
        key: path.display().to_string(),
    }
}

/// The error for a path that exists but is not a regular file.
fn not_regular(path: &Path) -> StorageError {
    StorageError::Io {
        key: path.display().to_string(),
        kind: ErrorKind::InvalidInput,
        message: "not a regular file".to_string(),
    }
}

/// Require `path` to be a real directory, not a link to one. A missing path
/// or a non-directory gives `missing()`.
pub(super) fn require_plain_dir(
    path: &Path,
    missing: impl Fn() -> StorageError,
) -> Result<(), StorageError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(link_error(path)),
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(missing()),
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            Err(missing())
        }
        Err(err) => Err(io_error(&path.display().to_string(), err)),
    }
}

/// What a path holds before it is opened, read without following links.
pub(super) enum Entry {
    /// Nothing is there.
    Missing,
    /// A regular file.
    File(Metadata),
    /// A directory or another non-regular object.
    Other,
}

/// Inspect `path` without following links; a link is an error.
pub(super) fn inspect(path: &Path) -> Result<Entry, StorageError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(link_error(path)),
        Ok(meta) if meta.is_file() => Ok(Entry::File(meta)),
        Ok(_) => Ok(Entry::Other),
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            Ok(Entry::Missing)
        }
        Err(err) => Err(io_error(&path.display().to_string(), err)),
    }
}

/// Whether two metadata records describe the same file. On Unix this
/// compares device and inode; elsewhere only that both are regular files.
#[cfg(unix)]
fn same_file(a: &Metadata, b: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

/// Whether two metadata records describe the same file. On Unix this
/// compares device and inode; elsewhere only that both are regular files.
#[cfg(not(unix))]
fn same_file(a: &Metadata, b: &Metadata) -> bool {
    a.is_file() && b.is_file()
}

/// Check an open handle against the path it was opened from: it must be a
/// regular file, the path must still not be a link, and both must be the same
/// file.
fn verify_handle(path: &Path, file: &File) -> Result<Metadata, StorageError> {
    let what = path.display().to_string();
    let opened = file.metadata().map_err(|e| io_error(&what, e))?;
    if !opened.is_file() {
        return Err(not_regular(path));
    }
    match inspect(path)? {
        Entry::File(now) if same_file(&now, &opened) => Ok(opened),
        _ => Err(link_error(path)),
    }
}

/// Open an existing regular file at `path` for reading without following
/// links. `Ok(None)` when nothing is there; an error for a link or a
/// non-regular file.
pub(super) fn open_existing(path: &Path) -> Result<Option<(File, Metadata)>, StorageError> {
    match inspect(path)? {
        Entry::Missing => return Ok(None),
        Entry::Other => return Err(not_regular(path)),
        Entry::File(_) => {}
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            return Ok(None)
        }
        Err(err) => return Err(io_error(&path.display().to_string(), err)),
    };
    let meta = verify_handle(path, &file)?;
    Ok(Some((file, meta)))
}

/// Open (creating if needed) a regular file at `path` for writing without
/// following links, for example a lock file.
pub(super) fn open_or_create(path: &Path) -> Result<File, StorageError> {
    if let Entry::Other = inspect(path)? {
        return Err(not_regular(path));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| io_error(&path.display().to_string(), e))?;
    verify_handle(path, &file)?;
    Ok(file)
}

/// Create a new regular file at `path` for writing. `create_new` never
/// follows a link or reuses an existing entry; the open handle is then
/// checked against the path like every other open. Used for temporary object
/// and part files and for upload key records.
pub(super) fn create_new_checked(path: &Path) -> Result<File, StorageError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| io_error(&path.display().to_string(), e))?;
    verify_handle(path, &file)?;
    Ok(file)
}

/// Read a small internal record of at most [`MAX_RECORD_BYTES`] as UTF-8.
/// `Ok(None)` when it is missing.
pub(super) fn read_record(path: &Path) -> Result<Option<String>, StorageError> {
    let what = path.display().to_string();
    let Some((file, meta)) = open_existing(path)? else {
        return Ok(None);
    };
    if meta.len() > MAX_RECORD_BYTES {
        return Err(StorageError::LimitExceeded {
            what: "internal record size",
            limit: MAX_RECORD_BYTES,
        });
    }
    let mut text = String::new();
    file.take(MAX_RECORD_BYTES)
        .read_to_string(&mut text)
        .map_err(|e| io_error(&what, e))?;
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A scratch directory under the system temporary directory, removed on
    /// drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("ocfl-internal-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("scratch");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn create_new_checked_creates_a_regular_file() {
        let scratch = Scratch::new("create");
        let path = scratch.0.join("record");
        let file = create_new_checked(&path).expect("create");
        assert!(file.metadata().expect("meta").is_file());
        assert!(matches!(inspect(&path), Ok(Entry::File(_))));
    }

    #[test]
    fn create_new_checked_never_reuses_an_existing_entry() {
        let scratch = Scratch::new("exists");
        let path = scratch.0.join("record");
        fs::write(&path, b"old").expect("existing");
        assert!(create_new_checked(&path).is_err());
        assert_eq!(fs::read(&path).expect("read"), b"old");
    }

    #[cfg(unix)]
    #[test]
    fn create_new_checked_never_follows_a_link() {
        let scratch = Scratch::new("link");
        let target = scratch.0.join("target");
        let path = scratch.0.join("record");
        std::os::unix::fs::symlink(&target, &path).expect("dangling link");
        assert!(create_new_checked(&path).is_err());
        assert!(!target.exists(), "the link target was created");
    }
}
