//! Version history operations of the file routes (Task API-F2.5; API-S-014, SRS-F-065,
//! API-S-028): labels on version listings, restore as one revert-or-reinstate operation,
//! labels set after commit, and the bytes a file's or a library's history keeps.
//!
//! Restore and label commits carry the structured `filesquid.operation.v1` message (kind
//! `revert`, `reinstate` or `label`). Once the host installs provenance
//! ([`FileService::install_provenance`]) they also write their PROV-O record in the same
//! version through a [`CommitRecorder`], and answer the journal entries to append after the
//! commit (the `ocfl.commit` entry and the operation's own, with IDs allocated before it).
//! Nothing here deletes, expires or thins a version: OCFL keeps every one.

use super::error::FileError;
use super::labels::{
    check_label, commit_label, refuse_reserved, Labels, LABELS_PATH, MAX_LABELS_FILE_BYTES,
};
use super::service::{
    change_model, check_logical_path, check_revision, commit_view, read, FileService, Library,
};
use super::view::{
    CommitView, FileHistorySizeView, HistoryQuery, HistoryView, LabelRequest,
    LibraryHistorySizeView, LibraryQuery, RestoreRequest,
};
use super::{LABEL_OPERATION, MAX_RESTORE_PATHS, RESTORE_OPERATION};
use crate::history::{commit_meta, CommitOperation, CommitRecorder, ProvenanceContext};
use crate::journal::{JournalEntry, JournalStore, Outcome};
use crate::library::mint_uuid_v4;
use filesquid_core::{OperationId, OperationKind};
use filesquid_ocfl::change::{ChangeError, ChangeModel, VersionMeta};
use filesquid_ocfl::inventory::{User, VersionNum};
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::{StorageBackend, StorageKey};
use filesquid_ocfl::view;
use filesquid_ocfl::write::{CommitReceipt, VersionBoundFiles, VersionReservations};
use std::collections::BTreeSet;
use std::sync::Arc;

/// Most objects one library history-size count opens (AGENTS.md §22).
pub const MAX_SIZE_OBJECTS: usize = 10_000;

/// Most distinct contents one library history-size count sizes (AGENTS.md §22).
pub const MAX_SIZE_CONTENTS: usize = 100_000;

/// Journal IDs a provenance commit allocates: its `ocfl.commit` entry and the operation's.
const COMMIT_IDS: usize = 2;

/// What a file service records provenance with: who commits, and the journal store that
/// hands out each commit's entry IDs before it.
#[derive(Clone)]
pub struct FileProvenance {
    context: ProvenanceContext,
    ids: Arc<dyn JournalStore>,
}

/// A published version and the journal entries its commit carried, to append after it.
/// `entries` is empty when no provenance is installed; the route journals the write itself
/// then.
#[derive(Clone, Debug, PartialEq)]
pub struct Committed {
    /// The version.
    pub view: CommitView,
    /// The commit's journal entries, with their pre-allocated IDs.
    pub entries: Vec<JournalEntry>,
}

/// The version metadata and the optional recorder of one commit.
struct Prepared {
    meta: VersionMeta,
    recorder: Option<CommitRecorder>,
}

impl Prepared {
    /// The recorder as the change model's version-bound file function.
    fn bound(&self) -> Option<&dyn VersionBoundFiles> {
        self.recorder.as_ref().map(|r| r as &dyn VersionBoundFiles)
    }

    /// The answer of the published `receipt`.
    fn finish(self, library_id: &str, receipt: CommitReceipt) -> Committed {
        let entries = self
            .recorder
            .map(|recorder| recorder.journal_entries(&receipt))
            .unwrap_or_default();
        Committed {
            view: commit_view(library_id, receipt),
            entries,
        }
    }
}

/// Which write one commit is, for its message and journal entry.
struct Write<'a> {
    actor: &'a str,
    kind: OperationKind,
    label: Option<String>,
    operation: &'static str,
    library_id: &'a str,
    object_id: &'a str,
}

impl<B, R, S> FileService<B, R, S> {
    /// Record provenance as `context`: restore and label commits then write their PROV-O
    /// record in the same version, naming journal entry IDs that `ids` allocates before the
    /// commit (API-S-028). Returns false, keeping the first, when already installed.
    pub fn install_provenance(
        &self,
        context: ProvenanceContext,
        ids: Arc<dyn JournalStore>,
    ) -> bool {
        self.provenance.set(FileProvenance { context, ids }).is_ok()
    }

    /// The version metadata and recorder of `write`: the installation `user` and a PROV-O
    /// recorder with provenance installed, else the actor as `user`; the structured
    /// operation message either way.
    fn prepare(&self, write: &Write<'_>) -> Result<Prepared, FileError> {
        let created = self.created();
        let operation_id = mint_uuid_v4()
            .ok()
            .and_then(|id| OperationId::new(id).ok())
            .ok_or_else(|| FileError::unavailable("no operation id could be minted"))?;
        let mut operation =
            CommitOperation::new(operation_id, write.kind, created.clone()).with_actor(write.actor);
        if let Some(label) = &write.label {
            operation = operation
                .with_label(label.clone())
                .map_err(|_| FileError::invalid("A label is too long."))?;
        }
        let Some(provenance) = self.provenance.get() else {
            let user = User {
                name: write.actor.to_string(),
                address: None,
            };
            let meta = VersionMeta::new(created)
                .with_user(user)
                .with_message(operation.message());
            return Ok(Prepared {
                meta,
                recorder: None,
            });
        };
        let recorder = recorder_of(provenance, &operation, write)?;
        Ok(Prepared {
            meta: commit_meta(&provenance.context, &operation, created),
            recorder: Some(recorder),
        })
    }
}

/// The recorder of `write`, with its `ocfl.commit` entry ID and the operation's own entry.
fn recorder_of(
    provenance: &FileProvenance,
    operation: &CommitOperation,
    write: &Write<'_>,
) -> Result<CommitRecorder, FileError> {
    let ids = provenance
        .ids
        .allocate_ids(COMMIT_IDS)
        .ok()
        .filter(|ids| ids.len() == COMMIT_IDS)
        .ok_or_else(|| FileError::unavailable("no journal ids for the commit record"))?;
    let mut entry = JournalEntry::new(write.operation, Outcome::Succeeded)
        .actor(write.actor)
        .target(write.library_id)
        .target(write.object_id);
    entry.id = Some(ids[1]);
    let recorder = CommitRecorder::new(provenance.context.clone(), operation.clone(), ids[0]);
    Ok(recorder.with_entries(vec![entry]))
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// Restore files or folders from an earlier version in exactly one new version: a
    /// `revert` when it returns the object's files to that version's, else a `reinstate`.
    ///
    /// `expected` is the object's head the caller read; any other is a `conflict`.
    pub async fn restore(
        &self,
        request: RestoreRequest,
        actor: &str,
        expected: &str,
    ) -> Result<Committed, FileError> {
        let version = restore_version(&request)?;
        let library = self.library(&request.library_id)?;
        let _writing = library.writes.lock().await;
        let root = library.locate(&request.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        check_revision(&object.inventory().head, expected)?;
        let files = restore_targets(&object, &version, &request.paths)?;
        let write = Write {
            actor,
            kind: restore_kind(&object, &version, &files)?,
            label: request.label.clone(),
            operation: RESTORE_OPERATION,
            library_id: &request.library_id,
            object_id: &request.object_id,
        };
        let staging = library.staging(&request.object_id)?;
        let model = change_model(&library, &root, &request.object_id, staging);
        require_clean(&model).await?;
        let prepared = self.prepare(&write)?;
        let paths: Vec<&str> = files.iter().map(String::as_str).collect();
        let receipt = model
            .reinstate_with(&version, &paths, prepared.meta.clone(), prepared.bound())
            .await?;
        Ok(prepared.finish(&request.library_id, receipt))
    }

    /// Set or clear the label of one version after commit, in one new version that holds
    /// the updated labels file.
    ///
    /// `expected` is the object's head the caller read; any other is a `conflict`.
    pub async fn set_label(
        &self,
        request: LabelRequest,
        actor: &str,
        expected: &str,
    ) -> Result<Committed, FileError> {
        if let Some(label) = &request.label {
            check_label(label)?;
        }
        let version = VersionNum::parse(&request.version)
            .map_err(|_| FileError::invalid("A label names one version, such as v1."))?;
        let library = self.library(&request.library_id)?;
        let _writing = library.writes.lock().await;
        let root = library.locate(&request.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        check_revision(&object.inventory().head, expected)?;
        let block = object
            .inventory()
            .versions
            .get(&version)
            .ok_or_else(FileError::not_found)?;
        let mut labels = read_labels(&object).await?;
        let committed = commit_label(block.message.as_deref());
        labels.set(&request.version, committed, request.label.clone())?;
        let write = Write {
            actor,
            kind: OperationKind::Label,
            label: None,
            operation: LABEL_OPERATION,
            library_id: &request.library_id,
            object_id: &request.object_id,
        };
        let staging = library.staging(&request.object_id)?;
        let model = change_model(&library, &root, &request.object_id, staging);
        require_clean(&model).await?;
        let prepared = self.prepare(&write)?;
        let receipt = commit_labels(&model, &labels, &prepared).await?;
        Ok(prepared.finish(&request.library_id, receipt))
    }
}

impl<B: StorageBackend, R, S> FileService<B, R, S> {
    /// Fill in each entry's size and later label.
    pub(super) async fn complete_history(
        &self,
        library: &Library<B, R, S>,
        object: &Object<'_, B>,
        view: &mut HistoryView,
    ) -> Result<(), FileError> {
        let labels = read_labels(object).await?;
        for entry in &mut view.entries {
            let version = VersionNum::parse(&entry.version.version)
                .map_err(|_| FileError::internal("an inventory names an unusable version"))?;
            entry.size = match &entry.digest {
                Some(digest) => Some(
                    library
                        .size_of(object, &version, &view.path, digest)
                        .await?,
                ),
                None => None,
            };
            let committed = entry.version.label.take();
            entry.version.label = labels.effective(&entry.version.version, committed);
        }
        Ok(())
    }

    /// The bytes one file's history keeps: each distinct content once.
    pub async fn history_size(
        &self,
        query: HistoryQuery,
    ) -> Result<FileHistorySizeView, FileError> {
        check_logical_path(&query.path)?;
        let library = self.library(&query.library_id)?;
        let root = library.locate(&query.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        let entries = view::file_history(&object, &query.path)?;
        if entries.is_empty() {
            return Err(FileError::not_found());
        }
        let mut seen = BTreeSet::new();
        let mut bytes = 0u64;
        for entry in &entries {
            let Some(digest) = &entry.digest else {
                continue;
            };
            if seen.insert(digest.to_ascii_lowercase()) {
                let version = &entry.version.version;
                bytes += library
                    .size_of(&object, version, &query.path, digest)
                    .await?;
            }
        }
        Ok(FileHistorySizeView {
            library_id: query.library_id,
            object_id: query.object_id,
            path: query.path,
            versions: entries.len(),
            contents: seen.len(),
            bytes,
        })
    }

    /// The bytes a library's history keeps: each distinct content of each object once,
    /// within [`MAX_SIZE_OBJECTS`] and [`MAX_SIZE_CONTENTS`]. `truncated` is also set when
    /// the storage root holds an object that could not be opened, which is left out.
    pub async fn library_history_size(
        &self,
        query: LibraryQuery,
    ) -> Result<LibraryHistorySizeView, FileError> {
        let library = self.library(&query.library_id)?;
        library.scan().await?;
        let roots = library.roots_snapshot();
        let stored = StorageRoot::open(library.files.backend.as_ref())
            .await?
            .objects()
            .len();
        let mut total = LibraryHistorySizeView {
            library_id: query.library_id,
            objects: 0,
            contents: 0,
            bytes: 0,
            // An object the scan could not open is not counted, so the total is partial.
            truncated: roots.len() > MAX_SIZE_OBJECTS || stored > roots.len(),
        };
        for root in roots.iter().take(MAX_SIZE_OBJECTS) {
            let object = Object::open(library.files.backend.as_ref(), root).await?;
            total.objects += 1;
            if !add_object_size(&library, &object, &mut total).await? {
                total.truncated = true;
                break;
            }
        }
        Ok(total)
    }
}

/// Add each distinct content of `object` to `total`; false when [`MAX_SIZE_CONTENTS`] was
/// reached first.
pub(super) async fn add_object_size<B: StorageBackend, R, S>(
    library: &Library<B, R, S>,
    object: &Object<'_, B>,
    total: &mut LibraryHistorySizeView,
) -> Result<bool, FileError> {
    for (digest, paths) in &object.inventory().manifest {
        if total.contents >= MAX_SIZE_CONTENTS {
            return Ok(false);
        }
        let Some(content_path) = paths.first() else {
            continue;
        };
        total.bytes += library
            .content_size(object.root(), content_path, digest)
            .await?;
        total.contents += 1;
    }
    Ok(true)
}

/// Replace each version's commit-time label with its later label, when one was set.
///
/// # Errors
///
/// The errors of reading the labels file.
pub(super) async fn apply_labels<B: StorageBackend>(
    object: &Object<'_, B>,
    versions: &mut [super::view::VersionView],
) -> Result<(), FileError> {
    let labels = read_labels(object).await?;
    for version in versions {
        let committed = version.label.take();
        version.label = labels.effective(&version.version, committed);
    }
    Ok(())
}

/// The labels file at the object's head, or no labels when it has none.
async fn read_labels<B: StorageBackend>(object: &Object<'_, B>) -> Result<Labels, FileError> {
    let head = object.head().clone();
    if object.state(&head)?.get(LABELS_PATH).is_none() {
        return Ok(Labels::new());
    }
    let size = view::file_details(object, &head, LABELS_PATH).await?.size;
    if size > MAX_LABELS_FILE_BYTES {
        return Err(FileError::internal(
            "a labels file is larger than its bound",
        ));
    }
    Labels::parse(&read(object, &head, LABELS_PATH, 0, size).await?)
}

/// Stage `labels` and commit it, resetting the staging area when the commit is refused so
/// the next write starts clean.
async fn commit_labels<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    labels: &Labels,
    prepared: &Prepared,
) -> Result<CommitReceipt, FileError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    model
        .stage_bytes(LABELS_PATH, &labels.to_bounded_bytes()?)
        .await?;
    let committed = model
        .commit_with(prepared.meta.clone(), prepared.bound())
        .await;
    let err = match committed {
        Ok(receipt) => return Ok(receipt),
        // Published: the version stands, and the staging area is kept as the engine asks.
        Err(ChangeError::StagingNotCleared { receipt, reason }) => {
            tracing::warn!(target: super::LOG_TARGET, %reason, "a label version kept its staging area");
            return Ok(*receipt);
        }
        Err(err) => err,
    };
    if let Err(reset) = model.reset().await {
        tracing::warn!(target: super::LOG_TARGET, error = %reset, "a refused label left its staging area");
    }
    Err(err.into())
}

/// Refuse a staging area that holds changes, so a write never commits someone else's.
async fn require_clean<B, R, S>(model: &ChangeModel<'_, B, R, S>) -> Result<(), FileError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    match model.status().await?.changes.is_empty() {
        true => Ok(()),
        false => Err(FileError::conflict(
            "Another change to this object is in progress; try again when it has finished.",
        )),
    }
}

/// The checked version, path count, paths and label of a restore.
fn restore_version(request: &RestoreRequest) -> Result<VersionNum, FileError> {
    if request.paths.is_empty() || request.paths.len() > MAX_RESTORE_PATHS {
        return Err(FileError::invalid(format!(
            "A restore names 1 to {MAX_RESTORE_PATHS} files or folders."
        )));
    }
    for path in &request.paths {
        check_logical_path(path)?;
        refuse_reserved(path)?;
    }
    if let Some(label) = &request.label {
        check_label(label)?;
    }
    VersionNum::parse(&request.version)
        .map_err(|_| FileError::invalid("A restore names one earlier version, such as v1."))
}

/// The files of `version` that `paths` name: each file itself, or every file below a
/// folder. `not_found` for a path the version holds neither as a file nor as a folder.
fn restore_targets<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    paths: &[String],
) -> Result<BTreeSet<String>, FileError> {
    let state = object.state(version)?;
    let mut files = BTreeSet::new();
    for path in paths {
        let folder = format!("{path}/");
        let found: Vec<&str> = state
            .paths()
            .filter(|p| *p == path.as_str() || p.starts_with(&folder))
            .collect();
        if found.is_empty() {
            return Err(FileError::not_found());
        }
        files.extend(found.into_iter().map(str::to_string));
    }
    Ok(files)
}

/// `revert` when restoring `files` returns every user file of the object to `version`'s
/// (the version's files are all restored and the head holds no other), else `reinstate`.
/// Files in the application-data and provenance areas are not user files.
fn restore_kind<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    files: &BTreeSet<String>,
) -> Result<OperationKind, FileError> {
    let earlier = user_files(object.state(version)?.paths());
    let head = user_files(object.state(object.head())?.paths());
    let whole = earlier.iter().all(|p| files.contains(p)) && head.is_subset(&earlier);
    Ok(match whole {
        true => OperationKind::Revert,
        false => OperationKind::Reinstate,
    })
}

/// The user files among `paths`: not application data, not provenance records.
fn user_files<'a>(paths: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    paths
        .filter(|p| !super::labels::in_reserved_folder(p))
        .map(str::to_string)
        .collect()
}

impl<B: StorageBackend, R, S> Library<B, R, S> {
    /// The object roots found so far.
    fn roots_snapshot(&self) -> Vec<String> {
        let roots = super::service::lock(&self.roots);
        let mut all: Vec<String> = roots.values().cloned().collect();
        all.sort();
        all.dedup();
        all
    }

    /// The size of the content stored at `content_path` of the object at `root`, whose
    /// digest is `digest`: from the cache, or one `head` of its storage key.
    async fn content_size(
        &self,
        root: &str,
        content_path: &str,
        digest: &str,
    ) -> Result<u64, FileError> {
        if let Some(size) = super::service::lock(&self.sizes).get(digest) {
            return Ok(*size);
        }
        let key = StorageKey::new(&format!("{root}/{content_path}"))
            .map_err(|_| FileError::internal("a manifest names an unusable content path"))?;
        let size = self.files.backend.head(&key).await?.size;
        let mut sizes = super::service::lock(&self.sizes);
        if sizes.len() >= super::service::MAX_CACHED_SIZES {
            sizes.clear();
        }
        sizes.insert(digest.to_string(), size);
        Ok(size)
    }
}
