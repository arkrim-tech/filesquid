//! Unit tests for the import job of Tasks DE-1.2 and DE-1.7 (F-002, NFR-003, DE-S-014,
//! DE-S-019): a folder imported as one durable job, one stable listing kept as the job's
//! manifest (unreadable entries kept with their codes), a checkpoint after each file, a quit
//! that suspends between files and a resume that imports only the remaining files, a file
//! committed before a crash answered and not imported twice, a file that keeps failing
//! quarantined after five attempts without stopping the batch, the staging reserve, the plans
//! store, the worker's paged recovery of every unfinished job, and journal entries with no
//! path. Since DE-1.7 every import names its destination collection and folder: each file is
//! registered there at its logical path in the library's index (ADR 041), a taken path is
//! numbered, and a crash between a file's object and its registration resumes.

use filesquid_api::jobs::{JobItemState, JobItems, JobState, JobStore, Jobs, MemoryJobStore};
use filesquid_api::journal::{JournalEntry, JournalQuery, MemoryJournal};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::library::{CatalogChange, CatalogCommit, LibraryCatalog};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::metrics::Metrics;
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_api::EventBus;
use filesquid_core::LibraryId;
use filesquid_core::{Collection, CollectionId, RepositoryId};
use filesquid_metadata::import::ImportMethod;
use filesquid_native::catalog::{LibraryFolder, PendingIntent, RegisterFile, Registration};
use filesquid_native::import::file::{import_file, FileImport};
use filesquid_native::import::host::Build;
use filesquid_native::import::index::ImportIndex;
use filesquid_native::import::items::SqliteJobItems;
use filesquid_native::import::job::{
    backoff, checkpoint_next, import_checkpoint, list_plan, manifest_of, reserve_for,
    run_import_job, submit_import, ImportContext, Listed, IMPORT_FILE_OPERATION, MAX_ATTEMPTS,
    MAX_BACKOFF, MAX_WALK_DEPTH,
};
use filesquid_native::import::plans::{ImportPlan, ManifestEntry, SqliteImportPlans};
use filesquid_native::import::worker::{ImportServices, ImportWorker};
use filesquid_native::{open_fs_library_root, HostFiles, HostLocations, SqliteCatalog};
use filesquid_ocfl::storage::FsBackend;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The library every test imports into.
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// The destination collection of every test's imports.
const COLLECTION: &str = "c0ffee00-1234-4abc-8def-0123456789ab";

/// The destination folder inside it.
const FOLDER: &str = "trips";

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

/// The host of one test: a library in its own folder, indexed by a catalog and served by a
/// file service through the same leases, a job service and a journal.
struct Rig {
    files: HostFiles,
    catalog: Arc<SqliteCatalog>,
    store: Arc<MemoryJobStore>,
    jobs: Jobs,
    plans: Arc<SqliteImportPlans>,
    items: Arc<SqliteJobItems>,
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
        std::fs::create_dir_all(&data).expect("data");
        let locations = HostLocations::new(&data);
        let location = locations.register(&root).expect("location");
        let host_root = locations.root_of(&location).expect("root");
        open_fs_library_root(&files, &id, &host_root, &data).expect("opened");
        let catalog = Arc::new(
            SqliteCatalog::open(
                &data.join("libraries"),
                Arc::new(MemoryJournal::new("desktop")),
            )
            .expect("catalog"),
        );
        let folder = LibraryFolder {
            library_id: id.clone(),
            repository_id: RepositoryId::new("repo-1").expect("repository"),
            name: "Library".into(),
            root: host_root,
            identity: locations.identity_of(&location).expect("identity"),
        };
        catalog.attach(&folder).expect("attached");
        add_collection(&catalog, &id);
        let store = Arc::new(MemoryJobStore::new());
        let jobs = Jobs::new(
            store.clone(),
            monitor.journal(),
            monitor.registry().clone(),
            "test-launch",
        );
        let plans = Arc::new(SqliteImportPlans::open(&dir.join("plans.sqlite")).expect("plans"));
        let items = Arc::new(SqliteJobItems::open(&dir.join("plans.sqlite")).expect("items"));
        Self {
            files,
            catalog,
            store,
            jobs,
            plans,
            items,
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
        self.context_over(journal, retry_unit, self.catalog.as_ref())
    }

    fn context_over<'a>(
        &'a self,
        journal: &'a filesquid_api::journal_writer::JournalSender,
        retry_unit: Duration,
        index: &'a dyn ImportIndex,
    ) -> ImportContext<'a> {
        ImportContext {
            jobs: &self.jobs,
            files: &self.files,
            index,
            plans: &self.plans,
            items: &self.items,
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

    /// The collection logical paths registered in the destination collection, sorted.
    fn registered(&self) -> Vec<String> {
        let collection = CollectionId::new(COLLECTION).expect("collection");
        let mut found = Vec::new();
        let mut stack = vec![String::new()];
        for _ in 0..1_000 {
            let Some(folder) = stack.pop() else {
                break;
            };
            let page = self
                .catalog
                .scope_entries(&collection, &folder, None, 1_000)
                .expect("entries")
                .expect("indexed");
            for entry in page.entries {
                let join = |name: &str| match folder.as_str() {
                    "" => name.to_string(),
                    folder => format!("{folder}/{name}"),
                };
                match entry {
                    filesquid_api::files::ScopeEntry::Folder { name } => stack.push(join(&name)),
                    filesquid_api::files::ScopeEntry::File { name, .. } => found.push(join(&name)),
                }
            }
        }
        found.sort();
        found
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
            progress: None,
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

/// Store the destination collection [`COLLECTION`] in `id`'s index (a library command would).
fn add_collection(catalog: &SqliteCatalog, id: &LibraryId) {
    let collection = Collection::new(
        CollectionId::new(COLLECTION).expect("collection"),
        id.clone(),
        "Holidays",
    )
    .expect("collection");
    let revision = catalog
        .library(id)
        .expect("read")
        .expect("library")
        .revision;
    let commit = CatalogCommit {
        operation_id: OperationId::new("op-collection").expect("op"),
        library_id: id.clone(),
        expected_revision: ExpectedRevision::new(revision).expect("revision"),
        changes: vec![CatalogChange::PutCollection(collection)],
        journal: JournalEntry::new(
            "library.collection.create",
            filesquid_api::journal::Outcome::Succeeded,
        ),
        created_id: None,
        deletion: None,
        restored_into: Vec::new(),
    };
    catalog.commit(commit).expect("collection");
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

/// The plan of importing `root` into [`COLLECTION`], folder [`FOLDER`].
fn plan(root: &Path) -> ImportPlan {
    ImportPlan {
        library_id: LIBRARY.into(),
        collection_id: Some(COLLECTION.into()),
        folder: Some(FOLDER.into()),
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
    // Each file's commit is journaled once as `ocfl.commit` under the job's operation ID and
    // names the version the file's `import.file` entry names (API-S-028).
    let commits = rig.entries("ocfl.commit");
    assert_eq!(commits.len(), 3, "one commit entry per file: {commits:?}");
    for entry in &entries {
        let reference = entry
            .object_ref
            .as_deref()
            .expect("the file's object version");
        let linked = commits.iter().filter(|c| {
            c.object_ref.as_deref() == Some(reference)
                && c.operation_id.as_deref() == Some("op-import")
        });
        assert_eq!(linked.count(), 1, "{reference}");
    }
    assert_eq!(
        rig.registered(),
        [
            "trips/Holidays/Day 2/c.txt",
            "trips/Holidays/a.jpg",
            "trips/Holidays/b.png"
        ],
        "each file in its home collection beneath the destination folder"
    );
}

/// The manifest of the holiday folder with the file at `index` given `file_id`, placed by
/// `rig`'s index.
fn manifest_with(rig: &Rig, folder: &Path, index: usize, file_id: &str) -> Vec<ManifestEntry> {
    let plan = plan(folder);
    let listed = list_plan(&plan).expect("listed");
    let mut manifest = manifest_of(listed, &plan, rig.catalog.as_ref()).expect("manifest");
    if let ManifestEntry::File { file_id: id, .. } = &mut manifest[index] {
        *id = file_id.to_string();
    }
    manifest
}

/// The state, bytes done and error code of every item of job `job_id`, in order.
fn item_states(rig: &Rig, job_id: &str) -> Vec<(JobItemState, u64, Option<String>)> {
    let page = rig.items.page(job_id, None, 1_000).expect("items");
    page.items
        .into_iter()
        .map(|item| (item.state, item.bytes_done, item.error_code))
        .collect()
}

#[tokio::test]
async fn each_file_s_state_is_kept_and_a_failed_file_is_retried_alone() {
    let dir = TempDir::new("items");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-items");
    let listed = list_plan(&plan).expect("listed");
    let manifest = manifest_of(listed, &plan, rig.catalog.as_ref()).expect("manifest");
    rig.plans
        .save_manifest(&job.id, &manifest)
        .expect("manifest");
    std::fs::remove_file(folder.join("b.png")).expect("b gone");
    let journal = rig.monitor.journal();
    let first = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(first.state, JobState::Pending, "waits for a retry");
    assert_eq!(first.quarantined, ["source_missing"]);
    let states = item_states(&rig, &job.id);
    assert_eq!(states[0], (JobItemState::Copied, 1, None), "Day 2/c.txt");
    assert_eq!(states[1], (JobItemState::Copied, 3, None), "a.jpg");
    assert_eq!(
        states[2],
        (JobItemState::Failed, 0, Some("source_missing".to_string()))
    );
    let items = rig.items.page(&job.id, None, 1_000).expect("items").items;
    assert_eq!(items[2].name, "b.png", "the base name only");
    std::fs::write(folder.join("b.png"), b"bbbb").expect("b back");
    std::fs::remove_file(folder.join("a.jpg")).expect("a gone: never read again");
    let failed = items[2].item_id.clone();
    assert_eq!(
        rig.items.retry(&job.id, Some(&failed)).expect("retry"),
        [failed]
    );
    let second = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(second.state, JobState::Completed);
    assert_eq!(second.imported.len(), 1, "only the retried file");
    assert!(second.quarantined.is_empty(), "{:?}", second.quarantined);
    assert_eq!(
        item_states(&rig, &job.id)[2],
        (JobItemState::Copied, 4, None)
    );
    assert_eq!(objects(&dir.0), 3);
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
    let manifest = manifest_with(&rig, &folder, 1, &taken);
    rig.plans
        .save_manifest(&job.id, &manifest)
        .expect("manifest");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(
        run.state,
        JobState::Pending,
        "a job with a failed file waits for a retry"
    );
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
async fn a_file_that_streams_longer_than_the_lease_keeps_the_job_s_lease_alive() {
    let dir = TempDir::new("lease");
    let rig = Rig::new(&dir.0).await;
    let folder = dir.0.join("Big");
    std::fs::create_dir_all(&folder).expect("folder");
    let big = std::fs::File::create(folder.join("big.raw")).expect("big");
    big.set_len(96 * 1024 * 1024).expect("sparse");
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-lease");
    let journal = rig.monitor.journal();
    let short = rig.jobs.clone().with_lease_ttl(LEASE_TTL_MS);
    let context = ImportContext {
        jobs: &short,
        ..rig.context(&journal)
    };
    let run = run_import_job(&context, &job.id, &plan)
        .await
        .expect("the lease held through the file");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(run.imported.len(), 1);
    assert_eq!(rig.jobs.job(&job.id).expect("job").attempts, 1, "one run");
}

/// The lease of [`a_file_that_streams_longer_than_the_lease_keeps_the_job_s_lease_alive`]:
/// far shorter than staging and committing its 96 MiB file takes (two reads, two hashes and
/// two writes of the file), so the run's lease must be renewed while the file is imported.
const LEASE_TTL_MS: i64 = 300;

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
        .save_manifest(&job.id, &manifest_with(&rig, &folder, 1, &taken))
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
    assert_eq!(
        run.state,
        JobState::Pending,
        "the conflicted file waits for a retry"
    );
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
        .save_manifest(&job.id, &manifest_with(&rig, &folder, 0, &committed))
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
            logical_path: Some("trips/a.jpg".into()),
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
        index: rig.catalog.clone(),
        plans: rig.plans.clone(),
        items: rig.items.clone(),
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

/// Longest wait for the worker to import every job: the 70 single-file jobs finish in about
/// 9 s alone, but a loaded full run (every test binary at once) once took longer than 60 s, so
/// the bound leaves room for that; a run that finishes sooner returns sooner.
const ALL_JOBS_WAIT: Duration = Duration::from_secs(300);

/// Wait (at most [`ALL_JOBS_WAIT`]) until `count` files are journaled as imported.
fn wait_for(rig: &Rig, count: usize) {
    let deadline = std::time::Instant::now() + ALL_JOBS_WAIT;
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

/// An index that forwards to the rig's catalog but answers every path as taken, fails the
/// first registration and sets a stop flag, as a crash between a file's object and its
/// registration would, or fails every registration, as an index that cannot be written.
struct Scripted<'a> {
    inner: &'a SqliteCatalog,
    all_taken: bool,
    crash_once: std::sync::Mutex<Option<&'a AtomicBool>>,
    refuse_writes: bool,
}

impl ImportIndex for Scripted<'_> {
    fn has_collection(
        &self,
        library: &LibraryId,
        collection: &CollectionId,
    ) -> Result<bool, String> {
        self.inner.has_collection(library, collection)
    }

    fn path_taken(&self, collection: &CollectionId, path: &str) -> Result<bool, String> {
        match self.all_taken {
            true => Ok(true),
            false => ImportIndex::path_taken(self.inner, collection, path),
        }
    }

    fn begin(&self, intent: &PendingIntent) -> Result<(), String> {
        self.inner.begin(intent)
    }

    fn register(
        &self,
        library: &LibraryId,
        file: &RegisterFile,
        journal: JournalEntry,
    ) -> Result<Registration, String> {
        if let Some(stop) = self.crash_once.lock().expect("lock").take() {
            stop.store(true, Ordering::SeqCst);
            return Err("crashed before the registration".into());
        }
        if self.refuse_writes {
            return Err("the index cannot be written".into());
        }
        self.inner.register(library, file, journal)
    }

    fn finish(&self, library: &LibraryId, operation: &OperationId) -> Result<(), String> {
        self.inner.finish(library, operation)
    }
}

#[tokio::test]
async fn a_folder_imported_again_is_numbered_beside_the_first() {
    let dir = TempDir::new("numbered-folder");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let journal = rig.monitor.journal();
    for op in ["op-first", "op-again"] {
        let plan = plan(&folder);
        let job = rig.submit(&plan, op);
        let run = run_import_job(&rig.context(&journal), &job.id, &plan)
            .await
            .expect("ran");
        assert_eq!(run.imported.len(), 3, "{op}");
    }
    let again: Vec<String> = rig
        .registered()
        .into_iter()
        .filter(|path| path.starts_with("trips/Holidays-1/"))
        .collect();
    assert_eq!(
        again,
        [
            "trips/Holidays-1/Day 2/c.txt",
            "trips/Holidays-1/a.jpg",
            "trips/Holidays-1/b.png"
        ]
    );
}

#[tokio::test]
async fn a_deleted_file_s_path_counts_as_taken() {
    let dir = TempDir::new("deleted-taken");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let journal = rig.monitor.journal();
    let single = ImportPlan {
        sources: vec![folder.join("a.jpg")],
        method: ImportMethod::FilePicker,
        ..plan(&folder)
    };
    let job = rig.submit(&single, "op-first");
    let first = run_import_job(&rig.context(&journal), &job.id, &single)
        .await
        .expect("ran");
    let file = filesquid_core::FileId::new(first.imported[0].file_id.clone()).expect("file");
    let id = LibraryId::new(LIBRARY).expect("library");
    let revision = rig
        .catalog
        .library(&id)
        .expect("read")
        .expect("library")
        .revision;
    let deletion = filesquid_api::library::Deletion {
        deleted_at_ms: 1_792_281_600_000,
        purge_after_ms: 1_792_281_600_000 + 30 * 86_400_000,
        cause: Default::default(),
    };
    let delete = CatalogCommit {
        operation_id: OperationId::new("op-delete").expect("op"),
        library_id: id.clone(),
        expected_revision: ExpectedRevision::new(revision).expect("revision"),
        changes: vec![CatalogChange::SetFileState {
            file_id: file,
            state: filesquid_api::library::FileState {
                hidden: false,
                deletion: Some(deletion),
            },
        }],
        journal: JournalEntry::new(
            "library.file.delete",
            filesquid_api::journal::Outcome::Succeeded,
        ),
        created_id: None,
        deletion: Some(deletion),
        restored_into: Vec::new(),
    };
    rig.catalog.commit(delete).expect("deleted");
    let job = rig.submit(&single, "op-again");
    run_import_job(&rig.context(&journal), &job.id, &single)
        .await
        .expect("ran");
    assert_eq!(
        rig.registered(),
        ["trips/a-1.jpg"],
        "the deleted file keeps its place"
    );
}

#[tokio::test]
async fn a_numbered_name_over_255_bytes_is_quarantined_as_name_too_long() {
    let dir = TempDir::new("too-long");
    let rig = Rig::new(&dir.0).await;
    let long = dir.0.join(format!("{}.jpg", "n".repeat(251)));
    std::fs::write(&long, b"long").expect("a 255-byte name");
    let journal = rig.monitor.journal();
    let single = ImportPlan {
        sources: vec![long.clone()],
        method: ImportMethod::FilePicker,
        ..plan(&long)
    };
    let job = rig.submit(&single, "op-first");
    let run = run_import_job(&rig.context(&journal), &job.id, &single)
        .await
        .expect("ran");
    assert_eq!(run.imported.len(), 1);
    let job = rig.submit(&single, "op-again");
    let run = run_import_job(&rig.context(&journal), &job.id, &single)
        .await
        .expect("ran");
    assert_eq!(run.quarantined, ["name_too_long"]);
    assert_eq!(objects(&dir.0), 1, "no object for the refused name");
    let failed: Vec<_> = rig
        .entries(IMPORT_FILE_OPERATION)
        .into_iter()
        .filter(|e| e.error_code.as_deref() == Some("name_too_long"))
        .collect();
    assert_eq!(failed.len(), 1);
    assert!(
        !format!("{failed:?}").contains("nnnn"),
        "no file name journaled"
    );
}

#[tokio::test]
async fn a_source_whose_numbers_are_all_taken_is_quarantined_as_path_suffix_exhausted() {
    let dir = TempDir::new("exhausted");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let index = Scripted {
        inner: rig.catalog.as_ref(),
        all_taken: true,
        crash_once: std::sync::Mutex::new(None),
        refuse_writes: false,
    };
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-exhausted");
    let journal = rig.monitor.journal();
    let context = rig.context_over(&journal, Duration::from_millis(1), &index);
    let run = run_import_job(&context, &job.id, &plan).await.expect("ran");
    assert_eq!(run.state, JobState::Completed);
    assert!(run.imported.is_empty());
    assert_eq!(run.quarantined, ["path_suffix_exhausted"; 3]);
    assert_eq!(objects(&dir.0), 0, "nothing committed");
}

#[tokio::test]
async fn a_crash_between_a_file_s_object_and_its_registration_resumes_and_registers_once() {
    let dir = TempDir::new("crash-register");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-crash");
    let journal = rig.monitor.journal();
    let index = Scripted {
        inner: rig.catalog.as_ref(),
        all_taken: false,
        crash_once: std::sync::Mutex::new(Some(&rig.stop)),
        refuse_writes: false,
    };
    let crashing = rig.context_over(&journal, Duration::from_millis(1), &index);
    let run = run_import_job(&crashing, &job.id, &plan)
        .await
        .expect("ran");
    assert_eq!(run.state, JobState::Pending, "suspended at the first file");
    assert_eq!(objects(&dir.0), 1, "its object was committed");
    assert!(rig.registered().is_empty(), "but not registered");
    let id = LibraryId::new(LIBRARY).expect("library");
    let pending = rig.catalog.pending_intents(&id, 10).expect("intents");
    assert_eq!(pending.len(), 1, "the import's operation stays pending");
    assert_eq!(pending[0].operation_id.as_str(), "op-crash");
    rig.stop.store(false, Ordering::SeqCst);
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("resumed");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(
        objects(&dir.0),
        3,
        "the first file was answered, not imported twice"
    );
    assert_eq!(rig.registered().len(), 3, "and registered once");
    assert!(
        rig.catalog
            .pending_intents(&id, 10)
            .expect("intents")
            .is_empty(),
        "the finished job clears its intent"
    );
}

#[tokio::test]
async fn a_plan_without_a_destination_fails_its_job_as_destination_required() {
    let dir = TempDir::new("no-destination");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let earlier = ImportPlan {
        collection_id: None,
        folder: None,
        ..plan(&folder)
    };
    let job = rig.submit(&earlier, "op-earlier");
    let journal = rig.monitor.journal();
    let failed = run_import_job(&rig.context(&journal), &job.id, &earlier).await;
    assert!(failed.is_err());
    let stored = rig.job(&job.id);
    assert_eq!(stored.state, JobState::Failed);
    assert_eq!(stored.error_code.as_deref(), Some("destination_required"));
    assert_eq!(objects(&dir.0), 0);
}

#[tokio::test]
async fn a_file_the_index_keeps_refusing_to_register_suspends_the_job_and_keeps_its_intent() {
    let dir = TempDir::new("register-down");
    let rig = Rig::new(&dir.0).await;
    let folder = holiday_folder(&dir.0);
    let plan = plan(&folder);
    let job = rig.submit(&plan, "op-down");
    let journal = rig.monitor.journal();
    let index = Scripted {
        inner: rig.catalog.as_ref(),
        all_taken: false,
        crash_once: std::sync::Mutex::new(None),
        refuse_writes: true,
    };
    let down = rig.context_over(&journal, Duration::from_millis(1), &index);
    let run = run_import_job(&down, &job.id, &plan).await.expect("ran");
    assert_eq!(run.state, JobState::Pending, "suspended, not finished");
    assert!(
        run.quarantined.is_empty(),
        "the committed file is not given up"
    );
    assert_eq!(objects(&dir.0), 1, "its object was committed");
    let id = LibraryId::new(LIBRARY).expect("library");
    let pending = rig.catalog.pending_intents(&id, 10).expect("intents");
    assert_eq!(pending.len(), 1, "the import's operation stays pending");
    let run = run_import_job(&rig.context(&journal), &job.id, &plan)
        .await
        .expect("resumed");
    assert_eq!(run.state, JobState::Completed);
    assert_eq!(
        objects(&dir.0),
        3,
        "the committed file was answered, not imported twice"
    );
    assert_eq!(rig.registered().len(), 3, "every file registered");
}

#[tokio::test]
async fn two_picked_folders_of_one_name_land_beside_each_other() {
    let dir = TempDir::new("same-name");
    let rig = Rig::new(&dir.0).await;
    let first = holiday_folder(&dir.0.join("camera"));
    let second = dir.0.join("phone").join("Holidays");
    std::fs::create_dir_all(&second).expect("second folder");
    std::fs::write(second.join("d.jpg"), b"dd").expect("d");
    let both = ImportPlan {
        sources: vec![first.clone(), second],
        ..plan(&first)
    };
    let job = rig.submit(&both, "op-both");
    let journal = rig.monitor.journal();
    let run = run_import_job(&rig.context(&journal), &job.id, &both)
        .await
        .expect("ran");
    assert_eq!(run.imported.len(), 4);
    assert_eq!(
        rig.registered(),
        [
            "trips/Holidays-1/d.jpg",
            "trips/Holidays/Day 2/c.txt",
            "trips/Holidays/a.jpg",
            "trips/Holidays/b.png"
        ]
    );
}
