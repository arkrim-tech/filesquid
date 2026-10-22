//! The file service's part of the view operation and of storage use per collection (Task
//! API-F2.2; API-S-011, API-S-027, SRS-F-062): what each file of a view is at its head, read
//! from its own object `files/<file-id>` and its metadata record, and the bytes the history of
//! a set of files keeps.
//!
//! A file the catalog lists whose object or record cannot be found is left out and reported,
//! so the view says it is incomplete instead of failing whole; any other storage failure
//! fails the request.

use super::drive::FileFacts;
use super::error::FileError;
use super::media;
use super::service::FileService;
use super::versions::add_object_size;
use super::view::LibraryHistorySizeView;
use crate::canonical::{file_object_id, file_object_root};
use crate::journal::parse_rfc3339_ms;
use crate::ErrorCode;
use filesquid_core::FileId;
use filesquid_metadata::file_record::safe_display_name;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::view;
use filesquid_ocfl::write::VersionReservations;
use serde::{Deserialize, Serialize};

/// Query of `GET /api/v1/files/collection-history-size`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionSizeQuery {
    /// The collection.
    pub collection_id: String,
}

/// The bytes the history of a collection's files keeps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CollectionHistorySizeView {
    /// The library.
    pub library_id: String,
    /// The collection.
    pub collection_id: String,
    /// The files whose home is the collection, hidden and deleted ones included.
    pub files: usize,
    /// The distinct contents counted.
    pub contents: usize,
    /// Their bytes, each content of a file counted once.
    pub bytes: u64,
    /// True when a bound stopped the count or a file could not be read, so the totals are
    /// lower bounds.
    pub truncated: bool,
}

/// The facts of the files a view read, and whether every file was read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FactsAnswer {
    /// The facts, in the order asked.
    pub facts: Vec<FileFacts>,
    /// False when a file's object or record could not be found.
    pub complete: bool,
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// What each of `files` of `library_id` is at its head.
    ///
    /// # Errors
    /// `not_found` for an unknown library; a storage failure other than a missing file.
    pub async fn file_facts(
        &self,
        library_id: &str,
        files: Vec<FileId>,
    ) -> Result<FactsAnswer, FileError> {
        let library = self.library(library_id)?;
        let backend = library.files.backend.as_ref();
        let mut answer = FactsAnswer {
            facts: Vec::with_capacity(files.len()),
            complete: true,
        };
        for file_id in files {
            match head_facts(&library, backend, &file_id).await {
                Ok(Some(facts)) => answer.facts.push(facts),
                Ok(None) => answer.complete = false,
                Err(err) if err.code() == ErrorCode::NotFound => answer.complete = false,
                Err(err) => return Err(err),
            }
        }
        Ok(answer)
    }

    /// The bytes the history of `files` of `library_id` keeps: each distinct content of each
    /// file's object once, within [`super::MAX_SIZE_CONTENTS`].
    ///
    /// # Errors
    /// `not_found` for an unknown library; a storage failure other than a missing file.
    pub async fn files_history_size(
        &self,
        library_id: &str,
        collection_id: &str,
        files: Vec<FileId>,
    ) -> Result<CollectionHistorySizeView, FileError> {
        let library = self.library(library_id)?;
        let backend = library.files.backend.as_ref();
        let mut total = LibraryHistorySizeView {
            library_id: library_id.to_string(),
            objects: 0,
            contents: 0,
            bytes: 0,
            truncated: false,
        };
        let count = files.len();
        for file_id in files {
            let root = file_object_root(&file_id).map_err(|_| FileError::not_found())?;
            let object = match Object::open(backend, &root).await.map_err(FileError::from) {
                Ok(object) => object,
                Err(err) if err.code() == ErrorCode::NotFound => {
                    total.truncated = true;
                    continue;
                }
                Err(err) => return Err(err),
            };
            total.objects += 1;
            if !add_object_size(&library, &object, &mut total).await? {
                total.truncated = true;
                break;
            }
        }
        Ok(CollectionHistorySizeView {
            library_id: total.library_id,
            collection_id: collection_id.to_string(),
            files: count,
            contents: total.contents,
            bytes: total.bytes,
            truncated: total.truncated,
        })
    }
}

/// What `file_id` is at its head, or `None` when its object holds no record.
async fn head_facts<B: StorageBackend, R, S>(
    library: &super::service::Library<B, R, S>,
    backend: &B,
    file_id: &FileId,
) -> Result<Option<FileFacts>, FileError> {
    let root = file_object_root(file_id).map_err(|_| FileError::not_found())?;
    let object = Object::open(backend, &root).await?;
    let head = object.head().clone();
    let Some(described) = library.described(&object, &head).await? else {
        return Ok(None);
    };
    let size = view::file_details(&object, &head, &described.original_path)
        .await?
        .size;
    let modified_ms = view::object_versions(&object)
        .last()
        .and_then(|version| parse_rfc3339_ms(&version.created))
        .unwrap_or_default();
    let display_name = safe_display_name(described.display_name());
    let media_type = described
        .media_type()
        .unwrap_or_else(|| media::media_type(&described.original_path).to_string());
    Ok(Some(FileFacts {
        file_id: file_id.as_str().to_string(),
        object_id: file_object_id(file_id),
        display_name,
        media_type,
        size,
        modified_ms,
        statements: described
            .record
            .statements()
            .statements
            .iter()
            .map(|statement| (statement.curie(), statement.value.clone()))
            .collect(),
    }))
}
