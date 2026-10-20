//! The import of one source file (Task DE-1.2; F-002, DE-S-008, ADR 038 Decision 1): gather
//! the operating system's facts about it first, then read its bytes once and hand both to
//! `filesquid-api`'s [`HostFiles::import_original`], which commits the original and its
//! metadata record in one version of the file's own object.
//!
//! Until `filesquid-ocfl` offers a streamed single-file stage, the bytes are read into memory,
//! so files above [`MAX_IN_MEMORY_BYTES`] are refused with a retryable code rather than
//! read partly.

use super::facts::{gather_source, SourceError};
use super::host::{gather_host, Build};
use super::time::now;
use crate::HostFiles;
use filesquid_api::files::{FileError, SourceImport, UploadView};
use filesquid_metadata::import::ImportMethod;
use std::io::Read;
use std::path::Path;

/// Largest file one import reads into memory: 512 MiB.
pub const MAX_IN_MEMORY_BYTES: u64 = 512 * 1024 * 1024;

/// One file to import.
#[derive(Clone, Debug)]
pub struct FileImport<'a> {
    /// The library to import into.
    pub library_id: &'a str,
    /// The source file.
    pub path: &'a Path,
    /// The folder the owner chose, or the file itself for a single file.
    pub import_root: &'a Path,
    /// How the file reached FileSquid.
    pub method: ImportMethod,
    /// The import job.
    pub job_id: Option<String>,
    /// The operation the import belongs to.
    pub operation_id: Option<String>,
    /// This FileSquid build.
    pub build: &'a Build,
    /// Who imports.
    pub actor: &'a str,
}

/// Why one file was not imported (each with an error code and no path).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportFailure {
    /// The source cannot be imported at all.
    Source(SourceError),
    /// The file is above [`MAX_IN_MEMORY_BYTES`].
    TooLarge,
    /// Reading the bytes failed; a retry may succeed.
    Read,
    /// The API refused or could not commit the import.
    Commit {
        /// The API error code.
        code: &'static str,
        /// Whether a retry may succeed.
        retryable: bool,
    },
}

impl ImportFailure {
    /// The journal error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            ImportFailure::Source(err) => err.error_code(),
            ImportFailure::TooLarge => "too_large_for_this_build",
            ImportFailure::Read => "read_failed",
            ImportFailure::Commit { code, .. } => code,
        }
    }

    /// Whether the same import may succeed if tried again.
    pub fn retryable(&self) -> bool {
        match self {
            ImportFailure::Source(SourceError::PermissionDenied) => true,
            ImportFailure::Source(_) => false,
            ImportFailure::TooLarge => true,
            ImportFailure::Read => true,
            ImportFailure::Commit { retryable, .. } => *retryable,
        }
    }
}

/// The failure of a commit the API refused.
fn commit_failure(err: FileError) -> ImportFailure {
    let size_mismatch = err.message().starts_with("size_mismatch");
    ImportFailure::Commit {
        code: if size_mismatch {
            "size_mismatch"
        } else {
            err.code().as_str()
        },
        retryable: size_mismatch || err.retryable() || err.code().as_str() == "conflict",
    }
}

/// Import the file `request` names into `files`: facts first, then one read of its bytes,
/// then the API's commit. Reads the filesystem and writes the library's storage root.
///
/// # Errors
/// [`ImportFailure`]; nothing is published on any error.
pub async fn import_file(
    files: &HostFiles,
    request: &FileImport<'_>,
) -> Result<UploadView, ImportFailure> {
    let started_at = now();
    let gathered =
        gather_source(request.path, request.import_root).map_err(ImportFailure::Source)?;
    let bytes = read_bounded(request.path)?;
    let import = SourceImport {
        library_id: request.library_id.to_string(),
        file_id: None,
        method: request.method,
        job_id: request.job_id.clone(),
        operation_id: request.operation_id.clone(),
        started_at,
        source: gathered.file,
        volume: gathered.volume,
        attributes: gathered.attributes,
        host: gather_host(request.build),
    };
    files
        .import_original(import, &bytes, request.actor)
        .await
        .map_err(commit_failure)
}

/// The bytes of `path`, read once, refusing a file above [`MAX_IN_MEMORY_BYTES`]. Reads the
/// filesystem.
fn read_bounded(path: &Path) -> Result<Vec<u8>, ImportFailure> {
    let file = std::fs::File::open(path).map_err(|_| ImportFailure::Read)?;
    let length = file.metadata().map_err(|_| ImportFailure::Read)?.len();
    if length > MAX_IN_MEMORY_BYTES {
        return Err(ImportFailure::TooLarge);
    }
    let mut bytes = Vec::with_capacity(usize::try_from(length).unwrap_or(0));
    file.take(MAX_IN_MEMORY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ImportFailure::Read)?;
    if bytes.len() as u64 > MAX_IN_MEMORY_BYTES {
        return Err(ImportFailure::TooLarge);
    }
    Ok(bytes)
}
