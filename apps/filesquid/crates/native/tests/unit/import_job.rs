//! Unit tests for the import job of Task DE-1.2 (F-002, NFR-003, DE-S-014): a folder imported
//! as one durable job, one stable file order, a checkpoint after each file, a quit that
//! suspends between files and a resume that skips the files already done, a file that keeps
//! failing quarantined after five attempts without stopping the batch, the staging reserve,
//! the plans store, and journal entries with no path.

use filesquid_api::jobs::{JobState, Jobs, MemoryJobStore};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::metrics::Metrics;
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::EventBus;
use filesquid_core::LibraryId;
use filesquid_metadata::import::ImportMethod;
use filesquid_native::import::host::Build;
use filesquid_native::import::job::{
    backoff, checkpoint_next, import_checkpoint, import_source, list_files, list_plan, reserve_for,
    run_import_job, submit_import, ImportContext, IMPORT_FILE_OPERATION, MAX_BACKOFF,
};
use filesquid_native::import::plans::{ImportPlan, SqliteImportPlans};
use filesquid_native::import::worker::{ImportServices, ImportWorker};
use filesquid_native::{open_fs_library, HostFiles};
use filesquid_ocfl::storage::FsBackend;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

/// The library every test imports into.
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// A fresh directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "filesquid-native-import-job-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.0) {
            eprintln!("could not remove the test directory: {err}");
        }
    }
}

/// The host of one test: a library served by a file service, a job service and a journal.
struct Rig {
    files: HostFiles,
    jobs: Jobs,
    monitor: Monitor,
    build: Build,
    stop: AtomicBool,
    data: PathBuf,
}

impl Rig {
    async fn new(dir: &Path) -> Self {
        let monitor = Monitor::start(
            Arc::new(MemoryJournal::new("desktop")),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("desktop"),
            EventBus::new(),
        );
        let root = dir.join("root");
        std::fs::create_dir_all(&root).expect("root");
        let backend = FsBackend::new(&root, dir.join("root-parts")).expect("backend");
        let id = LibraryId::new(LIBRARY).expect("library");
        create_root(&backend, &id).await;
        let files = HostFiles::new();
        let data = dir.join("data");
        open_fs_library(&files, &id, &root, &data).expect("opened");
        let jobs = Jobs::new(
            Arc::new(MemoryJobStore::new()),
            monitor.journal(),
            monitor.registry().clone(),
            "test-launch",
        );
        Self {
            files,
            jobs,
            monitor,
            build: Build {
                version: "0.1.0".into(),
                build: "test".into(),
                distribution: "filesquid".into(),
            },
            stop: AtomicBool::new(false),
            data,
        }
    }

    fn context<'a>(
        &'a self,
        journal: &'a filesquid_api::journal_writer::JournalSender,
    ) -> ImportContext<'a> {
        ImportContext {
            jobs: &self.jobs,
            files: &self.files,
            journal,
            build: &self.build,
            actor: "owner",
            staging_dir: &self.data,
            stop: &self.stop,
            retry_unit: Duration::from_millis(1),
        }
    }

    /// Every journal entry of `operation`.
    fn entries(&self, operation: &str) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let query = JournalQuery {
            operation: Some(operation.into()),
            limit: 1000,
            ..JournalQuery::default()
        };
        self.monitor.store().query(&query).expect("query").entries
    }
}

/// An empty OCFL storage root on `backend` for `id`.
async fn create_root(backend: &FsBackend, id: &LibraryId) {
    let writer = JournalWriter::start(
        Arc::new(MemoryJournal::new("desktop")),
        WriterConfig::default(),
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    LocalLibraryService::new(writer.sender(), Arc::new(MemoryRepairLedger::new()))
        .create_root("owner", id, backend)
        .await
        .expect("create root");
    writer.shutdown();
}

/// A folder `<dir>/Holidays` with three files, one in a subfolder.
fn holiday_folder(dir: &Path) -> PathBuf {
    let folder = dir.join("Holidays");
    std::fs::create_dir_all(folder.join("Day 2")).expect("folders");
    std::fs::write(folder.join("a.jpg"), b"aaa").expect("a");
    std::fs::write(folder.join("b.png"), b"bbbb").expect("b");
    std::fs::write(folder.join("Day 2").join("c.txt"), b"c").expect("c");
    folder
}

/// The plan of importing `root`.
fn plan(root: &Path) -> ImportPlan {
    ImportPlan {
        library_id: LIBRARY.into(),
        collection_id: None,
        sources: vec![root.to_path_buf()],
        method: ImportMethod::Folder,
    }
}

#[test]
fn a_folder_is_listed_in_one_stable_order_with_its_subfolders() {
    let dir = TempDir::new("list");
    let folder = holiday_folder(&dir.0);
    let names: Vec<_> = list_files(&folder)
        .expect("listed")
        .iter()
        .map(|p| p.strip_prefix(&folder).expect("inside").to_path_buf())
        .collect();
    assert_eq!(
        names,
        [
            PathBuf::from("Day 2/c.txt"),
            PathBuf::from("a.jpg"),
            PathBuf::from("b.png")
        ]
    );
    let single = folder.join("a.jpg");
    assert_eq!(
        list_files(&single).expect("file"),
        std::slice::from_ref(&single)
    );
    assert!(list_files(&dir.0.join("gone")).is_err());
}

#[tokio::test]
async fn a_folder_is_imported_as_one_job_with_a_checkpoint_per_file() {
    let dir = TempDir::new("import");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let plan = plan(&folder);
    let job = submit_import(&rig.jobs, &plan, "op-import").expect("job");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(run.imported.len(), 3);
    assert!(run.quarantined.is_empty());
    let entries = rig.entries(IMPORT_FILE_OPERATION);
    assert_eq!(entries.len(), 3, "one entry per file");
    let text = format!("{entries:?}");
    assert!(
        !text.contains("Holidays") && !text.contains("a.jpg"),
        "{text}"
    );
    assert!(entries
        .iter()
        .all(|e| e.target_ids.contains(&job.id) && e.operation_id.as_deref() == Some("op-import")));
}

#[tokio::test]
async fn a_quit_suspends_between_files_and_the_resume_skips_what_was_done() {
    let dir = TempDir::new("resume");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let plan = plan(&folder);
    let job = submit_import(&rig.jobs, &plan, "op-resume").expect("job");
    let journal = rig.monitor.journal();
    rig.stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(run.state, JobState::Pending, "suspended at quit");
    assert!(run.imported.is_empty());
    let mut running = rig
        .jobs
        .start(&job.id, &import_source(&plan))
        .expect("restart");
    running
        .checkpoint(&import_checkpoint(2), 2, Some(3))
        .expect("checkpoint");
    running.suspend().expect("suspend");
    rig.stop.store(false, std::sync::atomic::Ordering::SeqCst);
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("resumed");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(run.imported.len(), 1, "only the file after the checkpoint");
}

#[tokio::test]
async fn a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on() {
    let dir = TempDir::new("quarantine");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let huge = std::fs::File::create(folder.join("huge.bin")).expect("huge");
    huge.set_len(513 * 1024 * 1024).expect("sparse");
    let plan = plan(&folder);
    let job = submit_import(&rig.jobs, &plan, "op-quarantine").expect("job");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(run.imported.len(), 3);
    assert_eq!(run.quarantined, ["too_large_for_this_build"]);
    let failed: Vec<_> = rig
        .entries(IMPORT_FILE_OPERATION)
        .into_iter()
        .filter(|e| e.error_code.is_some())
        .collect();
    assert_eq!(failed.len(), 1, "quarantined once, after its attempts");
}

#[test]
fn checkpoints_backoff_and_reserve_follow_their_rules() {
    assert_eq!(checkpoint_next(None), 0);
    assert_eq!(checkpoint_next(Some(&import_checkpoint(7))), 7);
    assert_eq!(checkpoint_next(Some("not json")), 0);
    let unit = Duration::from_secs(1);
    assert!(backoff(2, unit) >= unit && backoff(2, unit) < unit + unit / 4 + unit);
    assert!(backoff(5, unit) >= Duration::from_secs(8));
    assert!(backoff(30, unit) <= MAX_BACKOFF + unit / 4);
    let gib = 1024 * 1024 * 1024;
    assert_eq!(reserve_for(10 * gib), 2 * gib, "the 2 GiB floor");
    assert_eq!(reserve_for(1000 * gib), 50 * gib, "5 % of a large volume");
}

#[test]
fn a_plan_survives_a_restart_and_one_operation_has_one_job() {
    let dir = TempDir::new("plans");
    let path = dir.0.join("filesquid.sqlite");
    let folder = holiday_folder(&dir.0);
    let picked = ImportPlan {
        collection_id: Some("collection-1".into()),
        sources: vec![folder.join("a.jpg"), folder.join("b.png")],
        method: ImportMethod::FilePicker,
        ..plan(&folder)
    };
    {
        let plans = SqliteImportPlans::open(&path).expect("open");
        plans.save("job-1", "op-1", &picked).expect("save");
        assert!(
            plans.save("job-2", "op-1", &picked).is_err(),
            "one job per operation"
        );
    }
    let plans = SqliteImportPlans::open(&path).expect("reopen");
    assert_eq!(plans.load("job-1").expect("load"), Some(picked));
    assert_eq!(
        plans.job_of_operation("op-1").expect("op"),
        Some("job-1".into())
    );
    assert_eq!(plans.job_of_operation("op-2").expect("op"), None);
    assert_eq!(plans.load("job-9").expect("load"), None);
}

#[tokio::test]
async fn picked_files_are_imported_each_with_itself_as_its_root() {
    let dir = TempDir::new("picked");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let picked = ImportPlan {
        sources: vec![folder.join("b.png"), folder.join("a.jpg")],
        method: ImportMethod::FilePicker,
        ..plan(&folder)
    };
    let listed = list_plan(&picked).expect("listed");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].path, folder.join("b.png"), "in the order picked");
    assert_eq!(listed[0].import_root, listed[0].path);
    let job = submit_import(&rig.jobs, &picked, "op-picked").expect("job");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &picked)
        .await
        .expect("ran");
    assert_eq!(run.imported.len(), 2);
}

#[tokio::test]
async fn the_worker_runs_queued_jobs_from_their_plans_and_resumes_unfinished_ones() {
    let dir = TempDir::new("worker");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let plans = Arc::new(SqliteImportPlans::open(&dir.0.join("filesquid.sqlite")).expect("plans"));
    let plan = plan(&folder);
    let job = submit_import(&rig.jobs, &plan, "op-worker").expect("job");
    plans.save(&job.id, "op-worker", &plan).expect("save");
    let stop = Arc::new(AtomicBool::new(false));
    let worker = ImportWorker::spawn(ImportServices {
        jobs: rig.jobs.clone(),
        files: rig.files.clone(),
        plans,
        journal: rig.monitor.journal(),
        build: rig.build.clone(),
        actor: "owner".into(),
        staging_dir: rig.data.clone(),
        stop: stop.clone(),
        retry_unit: Duration::from_millis(1),
    })
    .expect("worker");
    assert_eq!(
        worker.resume_unfinished(&rig.jobs),
        1,
        "the pending job is queued"
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while rig.entries(IMPORT_FILE_OPERATION).len() < 3 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    worker.wait(Duration::from_secs(10));
    assert_eq!(rig.entries(IMPORT_FILE_OPERATION).len(), 3);
    let done = rig.jobs.unfinished_of("import", 10).expect("jobs");
    assert!(done.is_empty(), "the job completed");
}
