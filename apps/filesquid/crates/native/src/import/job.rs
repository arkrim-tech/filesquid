//! The import job (Task DE-1.2; F-002, NFR-003, DE-S-014): the files of a chosen folder (or one
//! chosen file) imported one by one as a durable job of `filesquid-api`'s [`Jobs`], resumable
//! after a quit or a crash.
//!
//! - The files are listed in one stable order (bounded walk; links to folders are not
//!   followed), and the checkpoint is the index of the next file, so a resumed run skips
//!   the files already done without reading them again.
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
use super::plans::ImportPlan;
use crate::HostFiles;
use filesquid_api::files::UploadView;
use filesquid_api::jobs::{JobError, JobSpec, JobState, Jobs, RunningJob};
use filesquid_api::journal::{JournalEntry, Level, Outcome};
use filesquid_api::journal_writer::JournalSender;
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
    /// The files quarantined in this run, by error code.
    pub quarantined: Vec<&'static str>,
}

/// The environment of an import job: its services, its build and who imports.
pub struct ImportContext<'a> {
    /// The job service.
    pub jobs: &'a Jobs,
    /// The file service the libraries are open in.
    pub files: &'a HostFiles,
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

/// The source identity of an import job: the library and a digest of the chosen root, so a
/// job is never resumed against another folder. Pure.
pub fn import_source(plan: &ImportPlan) -> String {
    use sha2::Digest as _;
    let digest = sha2::Sha256::digest(plan.root.as_os_str().as_encoded_bytes());
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

/// The files of `root` in one stable order: `root` itself when it is a file, else every
/// regular file below it (byte order of names, folders before their contents), at most
/// [`MAX_IMPORT_FILES`] and [`MAX_WALK_DEPTH`] levels; links to folders are not followed.
/// Reads the filesystem.
///
/// # Errors
/// A message (no path) when `root` cannot be read.
pub fn list_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let meta = std::fs::metadata(root).map_err(|_| "the chosen source is gone".to_string())?;
    if meta.is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    let mut found = Vec::new();
    walk(root, 0, &mut found);
    Ok(found)
}

/// Collect the regular files below `dir` into `found`, in name order, within the bounds.
fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > MAX_WALK_DEPTH || found.len() >= MAX_IMPORT_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort_by(|a, b| {
        a.as_os_str()
            .as_encoded_bytes()
            .cmp(b.as_os_str().as_encoded_bytes())
    });
    for path in paths {
        if found.len() >= MAX_IMPORT_FILES {
            return;
        }
        let Ok(link) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if link.is_dir() {
            walk(&path, depth + 1, found);
        } else if std::fs::metadata(&path).is_ok_and(|m| m.is_file()) {
            found.push(path);
        }
    }
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

/// What happened to one file.
enum FileOutcome {
    /// Imported.
    Imported(UploadView),
    /// Quarantined with its error code.
    Quarantined(&'static str),
    /// Not tried: the job must suspend (quit, or no room yet).
    Suspend,
}

/// Run the import job `job_id` for `plan`: start it (resuming at its checkpoint), import
/// every file not done yet, checkpointing after each, and complete it, or suspend it with
/// its checkpoint when `context.stop` is set or space runs short. Filesystem, database and
/// journal I/O.
///
/// # Errors
/// The job could not be started or written, or its lease was lost; the chosen source is
/// gone (the job fails with `source_missing`).
pub async fn run_import_job(
    context: &ImportContext<'_>,
    job_id: &str,
    plan: &ImportPlan,
) -> Result<ImportRun, JobError> {
    let mut running = context.jobs.start(job_id, &import_source(plan))?;
    let files = match list_files(&plan.root) {
        Ok(files) => files,
        Err(_) => {
            running.fail("source_missing")?;
            return Err(JobError::Conflict("source_missing"));
        }
    };
    let mut run = ImportRun {
        job_id: job_id.to_string(),
        state: JobState::Running,
        imported: Vec::new(),
        quarantined: Vec::new(),
    };
    let next = checkpoint_next(running.checkpoint_text());
    let total = Some(files.len() as u64);
    for (index, path) in files.iter().enumerate().skip(next) {
        match import_with_attempts(context, &running, plan, path).await {
            FileOutcome::Imported(view) => run.imported.push(view),
            FileOutcome::Quarantined(code) => run.quarantined.push(code),
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
    run.state = JobState::Completed;
    Ok(run)
}

/// Import `path` with at most [`MAX_ATTEMPTS`] attempts, journaling the outcome. Reads the
/// filesystem, writes the library and the journal.
async fn import_with_attempts(
    context: &ImportContext<'_>,
    running: &RunningJob,
    plan: &ImportPlan,
    path: &Path,
) -> FileOutcome {
    let request = FileImport {
        library_id: &plan.library_id,
        path,
        import_root: &plan.root,
        method: plan.method,
        job_id: Some(running.job().id.clone()),
        operation_id: Some(running.job().operation_id.clone()),
        build: context.build,
        actor: context.actor,
    };
    let mut last = "import_failed";
    for attempt in 1..=MAX_ATTEMPTS {
        if context.stop.load(Ordering::SeqCst) {
            return FileOutcome::Suspend;
        }
        let size = std::fs::metadata(path).map_or(0, |m| m.len());
        match has_room(context.staging_dir, size) {
            Err("staging_space") | Err("staging_space_unknown") => return FileOutcome::Suspend,
            Err(code) => return quarantine(context, running, code),
            Ok(()) => {}
        }
        match import_file(context.files, &request).await {
            Ok(view) => return imported(context, running, view),
            Err(failure) if !failure.retryable() => {
                return quarantine(context, running, failure.error_code())
            }
            Err(failure) => last = failure.error_code(),
        }
        let wait = backoff(attempt + 1, context.retry_unit);
        if attempt < MAX_ATTEMPTS && !wait_unless_stopped(wait, context.stop) {
            return FileOutcome::Suspend;
        }
        let _ = running.renew();
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

/// Journal a quarantined file (its error code only, never its path) and answer it.
fn quarantine(
    context: &ImportContext<'_>,
    running: &RunningJob,
    code: &'static str,
) -> FileOutcome {
    let entry = JournalEntry::new(IMPORT_FILE_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .actor(context.actor)
        .target(running.job().id.as_str())
        .operation_id(running.job().operation_id.as_str())
        .error_code(code);
    context.journal.send(entry);
    FileOutcome::Quarantined(code)
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
