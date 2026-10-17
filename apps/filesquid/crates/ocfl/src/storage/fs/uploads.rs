//! Multipart uploads for the filesystem backend.
//!
//! Each open upload is a directory
//! `staging/uploads/<sha256 of the resolved root>/<id>/` holding the
//! destination key and the staged parts. Namespacing by root means an upload
//! started on one root can never be completed, extended or aborted through
//! another root that shares the staging directory. The lifecycle is guarded
//! without the root lock:
//!
//! - Parts are written into the existing directory only; a part never
//!   recreates a directory, so a late part cannot reopen an upload.
//! - Completing or aborting first renames the directory to `<id>.closed` (an
//!   atomic step), which closes the upload for every other caller, then
//!   assembles or discards the parts. If assembling or removing fails, the
//!   directory is renamed back so the caller can retry or abort.
//! - The staging, uploads, per-root and upload directories are checked
//!   without following links on every operation, and the key record and the
//!   parts are opened as regular files only, without following links and
//!   with a size cap (see [`super::internal`]).

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use super::super::error::StorageError;
use super::super::key::StorageKey;
use super::super::types::{
    check_part, check_part_order, CompletedPart, ETag, ObjectMeta, Precondition, UploadId,
};
use super::files::{append_parts, part_path, write_atomic, Parent, Publish};
use super::internal::{read_record, require_plain_dir};
use super::paths::key_path;
use super::{io_error, FsBackend};

/// File in an upload directory that records the destination key.
const UPLOAD_KEY_FILE: &str = "key";

/// Suffix of a closed upload directory; such names are never valid IDs.
const CLOSED_SUFFIX: &str = ".closed";

/// The per-root uploads directory: `staging/uploads/<sha256 of root>`.
pub(super) fn uploads_path(staging: &Path, root: &Path) -> PathBuf {
    let root_tag = ETag::sha256_of(root.as_os_str().as_encoded_bytes());
    staging.join("uploads").join(root_tag.as_str())
}

/// The unknown-upload error for `id`.
fn unknown(id: &str) -> StorageError {
    StorageError::UnknownUpload {
        upload_id: id.to_string(),
    }
}

/// Remove a closed upload directory after its object was published; a
/// failure is logged because the upload is closed and the outcome decided.
fn remove_closed_logged(dir: &Path) {
    if let Err(err) = fs::remove_dir_all(dir) {
        tracing::warn!(path = %dir.display(), error = %err, "could not remove closed multipart staging");
    }
}

impl FsBackend {
    /// Check that the staging directory, `staging/uploads` and this root's
    /// uploads directory are real directories, not links.
    fn check_upload_chain(&self, id: &str) -> Result<(), StorageError> {
        let parent = self.uploads.parent().unwrap_or(&self.staging);
        for dir in [self.staging.as_path(), parent, self.uploads.as_path()] {
            require_plain_dir(dir, || unknown(id))?;
        }
        Ok(())
    }

    /// The directory of open upload `upload`, after checking its identifier
    /// and every directory on the way without following links.
    fn upload_dir(&self, upload: &UploadId) -> Result<PathBuf, StorageError> {
        let id = upload.as_str();
        let safe = !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit() || b == b'-');
        if !safe {
            return Err(unknown(id));
        }
        self.check_upload_chain(id)?;
        let dir = self.uploads.join(id);
        require_plain_dir(&dir, || unknown(id))?;
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

    /// Reopen a closed upload after a failed completion or removal. A
    /// failure is logged and the upload stays closed; the caller still gets
    /// the original error.
    fn reopen_upload_logged(&self, upload: &UploadId, closed: &Path) {
        let open = self.uploads.join(upload.as_str());
        if let Err(err) = fs::rename(closed, &open) {
            tracing::warn!(upload = upload.as_str(), error = %err, "could not reopen multipart upload");
        }
    }

    /// Read the destination key recorded in an upload directory.
    fn upload_key(&self, dir: &Path, upload: &UploadId) -> Result<StorageKey, StorageError> {
        let raw = read_record(&dir.join(UPLOAD_KEY_FILE))?;
        let raw = raw.ok_or_else(|| unknown(upload.as_str()))?;
        StorageKey::new(&raw)
    }

    /// Blocking body of [`StorageBackend::create_multipart`](super::super::StorageBackend::create_multipart).
    pub(super) fn create_multipart_blocking(
        &self,
        key: &StorageKey,
    ) -> Result<UploadId, StorageError> {
        key_path(&self.root, key)?;
        let id = self.unique_suffix();
        self.check_upload_chain(&id)?;
        let dir = self.uploads.join(&id);
        fs::create_dir(&dir).map_err(|e| io_error(&dir.display().to_string(), e))?;
        if let Err(err) = write_key_record(&dir, key) {
            remove_closed_logged(&dir);
            return Err(err);
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
        let key = self.upload_key(&self.upload_dir(upload)?, upload)?;
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
        if let Err(err) = fs::remove_dir_all(&closed) {
            self.reopen_upload_logged(upload, &closed);
            return Err(io_error(&closed.display().to_string(), err));
        }
        Ok(())
    }
}

/// Write the key record of a new upload directory. `create_new` never
/// follows or replaces an existing entry.
fn write_key_record(dir: &Path, key: &StorageKey) -> Result<(), StorageError> {
    let record = dir.join(UPLOAD_KEY_FILE);
    let what = record.display().to_string();
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&record)
        .map_err(|e| io_error(&what, e))?;
    file.write_all(key.as_str().as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| io_error(&what, e))
}
