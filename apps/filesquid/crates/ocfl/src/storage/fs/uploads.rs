//! Multipart uploads for the filesystem backend.
//!
//! Each open upload is a directory `staging/uploads/<id>/` holding the
//! destination key and the staged parts. Its lifecycle is guarded without the
//! root lock:
//!
//! - Parts are written into the existing directory only; a part never
//!   recreates a directory, so a late part cannot reopen an upload.
//! - Completing or aborting first renames the directory to
//!   `<id>.closed` (an atomic step), which closes the upload for every
//!   other caller, then assembles or discards the parts. A failed completion
//!   renames it back so the caller can retry or abort.
//! - The uploads directory and each upload directory are checked without
//!   following links, so an upload identifier cannot lead outside staging.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use super::super::error::StorageError;
use super::super::key::StorageKey;
use super::super::types::{
    check_part, check_part_order, CompletedPart, ETag, ObjectMeta, Precondition, UploadId,
};
use super::files::{append_parts, part_path, write_atomic, Parent, Publish};
use super::paths::key_path;
use super::{io_error, FsBackend};

/// File in an upload directory that records the destination key.
const UPLOAD_KEY_FILE: &str = "key";

/// Suffix of a closed upload directory; such names are never valid IDs.
const CLOSED_SUFFIX: &str = ".closed";

/// The unknown-upload error for `id`.
fn unknown(id: &str) -> StorageError {
    StorageError::UnknownUpload {
        upload_id: id.to_string(),
    }
}

/// Require `path` to be a real directory, not a link to one.
fn require_plain_dir(path: &Path, id: &str) -> Result<(), StorageError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(StorageError::SymlinkForbidden {
            key: path.display().to_string(),
        }),
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(unknown(id)),
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            Err(unknown(id))
        }
        Err(err) => Err(io_error(&path.display().to_string(), err)),
    }
}

/// Remove a closed upload directory; a failure is logged because the
/// upload is already closed and the caller's outcome is decided.
fn remove_closed_logged(dir: &Path) {
    if let Err(err) = fs::remove_dir_all(dir) {
        tracing::warn!(path = %dir.display(), error = %err, "could not remove closed multipart staging");
    }
}

impl FsBackend {
    /// The directory of open upload `upload`, after checking its identifier
    /// and that neither the uploads directory nor the upload directory is a
    /// link.
    fn upload_dir(&self, upload: &UploadId) -> Result<PathBuf, StorageError> {
        let id = upload.as_str();
        let safe = !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit() || b == b'-');
        if !safe {
            return Err(unknown(id));
        }
        require_plain_dir(&self.uploads, id)?;
        let dir = self.uploads.join(id);
        require_plain_dir(&dir, id)?;
        Ok(dir)
    }

    /// Close `upload` by renaming its directory; only one caller can win.
    fn close_upload(&self, upload: &UploadId) -> Result<PathBuf, StorageError> {
        let open = self.upload_dir(upload)?;
        let closed = self
            .uploads
            .join(format!("{}{CLOSED_SUFFIX}", upload.as_str()));
        fs::rename(&open, &closed).map_err(|e| match e.kind() {
            ErrorKind::NotFound => unknown(upload.as_str()),
            _ => io_error(&open.display().to_string(), e),
        })?;
        Ok(closed)
    }

    /// Reopen a closed upload after a failed completion. A failure is logged
    /// and the upload stays closed; the original error is still returned.
    fn reopen_upload_logged(&self, upload: &UploadId, closed: &Path) {
        let open = self.uploads.join(upload.as_str());
        if let Err(err) = fs::rename(closed, &open) {
            tracing::warn!(upload = upload.as_str(), error = %err, "could not reopen multipart upload");
        }
    }

    /// Blocking body of [`StorageBackend::create_multipart`](super::super::StorageBackend::create_multipart).
    pub(super) fn create_multipart_blocking(
        &self,
        key: &StorageKey,
    ) -> Result<UploadId, StorageError> {
        key_path(&self.root, key)?;
        let id = self.unique_suffix();
        let dir = self.uploads.join(&id);
        fs::create_dir(&dir).map_err(|e| io_error(&dir.display().to_string(), e))?;
        let record = dir.join(UPLOAD_KEY_FILE);
        if let Err(err) = fs::write(&record, key.as_str()) {
            remove_closed_logged(&dir);
            return Err(io_error(&record.display().to_string(), err));
        }
        Ok(UploadId::new(id))
    }

    /// Blocking body of [`StorageBackend::upload_part`](super::super::StorageBackend::upload_part).
    pub(super) fn upload_part_blocking(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        check_part(part_number, bytes)?;
        let path = part_path(&self.upload_dir(upload)?, part_number);
        let what = path.display().to_string();
        let modes = (Parent::MustExist, Publish::Replace);
        write_atomic(&path, &what, &self.unique_suffix(), modes, |file| {
            file.write_all(bytes).map_err(|e| io_error(&what, e))
        })
        .map_err(|err| match err {
            StorageError::NotFound { .. } => unknown(upload.as_str()),
            other => other,
        })?;
        Ok(CompletedPart {
            part_number,
            etag: ETag::sha256_of(bytes),
        })
    }

    /// Blocking body of [`StorageBackend::complete_multipart`](super::super::StorageBackend::complete_multipart).
    pub(super) fn complete_multipart_blocking(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        check_part_order(parts)?;
        let record = self.upload_dir(upload)?.join(UPLOAD_KEY_FILE);
        let raw_key =
            fs::read_to_string(&record).map_err(|e| io_error(&record.display().to_string(), e))?;
        let key = StorageKey::new(&raw_key)?;
        let dest = key_path(&self.root, &key)?;
        let _lock = self.lock()?;
        let mode = match precondition {
            Some(condition) => self.admit(&key, condition)?,
            None => Publish::Replace,
        };
        let closed = self.close_upload(upload)?;
        let suffix = self.unique_suffix();
        let published = write_atomic(&dest, key.as_str(), &suffix, (Parent::Create, mode), |f| {
            append_parts(&closed, parts, f)
        });
        match published {
            Ok(meta) => {
                remove_closed_logged(&closed);
                Ok(meta)
            }
            Err(err) => {
                self.reopen_upload_logged(upload, &closed);
                Err(err)
            }
        }
    }

    /// Blocking body of [`StorageBackend::abort_multipart`](super::super::StorageBackend::abort_multipart).
    pub(super) fn abort_multipart_blocking(&self, upload: &UploadId) -> Result<(), StorageError> {
        let closed = self.close_upload(upload)?;
        fs::remove_dir_all(&closed).map_err(|e| io_error(&closed.display().to_string(), e))
    }
}
