//! `ocfl` store: `metadata.md` as a logical file of an OCFL object, each edit
//! committed as a new OCFL version through the `filesquid-ocfl` change model.
//!
//! [`OcflStore`] stages the new `metadata.md` with
//! [`ChangeModel::stage_bytes`] and publishes it with
//! [`ChangeModel::commit`], the engine's only write path (IO-0.10,
//! OCFL-S-015): the fenced `vN` reservation and head compare-and-swap. This
//! crate builds no inventory and publishes no `vN` itself, and an edit never
//! rewrites an earlier version (MD-S-008; owner decision 2026-10-18,
//! [ADR 027](../../../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md)
//! Decision 2). A head without its version inventory (W010) stays refused
//! with `MissingVersionInventory`: it cannot be a commit base (OCFL-S-018)
//! and is never repaired here (OCFL-S-020).
//!
//! Reads go through [`Object`]: the record of a pinned version, or of the
//! head, streamed in bounded chunks and checked against its digest. The
//! store runs over any [`StorageBackend`] the host supplies, natively and in
//! WebAssembly ([ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md)).
//! With the `ocfl-fs` feature on a native target, [`LocalObject`] is the
//! path-based form the CLI uses: one object directory on the local disk.

use std::io::{Error as IoError, ErrorKind};

use filesquid_ocfl::change::{ChangeError, ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::inventory::User;
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{StorageBackend, StorageKey, MAX_CHUNK_BYTES, MAX_OBJECT_BYTES};
use filesquid_ocfl::write::{
    rfc3339_utc, Clock, CommitReceipt, VersionBoundFiles, VersionReservations,
};
use filesquid_ocfl::{DigestAlgorithm, VersionNum};
use sha2::{Digest, Sha256};

use crate::file_record::{FileRecord, RecordError};
use crate::file_ref::FileRef;
use crate::import::ImportEntry;
use crate::json::lower_hex;
use crate::markdown::{from_markdown, to_markdown};
use crate::record::Record;
use crate::store::{MetadataStore, StoreError, METADATA_MD};

/// Largest `metadata.md` the store reads or commits: 64 MiB.
pub const MAX_RECORD_BYTES: u64 = 64 * 1024 * 1024;

/// The `message` of every version a metadata edit publishes.
pub const EDIT_MESSAGE: &str = "Edit metadata.md";

/// The caller's options for one `ocfl`-store commit (MD-S-014). The default
/// is the stand-alone behaviour: the message [`EDIT_MESSAGE`], no `user`
/// and no version-bound files.
#[derive(Default)]
pub struct CommitOptions<'o> {
    /// The OCFL version `user` (FileSquid: name `FileSquid installation`,
    /// address `urn:uuid:<installation-uuid>`).
    pub user: Option<User>,
    /// The version message instead of [`EDIT_MESSAGE`] (FileSquid: its
    /// `filesquid.operation.v1` message).
    pub message: Option<String>,
    /// Version-bound files committed in the same version (OCFL-S-024), such
    /// as the version's PROV-O record.
    pub bound: Option<&'o dyn VersionBoundFiles>,
}

/// Stores records as `metadata.md` in OCFL objects, one new version per edit.
///
/// The record describes the whole object: the [`FileRef`]'s object
/// identifier names the object, [`object_root`](Self::object_root) maps it
/// to the object root in [`storage`](Self::storage) (the host's storage
/// layout), and its logical path does not change where the record lives. A
/// pinned version is read by [`load`](MetadataStore::load); on
/// [`save`](MetadataStore::save) it MUST be the base the change set is
/// staged against, else [`StoreError::Conflict`]. If the head then moves,
/// the edit is rebased onto it (OCFL-S-016), and only a concurrent change to
/// the same `metadata.md` is a conflict; nothing is overwritten.
///
/// The fields are the host's wiring, as `filesquid-ocfl` asks for it: the
/// reservations MUST be shared by every writer of the storage root, and
/// neither they nor the staging area may live inside it.
pub struct OcflStore<'a, B, R, S, C, L> {
    /// The OCFL storage root.
    pub storage: &'a B,
    /// Version reservations shared by every writer of the storage root.
    pub reservations: &'a R,
    /// Where change sets are staged, outside the storage root.
    pub staging: &'a S,
    /// Key prefix of this store's staging areas on `staging`.
    pub staging_prefix: String,
    /// The writer name recorded on reservations.
    pub holder: String,
    /// Clock for each version's `created` value (RFC 3339 UTC).
    pub clock: C,
    /// Maps an object identifier to its object root in `storage`.
    pub object_root: L,
}

impl<'a, B, R, S, C, L> OcflStore<'a, B, R, S, C, L>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
    C: Clock,
    L: Fn(&str) -> String,
{
    /// The staging prefix of `object_id` for this writer:
    /// `<staging_prefix>/<sha256 of holder and identifier>`, one per object
    /// and writer, as [`StagingArea::new`] asks.
    pub fn staging_prefix_for(&self, object_id: &str) -> String {
        let name = format!("{}\n{object_id}", self.holder);
        let digest = DigestAlgorithm::Sha256.digest_hex(name.as_bytes());
        match self.staging_prefix.trim_end_matches('/') {
            "" => digest,
            prefix => format!("{prefix}/{digest}"),
        }
    }

    /// The committed head of `object_id`, or `None` when it has no version.
    ///
    /// # Errors
    ///
    /// [`StoreError::InvalidPath`] for an object root that could escape the
    /// storage root, or a backend failure.
    pub async fn head(&self, object_id: &str) -> Result<Option<VersionNum>, StoreError> {
        let log = self.model(object_id)?.log().await.map_err(change_error)?;
        Ok(log.into_iter().next().map(|info| info.version))
    }

    /// Commit `record` as the object's `metadata.md` in exactly one new
    /// version and return it; `None` when the record is unchanged, which
    /// publishes nothing. A change set this writer left staged by an earlier
    /// failure is discarded first. The first edit of an object that has no
    /// version yet creates it as `v1`.
    ///
    /// # Errors
    ///
    /// [`StoreError::Conflict`] when `file` pins a version that is not the
    /// head, or another writer changed `metadata.md` first;
    /// [`StoreError::InvalidPath`]; [`StoreError::Backend`] with the
    /// [`ChangeError`] for any other refusal, such as
    /// `MissingVersionInventory` for a head without its version inventory
    /// (W010), or a record larger than [`MAX_RECORD_BYTES`]. Nothing is
    /// published on any error.
    pub async fn commit_record(
        &self,
        file: &FileRef,
        record: &Record,
    ) -> Result<Option<VersionNum>, StoreError> {
        let options = CommitOptions::default();
        let receipt = self.commit_record_with(file, record, &options).await?;
        Ok(receipt.map(|receipt| receipt.version))
    }

    /// [`commit_record`](Self::commit_record) with the caller's commit
    /// options (MD-S-014): the OCFL version `user`, the version message
    /// (FileSquid passes its `filesquid.operation.v1` message) and
    /// version-bound files committed in the same version (OCFL-S-024).
    /// Returns the commit receipt, so the caller can journal the commit;
    /// `None` when the record is unchanged, in which case nothing is
    /// committed and `options.bound` is not called.
    ///
    /// # Errors
    ///
    /// As [`commit_record`](Self::commit_record), and
    /// [`StoreError::Backend`] with the [`ChangeError`] when the bound-file
    /// function fails. A failed commit publishes nothing and keeps its
    /// staged change set (never clearing an edit staged meanwhile); the
    /// next save restages `metadata.md` over it and publishes it.
    pub async fn commit_record_with(
        &self,
        file: &FileRef,
        record: &Record,
        options: &CommitOptions<'_>,
    ) -> Result<Option<CommitReceipt>, StoreError> {
        let base = self.read_text(file).await?;
        let bytes = statements_bytes(file, base.as_deref(), record)?;
        self.commit_bytes(file, &bytes, options, true).await
    }

    /// Commit `record`, the whole per-file record, as the file's metadata
    /// record in exactly one new version, discarding any change set this
    /// writer left staged first. The save is refused when it edits or removes
    /// an existing `import` entry or the original file name
    /// ([`RecordError`], MD-S-013), when a new entry cites another object or
    /// path than `file`, or when the path holds a record that is not a file
    /// record. `None` when the record is unchanged. A rename is such a save
    /// with a new display name.
    ///
    /// # Errors
    ///
    /// [`StoreError::Record`] for the refusals above, otherwise as
    /// [`commit_record_with`](Self::commit_record_with). Nothing is published
    /// on any error.
    pub async fn commit_file_record_with(
        &self,
        file: &FileRef,
        record: &FileRecord,
        options: &CommitOptions<'_>,
    ) -> Result<Option<CommitReceipt>, StoreError> {
        let base = self.read_base(file).await?;
        let bytes = file_record_bytes(file, base.as_ref(), record)?;
        self.commit_bytes(file, &bytes, options, true).await
    }

    /// [`commit_file_record_with`](Self::commit_file_record_with) with the
    /// stand-alone defaults, returning only the new version.
    ///
    /// # Errors
    ///
    /// As [`commit_file_record_with`](Self::commit_file_record_with).
    pub async fn commit_file_record(
        &self,
        file: &FileRef,
        record: &FileRecord,
    ) -> Result<Option<VersionNum>, StoreError> {
        let options = CommitOptions::default();
        let receipt = self.commit_file_record_with(file, record, &options).await?;
        Ok(receipt.map(|receipt| receipt.version))
    }

    /// Commit an import: the file record over the content the caller staged
    /// through [`change_model`](Self::change_model), in one version, so the
    /// record and the content are never apart (ADR 038 Decision 1). Nothing
    /// staged is discarded, so a failure leaves a retryable change set:
    /// calling this again restages the record and publishes both. The
    /// refusals and errors are those of
    /// [`commit_file_record_with`](Self::commit_file_record_with).
    ///
    /// # Errors
    ///
    /// As [`commit_file_record_with`](Self::commit_file_record_with).
    pub async fn commit_import_with(
        &self,
        file: &FileRef,
        record: &FileRecord,
        options: &CommitOptions<'_>,
    ) -> Result<Option<CommitReceipt>, StoreError> {
        let base = self.read_base(file).await?;
        let bytes = file_record_bytes(file, base.as_ref(), record)?;
        self.commit_bytes(file, &bytes, options, false).await
    }

    /// [`commit_import_with`](Self::commit_import_with) with the stand-alone
    /// defaults, returning only the new version.
    ///
    /// # Errors
    ///
    /// As [`commit_import_with`](Self::commit_import_with).
    pub async fn commit_import(
        &self,
        file: &FileRef,
        record: &FileRecord,
    ) -> Result<Option<VersionNum>, StoreError> {
        let options = CommitOptions::default();
        let receipt = self.commit_import_with(file, record, &options).await?;
        Ok(receipt.map(|receipt| receipt.version))
    }

    /// Append the entry of a re-import to the file's record in a new
    /// version; earlier entries stay exactly as they are.
    ///
    /// # Errors
    ///
    /// [`StoreError::Record`] with [`RecordError::NoFileRecord`] when the
    /// file has no record, [`RecordError::Import`] for an invalid entry, or
    /// as [`commit_file_record_with`](Self::commit_file_record_with).
    pub async fn append_import_with(
        &self,
        file: &FileRef,
        entry: ImportEntry,
        options: &CommitOptions<'_>,
    ) -> Result<Option<CommitReceipt>, StoreError> {
        let base = self.read_base(file).await?;
        let mut next = base.clone().ok_or(RecordError::NoFileRecord)?;
        next.append_import(entry)?;
        let bytes = file_record_bytes(file, base.as_ref(), &next)?;
        self.commit_bytes(file, &bytes, options, true).await
    }

    /// The whole per-file record of the pinned version of `file`, or of the
    /// head; `None` when the object, the version or the record is absent.
    ///
    /// # Errors
    ///
    /// [`StoreError::Record`] when the record is not a valid
    /// `filesquid.original.v1` file record, otherwise as
    /// [`read_record`](Self::read_record).
    pub async fn read_file_record(&self, file: &FileRef) -> Result<Option<FileRecord>, StoreError> {
        self.read_base(file).await
    }

    /// The change model of `object_id`, so a host can stage the content of
    /// an import ([`ChangeModel::stage_bytes`]) before
    /// [`commit_import_with`](Self::commit_import_with) commits the record
    /// over it. The staging prefix is this writer's own for the object.
    ///
    /// # Errors
    ///
    /// [`StoreError::InvalidPath`] for an object root that could escape the
    /// storage root.
    pub fn change_model(&self, object_id: &str) -> Result<ChangeModel<'a, B, R, S>, StoreError> {
        self.model(object_id)
    }

    /// Stage `bytes` as the record of `file` and publish one version.
    /// `reset_first` discards a change set this writer left staged before
    /// staging; an import passes `false` to commit over its staged content.
    async fn commit_bytes(
        &self,
        file: &FileRef,
        bytes: &[u8],
        options: &CommitOptions<'_>,
        reset_first: bool,
    ) -> Result<Option<CommitReceipt>, StoreError> {
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(too_large(record_path(file), MAX_RECORD_BYTES));
        }
        let model = self.model(file.object_id())?;
        if reset_first {
            model.reset().await.map_err(change_error)?;
        }
        model
            .stage_bytes(record_path(file), bytes)
            .await
            .map_err(change_error)?;
        if let Err(err) = check_pinned(&model, file).await {
            if reset_first {
                model.reset().await.map_err(change_error)?;
            }
            return Err(err);
        }
        let meta = self.edit_meta(options)?;
        publish(&model, meta, options.bound).await
    }

    /// The file record at the record path, `None` when there is none.
    async fn read_base(&self, file: &FileRef) -> Result<Option<FileRecord>, StoreError> {
        let Some(text) = self.read_text(file).await? else {
            return Ok(None);
        };
        Ok(Some(FileRecord::from_markdown(&text)?))
    }

    /// The record of the pinned version of `file`, or of the head. An object
    /// with no version, or a version without `metadata.md`, is an empty
    /// record.
    ///
    /// # Errors
    ///
    /// [`StoreError::InvalidPath`], or [`StoreError::Backend`] for a version
    /// the object lacks, a record that is not UTF-8 (so a later save never
    /// overwrites it with a lossy copy) or larger than [`MAX_RECORD_BYTES`],
    /// or a read failure.
    pub async fn read_record(&self, file: &FileRef) -> Result<Record, StoreError> {
        let text = self.read_text(file).await?;
        Ok(text.map_or_else(Record::default, |text| from_markdown(&text)))
    }

    /// The text of the record of the pinned version of `file`, or of the
    /// head; `None` for an object with no version or a version without it.
    async fn read_text(&self, file: &FileRef) -> Result<Option<String>, StoreError> {
        if self.head(file.object_id()).await?.is_none() {
            return Ok(None);
        }
        let object = self.open(file.object_id()).await?;
        let version = pinned_or_head(&object, file)?;
        let path = record_path(file);
        if object.state(&version).map_err(backend)?.get(path).is_none() {
            return Ok(None);
        }
        let bytes = read_logical(&object, &version, path, MAX_RECORD_BYTES).await?;
        let text = String::from_utf8(bytes)
            .map_err(|e| StoreError::Backend(Box::new(IoError::new(ErrorKind::InvalidData, e))))?;
        Ok(Some(text))
    }

    /// The bytes of the logical file `file` names, at its pinned version or
    /// the head, read through `filesquid-ocfl` and checked against the
    /// inventory digest (for example to extract its embedded metadata).
    ///
    /// # Errors
    ///
    /// [`StoreError::WrongObject`] when the object root holds another
    /// object, or [`StoreError::Backend`] for a missing object, version or
    /// logical path, a file larger than `limit` bytes, or a read failure.
    pub async fn read_file(&self, file: &FileRef, limit: u64) -> Result<Vec<u8>, StoreError> {
        let object = self.open(file.object_id()).await?;
        let version = pinned_or_head(&object, file)?;
        read_logical(&object, &version, file.logical_path(), limit).await
    }

    /// The hex SHA-256 of the logical file `file` names, at its pinned
    /// version or the head, streamed through `filesquid-ocfl` (the binding
    /// of a JSON sidecar, MD-S-010).
    ///
    /// # Errors
    ///
    /// As [`read_file`](Self::read_file).
    pub async fn file_sha256(&self, file: &FileRef) -> Result<String, StoreError> {
        let object = self.open(file.object_id()).await?;
        let version = pinned_or_head(&object, file)?;
        let mut hasher = Sha256::new();
        let path = file.logical_path();
        for_each_chunk(&object, &version, path, MAX_OBJECT_BYTES, |chunk| {
            hasher.update(chunk);
        })
        .await?;
        Ok(lower_hex(&hasher.finalize()))
    }

    /// The object root of `object_id`, refused when it could escape the
    /// storage root.
    fn root_of(&self, object_id: &str) -> Result<String, StoreError> {
        let root = (self.object_root)(object_id);
        let trimmed = root.strip_suffix('/').unwrap_or(&root);
        if !trimmed.is_empty() && StorageKey::new(trimmed).is_err() {
            return Err(StoreError::InvalidPath);
        }
        Ok(trimmed.to_string())
    }

    /// Open the object `object_id` for reading, refusing an object root
    /// that holds another object.
    async fn open(&self, object_id: &str) -> Result<Object<'a, B>, StoreError> {
        let root = self.root_of(object_id)?;
        let object = Object::open(self.storage, &root).await.map_err(backend)?;
        if object.id() != object_id {
            return Err(StoreError::WrongObject {
                expected: object_id.to_string(),
                found: object.id().to_string(),
            });
        }
        Ok(object)
    }

    /// The change model of `object_id`, staging under its own prefix.
    fn model(&self, object_id: &str) -> Result<ChangeModel<'a, B, R, S>, StoreError> {
        let root = self.root_of(object_id)?;
        let prefix = self.staging_prefix_for(object_id);
        let staging = StagingArea::new(self.staging, &prefix).map_err(change_error)?;
        Ok(ChangeModel::new(
            self.storage,
            self.reservations,
            self.holder.clone(),
            &root,
            object_id,
            staging,
        ))
    }

    /// The version metadata of an edit made now.
    fn edit_meta(&self, options: &CommitOptions<'_>) -> Result<VersionMeta, StoreError> {
        let millis = self.clock.now_millis().ok_or_else(|| {
            StoreError::Backend(Box::new(IoError::other("the clock has no current time")))
        })?;
        let message = options.message.as_deref().unwrap_or(EDIT_MESSAGE);
        let meta = VersionMeta::new(rfc3339_utc(millis)).with_message(message);
        Ok(match &options.user {
            Some(user) => meta.with_user(user.clone()),
            None => meta,
        })
    }
}

impl<B, R, S, C, L> MetadataStore for OcflStore<'_, B, R, S, C, L>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
    C: Clock,
    L: Fn(&str) -> String,
{
    async fn load(&self, file: &FileRef) -> Result<Record, StoreError> {
        self.read_record(file).await
    }

    async fn save(&self, file: &FileRef, record: &Record) -> Result<(), StoreError> {
        self.commit_record(file, record).await.map(|_| ())
    }
}

/// The logical path of the record of `file`: the path its caller named, else
/// `metadata.md`.
fn record_path(file: &FileRef) -> &str {
    file.record_path().unwrap_or(METADATA_MD)
}

/// The bytes of a statements-only save of `file`. A record at the path that
/// is a file record keeps its header and every import entry; a plain
/// `metadata.md` is replaced by the statements; a file whose caller named a
/// record path needs the record to exist already.
fn statements_bytes(
    file: &FileRef,
    base: Option<&str>,
    record: &Record,
) -> Result<Vec<u8>, StoreError> {
    let plain = || Ok(to_markdown(record).into_bytes());
    let Some(text) = base else {
        return match file.record_path() {
            Some(_) => Err(RecordError::NoFileRecord.into()),
            None => plain(),
        };
    };
    match FileRecord::from_markdown(text) {
        Ok(existing) => Ok(existing
            .with_statements(record.clone())
            .to_markdown()
            .into_bytes()),
        Err(RecordError::UnsupportedSchema) if file.record_path().is_none() => plain(),
        Err(err) => Err(err.into()),
    }
}

/// The bytes of a save of the whole file record `next` over `base`: valid,
/// extending `base` append-only, and with every new entry citing `file`.
fn file_record_bytes(
    file: &FileRef,
    base: Option<&FileRecord>,
    next: &FileRecord,
) -> Result<Vec<u8>, StoreError> {
    next.validate()?;
    let first_new = base.map_or(0, |base| base.imports().len());
    if let Some(base) = base {
        next.check_extends(base)?;
    }
    next.check_binding(first_new, file.object_id(), file.logical_path())?;
    Ok(next.to_markdown().into_bytes())
}

/// Refuse an edit whose pinned version is not the base the change set was
/// staged against. The change model then publishes onto that base, or, as
/// OCFL-S-016 requires, rebases onto a later head only when no intervening
/// version changed `metadata.md` (any such change is a conflict). So an edit
/// made from an older version never overwrites a newer record, while edits
/// to other logical files in between do not block it.
async fn check_pinned<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    file: &FileRef,
) -> Result<(), StoreError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let Some(pinned) = file.version() else {
        return Ok(());
    };
    let base = model.status().await.map_err(change_error)?.base;
    let base = base.map(|version| version.to_string());
    if base.as_deref() == Some(pinned) {
        return Ok(());
    }
    Err(StoreError::Conflict {
        reason: format!(
            "the edit was made from {pinned}, but the head is {}",
            base.as_deref().unwrap_or("no version")
        ),
    })
}

/// Publish the staged change set as one version, binding `bound`'s files to
/// it. An unchanged record publishes nothing and clears the staging area; a
/// published version whose staging area could not be cleared is still the
/// edit, and is logged; any other failure leaves the staging area as it is,
/// so a change another edit staged meanwhile is never lost (the next edit
/// restages `metadata.md` over it).
async fn publish<B, R, S>(
    model: &ChangeModel<'_, B, R, S>,
    meta: VersionMeta,
    bound: Option<&dyn VersionBoundFiles>,
) -> Result<Option<CommitReceipt>, StoreError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let err = match model.commit_with(meta, bound).await {
        Ok(receipt) => {
            tracing::info!(object = %receipt.object_id, version = %receipt.version, "committed metadata record");
            return Ok(Some(receipt));
        }
        Err(ChangeError::NothingToCommit) => {
            model.reset().await.map_err(change_error)?;
            return Ok(None);
        }
        Err(ChangeError::StagingNotCleared { receipt, reason }) => {
            tracing::warn!(object = %receipt.object_id, version = %receipt.version, %reason, "committed metadata record; staging area kept");
            return Ok(Some(*receipt));
        }
        Err(err) => err,
    };
    Err(match err {
        ChangeError::Conflict { .. } | ChangeError::TooManyRebases { .. } => StoreError::Conflict {
            reason: err.to_string(),
        },
        other => change_error(other),
    })
}

/// The version `file` pins, or the object's head.
fn pinned_or_head<B: StorageBackend>(
    object: &Object<'_, B>,
    file: &FileRef,
) -> Result<VersionNum, StoreError> {
    match file.version() {
        Some(name) => VersionNum::parse(name).map_err(backend),
        None => Ok(object.head().clone()),
    }
}

/// Read the logical file `path` of `version` whole, refusing more than
/// `limit` bytes.
async fn read_logical<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    path: &str,
    limit: u64,
) -> Result<Vec<u8>, StoreError> {
    let mut bytes = Vec::new();
    for_each_chunk(object, version, path, limit, |chunk| {
        bytes.extend_from_slice(chunk);
    })
    .await?;
    Ok(bytes)
}

/// Stream the logical file `path` of `version` in chunks of at most
/// [`MAX_CHUNK_BYTES`] into `sink`, refusing more than `limit` bytes. The
/// reader checks the bytes against the inventory digest.
async fn for_each_chunk<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    path: &str,
    limit: u64,
    mut sink: impl FnMut(&[u8]),
) -> Result<(), StoreError> {
    let mut reader = object
        .stream_file(version, path, MAX_CHUNK_BYTES)
        .map_err(backend)?;
    let mut total: u64 = 0;
    let max_chunks = limit / MAX_CHUNK_BYTES + 2;
    for _ in 0..max_chunks {
        let Some(chunk) = reader.next_chunk().await.map_err(backend)? else {
            return Ok(());
        };
        total += chunk.len() as u64;
        if total > limit {
            return Err(too_large(path, limit));
        }
        sink(&chunk);
    }
    Err(too_large(path, limit))
}

/// The error for a logical file larger than `limit` bytes.
fn too_large(path: &str, limit: u64) -> StoreError {
    StoreError::Backend(Box::new(IoError::new(
        ErrorKind::InvalidData,
        format!("{path} is larger than {limit} bytes"),
    )))
}

/// Wrap a read or format failure as a backend error.
fn backend(err: impl std::error::Error + Send + Sync + 'static) -> StoreError {
    StoreError::Backend(Box::new(err))
}

/// Map a change-model failure onto the store's error type, keeping the
/// [`ChangeError`] as the source so a host can tell the refusals apart.
fn change_error(err: ChangeError) -> StoreError {
    StoreError::Backend(Box::new(err))
}

#[cfg(all(feature = "ocfl-fs", not(target_arch = "wasm32")))]
pub use native::{LocalObject, MAX_READ_BYTES};

/// One object directory on the local disk (native only), as the CLI uses it.
#[cfg(all(feature = "ocfl-fs", not(target_arch = "wasm32")))]
mod native {
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    use filesquid_ocfl::read::Object;
    use filesquid_ocfl::storage::{complete_immediately, FsBackend, StorageBackend, StorageKey};
    use filesquid_ocfl::write::{LeaseConfig, LeaseStore, SystemClock};
    use filesquid_ocfl::{reject_git_in_ocfl_root, DigestAlgorithm, OcflError, VersionNum};

    use super::{backend as boxed, OcflStore};
    use crate::file_ref::FileRef;
    use crate::record::Record;
    use crate::store::{MetadataStore, StoreError, METADATA_MD};

    /// Largest logical file [`LocalObject::read_file`] reads: 256 MiB.
    pub const MAX_READ_BYTES: u64 = 256 * 1024 * 1024;

    /// The work directory's name in the system temporary directory.
    const WORK_DIR: &str = "filesquid-metadata";

    /// The lease store a [`LocalObject`] uses.
    type Leases = LeaseStore<FsBackend, SystemClock>;

    /// The `ocfl` store over one object directory.
    type DirStore<'s> =
        OcflStore<'s, FsBackend, Leases, FsBackend, SystemClock, fn(&str) -> String>;

    /// The object root inside the object directory's backend: its root.
    fn directory_root(_object_id: &str) -> String {
        String::new()
    }

    /// An OCFL object directory, with the reservations and change staging
    /// its edits need in a work directory outside it
    /// (`filesquid-metadata` in the system temporary directory).
    pub struct LocalObject {
        /// The object directory as a storage backend; the object is its root.
        storage: FsBackend,
        /// Version reservations in the work directory.
        leases: Leases,
        /// Change staging in the work directory.
        work: FsBackend,
        /// This directory's staging prefix on `work`: `changes/<dir_key>`.
        staging_prefix: String,
        /// This handle's writer name, unique in the process, so two handles
        /// never share a staging area.
        holder: String,
        /// The object identifier.
        object_id: String,
    }

    impl LocalObject {
        /// Open the object in `object_dir`. An existing object keeps its
        /// identifier and `object_id`, when given, MUST match it; a directory
        /// without an object needs `object_id`, and the first save creates
        /// the object there (file I/O).
        ///
        /// # Errors
        ///
        /// [`StoreError::Git`] for a Git working tree,
        /// [`StoreError::WrongObject`], [`StoreError::MissingObjectId`],
        /// [`StoreError::InvalidPath`] when the work directory lies inside
        /// the object directory, or a backend failure.
        pub fn open(object_dir: &Path, object_id: Option<&str>) -> Result<Self, StoreError> {
            reject_git_in_ocfl_root(object_dir).map_err(git_error)?;
            let storage = FsBackend::with_default_staging(object_dir).map_err(boxed)?;
            let work_dir = std::env::temp_dir().join(WORK_DIR);
            let work = FsBackend::with_default_staging(&work_dir).map_err(boxed)?;
            if work.root().starts_with(storage.root()) {
                return Err(StoreError::InvalidPath);
            }
            let dir_key =
                DigestAlgorithm::Sha256.digest_hex(storage.root().to_string_lossy().as_bytes());
            let leases = open_leases(&work_dir, &dir_key)?;
            let object_id = resolve_id(&storage, object_id)?;
            Ok(Self {
                storage,
                leases,
                work,
                staging_prefix: format!("changes/{dir_key}"),
                holder: next_holder(),
                object_id,
            })
        }

        /// The object identifier.
        pub fn object_id(&self) -> &str {
            &self.object_id
        }

        /// The record of the head (empty before the first edit).
        ///
        /// # Errors
        ///
        /// As [`OcflStore::read_record`].
        pub fn load(&self) -> Result<Record, StoreError> {
            let file = self.file(METADATA_MD)?;
            complete_immediately(self.store().load(&file)).map_err(boxed)?
        }

        /// The head and its record, for an edit to be saved with
        /// [`save_over`](Self::save_over) (`None` and an empty record before
        /// the first edit).
        ///
        /// # Errors
        ///
        /// As [`OcflStore::head`] and [`OcflStore::read_record`].
        pub fn load_head(&self) -> Result<(Option<VersionNum>, Record), StoreError> {
            let head =
                complete_immediately(self.store().head(&self.object_id)).map_err(boxed)??;
            let file = self.pinned(METADATA_MD, head.as_ref())?;
            let record = complete_immediately(self.store().load(&file)).map_err(boxed)??;
            Ok((head, record))
        }

        /// Commit `record` as one new version; `None` when unchanged.
        ///
        /// # Errors
        ///
        /// As [`OcflStore::commit_record`].
        pub fn save(&self, record: &Record) -> Result<Option<VersionNum>, StoreError> {
            self.save_over(None, record)
        }

        /// Commit `record`, edited from version `base`, as one new version;
        /// [`StoreError::Conflict`] when another writer moved the head past
        /// `base` and changed `metadata.md`. `None` for `base` pins nothing.
        ///
        /// # Errors
        ///
        /// As [`OcflStore::commit_record`].
        pub fn save_over(
            &self,
            base: Option<&VersionNum>,
            record: &Record,
        ) -> Result<Option<VersionNum>, StoreError> {
            let file = self.pinned(METADATA_MD, base)?;
            complete_immediately(self.store().commit_record(&file, record)).map_err(boxed)?
        }

        /// The bytes of the logical file `logical_path` of `version` (the
        /// head when `None`), at most [`MAX_READ_BYTES`].
        ///
        /// # Errors
        ///
        /// As [`OcflStore::read_file`].
        pub fn read_file(
            &self,
            logical_path: &str,
            version: Option<&VersionNum>,
        ) -> Result<Vec<u8>, StoreError> {
            let file = self.pinned(logical_path, version)?;
            complete_immediately(self.store().read_file(&file, MAX_READ_BYTES)).map_err(boxed)?
        }

        /// The hex SHA-256 of the logical file `logical_path` of `version`
        /// (the head when `None`), so a JSON sidecar binds the record and the
        /// file of one version.
        ///
        /// # Errors
        ///
        /// As [`OcflStore::file_sha256`].
        pub fn file_sha256(
            &self,
            logical_path: &str,
            version: Option<&VersionNum>,
        ) -> Result<String, StoreError> {
            let file = self.pinned(logical_path, version)?;
            complete_immediately(self.store().file_sha256(&file)).map_err(boxed)?
        }

        /// A reference to `logical_path` in this object.
        fn file(&self, logical_path: &str) -> Result<FileRef, StoreError> {
            FileRef::new(&self.object_id, logical_path).map_err(|_| StoreError::InvalidPath)
        }

        /// A reference to `logical_path` in this object, pinned to `version`.
        fn pinned(
            &self,
            logical_path: &str,
            version: Option<&VersionNum>,
        ) -> Result<FileRef, StoreError> {
            let file = self.file(logical_path)?;
            Ok(match version {
                Some(version) => file.with_version(&version.to_string()),
                None => file,
            })
        }

        /// The `ocfl` store over this directory, writing as this process.
        fn store(&self) -> DirStore<'_> {
            OcflStore {
                storage: &self.storage,
                reservations: &self.leases,
                staging: &self.work,
                staging_prefix: self.staging_prefix.clone(),
                holder: self.holder.clone(),
                clock: SystemClock,
                object_root: directory_root,
            }
        }
    }

    /// A writer name unique to this handle: the process id and a
    /// process-wide sequence number.
    fn next_holder() -> String {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        format!("fsquid-metadata-{}-{n}", std::process::id())
    }

    /// The lease store of one object directory: `work_dir/leases/<dir_key>`,
    /// where `dir_key` is the sha256 of the resolved directory, so every
    /// writer of one directory shares its reservations and other directories
    /// never wait on them (file I/O).
    fn open_leases(work_dir: &Path, dir_key: &str) -> Result<Leases, StoreError> {
        let prefix = format!("leases/{dir_key}");
        let backend = FsBackend::with_default_staging(work_dir).map_err(boxed)?;
        LeaseStore::new(backend, &prefix, SystemClock, LeaseConfig::default()).map_err(boxed)
    }

    /// The identifier of the object in `storage`, checked against `named`;
    /// `named` itself when the directory holds no object yet (file I/O).
    fn resolve_id(storage: &FsBackend, named: Option<&str>) -> Result<String, StoreError> {
        let inventory = StorageKey::new("inventory.json").map_err(boxed)?;
        let exists = complete_immediately(storage.exists(&inventory))
            .map_err(boxed)?
            .map_err(boxed)?;
        if !exists {
            return named.map(str::to_string).ok_or(StoreError::MissingObjectId);
        }
        let object = complete_immediately(Object::open(storage, ""))
            .map_err(boxed)?
            .map_err(boxed)?;
        match named {
            Some(id) if id != object.id() => Err(StoreError::WrongObject {
                expected: id.to_string(),
                found: object.id().to_string(),
            }),
            _ => Ok(object.id().to_string()),
        }
    }

    /// Map a Git-guard refusal onto [`StoreError::Git`].
    fn git_error(err: OcflError) -> StoreError {
        match err {
            OcflError::GitDirectoryForbidden { .. } => StoreError::Git,
            other => boxed(other),
        }
    }
}
