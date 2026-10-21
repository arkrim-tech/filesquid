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
//!
//! Every file's outcome is journaled as [`IMPORT_FILE_OPERATION`] with opaque IDs and an error
//! code only, never a path or a file name.

use super::file::{import_file, FileImport};
use super::host::Build;
use super::plans::{ImportPlan, ManifestEntry, SqliteImportPlans};
use crate::HostFiles;
use filesquid_api::files::UploadView;
use filesquid_api::jobs::{JobError, JobSpec, JobState, Jobs, RunningJob};
use filesquid_api::journal::{JournalEntry, Level, Outcome};
use filesquid_api::journal_writer::JournalSender;
use filesquid_api::library::mint_uuid_v4;
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

/// The manifest entries of `listed`, each file with a newly minted file identifier. Reads
/// the random source.
///
/// # Errors
/// `unavailable` when no identifier can be minted.
pub fn manifest_of(listed: Vec<Listed>) -> Result<Vec<ManifestEntry>, &'static str> {
    listed
        .into_iter()
        .map(|entry| match entry {
            Listed::File { path, source } => Ok(ManifestEntry::File {
                path,
                source,
                file_id: mint_uuid_v4().map_err(|_| "unavailable")?,
            }),
            Listed::Skipped { source, code } => Ok(ManifestEntry::Skipped {
                source,
                code: code.to_string(),
            }),
        })
        .collect()
}

/// The manifest of `job_id`: the one kept by an earlier run, or the listing of `plan` kept
/// now. Reads the filesystem and the plans, writes the plans.
///
/// # Errors
/// `too_many_files` for an over-large selection; `unavailable` when the manifest cannot be
/// read, minted or kept.
fn job_manifest(
    plans: &SqliteImportPlans,
    job_id: &str,
    plan: &ImportPlan,
) -> Result<Vec<ManifestEntry>, &'static str> {
    let kept = plans.manifest(job_id).map_err(|_| "unavailable")?;
    if !kept.is_empty() {
        return Ok(kept);
    }
    let entries = manifest_of(list_plan(plan)?)?;
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
    let mut running = context.jobs.start(job_id, &import_source(plan))?;
    let manifest = match job_manifest(context.plans, job_id, plan) {
        Ok(manifest) => manifest,
        Err(code) => {
            running.fail(code)?;
            mark_finished(context, job_id);
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
    let next = checkpoint_next(running.checkpoint_text());
    let total = Some(manifest.len() as u64);
    for (index, entry) in manifest.iter().enumerate().skip(next) {
        match import_entry(context, &running, plan, entry, &mut run.retries).await {
            FileOutcome::Imported(view) => run.imported.push(view),
            FileOutcome::Quarantined(code) => run.quarantined.push(code),
            FileOutcome::Lost(err) => return Err(err),
            FileOutcome::Suspend => {
                running.checkpoint(&import_checkpoint(index), index as u64, total)?;
                running.suspend()?;
                run.state = JobState::Pending;
                return Ok(run);
            }
        }
        running.checkpoint(&import_checkpoint(index + 1), index as u64 + 1, total)?;
    }
    running.complete()?;
    mark_finished(context, job_id);
    run.state = JobState::Completed;
    Ok(run)
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
            path,
            source,
            file_id,
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
            import_with_attempts(context, running, &request, retries).await
        }
    }
}

/// Import `request` with at most [`MAX_ATTEMPTS`] attempts, counting each retry in
/// `retries` and journaling the outcome. Reads the filesystem, writes the library and the
/// journal.
async fn import_with_attempts(
    context: &ImportContext<'_>,
    running: &RunningJob,
    request: &FileImport<'_>,
    retries: &mut u32,
) -> FileOutcome {
    let mut last = "import_failed";
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
        match import_file(context.files, request).await {
            Ok(view) => return imported(context, running, view),
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
    }
    quarantine(context, running, last)
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
