//! One validation pass over a library (API-S-022), generic over the host's storage backend.
//!
//! A run opens the storage root, indexes its objects by identifier (an object whose
//! inventory cannot be read is indexed as `root:<digest of its root>`), closes the findings
//! of objects that no longer exist, and then validates objects in identifier order. It
//! starts after the stored cursor, moves the cursor past each object it checks, and wraps to
//! the start of the order when it reaches the end. It stops at its object cap, when it is
//! cancelled, or once every object has been checked in this run. Each object's findings
//! replace its previous ones in the store. Every newly opened error finding is journaled as
//! `ocfl.validation.finding`, and every closed finding as `ocfl.validation.finding_closed`,
//! with the opaque object ID, version and code only.

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

/// What a run works with.
pub(super) struct RunContext<'a> {
    pub(super) store: &'a dyn FindingsStore,
    pub(super) journal: &'a JournalSender,
    pub(super) library_id: &'a LibraryId,
    pub(super) actor: &'a str,
    pub(super) cancel: &'a AtomicBool,
    pub(super) process: Option<&'a ProcessHandle>,
}

/// One object of the library, as indexed.
pub(super) struct Indexed {
    pub(super) id: String,
    root: String,
    version: Option<String>,
}

/// What a pass did.
pub(super) struct PassResult {
    pub(super) outcome: RunOutcome,
    pub(super) checked: u64,
    pub(super) total: u64,
    pub(super) changed: bool,
}

/// Map a read failure: a storage outage is `unavailable`; any other failure to open the
/// storage root is `internal`.
fn read_error(err: &ReadError) -> ValidationError {
    tracing::info!(target: LOG_TARGET, error = %err, "validation could not read the library");
    match err {
        ReadError::Storage(StorageError::Io { .. } | StorageError::Backend { .. }) => {
            ValidationError::Unavailable
        }
        _ => ValidationError::Internal,
    }
}

/// Every object of the storage root of `backend`, sorted by identifier (then root). An
/// object that cannot be opened is indexed by its stand-in identifier and validated like the
/// others, so its findings say why.
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
            Ok(object) => Indexed {
                id: object.id().to_string(),
                root: root.clone(),
                version: Some(object.head().to_string()),
            },
            Err(ReadError::Storage(
                err @ (StorageError::Io { .. } | StorageError::Backend { .. }),
            )) => {
                return Err(read_error(&ReadError::Storage(err)));
            }
            Err(_) => Indexed {
                id: unreadable_id(root),
                root: root.clone(),
                version: None,
            },
        };
        objects.push(indexed);
    }
    objects.sort_by(|a, b| (&a.id, &a.root).cmp(&(&b.id, &b.root)));
    Ok(objects)
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

/// Run one pass over `objects` of `backend`: close the findings of deleted objects, then
/// check objects in [`order`] until the cap, a cancel, or the end of the pass.
///
/// # Errors
///
/// `unavailable` for a store or storage outage (the cursor stays at the last object
/// checked).
pub(super) async fn pass<B: StorageBackend>(
    ctx: &RunContext<'_>,
    backend: &B,
    objects: &[Indexed],
    from_start: bool,
    cap: u64,
) -> Result<PassResult, ValidationError> {
    let mut changed = close_deleted(ctx, objects)?;
    let cursor = ctx.store.cursor(ctx.library_id)?;
    let visit = order(objects, cursor.as_deref(), from_start, cap);
    let total = objects.len() as u64;
    let mut checked = 0u64;
    for position in &visit {
        if ctx.cancel.load(Ordering::SeqCst) {
            return Ok(result(RunOutcome::Cancelled, checked, total, changed));
        }
        let object = &objects[*position];
        changed |= !check_object(ctx, backend, object).await?.is_empty();
        ctx.store.set_cursor(ctx.library_id, Some(&object.id))?;
        checked += 1;
        if let Some(process) = ctx.process {
            process.progress(checked, visit.len() as u64);
        }
    }
    let outcome = if checked < total {
        RunOutcome::Capped
    } else {
        RunOutcome::Completed
    };
    Ok(result(outcome, checked, total, changed))
}

/// A pass result.
fn result(outcome: RunOutcome, checked: u64, total: u64, changed: bool) -> PassResult {
    PassResult {
        outcome,
        checked,
        total,
        changed,
    }
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

/// Validate one object, store its findings and journal what opened and closed.
///
/// # Errors
///
/// `unavailable` for a store or storage outage.
pub(super) async fn check_object<B: StorageBackend>(
    ctx: &RunContext<'_>,
    backend: &B,
    object: &Indexed,
) -> Result<FindingChange, ValidationError> {
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
    let findings = distinct(findings);
    let change = ctx
        .store
        .replace_findings(ctx.library_id, &object.id, findings)?;
    journal_change(ctx, &change);
    Ok(change)
}

/// `findings` without repeats: the validator can report one fault twice (for example an
/// unlisted file against the root and the version inventory), which is one finding.
fn distinct(findings: Vec<Finding>) -> Vec<Finding> {
    let mut kept: Vec<Finding> = Vec::with_capacity(findings.len());
    for found in findings {
        if !kept.iter().any(|k| k.same_as(&found)) {
            kept.push(found);
        }
    }
    kept
}

/// The finding for an object the validator could not validate at all (an engine bound, an
/// unsupported OCFL version): one error finding with the read error's code. A storage outage
/// stops the run instead.
fn unvalidated(
    ctx: &RunContext<'_>,
    object: &Indexed,
    err: &ReadError,
) -> Result<Vec<Finding>, ValidationError> {
    if let ReadError::Storage(StorageError::Io { .. } | StorageError::Backend { .. }) = err {
        return Err(read_error(err));
    }
    tracing::info!(target: LOG_TARGET, error = %err, "object could not be validated");
    let code = err.code().unwrap_or("unvalidated");
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

/// Journal every newly opened error finding and every closed finding (opaque object ID,
/// version and code only).
fn journal_change(ctx: &RunContext<'_>, change: &FindingChange) {
    let opened = change
        .opened
        .iter()
        .filter(|f| f.severity == Severity::Error);
    for found in opened {
        ctx.journal
            .send(finding_entry(ctx, FINDING_OPERATION, found).level(Level::Warn));
    }
    for closed in &change.closed {
        ctx.journal
            .send(finding_entry(ctx, FINDING_CLOSED_OPERATION, closed));
    }
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
    match ctx.process {
        Some(process) => entry.process(process.id()),
        None => entry,
    }
}
