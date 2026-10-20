//! The metadata record of a per-file object, as the file routes read it (Task API-F1.4;
//! API-S-018, API-S-026, API-S-027).
//!
//! A per-file object `files/<file-id>` (identifier `urn:uuid:<file-id>`) holds its original at
//! `original/<file-uuid>.<ext>` and its `filesquid-metadata` record beside it. The content and
//! download answers of that original carry the record's display name and media type, and a
//! listing names the original by its display name and leaves the record out, so neither the
//! stored name nor the record's file name is shown as a name. Records are content-addressed,
//! so a record read once is cached by its digest and never goes stale.

use super::error::FileError;
use super::service::{lock, read, Library};
use crate::canonical::original::{companion_path, decode_record, original_path_of};
use filesquid_core::FileId;
use filesquid_metadata::file_record::FileRecord;
use filesquid_metadata::store::ocfl::MAX_RECORD_BYTES;
use filesquid_ocfl::inventory::VersionNum;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::view;
use std::sync::Arc;

/// Most records one library keeps; the cache starts over when it is full.
pub(super) const MAX_CACHED_RECORDS: usize = 1_024;

/// What a file object's record says about its original at one version.
#[derive(Clone, Debug)]
pub(crate) struct Described {
    /// The logical path of the original.
    pub original_path: String,
    /// The logical path of the record.
    pub record_path: String,
    /// The whole record.
    pub record: Arc<FileRecord>,
}

impl Described {
    /// The record's media type, when it names one.
    pub fn media_type(&self) -> Option<String> {
        self.record.media_type().map(str::to_string)
    }

    /// The record's display name.
    pub fn display_name(&self) -> &str {
        self.record.display_name()
    }
}

/// The file identifier of a per-file object identifier `urn:uuid:<file-id>`, or `None`.
pub(crate) fn file_id_of(object_id: &str) -> Option<FileId> {
    let id = object_id.strip_prefix("urn:uuid:")?;
    FileId::new(id).ok()
}

impl<B: StorageBackend, R, S> Library<B, R, S> {
    /// What the record of `object` says at `version`: `None` when the object is not a
    /// per-file object or holds no record at that version.
    ///
    /// # Errors
    /// `internal` for a record that is too large or not a valid file record; a storage
    /// failure otherwise.
    pub(crate) async fn described(
        &self,
        object: &Object<'_, B>,
        version: &VersionNum,
    ) -> Result<Option<Described>, FileError> {
        let Some(file_id) = file_id_of(object.id()) else {
            return Ok(None);
        };
        let record_path = companion_path(&file_id).map_err(|_| FileError::not_found())?;
        let state = object.state(version)?;
        let Some(stored) = state.get(&record_path) else {
            return Ok(None);
        };
        let digest = stored.digest.clone();
        let record = match self.cached_record(&digest) {
            Some(record) => record,
            None => {
                self.read_record(object, version, &record_path, &digest)
                    .await?
            }
        };
        let original_path =
            original_path_of(&file_id, &record).map_err(|_| FileError::not_found())?;
        Ok(Some(Described {
            original_path,
            record_path,
            record,
        }))
    }

    /// The record cached under `digest`.
    fn cached_record(&self, digest: &str) -> Option<Arc<FileRecord>> {
        lock(&self.records).get(digest).cloned()
    }

    /// Read, decode and cache the record at `path` of `version`, whose digest is `digest`.
    async fn read_record(
        &self,
        object: &Object<'_, B>,
        version: &VersionNum,
        path: &str,
        digest: &str,
    ) -> Result<Arc<FileRecord>, FileError> {
        let size = view::file_details(object, version, path).await?.size;
        if size > MAX_RECORD_BYTES {
            return Err(FileError::internal(
                "a metadata record is over its read bound",
            ));
        }
        let bytes = read(object, version, path, 0, size).await?;
        let record = decode_record(path, &bytes)
            .map_err(|_| FileError::internal("a metadata record is not a valid file record"))?;
        let record = Arc::new(record);
        let mut records = lock(&self.records);
        if records.len() >= MAX_CACHED_RECORDS {
            records.clear();
        }
        records.insert(digest.to_string(), record.clone());
        Ok(record)
    }
}
