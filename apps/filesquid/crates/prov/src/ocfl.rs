//! Records in OCFL objects (the optional `ocfl` feature; PROV-S-006,
//! OCFL-S-024; [ADR 040](../../../../../resources/architecture/040-prov-o-history.md)
//! Decisions 3 and 7).
//!
//! The record of a version is the logical file `provenance/<version>.jsonld`
//! of the same object, written in the commit it describes. [`RecordHook`] is
//! the change model's version-bound file function that writes it: it is
//! called after `vN` is reserved, so the record names the final version and
//! its inventory `created` time, and again after a rebase. [`read_record`],
//! [`list_records`] and [`missing_records`] read records back;
//! [`reconstruct`] and [`stage_reconstructed`] derive records for versions
//! committed before the history existed, marked `reconstructed`, inventing
//! no agent. Nothing here writes outside the change model.

use std::collections::{BTreeMap, BTreeSet};

use filesquid_ocfl::change::{ChangeError, ChangeModel};
use filesquid_ocfl::inventory::{DigestMap, Inventory};
use filesquid_ocfl::read::{Object, ReadError};
use filesquid_ocfl::storage::{StorageBackend, MAX_CHUNK_BYTES};
use filesquid_ocfl::write::{BoundContext, VersionBoundFiles, VersionReservations};
use filesquid_ocfl::VersionNum;
use thiserror::Error;

use crate::builder::{build_record, OcflUser, Reconstruction, VersionFacts, RECORD_PREFIX};
use crate::error::{BuildError, ProvError};
use crate::model::{HostFacts, Record};
use crate::profile::{parse_record, to_json_ld, MAX_RECORD_BYTES};
use crate::scalar::{check_identifier, check_kind};

/// File-name suffix of a record.
pub const RECORD_SUFFIX: &str = ".jsonld";

/// Prefix of the structured FileSquid version message.
const OPERATION_PREFIX: &str = "filesquid.operation.v1 ";

/// Operation ID of a reconstructed version whose message names none.
pub const UNRECORDED_OPERATION: &str = "unrecorded";

/// Kind of a reconstructed version whose message names none.
pub const UNKNOWN_KIND: &str = "unknown";

/// Why a record could not be read, built or staged.
#[derive(Debug, Error)]
pub enum RecordError {
    /// The object or a version could not be read.
    #[error(transparent)]
    Read(#[from] ReadError),
    /// A stored record breaks the profile.
    #[error(transparent)]
    Profile(#[from] ProvError),
    /// The facts of a version do not make a record.
    #[error(transparent)]
    Build(#[from] BuildError),
    /// The change model refused to stage a record.
    #[error(transparent)]
    Change(#[from] ChangeError),
    /// A stored record is larger than the profile allows.
    #[error("PROV_TOO_LARGE: a stored record exceeds the profile's size bound")]
    TooLarge,
}

/// The logical path of the record of `version`: `provenance/<version>.jsonld`.
pub fn record_path(version: &str) -> String {
    format!("{RECORD_PREFIX}{version}{RECORD_SUFFIX}")
}

/// What the caller knows about the operation a commit belongs to; the hook
/// adds the version facts the change model reports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperationFacts {
    /// The operation ID (`fsq:operation`).
    pub operation_id: String,
    /// The operation kind (`fsq:kind`).
    pub kind: String,
    /// When the operation started (RFC 3339 UTC).
    pub started: String,
    /// The installation UUID (v4), when known.
    pub installation: Option<String>,
    /// The FileSquid host, when known.
    pub host: Option<HostFacts>,
    /// The commit's journal entry IDs, assigned before the commit.
    pub journal_entries: Vec<String>,
    /// Moves in this commit: old logical path to new logical path.
    pub moves: BTreeMap<String, String>,
    /// The OCFL version `user` the commit sets.
    pub user: Option<OcflUser>,
    /// The version message the commit sets.
    pub message: Option<String>,
}

/// The version facts of the commit `context` describes, for `operation`.
pub fn version_facts(context: &BoundContext<'_>, operation: &OperationFacts) -> VersionFacts {
    VersionFacts {
        object_id: context.object_id.to_string(),
        version: context.version.to_string(),
        base_version: context.base.map(ToString::to_string),
        created: context.created.to_string(),
        user: operation.user.clone(),
        message: operation.message.clone(),
        base_state: context.base_state.clone(),
        new_state: context.state.clone(),
        moves: operation.moves.clone(),
        operation_id: operation.operation_id.clone(),
        kind: operation.kind.clone(),
        started: operation.started.clone(),
        installation: operation.installation.clone(),
        host: operation.host.clone(),
        journal_entries: operation.journal_entries.clone(),
        reconstruction: None,
    }
}

/// The record of the commit `context` describes, as its logical path and
/// its JSON-LD bytes.
///
/// # Errors
///
/// [`BuildError`] when the facts do not make a record.
pub fn bound_record(
    context: &BoundContext<'_>,
    operation: &OperationFacts,
) -> Result<(String, Vec<u8>), BuildError> {
    let record = build_record(&version_facts(context, operation))?;
    let path = record_path(&context.version.to_string());
    Ok((path, to_json_ld(&record).into_bytes()))
}

/// The change model's version-bound file function that writes the record
/// of each commit it is handed to (OCFL-S-024).
#[derive(Debug, Clone)]
pub struct RecordHook {
    operation: OperationFacts,
}

impl RecordHook {
    /// A hook that records commits of `operation`.
    pub fn new(operation: OperationFacts) -> Self {
        Self { operation }
    }
}

impl VersionBoundFiles for RecordHook {
    fn files(&self, context: &BoundContext<'_>) -> Result<BTreeMap<String, Vec<u8>>, String> {
        let (path, bytes) = bound_record(context, &self.operation).map_err(|e| e.to_string())?;
        Ok(BTreeMap::from([(path, bytes)]))
    }
}

/// The record of `version` of `object`, or `None` when it has none. A
/// record stays in every later state, and a reconstructed one first appears
/// in the version that backfilled it, so the head is read first and the
/// version itself second. Read through the engine, bounded and checked
/// against its digest.
///
/// # Errors
///
/// [`RecordError`] for a version the object lacks, a read failure, or a
/// record that breaks the profile.
pub async fn read_record<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
) -> Result<Option<Record>, RecordError> {
    let path = record_path(&version.to_string());
    // The version is checked first, so a record path in the head never
    // answers for a version the object has not committed.
    let in_version = object.state(version)?.get(&path).is_some();
    let in_head = object.state(object.head())?.get(&path).is_some();
    let holder = match (in_head, in_version) {
        (true, _) => object.head().clone(),
        (false, true) => version.clone(),
        (false, false) => return Ok(None),
    };
    let mut reader = object.stream_file(&holder, &path, MAX_CHUNK_BYTES)?;
    let mut bytes = Vec::new();
    let max_chunks = MAX_RECORD_BYTES as u64 / MAX_CHUNK_BYTES + 2;
    for _ in 0..max_chunks {
        let Some(chunk) = reader.next_chunk().await? else {
            return Ok(Some(parse_record(&bytes)?));
        };
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(RecordError::TooLarge);
        }
    }
    Err(RecordError::TooLarge)
}

/// The versions that have a record [`read_record`] can read, in version
/// order: its record is in the head or in the version's own state (a record
/// a later revert removed from the head still describes its version).
pub fn list_records<B: StorageBackend>(object: &Object<'_, B>) -> Vec<VersionNum> {
    let inventory = object.inventory();
    let in_head = recorded_versions(inventory);
    inventory
        .versions
        .iter()
        .filter(|(version, block)| {
            in_head.contains(*version) || holds_own_record(version, &block.state)
        })
        .map(|(version, _)| version.clone())
        .collect()
}

/// The versions of the object that have no record, in version order: what a
/// reconstruction would derive. A version counts as recorded when its record
/// is in the head or in its own state, as [`read_record`] reads it, so a
/// record a later revert removed from the head is never reconstructed over.
pub fn missing_records<B: StorageBackend>(object: &Object<'_, B>) -> Vec<VersionNum> {
    let inventory = object.inventory();
    let recorded = recorded_versions(inventory);
    inventory
        .versions
        .iter()
        .filter(|(version, _)| !recorded.contains(*version))
        .filter(|(version, block)| !holds_own_record(version, &block.state))
        .map(|(version, _)| version.clone())
        .collect()
}

/// Whether a version's own `state` holds the record of `version`.
fn holds_own_record(version: &VersionNum, state: &DigestMap) -> bool {
    let path = record_path(&version.to_string());
    state.values().flatten().any(|held| *held == path)
}

/// The versions named by record paths in the head's state, as an ordered
/// set (so a membership test is logarithmic, not linear).
fn recorded_versions(inventory: &Inventory) -> BTreeSet<VersionNum> {
    let Some(head) = inventory.versions.get(&inventory.head) else {
        return BTreeSet::new();
    };
    head.state
        .values()
        .flatten()
        .filter_map(|path| path.strip_prefix(RECORD_PREFIX))
        .filter_map(|name| name.strip_suffix(RECORD_SUFFIX))
        .filter_map(|name| VersionNum::parse(name).ok())
        .filter(|version| inventory.versions.contains_key(version))
        .collect()
}

/// Derive the record of `version` from `inventory` alone: `created` as the
/// end (and start) time, the `user` address as the installation agent when
/// it is one, the operation ID and kind from a `filesquid.operation.v1`
/// message (otherwise [`UNRECORDED_OPERATION`] and [`UNKNOWN_KIND`]), and
/// the entities from the state diff with the previous version. Marked
/// `reconstructed`, recorded at `recorded_at`; no agent is invented.
///
/// # Errors
///
/// [`RecordError::Read`] for a version the inventory lacks;
/// [`RecordError::Build`] when its facts do not make a record.
pub fn reconstruct(
    inventory: &Inventory,
    version: &VersionNum,
    recorded_at: &str,
) -> Result<Record, RecordError> {
    let block = inventory
        .versions
        .get(version)
        .ok_or_else(|| ReadError::VersionNotFound {
            version: version.to_string(),
        })?;
    let base = inventory
        .versions
        .range(..version.clone())
        .next_back()
        .map(|(name, block)| (name.clone(), flatten(&block.state)));
    let (operation_id, kind) = operation_of(block.message.as_deref());
    let facts = VersionFacts {
        object_id: inventory.id.clone(),
        version: version.to_string(),
        base_version: base.as_ref().map(|(name, _)| name.to_string()),
        created: block.created.clone(),
        user: block.user.as_ref().map(|user| OcflUser {
            name: user.name.clone(),
            address: user.address.clone(),
        }),
        message: block.message.clone(),
        base_state: base.map(|(_, state)| state).unwrap_or_default(),
        new_state: flatten(&block.state),
        operation_id,
        kind,
        started: block.created.clone(),
        reconstruction: Some(Reconstruction {
            recorded_at: recorded_at.to_string(),
        }),
        ..VersionFacts::default()
    };
    Ok(build_record(&facts)?)
}

/// Stage the reconstructed records of at most `max` versions of `object`
/// that have none, oldest first, through `model`, and return those
/// versions. The caller then commits them in one version (with the record
/// of the reconstruction run as its own record). Versions that already have
/// a record are skipped, so a second run stages nothing.
///
/// # Errors
///
/// [`RecordError::Change`] with [`ChangeError::WrongObject`] when `object`
/// is not the object `model` commits to (nothing is staged);
/// [`RecordError`] when a record cannot be derived or staged.
pub async fn stage_reconstructed<B, R, S, O>(
    model: &ChangeModel<'_, B, R, S>,
    object: &Object<'_, O>,
    recorded_at: &str,
    max: usize,
) -> Result<Vec<VersionNum>, RecordError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
    O: StorageBackend,
{
    check_same_object(model, object)?;
    let missing: Vec<VersionNum> = missing_records(object).into_iter().take(max).collect();
    for version in &missing {
        let record = reconstruct(object.inventory(), version, recorded_at)?;
        let path = record_path(&version.to_string());
        model
            .stage_bytes(&path, to_json_ld(&record).as_bytes())
            .await?;
    }
    Ok(missing)
}

/// Refuse an `object` that is not the object `model` commits to, so records
/// of one object are never staged into another.
fn check_same_object<B, R, S, O>(
    model: &ChangeModel<'_, B, R, S>,
    object: &Object<'_, O>,
) -> Result<(), ChangeError>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
    O: StorageBackend,
{
    let found = &object.inventory().id;
    if found == model.object_id() {
        return Ok(());
    }
    Err(ChangeError::WrongObject {
        expected: model.object_id().to_string(),
        found: found.clone(),
    })
}

/// A digest → paths state as path → digest.
fn flatten(state: &DigestMap) -> BTreeMap<String, String> {
    state
        .iter()
        .flat_map(|(digest, paths)| paths.iter().map(move |path| (path.clone(), digest.clone())))
        .collect()
}

/// The operation ID and kind a version message names. Each falls back on
/// its own placeholder ([`UNRECORDED_OPERATION`], [`UNKNOWN_KIND`]) when the
/// message names none, cannot be read, or names one the profile refuses. A
/// catalog message without `kind` (written before kinds were recorded)
/// keeps its operation ID; other members (`revision`, `created_id`,
/// `deletion`, `label`) are ignored.
fn operation_of(message: Option<&str>) -> (String, String) {
    let value = message
        .and_then(|text| text.strip_prefix(OPERATION_PREFIX))
        .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        .unwrap_or_default();
    let field = |name: &str| value.get(name).and_then(serde_json::Value::as_str);
    let operation_id = field("operation_id")
        .filter(|id| check_identifier(id))
        .unwrap_or(UNRECORDED_OPERATION);
    let kind = field("kind")
        .filter(|kind| check_kind(kind))
        .unwrap_or(UNKNOWN_KIND);
    (operation_id.to_string(), kind.to_string())
}
