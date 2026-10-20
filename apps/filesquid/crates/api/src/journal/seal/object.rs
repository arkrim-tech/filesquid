//! Journal OCFL objects and their segments (API-S-032).
//!
//! A journal object is an ordinary OCFL 1.1 object, `urn:filesquid:journal:<scope-id>` at
//! [`JOURNAL_OBJECT_ROOT`] of a storage root. Each seal adds one logical file,
//! `segments/<first>-<through>.jsonl` (20-digit, zero-padded journal IDs): the entries
//! sealed, one JSON object per line, from journal ID `first`, having read the journal through
//! ID `through`. The highest `through` among the head's segments is how far the object is
//! sealed, so the cursor is part of the commit. [`read_sealed_entries`] reads every segment
//! back with a JSON parser (the rebuild of CL-1.9).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use filesquid_core::InstallationId;
use filesquid_ocfl::change::{ChangeError, ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::read::{LogicalFile, Object, ReadError};
use filesquid_ocfl::storage::{read_verified, StorageBackend, StorageKey};
use filesquid_ocfl::write::{rfc3339_utc, VersionReservations};
use filesquid_ocfl::{Declaration, SpecVersion};

use crate::canonical::ProfileTransform;
use crate::history::version_user;
use crate::journal::JournalEntry;

/// Where a journal object lives in its storage root.
pub const JOURNAL_OBJECT_ROOT: &str = "journal";

/// Prefix of every journal object ID.
pub const JOURNAL_OBJECT_PREFIX: &str = "urn:filesquid:journal:";

/// The directory of segments inside a journal object.
pub const SEGMENT_DIR: &str = "segments";

/// Largest segment in bytes; a run stops adding entries before it.
pub const MAX_SEGMENT_BYTES: usize = 16 * 1024 * 1024;

/// Most segments one object's head may list; the reader refuses more (AGENTS.md §22).
pub const MAX_SEGMENTS: usize = 1_000_000;

/// Bytes read per request when a segment is read back.
const READ_CHUNK: u64 = 1024 * 1024;

/// The version message prefix of a seal commit.
const SEAL_MESSAGE_PREFIX: &str = "filesquid.journal.seal.v1 ";

/// A boxed, sendable future of a journal object.
pub type SealFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Why a journal object could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SealError {
    /// The storage root or its reservations are unreachable; retry later.
    Unavailable(String),
    /// The object holds something a seal cannot read back.
    Invalid(String),
}

impl SealError {
    /// The journal error code: `unavailable` or `invalid`.
    pub fn code(&self) -> &'static str {
        match self {
            SealError::Unavailable(_) => "unavailable",
            SealError::Invalid(_) => "invalid",
        }
    }
}

impl std::fmt::Display for SealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SealError::Unavailable(detail) | SealError::Invalid(detail) => f.write_str(detail),
        }
    }
}

/// One segment to commit.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// The first sealed entry's journal ID.
    pub first: i64,
    /// The highest journal ID the run read for this scope (at least the last sealed ID).
    pub through: i64,
    /// The entries, in journal ID order, as the store holds them (redacted).
    pub entries: Vec<JournalEntry>,
    /// The seal run's ID (the commit's operation ID).
    pub run_id: String,
    /// When the run sealed, in Unix milliseconds (UTC).
    pub sealed_ms: i64,
}

/// A journal object the seal service writes into.
pub trait JournalObject: Send + Sync {
    /// The object's ID.
    fn object_id(&self) -> &str;
    /// The highest journal ID the object is sealed through, 0 when it has no segment yet.
    /// Storage I/O.
    fn last_sealed(&self) -> SealFuture<'_, Result<i64, SealError>>;
    /// Commit `segment` as one new version; answers the version name. Storage I/O.
    fn commit_segment<'a>(
        &'a self,
        segment: &'a Segment,
    ) -> SealFuture<'a, Result<String, SealError>>;
}

/// The storage root a journal object lives in, as a host injects it.
pub struct JournalRoot<B, R, S> {
    /// The OCFL storage root.
    pub backend: Arc<B>,
    /// Its version reservations.
    pub reservations: Arc<R>,
    /// Where a commit stages (never inside the root).
    pub staging: Arc<S>,
    /// The root's profile transform (segments are sealed like entities).
    pub transform: Arc<dyn ProfileTransform>,
    /// The reservation holder name of the seal service.
    pub holder: String,
}

impl<B, R, S> Clone for JournalRoot<B, R, S> {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            reservations: self.reservations.clone(),
            staging: self.staging.clone(),
            transform: self.transform.clone(),
            holder: self.holder.clone(),
        }
    }
}

/// The ID of the journal object of `scope_id` (a library ID or the installation UUID).
pub fn journal_object_id(scope_id: &str) -> String {
    format!("{JOURNAL_OBJECT_PREFIX}{scope_id}")
}

/// The logical path of the segment from `first` through `through`.
pub fn segment_path(first: i64, through: i64) -> String {
    format!("{SEGMENT_DIR}/{first:020}-{through:020}.jsonl")
}

/// The `through` ID of a segment path, only when the path is exactly the canonical form
/// [`segment_path`] writes: two 20-digit IDs with `first <= through`. Pure.
fn segment_through(path: &str) -> Option<i64> {
    let name = path.strip_prefix(&format!("{SEGMENT_DIR}/"))?;
    let range = name.strip_suffix(".jsonl")?;
    let (first, through) = range.split_once('-')?;
    let id = |text: &str| -> Option<i64> {
        let canonical = text.len() == 20 && text.bytes().all(|b| b.is_ascii_digit());
        canonical.then(|| text.parse().ok()).flatten()
    };
    let (first, through) = (id(first)?, id(through)?);
    (first <= through).then_some(through)
}

/// A journal object in an OCFL storage root, committed through the `filesquid-ocfl` change
/// model as the installation's user.
pub struct OcflJournalObject<B, R, S> {
    root: JournalRoot<B, R, S>,
    object_id: String,
    user: filesquid_ocfl::inventory::User,
}

impl<B, R, S> OcflJournalObject<B, R, S> {
    /// The journal object of `scope_id` in `root`, committed as `installation`.
    pub fn new(root: JournalRoot<B, R, S>, scope_id: &str, installation: &InstallationId) -> Self {
        Self {
            root,
            object_id: journal_object_id(scope_id),
            user: version_user(installation),
        }
    }
}

impl<B, R, S> JournalObject for OcflJournalObject<B, R, S>
where
    B: StorageBackend + Send + Sync + 'static,
    R: VersionReservations + Send + Sync + 'static,
    S: StorageBackend + Send + Sync + 'static,
{
    fn object_id(&self) -> &str {
        &self.object_id
    }

    fn last_sealed(&self) -> SealFuture<'_, Result<i64, SealError>> {
        Box::pin(async move {
            let paths = head_paths(self.root.backend.as_ref()).await?;
            Ok(paths
                .iter()
                .filter_map(|p| segment_through(p))
                .max()
                .unwrap_or(0))
        })
    }

    fn commit_segment<'a>(
        &'a self,
        segment: &'a Segment,
    ) -> SealFuture<'a, Result<String, SealError>> {
        Box::pin(async move { self.commit(segment).await })
    }
}

impl<B, R, S> OcflJournalObject<B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// Stage `segment` alone and commit it. Storage I/O.
    async fn commit(&self, segment: &Segment) -> Result<String, SealError> {
        let path = segment_path(segment.first, segment.through);
        let bytes = segment_bytes(&segment.entries)?;
        let sealed = self
            .root
            .transform
            .seal(&path, bytes)
            .map_err(|e| SealError::Invalid(format!("{e:?}")))?;
        let staging_prefix = format!("changes/{}", segment.run_id);
        let area =
            StagingArea::new(self.root.staging.as_ref(), &staging_prefix).map_err(change_error)?;
        let model = ChangeModel::new(
            self.root.backend.as_ref(),
            self.root.reservations.as_ref(),
            self.root.holder.as_str(),
            JOURNAL_OBJECT_ROOT,
            self.object_id.as_str(),
            area,
        );
        model.reset().await.map_err(change_error)?;
        model
            .stage_bytes(&path, &sealed)
            .await
            .map_err(change_error)?;
        match model.commit(self.meta(segment)).await {
            Ok(receipt) => Ok(receipt.version.to_string()),
            Err(ChangeError::StagingNotCleared { receipt, .. }) => Ok(receipt.version.to_string()),
            Err(err) => {
                // Nothing was published: clear the run's staging, which no later run reuses.
                if let Err(reset) = model.reset().await {
                    tracing::warn!(error = %reset, "a refused journal segment left its staging area");
                }
                Err(change_error(err))
            }
        }
    }

    /// The version metadata of a seal: the installation user and the seal message.
    fn meta(&self, segment: &Segment) -> VersionMeta {
        let created = rfc3339_utc(u64::try_from(segment.sealed_ms).unwrap_or_default());
        let message = serde_json::json!({
            "seal_id": segment.run_id,
            "first": segment.first,
            "through": segment.through,
            "entries": segment.entries.len(),
        });
        VersionMeta::new(created)
            .with_user(self.user.clone())
            .with_message(format!("{SEAL_MESSAGE_PREFIX}{message}"))
    }
}

/// A change-model failure as a seal error.
fn change_error(err: ChangeError) -> SealError {
    SealError::Unavailable(err.to_string())
}

/// A read failure as a seal error.
fn read_error(err: ReadError) -> SealError {
    match err {
        ReadError::Storage(e) => SealError::Unavailable(e.to_string()),
        other => SealError::Invalid(other.to_string()),
    }
}

/// `entries` as JSON Lines. Pure.
fn segment_bytes(entries: &[JournalEntry]) -> Result<Vec<u8>, SealError> {
    let mut out = Vec::new();
    for entry in entries {
        let line = serde_json::to_vec(entry).map_err(|e| SealError::Invalid(format!("{e:?}")))?;
        out.extend_from_slice(&line);
        out.push(b'\n');
    }
    Ok(out)
}

/// Whether `backend` holds the journal object. Storage I/O.
async fn has_journal_object<B: StorageBackend>(backend: &B) -> Result<bool, SealError> {
    let declaration = Declaration::object(SpecVersion::V1_1).file_name();
    let key = StorageKey::new(&format!("{JOURNAL_OBJECT_ROOT}/{declaration}"))
        .map_err(|e| SealError::Invalid(format!("{e:?}")))?;
    backend
        .exists(&key)
        .await
        .map_err(|e| SealError::Unavailable(e.to_string()))
}

/// The logical paths of the journal object's head, empty without the object. Storage I/O.
async fn head_paths<B: StorageBackend>(backend: &B) -> Result<Vec<String>, SealError> {
    if !has_journal_object(backend).await? {
        return Ok(Vec::new());
    }
    let object = Object::open(backend, JOURNAL_OBJECT_ROOT)
        .await
        .map_err(read_error)?;
    let state = object.state(object.head()).map_err(read_error)?;
    let paths: Vec<String> = state.paths().map(str::to_string).collect();
    if paths.len() > MAX_SEGMENTS {
        return Err(SealError::Invalid(
            "the journal object lists too many files".into(),
        ));
    }
    Ok(paths)
}

/// Every entry the journal object in `backend` holds, segment by segment in journal ID order
/// (an entry sealed twice appears twice; the rebuild de-duplicates by ID). Empty without
/// the object. Storage I/O.
///
/// # Errors
/// [`SealError::Invalid`] for a segment that is too large, does not match its digest or does
/// not parse; [`SealError::Unavailable`] for storage trouble.
pub async fn read_sealed_entries<B: StorageBackend>(
    backend: &B,
    transform: &dyn ProfileTransform,
) -> Result<Vec<JournalEntry>, SealError> {
    let mut entries = Vec::new();
    let mut failed = None;
    let mut keep = |segment: Result<Vec<JournalEntry>, SealError>| match segment {
        Ok(read) => {
            entries.extend(read);
            true
        }
        Err(err) => {
            failed = Some(err);
            false
        }
    };
    visit_sealed_segments(backend, transform, &mut keep).await?;
    match failed {
        Some(err) => Err(err),
        None => Ok(entries),
    }
}

/// Hand each segment of the journal object in `backend`, in journal ID order, to `visit`:
/// its entries, or why that one segment could not be read, so a caller can keep the other
/// segments. `visit` answers `false` to stop. Nothing is visited without the object.
/// Storage I/O.
///
/// # Errors
/// Opening the object or listing its head: [`SealError::Invalid`] for an object that cannot
/// be opened or holds more than one million segments; [`SealError::Unavailable`] for
/// storage trouble.
pub async fn visit_sealed_segments<B: StorageBackend>(
    backend: &B,
    transform: &dyn ProfileTransform,
    visit: &mut (dyn FnMut(Result<Vec<JournalEntry>, SealError>) -> bool + Send),
) -> Result<(), SealError> {
    if !has_journal_object(backend).await? {
        return Ok(());
    }
    let object = Object::open(backend, JOURNAL_OBJECT_ROOT)
        .await
        .map_err(read_error)?;
    let state = object.state(object.head()).map_err(read_error)?;
    let mut files: Vec<_> = state
        .files()
        .filter(|f| segment_through(&f.path).is_some())
        .collect();
    if files.len() > MAX_SEGMENTS {
        return Err(SealError::Invalid(format!(
            "the journal object holds more than {MAX_SEGMENTS} segments"
        )));
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    for file in files {
        let segment = read_segment(backend, &object, file, transform).await;
        if !visit(segment) {
            break;
        }
    }
    Ok(())
}

/// The entries of one segment, read within [`MAX_SEGMENT_BYTES`] and checked against its
/// digest. Storage I/O.
async fn read_segment<B: StorageBackend>(
    backend: &B,
    object: &Object<'_, B>,
    file: &LogicalFile,
    transform: &dyn ProfileTransform,
) -> Result<Vec<JournalEntry>, SealError> {
    let key = object.content_key(file).map_err(read_error)?;
    let unavailable =
        |e: filesquid_ocfl::storage::StorageError| SealError::Unavailable(e.to_string());
    if backend.head(&key).await.map_err(unavailable)?.size > MAX_SEGMENT_BYTES as u64 {
        return Err(SealError::Invalid(format!(
            "{}: segment too large",
            file.path
        )));
    }
    let stored = read_verified(backend, &key, READ_CHUNK)
        .await
        .map_err(unavailable)?;
    let digest = DigestAlgorithm::ENGINE.digest_hex(&stored);
    if !DigestAlgorithm::ENGINE.matches(&digest, &file.digest) {
        return Err(SealError::Invalid(format!(
            "{}: digest mismatch",
            file.path
        )));
    }
    let plain = transform
        .open(&file.path, stored)
        .map_err(|e| SealError::Invalid(format!("{e:?}")))?;
    parse_segment(&file.path, &plain)
}

/// When the last seal of the journal object in `backend` was committed: the RFC 3339
/// `created` time of its head version, or `None` without the object. Storage I/O.
///
/// # Errors
/// [`SealError::Invalid`] for an object that cannot be opened; [`SealError::Unavailable`]
/// for storage trouble.
pub async fn last_sealed_at<B: StorageBackend>(backend: &B) -> Result<Option<String>, SealError> {
    if !has_journal_object(backend).await? {
        return Ok(None);
    }
    let object = Object::open(backend, JOURNAL_OBJECT_ROOT)
        .await
        .map_err(read_error)?;
    let head = object.inventory().versions.get(object.head());
    Ok(head.map(|version| version.created.clone()))
}

/// The entries of one segment's bytes. Pure.
fn parse_segment(path: &str, bytes: &[u8]) -> Result<Vec<JournalEntry>, SealError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| SealError::Invalid(format!("{path}: not UTF-8")))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|e| SealError::Invalid(format!("{path}: {e}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_paths_round_trip_their_range() {
        let path = segment_path(7, 42);
        assert_eq!(
            path,
            "segments/00000000000000000007-00000000000000000042.jsonl"
        );
        assert_eq!(segment_through(&path), Some(42));
        assert_eq!(segment_through("segments/notes.txt"), None);
        assert_eq!(segment_through("other/1-2.jsonl"), None);
        assert_eq!(segment_through("segments/x-99.jsonl"), None, "not an ID");
        assert_eq!(
            segment_through("segments/1-99.jsonl"),
            None,
            "not 20 digits"
        );
        let reversed = format!("segments/{:020}-{:020}.jsonl", 9, 3);
        assert_eq!(segment_through(&reversed), None, "first after through");
        let huge = format!("segments/{:020}-{}.jsonl", 1, "9".repeat(20));
        assert_eq!(segment_through(&huge), None, "out of range");
    }
}
