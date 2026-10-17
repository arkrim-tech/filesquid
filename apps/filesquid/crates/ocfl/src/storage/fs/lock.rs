//! The storage-root write lock: an operating-system advisory lock on a file
//! in the staging directory, named after the resolved storage root. Every
//! backend instance, in this process or another, that uses the same staging
//! directory for the same root serialises its writes through it.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::super::error::StorageError;
use super::super::types::ETag;
use super::io_error;

/// How long to wait between attempts to take the lock.
const LOCK_RETRY: Duration = Duration::from_millis(5);

/// Attempts before giving up: about ten seconds in all.
const LOCK_ATTEMPTS: u32 = 2_000;

/// The lock file for `root` inside the staging directory.
pub(super) fn lock_path(staging: &Path, root: &Path) -> PathBuf {
    let name = ETag::sha256_of(root.as_os_str().as_encoded_bytes());
    staging
        .join("locks")
        .join(format!("{}.lock", name.as_str()))
}

/// A held root lock. Dropping it unlocks and closes the lock file.
#[derive(Debug)]
pub(super) struct RootLock {
    file: File,
}

impl RootLock {
    /// Take the lock at `path`, retrying a bounded number of times while
    /// another writer holds it.
    ///
    /// # Errors
    ///
    /// An I/O error opening or locking the file, or [`StorageError::Backend`]
    /// when the lock is still held after the last attempt.
    pub(super) fn acquire(path: &Path) -> Result<Self, StorageError> {
        let what = path.display().to_string();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| io_error(&what, e))?;
        for _ in 0..LOCK_ATTEMPTS {
            match file.try_lock() {
                Ok(()) => return Ok(Self { file }),
                Err(TryLockError::WouldBlock) => std::thread::sleep(LOCK_RETRY),
                Err(TryLockError::Error(err)) => return Err(io_error(&what, err)),
            }
        }
        Err(StorageError::Backend {
            reason: format!("timed out waiting for the storage root lock {what}"),
        })
    }
}

impl Drop for RootLock {
    /// Release the lock explicitly; closing the file would also release it,
    /// so a failure here is logged rather than lost.
    fn drop(&mut self) {
        if let Err(err) = self.file.unlock() {
            tracing::warn!(error = %err, "could not release the storage root lock");
        }
    }
}
