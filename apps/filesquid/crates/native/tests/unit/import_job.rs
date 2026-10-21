//! Unit tests for the import job of Task DE-1.2 (F-002, NFR-003, DE-S-014): a folder imported
//! as one durable job, one stable listing kept as the job's manifest (unreadable entries kept
//! with their codes), a checkpoint after each file, a quit that suspends between files and a
//! resume that imports only the remaining files, a file committed before a crash answered and
//! not imported twice, a file that keeps failing quarantined after five attempts without
//! stopping the batch, the staging reserve, the plans store, the worker's paged recovery of
//! every unfinished job, and journal entries with no path.

use filesquid_api::jobs::{JobState, JobStore, Jobs, MemoryJobStore};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::metrics::Metrics;
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::EventBus;
use filesquid_core::LibraryId;
use filesquid_metadata::import::ImportMethod;
use filesquid_native::import::file::{import_file, FileImport};
use filesquid_native::import::host::Build;
use filesquid_native::import::job::{
    backoff, checkpoint_next, import_checkpoint, list_plan, manifest_of, reserve_for,
    run_import_job, submit_import, ImportContext, Listed, IMPORT_FILE_OPERATION, MAX_ATTEMPTS,
    MAX_BACKOFF, MAX_WALK_DEPTH,
};
use filesquid_native::import::plans::{ImportPlan, ManifestEntry, SqliteImportPlans};
use filesquid_native::import::worker::{ImportServices, ImportWorker};
use filesquid_native::{open_fs_library, HostFiles};
use filesquid_ocfl::storage::FsBackend;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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
    store: Arc<MemoryJobStore>,
    jobs: Jobs,
    plans: Arc<SqliteImportPlans>,
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
        let store = Arc::new(MemoryJobStore::new());
        let jobs = Jobs::new(
            store.clone(),
            monitor.journal(),
            monitor.registry().clone(),
            "test-launch",
        );
        let plans = Arc::new(SqliteImportPlans::open(&dir.join("plans.sqlite")).expect("plans"));
        Self {
            files,
            store,
            jobs,
            plans,
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
        self.context_with(journal, Duration::from_millis(1))
    }

    fn context_with<'a>(
        &'a self,
        journal: &'a filesquid_api::journal_writer::JournalSender,
        retry_unit: Duration,
    ) -> ImportContext<'a> {
        ImportContext {
            jobs: &self.jobs,
            files: &self.files,
            plans: &self.plans,
            journal,
            build: &self.build,
            actor: "owner",
            staging_dir: &self.data,
            stop: &self.stop,
            retry_unit,
        }
    }

    /// Submit the import job of `plan` under `operation` and keep its plan, as the import
    /// route does.
    fn submit(&self, plan: &ImportPlan, operation: &str) -> filesquid_api::jobs::Job {
        let job = submit_import(&self.jobs, plan, operation).expect("job");
        self.plans.save(&job.id, operation, plan).expect("plan");
        job
    }

    /// The stored job `id`.
    fn job(&self, id: &str) -> filesquid_api::jobs::Job {
        self.store.get(id).expect("store").expect("job")
    }

    /// Import `path` alone into a new object `file_id` as job `job_id` (another run's
    /// commit).
    async fn import_as(&self, path: &Path, file_id: &str, job_id: &str) {
        let request = FileImport {
            library_id: LIBRARY,
            file_id,
            path,
            import_root: path,
            method: ImportMethod::FilePicker,
            job_id: Some(job_id.into()),
            operation_id: None,
            build: &self.build,
            actor: "owner",
        };
        import_file(&self.files, &request).await.expect("imported");
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
        sources: vec![root.to_path_buf()],
        method: ImportMethod::Folder,
    }
}

/// How many file objects the library's storage root holds.
fn objects(dir: &Path) -> usize {
    std::fs::read_dir(dir.join("root").join("files")).map_or(0, |d| d.count())
}

/// A fresh file identifier.
fn new_id() -> String {
    filesquid_api::library::mint_uuid_v4().expect("id")
}

/// The files of `listed`, relative to `root`.
fn files_of(listed: &[Listed], root: &Path) -> Vec<PathBuf> {
    listed
        .iter()
        .filter_map(|entry| match entry {
            Listed::File { path, .. } => Some(path.strip_prefix(root).expect("inside").into()),
            Listed::Skipped { .. } => None,
        })
        .collect()
}

/// The codes of the skipped entries of `listed`.
fn skipped_of(listed: &[Listed]) -> Vec<&'static str> {
    listed
        .iter()
        .filter_map(|entry| match entry {
            Listed::Skipped { code, .. } => Some(*code),
            Listed::File { .. } => None,
        })
        .collect()
}

#[test]
fn a_folder_is_listed_in_one_stable_order_with_its_subfolders() {
    let dir = TempDir::new("list");
    let folder = holiday_folder(&dir.0);
    let listed = list_plan(&plan(&folder)).expect("listed");
    assert_eq!(
        files_of(&listed, &folder),
        [
            PathBuf::from("Day 2/c.txt"),
            PathBuf::from("a.jpg"),
            PathBuf::from("b.png")
        ]
    );
    assert!(skipped_of(&listed).is_empty());
}

#[test]
fn what_cannot_be_listed_is_kept_with_its_code() {
    let dir = TempDir::new("skipped");
    let folder = holiday_folder(&dir.0);
    let mut deep = folder.join("deep");
    for level in 0..=MAX_WALK_DEPTH {
        deep = deep.join(format!("{level}"));
    }
    std::fs::create_dir_all(&deep).expect("deep");
    std::fs::write(deep.join("lost.txt"), b"x").expect("deep file");
    #[cfg(unix)]
    std::os::unix::fs::symlink(folder.join("Day 2"), folder.join("link")).expect("link");
    let picked = ImportPlan {
        sources: vec![folder.clone(), dir.0.join("gone.jpg")],
        ..plan(&folder)
    };
    let listed = list_plan(&picked).expect("listed");
    let mut skipped = skipped_of(&listed);
    skipped.sort_unstable();
    #[cfg(unix)]
    assert_eq!(
        skipped,
        [
            "folder_link_not_followed",
            "folder_too_deep",
            "source_missing"
        ]
    );
    assert_eq!(
        files_of(&listed, &folder).len(),
        3,
        "the files are still listed"
    );
}

#[tokio::test]
async fn a_folder_is_imported_as_one_job_with_a_checkpoint_per_file() {
    let dir = TempDir::new("import");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-import");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(run.imported.len(), 3);
    assert!(run.quarantined.is_empty());
    let stored = rig.job(&job.id);
    assert_eq!(
        stored.checkpoint.as_deref(),
        Some(import_checkpoint(3).as_str())
    );
    assert_eq!((stored.done, stored.total), (3, Some(3)));
    assert_eq!(rig.plans.manifest(&job.id).expect("manifest").len(), 3);
    assert!(
        rig.plans
            .unfinished_after(None, 10)
            .expect("plans")
            .is_empty(),
        "no plan was kept, so none is unfinished"
    );
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

/// The manifest of the holiday folder with the file at `index` given `file_id`.
fn manifest_with(folder: &Path, index: usize, file_id: &str) -> Vec<ManifestEntry> {
    let mut manifest = manifest_of(list_plan(&plan(folder)).expect("listed")).expect("manifest");
    if let ManifestEntry::File { file_id: id, .. } = &mut manifest[index] {
        *id = file_id.to_string();
    }
    manifest
}

#[tokio::test]
async fn a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on() {
    let dir = TempDir::new("quarantine");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let taken = new_id();
    rig.import_as(&folder.join("b.png"), &taken, "job-other")
        .await;
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-quarantine");
    let manifest = manifest_with(&folder, 1, &taken);
    rig.plans
        .save_manifest(&job.id, &manifest)
        .expect("manifest");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(run.imported.len(), 2, "the rest go on");
    assert_eq!(
        run.quarantined,
        ["conflict"],
        "a retryable failure, retried"
    );
    assert_eq!(run.retries, MAX_ATTEMPTS - 1, "five attempts in all");
    let failed: Vec<_> = rig
        .entries(IMPORT_FILE_OPERATION)
        .into_iter()
        .filter(|e| e.error_code.is_some())
        .collect();
    assert_eq!(failed.len(), 1, "quarantined once, after its attempts");
}

#[tokio::test]
async fn a_file_too_large_for_this_build_is_quarantined_without_retries() {
    let dir = TempDir::new("large");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let huge = std::fs::File::create(folder.join("huge.bin")).expect("huge");
    huge.set_len(513 * 1024 * 1024).expect("sparse");
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-large");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(run.imported.len(), 3);
    assert_eq!(run.quarantined, ["too_large_for_this_build"]);
    assert_eq!(run.retries, 0);
}

#[tokio::test]
async fn a_quit_suspends_between_files_and_the_resume_imports_only_the_rest() {
    let dir = TempDir::new("resume");
    let rig = Arc::new(Rig::new(&dir.0).await);
    let folder = holiday_folder(&dir.0);
    let taken = new_id();
    rig.import_as(&folder.join("a.jpg"), &taken, "job-other")
        .await;
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-resume");
    rig.plans
        .save_manifest(&job.id, &manifest_with(&folder, 1, &taken))
        .expect("manifest");
    let watcher = {
        let rig = rig.clone();
        std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while rig.entries(IMPORT_FILE_OPERATION).is_empty()
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            rig.stop.store(true, Ordering::SeqCst);
        })
    };
    let journal = rig.monitor.journal();
    let slow = rig.context_with(&journal, Duration::from_secs(5));
    let run = run_import_job(&slow, &job.id, &plan).await.expect("ran");
    watcher.join().expect("watcher");
    assert_eq!(run.state, JobState::Pending, "suspended at quit");
    assert_eq!(run.imported.len(), 1, "the file before the quit");
    assert_eq!(
        rig.job(&job.id).checkpoint.as_deref(),
        Some(import_checkpoint(1).as_str())
    );
    std::fs::write(folder.join("zz-new.txt"), b"new").expect("a file added meanwhile");
    rig.stop.store(false, Ordering::SeqCst);
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("resumed");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(run.imported.len(), 1, "only the last file of the manifest");
    assert_eq!(run.quarantined, ["conflict"]);
    assert_eq!(
        objects(&dir.0),
        3,
        "the other job's object and two of this job"
    );
}

#[tokio::test]
async fn a_file_committed_before_a_crash_is_answered_not_imported_twice() {
    let dir = TempDir::new("replay");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-replay");
    let committed = new_id();
    rig.import_as(&folder.join("Day 2").join("c.txt"), &committed, &job.id)
        .await;
    rig.plans
        .save_manifest(&job.id, &manifest_with(&folder, 0, &committed))
        .expect("manifest");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(run.imported.len(), 3);
    assert_eq!(run.imported[0].file_id, committed);
    assert_eq!(run.imported[0].version, "v1");
    assert_eq!(
        objects(&dir.0),
        3,
        "no second object for the committed file"
    );
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
fn a_plan_and_its_manifest_survive_a_restart_and_one_operation_has_one_job() {
    let dir = TempDir::new("plans");
    let path = dir.0.join("filesquid.sqlite");
    let folder = holiday_folder(&dir.0);
    let picked = ImportPlan {
        sources: vec![folder.join("a.jpg"), folder.join("b.png")],
        method: ImportMethod::FilePicker,
        ..plan(&folder)
    };
    let manifest = vec![
        ManifestEntry::File {
            path: folder.join("a.jpg"),
            source: 0,
            file_id: new_id(),
        },
        ManifestEntry::Skipped {
            source: 1,
            code: "source_missing".into(),
        },
    ];
    {
        let plans = SqliteImportPlans::open(&path).expect("open");
        plans.save("job-1", "op-1", &picked).expect("save");
        assert!(
            plans.save("job-2", "op-1", &picked).is_err(),
            "one job per operation"
        );
        assert_eq!(
            plans.save_manifest("job-1", &manifest).expect("kept"),
            manifest
        );
        let other = vec![manifest[1].clone()];
        assert_eq!(
            plans.save_manifest("job-1", &other).expect("kept"),
            manifest,
            "the first manifest stays"
        );
    }
    let plans = SqliteImportPlans::open(&path).expect("reopen");
    assert_eq!(plans.load("job-1").expect("load"), Some(picked));
    assert_eq!(plans.manifest("job-1").expect("manifest"), manifest);
    assert_eq!(
        plans.job_of_operation("op-1").expect("op"),
        Some("job-1".into())
    );
    assert_eq!(plans.job_of_operation("op-2").expect("op"), None);
    assert_eq!(plans.load("job-9").expect("load"), None);
    assert_eq!(plans.unfinished_after(None, 10).expect("page"), ["job-1"]);
    plans.finish("job-1").expect("finish");
    assert!(plans.unfinished_after(None, 10).expect("page").is_empty());
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
    assert_eq!(
        listed,
        [
            Listed::File {
                path: folder.join("b.png"),
                source: 0
            },
            Listed::File {
                path: folder.join("a.jpg"),
                source: 1
            }
        ],
        "in the order picked"
    );
    let job = rig.submit(&picked, "op-picked");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &picked)
        .await
        .expect("ran");
    assert_eq!(run.imported.len(), 2);
}

/// The worker of `rig` over its plans, recovering unfinished jobs.
fn worker(rig: &Rig, stop: &Arc<AtomicBool>) -> ImportWorker {
    ImportWorker::spawn(ImportServices {
        jobs: rig.jobs.clone(),
        files: rig.files.clone(),
        plans: rig.plans.clone(),
        journal: rig.monitor.journal(),
        build: rig.build.clone(),
        actor: "owner".into(),
        staging_dir: rig.data.clone(),
        stop: stop.clone(),
        retry_unit: Duration::from_millis(1),
        recover: true,
    })
    .expect("worker")
}

/// Wait (at most 60 s) until `count` files are journaled as imported.
fn wait_for(rig: &Rig, count: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while rig.entries(IMPORT_FILE_OPERATION).len() < count && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[tokio::test]
async fn the_worker_runs_every_unfinished_job_beyond_one_queue_full() {
    let dir = TempDir::new("worker");
    let rig = Rig::new(&dir.0).await;
    let jobs = 70;
    for n in 0..jobs {
        let file = dir.0.join(format!("file-{n}.txt"));
        std::fs::write(&file, format!("{n}")).expect("file");
        let single = ImportPlan {
            sources: vec![file.clone()],
            method: ImportMethod::FilePicker,
            ..plan(&file)
        };
        let job = submit_import(&rig.jobs, &single, &format!("op-{n}")).expect("job");
        rig.plans
            .save(&job.id, &format!("op-{n}"), &single)
            .expect("save");
    }
    let stop = Arc::new(AtomicBool::new(false));
    let worker = worker(&rig, &stop);
    wait_for(&rig, jobs);
    stop.store(true, Ordering::SeqCst);
    worker.wait(Duration::from_secs(10));
    assert_eq!(rig.entries(IMPORT_FILE_OPERATION).len(), jobs);
    assert!(rig
        .jobs
        .unfinished_of("import", 100)
        .expect("jobs")
        .is_empty());
    assert!(rig
        .plans
        .unfinished_after(None, 100)
        .expect("plans")
        .is_empty());
}

#[tokio::test]
async fn a_queued_job_without_a_plan_is_failed_not_left_pending() {
    let dir = TempDir::new("no-plan");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let job = submit_import(&rig.jobs, &plan(&folder), "op-lost").expect("job");
    let stop = Arc::new(AtomicBool::new(false));
    let worker = worker(&rig, &stop);
    worker.enqueue(&job.id, &job.source).expect("queued");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while rig.job(&job.id).state != JobState::Failed && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    stop.store(true, Ordering::SeqCst);
    worker.wait(Duration::from_secs(10));
    let stored = rig.job(&job.id);
    assert_eq!(stored.state, JobState::Failed);
    assert_eq!(stored.error_code.as_deref(), Some("plan_missing"));
}
