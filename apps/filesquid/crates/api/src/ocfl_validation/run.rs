//! One validation pass over a library (API-S-022), generic over the host's storage backend.
//!
//! A run opens the storage root, indexes its objects by identifier (an object whose
//! inventory cannot be read, and every root after the first that declares an identifier
//! already indexed, is indexed as `root:<digest of its root>`), closes the findings
//! of objects that no longer exist, and then validates objects in identifier order. It
//! starts after the stored cursor, moves the cursor past each object it checks, and wraps to
//! the start of the order when it reaches the end. It stops at its object cap, when it is
//! cancelled, on a store or storage outage, or once every object has been checked in this
//! run. An object whose root has gone since it was indexed has its findings closed instead
//! of validated. Each object's findings replace its previous ones in the store, under the library's
//! object lock, so a run and a takeover check never interleave on one object. Every newly
//! opened error finding is journaled as `ocfl.validation.finding`, and every closed finding
//! as `ocfl.validation.finding_closed`, with the opaque object ID, version and code only, and
//! the `<object-id>@vN` object reference when the version is known.
//! Diagnostics on stdout name error codes only, never a storage key or location.

use super::error::ValidationError;
use super::store::{Finding, FindingChange, FindingsStore, RunOutcome, Severity};
use super::{FINDING_CLOSED_OPERATION, FINDING_OPERATION, LOG_TARGET};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::journal_writer::JournalSender;
use crate::registry::ProcessHandle;
use filesquid_core::LibraryId;
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::{StorageBackend, StorageError};
use filesquid_ocfl::{validate_object, ReadError};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};

/// What a run or a takeover check works with.
pub(super) struct RunContext<'a> {
    pub(super) store: &'a dyn FindingsStore,
    pub(super) journal: &'a JournalSender,
    pub(super) library_id: &'a LibraryId,
    pub(super) actor: &'a str,
    pub(super) cancel: &'a AtomicBool,
    pub(super) process: Option<&'a ProcessHandle>,
    /// Held while one object is validated and its findings stored.
    pub(super) objects: &'a tokio::sync::Mutex<()>,
}

/// One object of the library, as indexed.
pub(super) struct Indexed {
    pub(super) id: String,
    root: String,
    version: Option<String>,
}

impl Indexed {
    /// An object at `root` that cannot be opened, under its stand-in identifier.
    pub(super) fn unreadable(root: &str) -> Self {
        Self {
            id: unreadable_id(root),
            root: root.to_string(),
            version: None,
        }
    }

    /// The opened object at `root`.
    pub(super) fn of<B: StorageBackend>(object: &Object<'_, B>, root: &str) -> Self {
        Self {
            id: object.id().to_string(),
            root: root.to_string(),
            version: Some(object.head().to_string()),
        }
    }
}

/// What a pass did. A pass that stopped on an outage carries its `error`, with the objects
/// it had already checked.
pub(super) struct PassResult {
    pub(super) outcome: RunOutcome,
    pub(super) checked: u64,
    pub(super) total: u64,
    pub(super) changed: bool,
    pub(super) error: Option<ValidationError>,
}

impl PassResult {
    /// A pass that failed before it checked anything.
    pub(super) fn failed(error: ValidationError) -> Self {
        Self {
            outcome: RunOutcome::Failed,
            checked: 0,
            total: 0,
            changed: false,
            error: Some(error),
        }
    }
}

/// Whether `err` is a storage outage rather than a broken object.
pub(super) fn is_outage(err: &ReadError) -> bool {
    matches!(
        err,
        ReadError::Storage(StorageError::Io { .. } | StorageError::Backend { .. })
    )
}

/// Map a failure to read the library: a storage outage is `unavailable`, anything else
/// `internal`. Only the error code reaches stdout.
pub(super) fn read_error(err: &ReadError) -> ValidationError {
    let code = err.code().unwrap_or("read_failed");
    tracing::info!(target: LOG_TARGET, error_code = code, "validation could not read the library");
    if is_outage(err) {
        ValidationError::Unavailable
    } else {
        ValidationError::Internal
    }
}

/// Every object of the storage root of `backend`, sorted by identifier (then root), each
/// under a distinct key. An object that cannot be opened is indexed by its stand-in
/// identifier and validated like the others, so its findings say why; so is every root after
/// the first that declares the same identifier, so two roots never share findings.
///
/// # Errors
///
/// `unavailable` or `internal` when the storage root cannot be opened, and `unavailable`
/// when the storage fails while an object is opened.
pub(super) async fn index<B: StorageBackend>(backend: &B) -> Result<Vec<Indexed>, ValidationError> {
    let storage = StorageRoot::open(backend)
        .await
        .map_err(|e| read_error(&e))?;
    let mut objects = Vec::with_capacity(storage.objects().len());
    for root in storage.objects() {
        let indexed = match Object::open(backend, root).await {
            Ok(object) => Indexed::of(&object, root),
            Err(err) if is_outage(&err) => return Err(read_error(&err)),
            Err(_) => Indexed::unreadable(root),
        };
        objects.push(indexed);
    }
    objects.sort_by(|a, b| (&a.id, &a.root).cmp(&(&b.id, &b.root)));
    Ok(distinct(objects))
}

/// `objects` (sorted by identifier, then root) with every repeated identifier after the
/// first replaced by its root's stand-in, sorted again. Pure.
fn distinct(mut objects: Vec<Indexed>) -> Vec<Indexed> {
    let mut seen = BTreeSet::new();
    for object in &mut objects {
        if !seen.insert(object.id.clone()) {
            tracing::info!(target: LOG_TARGET, "two object roots declare one identifier");
            object.id = unreadable_id(&object.root);
        }
    }
    objects.sort_by(|a, b| (&a.id, &a.root).cmp(&(&b.id, &b.root)));
    objects
}

/// Whether anything is still stored under the object root `root`.
///
/// # Errors
///
/// `unavailable` when the storage fails.
async fn root_present<B: StorageBackend>(backend: &B, root: &str) -> Result<bool, ValidationError> {
    let page = backend
        .list(&format!("{root}/"), None, 1)
        .await
        .map_err(|e| read_error(&ReadError::Storage(e)))?;
    Ok(!page.keys.is_empty())
}

/// The opaque stand-in identifier of an object whose inventory cannot be read: `root:` and
/// the first 32 hex digits of the SHA-256 of its root. It names no path.
fn unreadable_id(root: &str) -> String {
    let digest = Sha256::digest(root.as_bytes());
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("root:{hex}")
}

/// The positions of `objects` a pass visits: after `cursor` (from the start when
/// `from_start` or no cursor), wrapping to the start, at most `cap` and each once.
fn order(objects: &[Indexed], cursor: Option<&str>, from_start: bool, cap: u64) -> Vec<usize> {
    let start = match (from_start, cursor) {
        (false, Some(after)) => objects.partition_point(|o| o.id.as_str() <= after),
        _ => 0,
    };
    let cap = usize::try_from(cap).unwrap_or(usize::MAX);
    (start..objects.len()).chain(0..start).take(cap).collect()
}

/// Progress of a pass so far.
struct Progress {
    checked: u64,
    total: u64,
    changed: bool,
}

impl Progress {
    /// The pass result with `outcome`, and `error` for a pass stopped by an outage.
    fn into_result(self, outcome: RunOutcome, error: Option<ValidationError>) -> PassResult {
        PassResult {
            outcome,
            checked: self.checked,
            total: self.total,
            changed: self.changed,
            error,
        }
    }
}

/// Run one pass over `objects` of `backend`: close the findings of deleted objects, then
/// check objects in [`order`] until the cap, a cancel, an outage, or the end of the pass.
/// The cursor stays at the last object checked.
pub(super) async fn pass<B: StorageBackend>(
    ctx: &RunContext<'_>,
    backend: &B,
    objects: &[Indexed],
    from_start: bool,
    cap: u64,
) -> PassResult {
    let mut progress = Progress {
        checked: 0,
        total: objects.len() as u64,
        changed: false,
    };
    let visited = visit(ctx, backend, objects, from_start, cap, &mut progress).await;
    let outcome = match &visited {
        Err(_) => RunOutcome::Failed,
        Ok(true) => RunOutcome::Cancelled,
        Ok(false) if progress.checked < progress.total => RunOutcome::Capped,
        Ok(false) => RunOutcome::Completed,
    };
    progress.into_result(outcome, visited.err())
}

/// The body of [`pass`]: answers whether the pass was cancelled.
async fn visit<B: StorageBackend>(
    ctx: &RunContext<'_>,
    backend: &B,
    objects: &[Indexed],
    from_start: bool,
    cap: u64,
    progress: &mut Progress,
) -> Result<bool, ValidationError> {
    progress.changed |= close_deleted(ctx, objects)?;
    let cursor = ctx.store.cursor(ctx.library_id)?;
    let positions = order(objects, cursor.as_deref(), from_start, cap);
    for position in &positions {
        if ctx.cancel.load(Ordering::SeqCst) {
            return Ok(true);
        }
        let object = &objects[*position];
        progress.changed |= !check_object(ctx, backend, object).await?.is_empty();
        ctx.store.set_cursor(ctx.library_id, Some(&object.id))?;
        progress.checked += 1;
        if let Some(process) = ctx.process {
            process.progress(progress.checked, positions.len() as u64);
        }
    }
    Ok(false)
}

/// Close the findings of every object that has open findings but is no longer in the
/// library, journaling each closure. Answers whether anything closed.
fn close_deleted(ctx: &RunContext<'_>, objects: &[Indexed]) -> Result<bool, ValidationError> {
    let present: BTreeSet<&str> = objects.iter().map(|o| o.id.as_str()).collect();
    let mut changed = false;
    for object_id in ctx.store.objects_with_findings(ctx.library_id)? {
        if present.contains(object_id.as_str()) {
            continue;
        }
        let change = ctx
            .store
            .replace_findings(ctx.library_id, &object_id, Vec::new())?;
        journal_change(ctx, &change);
        changed |= !change.is_empty();
    }
    Ok(changed)
}

/// Validate one object under the library's object lock, store its findings and journal what
/// opened and closed. An object whose root has gone since it was indexed is not validated:
/// its findings close, as for a deleted object. The lock is taken without waiting when it is
/// free, so an uncontended check never yields to the executor for it.
///
/// # Errors
///
/// `unavailable` (or `internal`) for a store or storage failure.
pub(super) async fn check_object<B: StorageBackend>(
    ctx: &RunContext<'_>,
    backend: &B,
    object: &Indexed,
) -> Result<FindingChange, ValidationError> {
    let _one_at_a_time = match ctx.objects.try_lock() {
        Ok(guard) => guard,
        Err(_) => ctx.objects.lock().await,
    };
    if !root_present(backend, &object.root).await? {
        let change = ctx
            .store
            .replace_findings(ctx.library_id, &object.id, Vec::new())?;
        journal_change(ctx, &change);
        return Ok(change);
    }
    let findings = match validate_object(backend, &object.root).await {
        Ok(report) => {
            let errors = report.errors().iter().map(|f| (f, Severity::Error));
            let warnings = report.warnings().iter().map(|f| (f, Severity::Warning));
            errors
                .chain(warnings)
                .map(|(f, severity)| finding(ctx, object, f.code, severity, &f.at))
                .collect()
        }
        Err(err) => unvalidated(ctx, object, &err)?,
    };
    let change = ctx
        .store
        .replace_findings(ctx.library_id, &object.id, findings)?;
    journal_change(ctx, &change);
    Ok(change)
}

/// The finding for an object the validator could not validate at all (an engine bound, an
/// unsupported OCFL version): one error finding with the read error's code. A storage outage
/// stops the run instead.
fn unvalidated(
    ctx: &RunContext<'_>,
    object: &Indexed,
    err: &ReadError,
) -> Result<Vec<Finding>, ValidationError> {
    if is_outage(err) {
        return Err(read_error(err));
    }
    let code = err.code().unwrap_or("unvalidated");
    tracing::info!(target: LOG_TARGET, error_code = code, "object could not be validated");
    Ok(vec![finding(
        ctx,
        object,
        code,
        Severity::Error,
        &object.root,
    )])
}

/// One finding of `object`.
fn finding(
    ctx: &RunContext<'_>,
    object: &Indexed,
    code: &str,
    severity: Severity,
    location: &str,
) -> Finding {
    Finding {
        library_id: ctx.library_id.clone(),
        object_id: object.id.clone(),
        version: object.version.clone(),
        code: code.to_string(),
        severity,
        location: location.to_string(),
    }
}

/// Journal every newly opened finding, errors and warnings alike, and every closed finding
/// (opaque object ID, version and code only). No finding is skipped, sampled or summarised
/// for volume: the bounded journal queue makes a large run wait instead (owner decision,
/// 2026-10-19). An error is journaled at level `warn`, a warning at level `info`; neither
/// changes health here.
fn journal_change(ctx: &RunContext<'_>, change: &FindingChange) {
    for found in &change.opened {
        let level = match found.severity {
            Severity::Error => Level::Warn,
            Severity::Warning => Level::Info,
        };
        ctx.journal
            .send(finding_entry(ctx, FINDING_OPERATION, found).level(level));
    }
    for closed in &change.closed {
        ctx.journal
            .send(finding_entry(ctx, FINDING_CLOSED_OPERATION, closed));
    }
}

/// The `<object-id>@vN` reference of the object version `found` is about, when it names a
/// real object at a known version: `None` for an object without a readable version, for a
/// root's stand-in identifier, which is not an OCFL object ID, and for a reference the journal
/// would not keep as opaque (it would store `redacted` instead). Pure.
fn object_ref(found: &Finding) -> Option<String> {
    let version = found.version.as_deref()?;
    if found.object_id.starts_with("root:") {
        return None;
    }
    filesquid_core::VersionRef::new(found.object_id.as_str(), version)
        .ok()
        .map(|reference| reference.object_ref())
        .filter(|object_ref| crate::journal::is_object_ref(object_ref))
}

/// The journal entry of `found` under `operation`.
fn finding_entry(ctx: &RunContext<'_>, operation: &str, found: &Finding) -> JournalEntry {
    let entry = JournalEntry::new(operation, Outcome::Info)
        .actor(ctx.actor)
        .target(ctx.library_id.as_str())
        .target(found.object_id.as_str())
        .error_code(found.code.as_str());
    let entry = match &found.version {
        Some(version) => entry.target(version.as_str()),
        None => entry,
    };
    let entry = match object_ref(found) {
        Some(object_ref) => entry.object_ref(object_ref),
        None => entry,
    };
    match ctx.process {
        Some(process) => entry.process(process.id()),
        None => entry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indexed(id: &str, root: &str) -> Indexed {
        Indexed {
            id: id.to_string(),
            root: root.to_string(),
            version: Some("v1".to_string()),
        }
    }

    /// Every root after the first that declares an identifier is keyed by its stand-in.
    #[test]
    fn repeated_identifiers_get_distinct_keys() {
        let objects = vec![
            indexed("urn:a", "obj-1"),
            indexed("urn:b", "obj-2"),
            indexed("urn:b", "obj-3"),
        ];
        let keys: Vec<(String, String)> = distinct(objects)
            .into_iter()
            .map(|o| (o.id, o.root))
            .collect();
        let stand_in = unreadable_id("obj-3");
        assert!(keys.contains(&("urn:a".into(), "obj-1".into())));
        assert!(keys.contains(&("urn:b".into(), "obj-2".into())));
        assert!(keys.contains(&(stand_in, "obj-3".into())));
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "sorted by key again");
    }
}
