//! The import job (Task DE-1.2; F-002, NFR-003, DE-S-014): the files of a chosen folder (or one
//! chosen file) imported one by one as a durable job of `filesquid-api`'s [`Jobs`], resumable
//! after a quit or a crash.
//!
//! - The first run lists the sources once in one stable order (bounded walk; links to folders
//!   are not followed) and keeps that listing as the job's manifest, each file with the
//!   identifier minted for its new file object. The checkpoint is the index of the next
//!   manifest entry, so a resumed run continues the same files whatever changed on disk, and
//!   a file committed just before a crash is found, not imported twice.
//! - Nothing that cannot be listed is dropped: a source, folder or entry that cannot be read,
//!   a folder deeper than [`MAX_WALK_DEPTH`], a link to a folder or a special file is kept in
//!   the manifest with its error code and quarantined in turn. A selection of more than
//!   [`MAX_IMPORT_FILES`] entries fails the job (`too_many_files`) before any file is
//!   imported.
//! - Each file gets at most [`MAX_ATTEMPTS`] attempts (NFR-003), with exponential backoff
//!   capped at [`MAX_BACKOFF`] plus jitter; a file that still fails, or fails for good, is
//!   quarantined: journaled with its error code and left, so one bad file never restarts the
//!   batch.
//! - Before each file the destination must keep the reserve max(2 GiB, 5 % of its capacity)
//!   free after the file, and no file may exceed the staging cap of [`STAGING_CAP_BYTES`];
//!   when space runs short the job is suspended with its checkpoint, to resume later.
//! - Setting the stop flag (quit) suspends the job between files with its checkpoint.
//! - A pause or cancel asked through the job routes (API-S-037, Task DE-1.9) is read after
//!   every per-file checkpoint and every lease renewal between a file's attempts; the job then
//!   halts only through the API's atomic [`RunningJob::halt_if_requested`], so a pause leaves
//!   it pending with its checkpoint after the file in hand, a cancel ends it cancelled with no
//!   further file read, and a resume that cleared the pause first keeps the run going. The API
//!   journals each control and the halt; the job journals none of them.
//!
//! - Every import names its destination collection and folder (ADR 041 "Must choose"): a
//!   plan without one fails its job (`destination_required`), as does a collection the library
//!   no longer holds (`collection_not_found`). The manifest gives each file its collection
//!   logical path ([`super::placing`]); a picked source whose path is taken is numbered beside
//!   it, and one that cannot be placed quarantines its files with its code before any byte is
//!   read. Each committed file is then registered in its home collection at that path, in the
//!   library's index and its snapshot, under the import's operation ID. The import's intent is
//!   recorded before its first file object and cleared when the job ends, so a crash between a
//!   file's object and its registration leaves the operation pending, and the resumed run
//!   answers the object and registers it once (DE-S-010).
//!
//! Every file's outcome is journaled as [`IMPORT_FILE_OPERATION`] with opaque IDs and an error
//! code only, never a path or a file name.

use super::file::{import_file, FileImport, ImportFailure};
use super::host::Build;
use super::index::ImportIndex;
use super::placing::{logical_path, source_roots};
use super::plans::{ImportPlan, ManifestEntry, SqliteImportPlans};
use crate::catalog::{PendingIntent, RegisterFile, Registration, REGISTER_OPERATION};
use crate::HostFiles;
use filesquid_api::files::UploadView;
use filesquid_api::jobs::{Halt, JobError, JobSpec, JobState, Jobs, RunningJob};
use filesquid_api::journal::{JournalEntry, Level, Outcome};
use filesquid_api::journal_writer::JournalSender;
use filesquid_api::library::mint_uuid_v4;
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_core::{CollectionId, FileId, LibraryId};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The job kind of an import.
pub const IMPORT_KIND: &str = "import";

/// Journal operation of one imported (or quarantined) file.
pub const IMPORT_FILE_OPERATION: &str = "import.file";

/// Most files one import lists.
pub const MAX_IMPORT_FILES: usize = 100_000;

/// Deepest folder level an import walks.
pub const MAX_WALK_DEPTH: usize = 64;

/// Attempts per file before quarantine, the first included (NFR-003).
pub const MAX_ATTEMPTS: u32 = 5;

/// Longest wait between two attempts, before jitter (NFR-003).
pub const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// The staging cap: the largest file one import stages (NFR-003, 10 GiB).
pub const STAGING_CAP_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// The smallest free-space reserve kept on the destination (NFR-003, 2 GiB).
pub const MIN_RESERVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Most halts one run lets a resume undo before the file in hand is done (AGENTS.md §22); a
/// run past it suspends with its checkpoint, to go on at the next resume or launch.
pub const MAX_CONTINUED_HALTS: usize = 1_000;

/// What one run of an import job did.
#[derive(Clone, Debug)]
pub struct ImportRun {
    /// The job.
    pub job_id: String,
    /// Where the job stands now: completed, or pending (suspended to resume).
    pub state: JobState,
    /// The files imported in this run.
    pub imported: Vec<UploadView>,
    /// The entries quarantined in this run, by error code.
    pub quarantined: Vec<String>,
    /// The attempts in this run that were retries of a file (at most
    /// [`MAX_ATTEMPTS`] - 1 per file).
    pub retries: u32,
}

/// The environment of an import job: its services, its build and who imports.
pub struct ImportContext<'a> {
    /// The job service.
    pub jobs: &'a Jobs,
    /// The file service the libraries are open in.
    pub files: &'a HostFiles,
    /// The library index each imported file is registered in.
    pub index: &'a dyn ImportIndex,
    /// Each job's plan and manifest.
    pub plans: &'a SqliteImportPlans,
    /// The host's journal.
    pub journal: &'a JournalSender,
    /// This FileSquid build.
    pub build: &'a Build,
    /// Who imports.
    pub actor: &'a str,
    /// The folder whose volume holds the destination's staging (the data directory).
    pub staging_dir: &'a Path,
    /// Set at quit: suspend between files.
    pub stop: &'a AtomicBool,
    /// The first retry's wait ([`RETRY_UNIT`] in a host; shorter in tests).
    pub retry_unit: Duration,
}

/// The first retry's wait; each later retry doubles it, up to [`MAX_BACKOFF`].
pub const RETRY_UNIT: Duration = Duration::from_secs(1);

/// The source identity of an import job: the library and a digest of the picked sources, so
/// a job is never resumed against other files. Pure.
pub fn import_source(plan: &ImportPlan) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    for source in &plan.sources {
        hasher.update(source.as_os_str().as_encoded_bytes());
        hasher.update([0u8]);
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("import:{}:{hex}", plan.library_id)
}

/// Submit a new import job for `plan` under `operation_id`.
///
/// # Errors
/// As [`Jobs::submit`].
pub fn submit_import(
    jobs: &Jobs,
    plan: &ImportPlan,
    operation_id: &str,
) -> Result<filesquid_api::jobs::Job, JobError> {
    jobs.submit(JobSpec {
        kind: IMPORT_KIND.into(),
        operation_id: operation_id.into(),
        source: import_source(plan),
    })
}

/// One entry of the listing of a plan's sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listed {
    /// A regular file to import.
    File {
        /// The file.
        path: PathBuf,
        /// The index of the plan's source it was listed from.
        source: usize,
    },
    /// An entry that cannot be imported, with its error code: a source or folder that could
    /// not be read (`source_missing`, `source_unreadable`, `folder_unreadable`,
    /// `entry_unreadable`), a folder deeper than [`MAX_WALK_DEPTH`] (`folder_too_deep`), a
    /// link to a folder, which is not followed (`folder_link_not_followed`), or an entry that
    /// is not a regular file (`not_a_file`).
    Skipped {
        /// The index of the plan's source it belongs to.
        source: usize,
        /// Why it is skipped.
        code: &'static str,
    },
}

/// Every entry of `plan`'s sources in one stable order: the sources in the order picked, each
/// a file or the entries below a folder (names in byte order, folders before their
/// contents), at most [`MAX_IMPORT_FILES`] in all. Nothing that cannot be read is dropped:
/// it is listed as [`Listed::Skipped`] with its code. Reads the filesystem.
///
/// # Errors
/// `too_many_files` when the sources hold more than [`MAX_IMPORT_FILES`] entries; the
/// listing stops there.
pub fn list_plan(plan: &ImportPlan) -> Result<Vec<Listed>, &'static str> {
    let mut listed = Vec::new();
    for (source, root) in plan.sources.iter().enumerate() {
        list_source(root, source, &mut listed)?;
    }
    Ok(listed)
}

/// List the source `root` (index `source`) into `out`. Reads the filesystem.
fn list_source(root: &Path, source: usize, out: &mut Vec<Listed>) -> Result<(), &'static str> {
    match std::fs::metadata(root) {
        Ok(meta) if meta.is_file() => push(
            out,
            Listed::File {
                path: root.to_path_buf(),
                source,
            },
        ),
        Ok(meta) if meta.is_dir() => walk(root, 0, source, out),
        Ok(_) => skip(out, source, "not_a_file"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            skip(out, source, "source_missing")
        }
        Err(_) => skip(out, source, "source_unreadable"),
    }
}

/// Add `entry` to `out`, refusing the [`MAX_IMPORT_FILES`]+1st. Pure.
fn push(out: &mut Vec<Listed>, entry: Listed) -> Result<(), &'static str> {
    if out.len() >= MAX_IMPORT_FILES {
        return Err("too_many_files");
    }
    out.push(entry);
    Ok(())
}

/// Add a skipped entry of `source` with `code` to `out`. Pure.
fn skip(out: &mut Vec<Listed>, source: usize, code: &'static str) -> Result<(), &'static str> {
    push(out, Listed::Skipped { source, code })
}

/// List the entries below the folder `dir`, `depth` levels below its source, into `out`.
/// Reads the filesystem.
fn walk(
    dir: &Path,
    depth: usize,
    source: usize,
    out: &mut Vec<Listed>,
) -> Result<(), &'static str> {
    if depth > MAX_WALK_DEPTH {
        return skip(out, source, "folder_too_deep");
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return skip(out, source, "folder_unreadable");
    };
    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(_) => skip(out, source, "entry_unreadable")?,
        }
        if out.len() + paths.len() > MAX_IMPORT_FILES {
            return Err("too_many_files");
        }
    }
    paths.sort_by(|a, b| {
        a.as_os_str()
            .as_encoded_bytes()
            .cmp(b.as_os_str().as_encoded_bytes())
    });
    for path in paths {
        list_entry(path, depth, source, out)?;
    }
    Ok(())
}

/// List one entry of a folder: a folder is walked, a link to a folder is not followed, a
/// regular file (or a link to one) is listed. Reads the filesystem.
fn list_entry(
    path: PathBuf,
    depth: usize,
    source: usize,
    out: &mut Vec<Listed>,
) -> Result<(), &'static str> {
    let Ok(link) = std::fs::symlink_metadata(&path) else {
        return skip(out, source, "entry_unreadable");
    };
    if link.is_dir() {
        return walk(&path, depth + 1, source, out);
    }
    match std::fs::metadata(&path) {
        Ok(meta) if meta.is_file() => push(out, Listed::File { path, source }),
        Ok(meta) if meta.is_dir() => skip(out, source, "folder_link_not_followed"),
        Ok(_) => skip(out, source, "not_a_file"),
        Err(_) => skip(out, source, "entry_unreadable"),
    }
}

/// The manifest entries of `listed` for `plan`: each file with a newly minted file identifier
/// and its collection logical path in the destination, numbered per source as `index` finds
/// paths taken ([`super::placing`]); a file that cannot be placed is skipped with its code.
/// Reads the random source, filesystem metadata and the index.
///
/// # Errors
/// `destination_required` for a plan without a collection; `unavailable` when no identifier
/// can be minted or the index cannot answer.
pub fn manifest_of(
    listed: Vec<Listed>,
    plan: &ImportPlan,
    index: &dyn ImportIndex,
) -> Result<Vec<ManifestEntry>, &'static str> {
    let collection = destination(plan)?;
    let roots = source_roots(plan, &collection, index)?;
    listed
        .into_iter()
        .map(|entry| manifest_entry(entry, plan, &roots))
        .collect()
}

/// The manifest entry of one listed entry, placed beneath its source's root. Reads the
/// random source.
fn manifest_entry(
    entry: Listed,
    plan: &ImportPlan,
    roots: &[Result<String, &'static str>],
) -> Result<ManifestEntry, &'static str> {
    let (path, source) = match entry {
        Listed::File { path, source } => (path, source),
        Listed::Skipped { source, code } => return Ok(skipped(source, code)),
    };
    let root = roots.get(source).cloned().unwrap_or(Err("invalid_path"));
    let placed = root.and_then(|root| {
        let from = plan.sources.get(source).map_or(path.as_path(), |s| s);
        logical_path(&root, from, &path)
    });
    match placed {
        Ok(logical) => Ok(ManifestEntry::File {
            path,
            source,
            file_id: mint_uuid_v4().map_err(|_| "unavailable")?,
            logical_path: Some(logical),
        }),
        Err(code) => Ok(skipped(source, code)),
    }
}

/// A skipped manifest entry of `source` with `code`. Pure.
fn skipped(source: usize, code: &str) -> ManifestEntry {
    ManifestEntry::Skipped {
        source,
        code: code.to_string(),
    }
}

/// The destination collection of `plan`. Pure.
///
/// # Errors
/// `destination_required` when the plan names none (a plan an earlier build kept), or one
/// that is not a collection identifier.
fn destination(plan: &ImportPlan) -> Result<CollectionId, &'static str> {
    plan.collection_id
        .as_deref()
        .and_then(|id| CollectionId::new(id).ok())
        .ok_or("destination_required")
}

/// The manifest of `job_id`: the one kept by an earlier run, or the listing of `plan` kept
/// now. Reads the filesystem and the plans, writes the plans.
///
/// # Errors
/// `too_many_files` for an over-large selection; `unavailable` when the manifest cannot be
/// read, minted or kept.
fn job_manifest(
    context: &ImportContext<'_>,
    job_id: &str,
    plan: &ImportPlan,
) -> Result<Vec<ManifestEntry>, &'static str> {
    let plans = context.plans;
    let kept = plans.manifest(job_id).map_err(|_| "unavailable")?;
    if !kept.is_empty() {
        return Ok(kept);
    }
    let entries = manifest_of(list_plan(plan)?, plan, context.index)?;
    plans
        .save_manifest(job_id, &entries)
        .map_err(|_| "unavailable")
}

/// The next file index a checkpoint names (0 when there is none). Pure.
pub fn checkpoint_next(checkpoint: Option<&str>) -> usize {
    checkpoint
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|value| value["next"].as_u64())
        .and_then(|next| usize::try_from(next).ok())
        .unwrap_or(0)
}

/// The checkpoint text naming `next`. Pure.
pub fn import_checkpoint(next: usize) -> String {
    serde_json::json!({ "next": next }).to_string()
}

/// The wait before attempt `attempt` (2 for the first retry): `unit` doubled per attempt,
/// capped at [`MAX_BACKOFF`], plus jitter of up to a quarter of `unit`. Reads the clock for
/// jitter.
pub fn backoff(attempt: u32, unit: Duration) -> Duration {
    let base = unit.saturating_mul(1u32 << attempt.saturating_sub(2).min(6));
    let quarter = (unit.as_nanos() / 4).max(1);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u128::from(d.subsec_nanos()));
    let jitter = u64::try_from(nanos % quarter).unwrap_or(0);
    base.min(MAX_BACKOFF) + Duration::from_nanos(jitter)
}

/// The free-space reserve of a volume of `total` bytes: max(2 GiB, 5 % of it). Pure.
pub fn reserve_for(total: u64) -> u64 {
    MIN_RESERVE_BYTES.max(total / 20)
}

/// Whether a file of `size` bytes may be staged now on the volume of `dir`: within the
/// staging cap, and leaving the reserve free. Reads the volume's space.
fn has_room(dir: &Path, size: u64) -> Result<(), &'static str> {
    if size > STAGING_CAP_BYTES {
        return Err("over_staging_cap");
    }
    let (Ok(free), Ok(total)) = (fs4::available_space(dir), fs4::total_space(dir)) else {
        return Err("staging_space_unknown");
    };
    match free.saturating_sub(size) >= reserve_for(total) {
        true => Ok(()),
        false => Err("staging_space"),
    }
}

/// What happened to one manifest entry.
enum FileOutcome {
    /// Imported (or found imported by an earlier run of the job).
    Imported(UploadView),
    /// Quarantined with its error code.
    Quarantined(String),
    /// Not tried: the job must suspend (quit, or no room yet).
    Suspend,
    /// The job's lease was lost; the run must stop without writing the job again.
    Lost(JobError),
    /// A pause or cancel was asked while the file was being retried: the file is not done,
    /// and the job must halt if the request still stands.
    Halt,
}

/// How the manifest's run ended.
enum Ended {
    /// Every entry is done; the run still holds its lease.
    Done(RunningJob),
    /// The run gave its lease back; the job is now in this state.
    Halted(JobState),
}

/// Run the import job `job_id` for `plan`: start it (resuming at its checkpoint), keep or
/// read its manifest, import every entry not done yet, checkpointing after each, and complete
/// it, or suspend it with its checkpoint when `context.stop` is set or space runs short.
/// Filesystem, database and journal I/O.
///
/// # Errors
/// The job could not be started or written, or its lease was lost; a selection over
/// [`MAX_IMPORT_FILES`] entries (the job fails with `too_many_files`) or a manifest that
/// cannot be kept (`unavailable`).
pub async fn run_import_job(
    context: &ImportContext<'_>,
    job_id: &str,
    plan: &ImportPlan,
) -> Result<ImportRun, JobError> {
    let running = context.jobs.start(job_id, &import_source(plan))?;
    let operation_id = running.job().operation_id.clone();
    let manifest = match prepare(context, &running, plan) {
        Ok(manifest) => manifest,
        Err(code) => {
            running.fail(code)?;
            finish(context, job_id, plan, &operation_id);
            return Err(JobError::Conflict(code));
        }
    };
    let mut run = ImportRun {
        job_id: job_id.to_string(),
        state: JobState::Running,
        imported: Vec::new(),
        quarantined: Vec::new(),
        retries: 0,
    };
    match run_manifest(context, running, plan, &manifest, &mut run).await? {
        Ended::Done(running) => {
            running.complete()?;
            finish(context, job_id, plan, &operation_id);
            run.state = JobState::Completed;
        }
        Ended::Halted(state) => {
            if state == JobState::Cancelled {
                finish(context, job_id, plan, &operation_id);
            }
            run.state = state;
        }
    }
    Ok(run)
}

/// Import every entry of `manifest` from the checkpoint of `running` on, checkpointing after
/// each; after each checkpoint a pause or cancel asked of the job halts it through
/// [`RunningJob::halt_if_requested`], and a quit or a lack of space suspends it with its
/// checkpoint. Filesystem, database and journal I/O.
///
/// # Errors
/// The job could not be written, or its lease was lost.
async fn run_manifest(
    context: &ImportContext<'_>,
    mut running: RunningJob,
    plan: &ImportPlan,
    manifest: &[ManifestEntry],
    run: &mut ImportRun,
) -> Result<Ended, JobError> {
    let total = Some(manifest.len() as u64);
    let mut index = checkpoint_next(running.checkpoint_text());
    for _ in 0..manifest.len().saturating_add(MAX_CONTINUED_HALTS) {
        let Some(entry) = manifest.get(index) else {
            return Ok(Ended::Done(running));
        };
        let next = match import_entry(context, &running, plan, entry, &mut run.retries).await {
            FileOutcome::Imported(view) => {
                run.imported.push(view);
                index + 1
            }
            FileOutcome::Quarantined(code) => {
                run.quarantined.push(code);
                index + 1
            }
            FileOutcome::Lost(err) => return Err(err),
            FileOutcome::Halt => index,
            FileOutcome::Suspend => {
                running.checkpoint(&import_checkpoint(index), index as u64, total)?;
                return Ok(Ended::Halted(running.suspend()?));
            }
        };
        running.checkpoint(&import_checkpoint(next), next as u64, total)?;
        index = next;
        running = match halt_if_asked(running)? {
            Halt::Continued(running) => *running,
            Halt::Halted(state) => return Ok(Ended::Halted(state)),
        };
    }
    Ok(Ended::Halted(running.suspend()?))
}

/// Halt `running` through the API's atomic check when the last checkpoint or renewal read a
/// pause or cancel asked of it; otherwise keep it running. Database I/O only when one was
/// read.
///
/// # Errors
/// [`JobError::Fenced`] or a store failure.
fn halt_if_asked(running: RunningJob) -> Result<Halt, JobError> {
    match running.requested() {
        Some(_) => running.halt_if_requested(),
        None => Ok(Halt::Continued(Box::new(running))),
    }
}

/// Check `plan`'s destination, record the import's intent and keep or read its manifest.
/// Reads the index, the filesystem and the plans; writes the intent and the plans.
///
/// # Errors
/// `destination_required`, `collection_not_found`, `too_many_files` or `unavailable`: the
/// job fails with that code.
fn prepare(
    context: &ImportContext<'_>,
    running: &RunningJob,
    plan: &ImportPlan,
) -> Result<Vec<ManifestEntry>, &'static str> {
    let library = LibraryId::new(&plan.library_id).map_err(|_| "library_not_found")?;
    let collection = destination(plan)?;
    let held = context.index.has_collection(&library, &collection);
    if !held.map_err(|_| "unavailable")? {
        return Err("collection_not_found");
    }
    let intent = import_intent(&library, running.job().operation_id.as_str())?;
    context.index.begin(&intent).map_err(|_| "unavailable")?;
    job_manifest(context, running.job().id.as_str(), plan)
}

/// The pending intent of the import `operation_id` into `library`: an import adds files and
/// is planned against no revision (`0`). Reads the clock.
fn import_intent(library: &LibraryId, operation_id: &str) -> Result<PendingIntent, &'static str> {
    Ok(PendingIntent {
        library_id: library.clone(),
        operation_id: OperationId::new(operation_id).map_err(|_| "unavailable")?,
        operation: IMPORT_FILE_OPERATION.to_string(),
        expected_revision: ExpectedRevision::new("0").map_err(|_| "unavailable")?,
        recorded_at_ms: filesquid_api::journal::now_ms(),
    })
}

/// Close out the import job `job`, found finished before it ran (cancelled while pending, for
/// example): mark it finished in the plans and clear the import's intent, as the end of a run
/// does. Database I/O; a failure is logged.
pub fn close_finished(
    context: &ImportContext<'_>,
    job: &filesquid_api::jobs::Job,
    plan: &ImportPlan,
) {
    finish(context, &job.id, plan, &job.operation_id);
}

/// Mark `job_id` finished in the plans and clear the import's intent; a failure of either is
/// logged with its code (the next launch finds the job finished, or the intent pending).
fn finish(context: &ImportContext<'_>, job_id: &str, plan: &ImportPlan, operation_id: &str) {
    mark_finished(context, job_id);
    let cleared = LibraryId::new(&plan.library_id)
        .ok()
        .zip(OperationId::new(operation_id).ok())
        .map(|(library, op)| context.index.finish(&library, &op));
    if !matches!(cleared, Some(Ok(()))) {
        tracing::warn!(
            operation = IMPORT_FILE_OPERATION,
            job_id,
            error_code = "unavailable",
            "an import's intent could not be cleared; it stays pending"
        );
    }
}

/// Mark `job_id` finished in the plans; a failure is logged, and the next launch then finds
/// the job finished when it tries to start it.
fn mark_finished(context: &ImportContext<'_>, job_id: &str) {
    if context.plans.finish(job_id).is_err() {
        tracing::warn!(
            operation = IMPORT_FILE_OPERATION,
            job_id,
            error_code = "unavailable",
            "an import job could not be marked finished in its plan"
        );
    }
}

/// Import one manifest entry: quarantine a skipped one with its code, import a file, counting
/// its retries in `retries`.
async fn import_entry(
    context: &ImportContext<'_>,
    running: &RunningJob,
    plan: &ImportPlan,
    entry: &ManifestEntry,
    retries: &mut u32,
) -> FileOutcome {
    match entry {
        ManifestEntry::Skipped { code, .. } => quarantine(context, running, code),
        ManifestEntry::File {
            logical_path: None, ..
        } => quarantine(context, running, "destination_required"),
        ManifestEntry::File {
            path,
            source,
            file_id,
            logical_path: Some(logical),
        } => {
            let import_root = plan.sources.get(*source).map_or(path.as_path(), |s| s);
            let request = FileImport {
                library_id: &plan.library_id,
                file_id,
                path,
                import_root,
                method: plan.method,
                job_id: Some(running.job().id.clone()),
                operation_id: Some(running.job().operation_id.clone()),
                build: context.build,
                actor: context.actor,
            };
            let placed = Placed {
                collection_id: plan.collection_id.as_deref().unwrap_or_default(),
                path: logical,
            };
            import_with_attempts(context, running, &request, &placed, retries).await
        }
    }
}

/// How often a run renews its lease while one file streams: a third of the lease's length,
/// so two renewals can be late before it ends. Pure.
pub fn renewal_interval(jobs: &Jobs) -> Duration {
    let third = u64::try_from(jobs.lease_ttl_ms() / 3).unwrap_or(1).max(1);
    Duration::from_millis(third)
}

/// Renew the lease of `running` while a file is imported; a failure is logged with its code
/// (a lost lease stops the run at its next checkpoint). Database I/O.
fn renew_lease(running: &RunningJob) {
    if let Err(err) = running.renew() {
        tracing::warn!(
            operation = IMPORT_FILE_OPERATION,
            job_id = running.job().id.as_str(),
            error_code = err.error_code(),
            "an import job's lease could not be renewed while a file was imported"
        );
    }
}

/// How long the heartbeat sleeps between two looks at the import.
const HEARTBEAT_STEP: Duration = Duration::from_millis(50);

/// Most looks one file's heartbeat takes (a day of [`HEARTBEAT_STEP`]s, AGENTS.md §22); past
/// it the heartbeat stops renewing and waits for the import to end.
const MAX_HEARTBEAT_STEPS: u32 = 24 * 60 * 60 * 20;

/// Import `request` on a scoped thread with a runtime of its own while this thread renews the
/// lease of `running` every [`renewal_interval`], so a file that takes longer than one lease
/// to stage and commit keeps the job's lease (Task DE-1.9; the storage backends block, so the
/// renewal cannot share the import's thread). Starts a thread; filesystem, storage and
/// database I/O.
///
/// # Errors
/// As [`import_file`]; a runtime that cannot be built is `unavailable`, an import that
/// panicked `internal`.
fn import_with_heartbeat(
    context: &ImportContext<'_>,
    running: &RunningJob,
    request: &FileImport<'_>,
) -> Result<UploadView, ImportFailure> {
    let every = renewal_interval(context.jobs);
    std::thread::scope(|scope| {
        let importing = scope.spawn(|| import_on_own_runtime(context.files, request));
        let mut renewed = Instant::now();
        for _ in 0..MAX_HEARTBEAT_STEPS {
            if importing.is_finished() {
                break;
            }
            std::thread::sleep(HEARTBEAT_STEP);
            if renewed.elapsed() >= every {
                renew_lease(running);
                renewed = Instant::now();
            }
        }
        importing.join().unwrap_or(Err(ImportFailure::Commit {
            code: "internal",
            retryable: false,
        }))
    })
}

/// Run [`import_file`] for `request` to its end on a current-thread runtime of this thread.
/// Filesystem and storage I/O.
fn import_on_own_runtime(
    files: &HostFiles,
    request: &FileImport<'_>,
) -> Result<UploadView, ImportFailure> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(|_| ImportFailure::Commit {
            code: "unavailable",
            retryable: true,
        })?;
    runtime.block_on(import_file(files, request))
}

/// Where a file of the manifest is registered.
struct Placed<'a> {
    /// Its home collection.
    collection_id: &'a str,
    /// Its collection logical path.
    path: &'a str,
}

/// What registering a committed file came to.
enum Registered {
    /// It is registered (now or before).
    Done,
    /// The index refused it with this code; the file is quarantined.
    Refused(&'static str),
    /// The index could not be written; a retry may succeed.
    Retry,
}

/// Register the committed file `file_id` at `placed`, as the import `running`, journaling the
/// registration with the import's operation ID and opaque IDs only. Writes the index.
fn register(
    context: &ImportContext<'_>,
    running: &RunningJob,
    library_id: &str,
    file_id: &str,
    placed: &Placed<'_>,
) -> Registered {
    let ids = LibraryId::new(library_id)
        .ok()
        .zip(FileId::new(file_id).ok())
        .zip(CollectionId::new(placed.collection_id).ok());
    let Some(((library, file), collection)) = ids else {
        return Registered::Refused("destination_required");
    };
    let request = RegisterFile {
        file_id: file,
        collection_id: collection,
        path: placed.path.to_string(),
    };
    let journal = JournalEntry::new(REGISTER_OPERATION, Outcome::Succeeded)
        .actor(context.actor)
        .target(running.job().id.as_str())
        .operation_id(running.job().operation_id.as_str());
    match context.index.register(&library, &request, journal) {
        Ok(Registration::Registered { .. } | Registration::AlreadyRegistered { .. }) => {
            Registered::Done
        }
        Ok(Registration::Refused(code)) => Registered::Refused(code),
        Err(_) => Registered::Retry,
    }
}

/// Import `request` with at most [`MAX_ATTEMPTS`] attempts, counting each retry in
/// `retries` and journaling the outcome. Reads the filesystem, writes the library and the
/// journal.
async fn import_with_attempts(
    context: &ImportContext<'_>,
    running: &RunningJob,
    request: &FileImport<'_>,
    placed: &Placed<'_>,
    retries: &mut u32,
) -> FileOutcome {
    let mut last = "import_failed";
    let mut committed = false;
    for attempt in 1..=MAX_ATTEMPTS {
        if context.stop.load(Ordering::SeqCst) {
            return FileOutcome::Suspend;
        }
        let size = std::fs::metadata(request.path).map_or(0, |m| m.len());
        match has_room(context.staging_dir, size) {
            Err("staging_space") | Err("staging_space_unknown") => return FileOutcome::Suspend,
            Err(code) => return quarantine(context, running, code),
            Ok(()) => {}
        }
        *retries += u32::from(attempt > 1);
        match import_with_heartbeat(context, running, request) {
            Ok(view) => match register(
                context,
                running,
                request.library_id,
                request.file_id,
                placed,
            ) {
                Registered::Done => return imported(context, running, view),
                Registered::Refused(code) => return quarantine(context, running, code),
                Registered::Retry => (last, committed) = ("unavailable", true),
            },
            Err(failure) if !failure.retry_now() => {
                return quarantine(context, running, failure.error_code())
            }
            Err(failure) => last = failure.error_code(),
        }
        let wait = backoff(attempt + 1, context.retry_unit);
        if attempt < MAX_ATTEMPTS && !wait_unless_stopped(wait, context.stop) {
            return FileOutcome::Suspend;
        }
        if let Err(err) = running.renew() {
            return FileOutcome::Lost(err);
        }
        if running.requested().is_some() {
            return FileOutcome::Halt;
        }
    }
    match committed {
        true => unregistered(running),
        false => quarantine(context, running, last),
    }
}

/// Suspend the job whose file object is committed but whose registration the index kept
/// refusing to take: the job and the import's intent stay, so the next run answers the same
/// object and registers it, rather than leave a committed object outside every collection.
/// Logs the code (stdout).
fn unregistered(running: &RunningJob) -> FileOutcome {
    tracing::warn!(
        operation = IMPORT_FILE_OPERATION,
        job_id = running.job().id.as_str(),
        error_code = "unavailable",
        "an imported file is committed but not yet registered; the job is suspended"
    );
    FileOutcome::Suspend
}

/// Journal an imported file and answer it.
fn imported(context: &ImportContext<'_>, running: &RunningJob, view: UploadView) -> FileOutcome {
    let entry = JournalEntry::new(IMPORT_FILE_OPERATION, Outcome::Succeeded)
        .actor(context.actor)
        .target(view.file_id.as_str())
        .target(running.job().id.as_str())
        .operation_id(running.job().operation_id.as_str());
    context.journal.send(entry);
    FileOutcome::Imported(view)
}

/// Journal a quarantined entry (its error code only, never its path) and answer it.
fn quarantine(context: &ImportContext<'_>, running: &RunningJob, code: &str) -> FileOutcome {
    let entry = JournalEntry::new(IMPORT_FILE_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .actor(context.actor)
        .target(running.job().id.as_str())
        .operation_id(running.job().operation_id.as_str())
        .error_code(code);
    context.journal.send(entry);
    FileOutcome::Quarantined(code.to_string())
}

/// Sleep `wait` in short steps; answer false as soon as `stop` is set.
fn wait_unless_stopped(wait: Duration, stop: &AtomicBool) -> bool {
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if stop.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50).min(deadline - Instant::now()));
    }
    !stop.load(Ordering::SeqCst)
}
