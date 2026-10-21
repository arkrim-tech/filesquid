//! Task DE-0.4: durable jobs on the desktop (DE-S-014, API-S-033; NFR-003, ADR 007). The
//! history backfill runs as a durable job in the operational store: quitting suspends it with
//! its checkpoint, the next launch resumes it without rescanning completed libraries, a
//! changed source is never resumed, and a crashed run is recovered within the restart
//! budget. Runs against SQLite files and OCFL storage roots on disk.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

use filesquid_api::history::backfill::BACKFILL_OPERATION;
use filesquid_api::jobs::{JobSpec, JobState, JobStore, Jobs, JOB_FAILED, JOB_SUSPENDED};
use filesquid_api::journal::JournalEntry;
use filesquid_api::library::{
    CommandRequest, LibraryCommand, LibraryService, NEW_LIBRARY_REVISION,
};
use filesquid_api::operation::{CommandEnvelope, ExpectedRevision, OperationId, OperationScope};
use filesquid_api::ApiState;
use filesquid_core::RESTART_RESUMABLE_WITHIN_SECS;
use filesquid_core::{InstallationId, LibraryId, RepositoryId};
use filesquid_desktop_lib::{history_backfill, open_host_monitor, open_library_catalog};
use filesquid_native::workers::history_backfill::{
    backfill_checkpoint, backfill_source, open_jobs, recover_jobs, run_backfill_job,
};
use filesquid_native::{
    HostMonitor, SqliteCatalog, SqliteJobs, SqliteJournal, CATALOG_DIR, JOURNAL_FILE,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const OTHER_INSTALLATION: &str = "1b2c3d4e-5f60-4b7c-9d8e-0f1a2b3c4d5e";

fn installation() -> InstallationId {
    InstallationId::parse(INSTALLATION).expect("installation")
}

/// A fresh, empty data directory for one test (filesystem I/O).
fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-desktop-jobs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    dir
}

/// Create a library as operation `op` through a catalog without provenance, so its first
/// version has no PROV-O record (filesystem and database I/O).
fn libraries_without_history(dir: &Path, ops: &[&str]) -> Vec<String> {
    let state = ApiState::new();
    let host = open_host_monitor(&state, dir, None).expect("monitor");
    let plain = SqliteCatalog::open(&dir.join(CATALOG_DIR), host.monitor().announcing_store())
        .expect("catalog");
    let plain = Arc::new(plain);
    let service = LibraryService::new(plain.clone(), host.monitor().journal());
    let ids = ops
        .iter()
        .map(|op| {
            let request = CommandRequest {
                envelope: CommandEnvelope::new(
                    OperationId::new(*op).expect("op"),
                    ExpectedRevision::new(NEW_LIBRARY_REVISION).expect("revision"),
                    OperationScope::new("library").expect("scope"),
                ),
                command: LibraryCommand::CreateLibrary {
                    repository_id: RepositoryId::new("repo-1").expect("repo"),
                    name: "Family archive".into(),
                },
            };
            let result = service.execute("owner", request).expect("create");
            result.created_id.expect("library id")
        })
        .collect();
    drop(service);
    drop(plain);
    host.stop();
    ids
}

/// One launch of the desktop host's journaling, catalogs and jobs.
struct Launch {
    dir: PathBuf,
    host: HostMonitor,
    catalog: Arc<SqliteCatalog>,
    jobs: Jobs,
}

impl Launch {
    fn open(dir: &Path) -> Self {
        let state = ApiState::new();
        let host = open_host_monitor(&state, dir, None).expect("monitor");
        let catalog = Arc::new(open_library_catalog(dir, &host, &installation()).expect("catalog"));
        let jobs = open_jobs(dir, &host).expect("jobs");
        Self {
            dir: dir.to_path_buf(),
            host,
            catalog,
            jobs,
        }
    }

    /// Stop the monitor (flushing the journal) and answer the journal's entries.
    fn stop(self) -> Vec<JournalEntry> {
        drop(self.catalog);
        self.host.stop();
        let store = SqliteJournal::open(&self.dir.join(JOURNAL_FILE), "desktop").expect("journal");
        journal::all_entries(&store)
    }
}

/// Whether the snapshot of `library` has a PROV-O record for `v1` (filesystem I/O).
fn has_record(dir: &Path, library: &str) -> bool {
    let snapshot = dir
        .join(CATALOG_DIR)
        .join(format!("{library}.ocfl"))
        .join("snapshot");
    (1..=4).any(|n| {
        snapshot
            .join(format!("v{n}"))
            .join("content")
            .join("provenance")
            .join("v1.jsonld")
            .exists()
    })
}

#[test]
fn a_backfill_job_completes_and_records_both_libraries_in_its_checkpoint() {
    let dir = data_dir("complete");
    let libs = libraries_without_history(&dir, &["op-a", "op-b"]);
    let launch = Launch::open(&dir);
    recover_jobs(&launch.jobs).expect("recovered");
    let backfill = history_backfill(&launch.host, &installation());
    let run = run_backfill_job(
        &launch.jobs,
        &launch.catalog,
        &backfill,
        &backfill_source(&installation()),
        &AtomicBool::new(false),
    )
    .expect("ran");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(run.ran.len(), 2);
    let store = SqliteJobs::open(&dir.join(JOURNAL_FILE)).expect("store");
    let job = store.get(&run.job_id).unwrap().expect("stored");
    assert_eq!(job.state, JobState::Completed);
    assert_eq!(job.kind, BACKFILL_OPERATION);
    assert_eq!((job.done, job.total), (2, Some(2)));
    for lib in &libs {
        assert!(job.checkpoint.as_deref().unwrap().contains(lib.as_str()));
        assert!(has_record(&dir, lib), "{lib} backfilled");
    }
    launch.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn quitting_suspends_the_backfill_job_with_its_checkpoint_kept() {
    let dir = data_dir("quit");
    libraries_without_history(&dir, &["op-a"]);
    let launch = Launch::open(&dir);
    let backfill = history_backfill(&launch.host, &installation());
    let run = run_backfill_job(
        &launch.jobs,
        &launch.catalog,
        &backfill,
        &backfill_source(&installation()),
        &AtomicBool::new(true),
    )
    .expect("ran");
    assert_eq!(run.state, JobState::Pending, "suspended, not finished");
    assert!(run.ran.is_empty(), "no library was started after quit");
    let entries = launch.stop();
    let store = SqliteJobs::open(&dir.join(JOURNAL_FILE)).expect("store");
    let job = store.get(&run.job_id).unwrap().expect("stored");
    assert_eq!(job.state, JobState::Pending);
    assert_eq!(job.holder, None);
    assert!(job.checkpoint.is_some(), "the checkpoint survives the quit");
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.operation == JOB_SUSPENDED)
            .count(),
        1
    );
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn the_next_launch_resumes_the_job_without_rescanning_completed_libraries() {
    let dir = data_dir("resume");
    let libs = libraries_without_history(&dir, &["op-a", "op-b"]);
    let done = LibraryId::new(libs[0].as_str()).expect("library");
    let first = Launch::open(&dir);
    let job = first
        .jobs
        .submit(JobSpec {
            kind: BACKFILL_OPERATION.into(),
            operation_id: "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b".into(),
            source: backfill_source(&installation()),
        })
        .expect("submitted");
    let mut running = first
        .jobs
        .start(&job.id, &backfill_source(&installation()))
        .expect("started");
    running
        .checkpoint(
            &backfill_checkpoint(std::slice::from_ref(&done)),
            1,
            Some(2),
        )
        .expect("checkpoint");
    running.suspend().expect("quit");
    first.stop();
    let started = Instant::now();
    let next = Launch::open(&dir);
    let recovered = recover_jobs(&next.jobs).expect("recovered");
    assert_eq!(recovered.len(), 1);
    assert!(
        started.elapsed() < Duration::from_secs(RESTART_RESUMABLE_WITHIN_SECS),
        "resumable within 30 s"
    );
    let backfill = history_backfill(&next.host, &installation());
    let run = run_backfill_job(
        &next.jobs,
        &next.catalog,
        &backfill,
        &backfill_source(&installation()),
        &AtomicBool::new(false),
    )
    .expect("ran");
    assert_eq!(run.job_id, job.id, "the same job resumed");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(run.ran.len(), 1, "only the library left");
    assert_eq!(run.ran[0].library_id.as_str(), libs[1]);
    assert!(
        !has_record(&dir, &libs[0]),
        "the completed library was not rescanned"
    );
    assert!(has_record(&dir, &libs[1]));
    next.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn a_job_for_another_installation_is_failed_and_a_new_one_runs() {
    let dir = data_dir("source");
    libraries_without_history(&dir, &["op-a"]);
    let launch = Launch::open(&dir);
    let other = InstallationId::parse(OTHER_INSTALLATION).expect("installation");
    let stale = launch
        .jobs
        .submit(JobSpec {
            kind: BACKFILL_OPERATION.into(),
            operation_id: "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b".into(),
            source: backfill_source(&other),
        })
        .expect("submitted");
    recover_jobs(&launch.jobs).expect("recovered");
    let backfill = history_backfill(&launch.host, &installation());
    let run = run_backfill_job(
        &launch.jobs,
        &launch.catalog,
        &backfill,
        &backfill_source(&installation()),
        &AtomicBool::new(false),
    )
    .expect("ran");
    assert_ne!(run.job_id, stale.id);
    assert_eq!(run.state, JobState::Completed);
    let entries = launch.stop();
    let store = SqliteJobs::open(&dir.join(JOURNAL_FILE)).expect("store");
    let old = store.get(&stale.id).unwrap().expect("stored");
    assert_eq!(old.state, JobState::Failed);
    assert_eq!(old.error_code.as_deref(), Some("source_changed"));
    let failed: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == JOB_FAILED)
        .collect();
    assert_eq!(failed.len(), 1);
    let text = serde_json::to_string(&entries).expect("json");
    assert!(
        !text.contains(OTHER_INSTALLATION),
        "no source identity in the journal"
    );
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn a_run_left_by_a_crash_is_waited_out_and_resumed() {
    let dir = data_dir("crash");
    libraries_without_history(&dir, &["op-a"]);
    let launch = Launch::open(&dir);
    let crashed = open_jobs(&dir, &launch.host)
        .expect("jobs")
        .with_lease_ttl(300);
    let job = crashed
        .submit(JobSpec {
            kind: BACKFILL_OPERATION.into(),
            operation_id: "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b".into(),
            source: backfill_source(&installation()),
        })
        .expect("submitted");
    let held = crashed
        .start(&job.id, &backfill_source(&installation()))
        .expect("leased");
    drop(held);
    let jobs = launch.jobs.clone().with_lease_ttl(300);
    recover_jobs(&jobs).expect("recovered");
    let backfill = history_backfill(&launch.host, &installation());
    let started = Instant::now();
    let run = run_backfill_job(
        &jobs,
        &launch.catalog,
        &backfill,
        &backfill_source(&installation()),
        &AtomicBool::new(false),
    )
    .expect("ran");
    assert_eq!(run.job_id, job.id, "the crashed job resumed");
    assert_eq!(run.state, JobState::Completed);
    assert!(started.elapsed() < Duration::from_secs(RESTART_RESUMABLE_WITHIN_SECS));
    launch.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn restart_recovery_reads_only_jobs_and_stays_within_the_budget() {
    let dir = data_dir("budget");
    std::fs::create_dir_all(&dir).expect("dir");
    let launch = Launch::open(&dir);
    for n in 0..500 {
        launch
            .jobs
            .submit(JobSpec {
                kind: "import".into(),
                operation_id: format!("op-{n}"),
                source: "s".into(),
            })
            .expect("submitted");
    }
    launch.stop();
    let started = Instant::now();
    let next = Launch::open(&dir);
    let recovered = recover_jobs(&next.jobs).expect("recovered");
    assert_eq!(recovered.len(), 500);
    assert!(
        started.elapsed() < Duration::from_secs(RESTART_RESUMABLE_WITHIN_SECS),
        "{:?}",
        started.elapsed()
    );
    next.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

/// Cubic round 1 (DE-0.8): an unreadable checkpoint counts as none, so every library is
/// visited again.
#[test]
fn an_unreadable_checkpoint_visits_every_library_again() {
    let dir = data_dir("unreadable");
    libraries_without_history(&dir, &["op-a", "op-b"]);
    for (n, bad) in [r#"{"done":"x"}"#, "not json"].into_iter().enumerate() {
        let launch = Launch::open(&dir);
        let job = launch
            .jobs
            .submit(JobSpec {
                kind: BACKFILL_OPERATION.into(),
                operation_id: format!("6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4{n}"),
                source: backfill_source(&installation()),
            })
            .expect("submitted");
        let mut running = launch
            .jobs
            .start(&job.id, &backfill_source(&installation()))
            .expect("started");
        running.checkpoint(bad, 1, Some(2)).expect("checkpoint");
        running.suspend().expect("quit");
        let backfill = history_backfill(&launch.host, &installation());
        let run = run_backfill_job(
            &launch.jobs,
            &launch.catalog,
            &backfill,
            &backfill_source(&installation()),
            &AtomicBool::new(false),
        )
        .expect("ran");
        assert_eq!(run.job_id, job.id);
        assert_eq!(run.ran.len(), 2, "{bad}: both libraries visited");
        assert_eq!(run.state, JobState::Completed);
        launch.stop();
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn a_paused_backfill_job_waits_until_it_is_resumed() {
    let dir = data_dir("paused");
    libraries_without_history(&dir, &["op-a"]);
    let launch = Launch::open(&dir);
    let backfill = history_backfill(&launch.host, &installation());
    let source = backfill_source(&installation());
    let quit = run_backfill_job(
        &launch.jobs,
        &launch.catalog,
        &backfill,
        &source,
        &AtomicBool::new(true),
    )
    .expect("suspended at quit");
    launch.jobs.pause(&quit.job_id).expect("pause");
    let held = run_backfill_job(
        &launch.jobs,
        &launch.catalog,
        &backfill,
        &source,
        &AtomicBool::new(false),
    )
    .expect_err("a paused job does not start");
    assert_eq!(held, filesquid_api::jobs::JobError::Conflict("job_paused"));
    launch.jobs.resume(&quit.job_id).expect("resume");
    let run = run_backfill_job(
        &launch.jobs,
        &launch.catalog,
        &backfill,
        &source,
        &AtomicBool::new(false),
    )
    .expect("ran");
    assert_eq!(run.job_id, quit.job_id, "the same job resumed");
    assert_eq!(run.state, JobState::Completed);
    launch.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}
