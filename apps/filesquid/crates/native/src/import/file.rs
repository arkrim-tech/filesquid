//! The import of one source file (Task DE-1.2; F-002, DE-S-008, ADR 038 Decision 1): gather
//! the operating system's facts about it first, then hand them and a read-only view of the
//! file ([`SourceFile`]) to `filesquid-api`'s [`HostFiles::import_original`], which stages the
//! original through the streamed `ChangeModel::stage_file` and commits it with its metadata
//! record in one version of the file's own object.
//!
//! The stage reads the file in bounded chunks at most twice, once to hash it and once to copy
//! it while hashing again (OCFL-S-026, Task DE-1.9), so no file is held in memory whatever its
//! size; a file whose two reads differ fails with the retryable code `source_changed`.

use super::facts::{gather_source, SourceError};
use super::host::{gather_host, Build};
use super::source::{SourceFile, SOURCE_KEY};
use super::time::now;
use crate::HostFiles;
use filesquid_api::files::{Committed, FileError, SourceImport, UploadView};
use filesquid_metadata::import::ImportMethod;
use std::path::Path;

/// One file to import.
#[derive(Clone, Debug)]
pub struct FileImport<'a> {
    /// The library to import into.
    pub library_id: &'a str,
    /// The new file's identifier (a UUID v4 minted when the import was planned), so a
    /// repeated import of the same job answers the object it already committed.
    pub file_id: &'a str,
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
    /// The system clock is before 1970 or after 9999, so no start time can be recorded.
    Clock,
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
            ImportFailure::Clock => "clock_unavailable",
            ImportFailure::Read => "read_failed",
            ImportFailure::Commit { code, .. } => code,
        }
    }

    /// Whether the same import may succeed if tried again.
    pub fn retryable(&self) -> bool {
        match self {
            ImportFailure::Source(SourceError::PermissionDenied | SourceError::Unreadable) => true,
            ImportFailure::Source(_) => false,
            ImportFailure::Clock => false,
            ImportFailure::Read => true,
            ImportFailure::Commit { retryable, .. } => *retryable,
        }
    }

    /// Whether trying again within the same job may succeed: every retryable failure, under
    /// the per-file attempt rule.
    pub fn retry_now(&self) -> bool {
        self.retryable()
    }
}

/// The codes the API puts at the start of a refusal's message for a source that was not
/// what it reported (its size, or its bytes between the stage's two reads) or that could not
/// be read while it was staged. Each is retried under the per-file attempt rule.
const SOURCE_CODES: [&str; 3] = ["size_mismatch", "source_changed", "source_unreadable"];

/// The failure of a commit the API refused: `size_mismatch`, `source_changed` and
/// `source_unreadable` by the code that starts its message, anything else by its API error code. Pure.
pub fn commit_failure(err: FileError) -> ImportFailure {
    let source = SOURCE_CODES
        .into_iter()
        .find(|code| err.message().starts_with(code));
    ImportFailure::Commit {
        code: source.unwrap_or(err.code().as_str()),
        retryable: source.is_some() || err.retryable() || err.code().as_str() == "conflict",
    }
}

/// Import the file `request` names into `files`: the source's and the host's facts first,
/// then the API's streamed stage and commit. Reads the filesystem and writes the library's
/// storage root.
///
/// # Errors
/// [`ImportFailure`]; nothing is published on any error.
pub async fn import_file(
    files: &HostFiles,
    request: &FileImport<'_>,
) -> Result<Committed<UploadView>, ImportFailure> {
    let started_at = now().ok_or(ImportFailure::Clock)?;
    let gathered =
        gather_source(request.path, request.import_root).map_err(ImportFailure::Source)?;
    let host = gather_host(request.build);
    let import = SourceImport {
        library_id: request.library_id.to_string(),
        file_id: request.file_id.to_string(),
        method: request.method,
        job_id: request.job_id.clone(),
        operation_id: request.operation_id.clone(),
        started_at,
        source: gathered.file,
        volume: gathered.volume,
        attributes: gathered.attributes,
        host,
    };
    let source = SourceFile::new(request.path);
    files
        .import_original(import, &source, SOURCE_KEY, request.actor)
        .await
        .map_err(commit_failure)
}
