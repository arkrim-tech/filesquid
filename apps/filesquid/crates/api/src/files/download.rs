//! The whole-file download (Task API-F2.2; API-S-011, NFR-003): one file version of any size,
//! streamed in pieces of at most [`MAX_RANGE_BYTES`] so the file is never held in memory.
//!
//! Each piece is one ranged read through the IO-1.2 view API on a freshly opened [`Object`],
//! so the stream owns no borrow of the storage and stops after at most
//! `size / MAX_RANGE_BYTES + 1` pieces (AGENTS.md section 22). A file that ends before its
//! recorded size ends the stream with an error, never with a short body that looks whole.

use super::error::FileError;
use super::names::StoredFile;
use super::service::{check_logical_path, resolve, FileService, Library};
use super::view::FileQuery;
use axum::body::Bytes;
use filesquid_ocfl::inventory::VersionNum;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::view::{self, MAX_RANGE_BYTES};
use futures_util::stream::{self, Stream};
use std::pin::Pin;
use std::sync::Arc;

/// The bytes of one download, piece by piece.
pub type DownloadStream = Pin<Box<dyn Stream<Item = Result<Bytes, FileError>> + Send>>;

/// One whole-file download: the file's facts and its bytes as a stream.
pub struct Download {
    /// The file's length in bytes.
    pub total: u64,
    /// The media type it carries: the record's for a per-file object's original.
    pub media_type: String,
    /// The display name the answer names the file by.
    pub file_name: String,
    /// The bytes, in pieces of at most [`MAX_RANGE_BYTES`].
    pub body: DownloadStream,
}

impl std::fmt::Debug for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download")
            .field("total", &self.total)
            .field("media_type", &self.media_type)
            .finish_non_exhaustive()
    }
}

/// Where the stream is: the file it reads and the next byte to read.
struct Cursor<B, R, S> {
    library: Arc<Library<B, R, S>>,
    root: String,
    version: VersionNum,
    path: String,
    offset: u64,
    total: u64,
    pieces_left: u64,
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend + 'static,
    R: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    /// The whole-file download of one file version: its facts now, its bytes as they are
    /// streamed. Reads the inventory and the file's length before the first byte, so a
    /// missing file is refused before an answer starts.
    pub async fn download(&self, query: FileQuery) -> Result<Download, FileError> {
        check_logical_path(&query.path)?;
        let library = self.library(&query.library_id)?;
        let root = library.locate(&query.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        let version = resolve(&object, query.version.as_deref())?;
        let digest = object
            .state(&version)?
            .get(&query.path)
            .map(|file| file.digest.clone())
            .ok_or_else(FileError::not_found)?;
        let total = library
            .size_of(&object, &version, &query.path, &digest)
            .await?;
        let described = library
            .described(&object, &version)
            .await?
            .filter(|d| d.original_path == query.path);
        let version_name = version.to_string();
        let file = StoredFile {
            library_id: &query.library_id,
            object_id: &query.object_id,
            version: &version_name,
            path: &query.path,
        };
        let (file_name, media_type) = self.answer_names(file, described.as_ref()).await;
        let cursor = Cursor {
            library: library.clone(),
            root,
            version,
            path: query.path.clone(),
            offset: 0,
            total,
            pieces_left: total / MAX_RANGE_BYTES + 1,
        };
        Ok(Download {
            total,
            media_type,
            file_name,
            body: Box::pin(stream::try_unfold(cursor, next_piece)),
        })
    }
}

/// Read the piece at `cursor`, or end the stream when the whole file was read.
async fn next_piece<B, R, S>(
    mut cursor: Cursor<B, R, S>,
) -> Result<Option<(Bytes, Cursor<B, R, S>)>, FileError>
where
    B: StorageBackend,
{
    if cursor.offset >= cursor.total {
        return Ok(None);
    }
    if cursor.pieces_left == 0 {
        return Err(FileError::internal(
            "a download read more pieces than its size needs",
        ));
    }
    cursor.pieces_left -= 1;
    let len = (cursor.total - cursor.offset).min(MAX_RANGE_BYTES);
    let backend = cursor.library.files.backend.as_ref();
    let object = Object::open(backend, &cursor.root).await?;
    let piece =
        view::read_range(&object, &cursor.version, &cursor.path, cursor.offset, len).await?;
    if piece.is_empty() {
        return Err(FileError::internal(
            "a stored file ended before its recorded size",
        ));
    }
    cursor.offset += piece.len() as u64;
    Ok(Some((Bytes::from(piece), cursor)))
}
