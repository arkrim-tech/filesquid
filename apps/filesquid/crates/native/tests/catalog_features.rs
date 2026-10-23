//! Cucumber harness for the per-library SQLite catalog Gherkin under
//! `tests/catalog_features/` (Task DE-0.2: F-001, F-009, DE-S-009; Task DE-0.3: NFR-001,
//! F-042, DE-S-010; Task DE-0.5: NFR-002, NFR-003, DE-S-016, the media workers under
//! `media_workers_NFR-002.feature`). Needs the `desktop` feature. Writes JUnit to `tests/allure-results/catalog-cucumber-junit.xml`.

#[path = "support/catalog_fixture.rs"]
mod catalog_fixture;

use catalog_fixture::library_ocfl_root;
use catalog_fixture::{all_entries, catalog_file, temp_root, Fixture, FlakyJournal};
use cucumber::{given, then, when, writer, World, WriterExt as _};
use filesquid_api::canonical::{read_snapshot, OwnerReadable};
use filesquid_api::journal::{JournalEntry, JournalStore, MemoryJournal, Outcome};
use filesquid_api::library::{
    CatalogChange, CatalogCommit, CatalogError, CommandResult, LibraryCatalog, LibraryCommand,
};
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_core::{CollectionId, FileId, LibraryId, VirtualCollectionId, VirtualMembership};
use filesquid_native::workers::media::{
    vips, MediaError, MediaWorkers, Rendered, ThumbnailRequest,
};
use filesquid_native::CommitProbe;
use filesquid_ocfl::storage::{complete_immediately, FsBackend};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// State shared by the steps of one scenario.
#[derive(Default, World)]
#[world(init = Self::default)]
struct CatalogWorld {
    root: Option<PathBuf>,
    flaky: Option<Arc<FlakyJournal>>,
    fixture: Option<Fixture>,
    libraries: BTreeMap<String, LibraryId>,
    collection: Option<CollectionId>,
    first: Option<CommandResult>,
    retry: Option<CommandResult>,
    failed: bool,
    backup: Option<PathBuf>,
    /// How many entries the host journal held right after the first index.
    journaled: Option<usize>,
    /// The media worker scenario's state.
    media: MediaRig,
}

/// A media worker scenario: its directory, monitor, worker pool and last answer.
#[derive(Default)]
struct MediaRig {
    dir: Option<PathBuf>,
    monitor: Option<filesquid_api::monitor::Monitor>,
    pool: Option<Arc<MediaWorkers>>,
    source: Option<PathBuf>,
    answer: Option<Result<Rendered, MediaError>>,
    /// Run the worker logic in this process (the desktop tests run it out of process).
    direct: bool,
}

impl std::fmt::Debug for CatalogWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CatalogWorld").finish_non_exhaustive()
    }
}

impl CatalogWorld {
    /// The fixture of the scenario.
    fn fx(&self) -> &Fixture {
        self.fixture.as_ref().expect("fixture")
    }

    /// Close the catalog, keeping its directory for a reopen.
    fn close(&mut self) {
        if let Some(fixture) = self.fixture.take() {
            fixture.release();
        }
    }

    /// The library created under `name`.
    fn library(&self, name: &str) -> LibraryId {
        self.libraries.get(name).cloned().expect("library")
    }

    /// Close the catalog and open the same directory again over `journal`.
    fn reopen(&mut self, journal: Arc<dyn JournalStore>) {
        self.close();
        let root = self.root.clone().expect("root");
        self.fixture = Some(Fixture::over(root, journal));
    }
}

/// Distinguishes the temporary directories of the scenarios of one run.
static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

/// A directory name unique within this run, starting with `prefix`.
fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", NEXT_DIR.fetch_add(1, Ordering::SeqCst))
}

/// The collection command the retry scenario repeats.
fn summer(library_id: &LibraryId) -> LibraryCommand {
    LibraryCommand::CreateCollection {
        library_id: library_id.clone(),
        title: "Summer".into(),
        folder_id: None,
        sort: Default::default(),
    }
}

#[given(expr = "an empty catalog directory")]
fn empty_directory(w: &mut CatalogWorld) {
    let fixture = Fixture::new(&unique("bdd"));
    w.root = Some(fixture.root.clone());
    w.fixture = Some(fixture);
}

#[given(expr = "an empty catalog directory whose host journal is unreachable")]
fn unreachable_journal(w: &mut CatalogWorld) {
    let root = temp_root(&unique("bdd-flaky"));
    let flaky = FlakyJournal::down();
    w.fixture = Some(Fixture::over(root.clone(), flaky.clone()));
    w.root = Some(root);
    w.flaky = Some(flaky);
}

#[given(expr = "the owner created the library {string} with the collection {string}")]
fn created_with_collection(w: &mut CatalogWorld, library: String, title: String) {
    let id = w.fx().create_library(&library);
    let op = OperationId::new("op-summer").expect("op");
    assert_eq!(title, "Summer");
    let result = w.fx().run_as(op, "1", summer(&id)).expect("collection");
    w.collection = result
        .created_id
        .clone()
        .map(|c| CollectionId::new(c).expect("id"));
    w.first = Some(result);
    w.libraries.insert(library, id);
}

#[when(expr = "the owner creates the libraries {string} and {string}")]
fn create_two(w: &mut CatalogWorld, a: String, b: String) {
    for name in [a, b] {
        let id = w.fx().create_library(&name);
        w.libraries.insert(name, id);
    }
}

#[when(expr = "the owner creates the library {string}")]
fn create_one(w: &mut CatalogWorld, name: String) {
    let id = w.fx().create_library(&name);
    w.libraries.insert(name, id);
}

#[when(expr = "the catalog is closed and opened again")]
fn close_and_open(w: &mut CatalogWorld) {
    w.reopen(Arc::new(MemoryJournal::new("desktop")));
}

#[when(expr = "the same collection command is retried after a restart")]
fn retry_after_restart(w: &mut CatalogWorld) {
    w.reopen(Arc::new(MemoryJournal::new("desktop")));
    let id = w.library("Family archive");
    let op = OperationId::new("op-summer").expect("op");
    w.retry = Some(w.fx().run_as(op, "1", summer(&id)).expect("retry"));
}

#[when(expr = "the owner renames the collection {string} to {string}")]
fn rename_collection(w: &mut CatalogWorld, _title: String, new_title: String) {
    let id = w.library("Family archive");
    let op = OperationId::new("op-rename").expect("op");
    let revision = w.fx().revision(&id);
    let command = LibraryCommand::RenameCollection {
        library_id: id,
        collection_id: w.collection.clone().expect("collection"),
        title: new_title,
    };
    w.fx().run_as(op, &revision, command).expect("rename");
}

#[when(expr = "the host journal is reachable again when the catalog reopens")]
fn journal_back(w: &mut CatalogWorld) {
    let flaky = w.flaky.clone().expect("flaky journal");
    w.close();
    flaky.down.store(false, Ordering::SeqCst);
    w.reopen(flaky);
}

#[when(expr = "a commit with a membership of a missing file is applied")]
fn bad_commit(w: &mut CatalogWorld) {
    let id = w.library("Family archive");
    let member = VirtualMembership::new(
        VirtualCollectionId::new("vc-missing").expect("vc"),
        FileId::new("f-missing").expect("file"),
        0,
    );
    let commit = CatalogCommit {
        operation_id: OperationId::new("op-bad").expect("op"),
        library_id: id.clone(),
        expected_revision: ExpectedRevision::new("2").expect("revision"),
        changes: vec![CatalogChange::AddVirtualMember(member)],
        journal: JournalEntry::new("library.virtual_collection.add", Outcome::Succeeded),
        created_id: None,
        deletion: None,
    };
    w.failed = w.fx().catalog.commit(commit).is_err();
}

#[when(expr = "the owner backs up the catalog of {string}")]
fn back_up(w: &mut CatalogWorld, name: String) {
    let id = w.library(&name);
    let dest_dir = temp_root(&unique("bdd-backup"));
    w.fx()
        .catalog
        .backup(&id, &catalog_file(&dest_dir, &id))
        .expect("backup");
    w.backup = Some(dest_dir);
}

#[then(expr = "the catalog directory should hold one catalog file per library")]
fn one_file_each(w: &mut CatalogWorld) {
    for id in w.libraries.values() {
        assert!(w.fx().file_of(id).exists(), "{id:?}");
    }
    let files = std::fs::read_dir(w.root.as_ref().expect("root"))
        .expect("dir")
        .filter(|e| {
            let name = e.as_ref().expect("entry").file_name();
            name.to_string_lossy().ends_with(".sqlite")
        })
        .count();
    assert_eq!(files, w.libraries.len());
}

#[then(expr = "no catalog file should be named after a library name")]
fn not_named_after_names(w: &mut CatalogWorld) {
    let root = w.root.as_ref().expect("root");
    for entry in std::fs::read_dir(root).expect("dir") {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        assert!(!name.contains("Family") && !name.contains("Work"), "{name}");
    }
}

#[then(expr = "the library {string} should be at revision {int}")]
fn at_revision(w: &mut CatalogWorld, name: String, revision: u32) {
    assert_eq!(w.fx().revision(&w.library(&name)), revision.to_string());
}

#[then(expr = "the collection {string} should still be listed")]
fn still_listed(w: &mut CatalogWorld, title: String) {
    let id = w.collection.clone().expect("collection");
    let found = w
        .fx()
        .catalog
        .collection(&id)
        .expect("read")
        .expect("there");
    assert_eq!(found.title(), title);
}

#[then(expr = "the retry should answer the first result")]
fn same_answer(w: &mut CatalogWorld) {
    let first = w.first.as_ref().expect("first");
    let retry = w.retry.as_ref().expect("retry");
    assert_eq!(retry.created_id, first.created_id);
    assert_eq!(retry.revision, first.revision);
}

#[then(expr = "the host journal should record {string}")]
fn journal_records(w: &mut CatalogWorld, operation: String) {
    let entries = all_entries(w.fx().journal.as_ref());
    assert!(
        entries.iter().any(|e| e.operation == operation),
        "{entries:?}"
    );
}

/// Index the file `file` of the "Family archive" library in its collection titled `title`
/// at listing order `order`, journaled as `library.file.index`. The scenario's only
/// collection is "Summer" (created by the background step), so any other title fails.
fn index_file(w: &CatalogWorld, file: &str, title: &str, order: i64) {
    let id = w.library("Family archive");
    let titles: Vec<String> = w
        .fx()
        .catalog
        .collections(&id, None, 10)
        .expect("collections")
        .into_iter()
        .map(|c| c.item.title().to_string())
        .collect();
    assert_eq!(
        titles,
        [title],
        "the scenario indexes into its only collection"
    );
    let home = w.collection.clone().expect("collection");
    let file = filesquid_core::File::new(FileId::new(file).expect("file"), id.clone(), home);
    let entry = JournalEntry::new("library.file.index", Outcome::Succeeded);
    w.fx()
        .catalog
        .index_files(&id, &[(file, order)], entry)
        .expect("index");
}

#[given(expr = "the file {string} was indexed in the collection {string} at listing order {int}")]
fn file_indexed(w: &mut CatalogWorld, file: String, title: String, order: i64) {
    index_file(w, &file, &title, order);
    w.journaled = Some(all_entries(w.fx().journal.as_ref()).len());
}

#[when(
    expr = "the file {string} is indexed in the collection {string} at listing order {int} again"
)]
fn file_indexed_again(w: &mut CatalogWorld, file: String, title: String, order: i64) {
    index_file(w, &file, &title, order);
}

#[then(expr = "the host journal should hold no entry since the first index")]
fn journal_unchanged_since_index(w: &mut CatalogWorld) {
    let entries = all_entries(w.fx().journal.as_ref());
    let since: Vec<_> = entries
        .iter()
        .skip(w.journaled.expect("first index"))
        .collect();
    assert!(since.is_empty(), "{since:?}");
}

#[then(regex = r#"^the host journal should record "([^"]+)" (\d+) times?$"#)]
fn journal_records_times(w: &mut CatalogWorld, operation: String, times: usize) {
    let entries = all_entries(w.fx().journal.as_ref());
    let found = entries.iter().filter(|e| e.operation == operation).count();
    assert_eq!(found, times, "{entries:?}");
}

#[then(expr = "the catalog should hold no receipt for the rename")]
fn no_rename_receipt(w: &mut CatalogWorld) {
    let op = OperationId::new("op-rename").expect("op");
    assert_eq!(w.fx().catalog.receipt(&op).expect("read"), None);
}

#[then(expr = "the host journal should not record {string}")]
fn journal_lacks(w: &mut CatalogWorld, operation: String) {
    let entries = all_entries(w.fx().journal.as_ref());
    assert!(
        entries.iter().all(|e| e.operation != operation),
        "{entries:?}"
    );
}

#[then(expr = "the commit should fail")]
fn commit_failed(w: &mut CatalogWorld) {
    assert!(w.failed);
}

#[then(expr = "the backup should open as a catalog at revision {int}")]
fn backup_opens(w: &mut CatalogWorld, revision: u32) {
    let dir = w.backup.clone().expect("backup");
    let library = w.library("Family archive");
    let copy = rusqlite::Connection::open(catalog_file(&dir, &library)).expect("open the copy");
    let stored: i64 = copy
        .query_row("SELECT revision FROM library", [], |r| r.get(0))
        .expect("the committed library");
    assert_eq!(stored, i64::from(revision));
    drop(copy);
    let _ = std::fs::remove_dir_all(dir);
}

/// Stops the commit of one operation before or after its snapshot, as a crash would.
struct CrashAt {
    operation: String,
    before_snapshot: bool,
}

impl CommitProbe for CrashAt {
    fn before_snapshot(&self, operation_id: &str) -> Result<(), CatalogError> {
        self.stop(operation_id, true)
    }

    fn after_snapshot(&self, operation_id: &str) -> Result<(), CatalogError> {
        self.stop(operation_id, false)
    }
}

impl CrashAt {
    fn stop(&self, operation_id: &str, before: bool) -> Result<(), CatalogError> {
        if operation_id == self.operation && before == self.before_snapshot {
            return Err(CatalogError::Unavailable("simulated crash".into()));
        }
        Ok(())
    }
}

/// Open an empty catalog directory whose commit of `operation` crashes at one stop.
fn crashing_directory(w: &mut CatalogWorld, operation: String, before_snapshot: bool) {
    let root = temp_root(&unique("bdd-crash"));
    let probe = Arc::new(CrashAt {
        operation,
        before_snapshot,
    });
    w.fixture = Some(Fixture::crashing(
        root.clone(),
        Arc::new(MemoryJournal::new("desktop")),
        probe,
    ));
    w.root = Some(root);
}

#[given(expr = "an empty catalog directory whose commits crash before the snapshot of {string}")]
fn crash_before(w: &mut CatalogWorld, operation: String) {
    crashing_directory(w, operation, true);
}

#[given(expr = "an empty catalog directory whose commits crash after the snapshot of {string}")]
fn crash_after(w: &mut CatalogWorld, operation: String) {
    crashing_directory(w, operation, false);
}

#[given(expr = "the owner created the library {string}")]
fn created_library(w: &mut CatalogWorld, name: String) {
    create_one(w, name);
}

#[when(expr = "the owner creates the library {string} with the collection {string}")]
fn creates_with_collection(w: &mut CatalogWorld, library: String, title: String) {
    created_with_collection(w, library, title);
}

#[when(expr = "the owner adds the collection {string} under operation {string}")]
fn add_collection(w: &mut CatalogWorld, title: String, operation: String) {
    let id = w.library("Family archive");
    let command = LibraryCommand::CreateCollection {
        library_id: id,
        title,
        folder_id: None,
        sort: Default::default(),
    };
    let op = OperationId::new(operation).expect("op");
    w.failed = w.fx().run_as(op, "1", command).is_err();
}

#[when(expr = "the host restarts")]
fn host_restarts(w: &mut CatalogWorld) {
    w.reopen(Arc::new(MemoryJournal::new("desktop")));
}

#[when(expr = "the catalog file is deleted and the host restarts")]
fn catalog_deleted(w: &mut CatalogWorld) {
    let file = w.fx().file_of(&w.library("Family archive"));
    w.close();
    std::fs::remove_file(&file).expect("remove the catalog file");
    for suffix in ["-wal", "-shm"] {
        let mut name = file.clone().into_os_string();
        name.push(suffix);
        match std::fs::remove_file(PathBuf::from(name)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!("remove a catalog sidecar: {err}"),
        }
    }
    let root = w.root.clone().expect("root");
    w.fixture = Some(Fixture::over(root, Arc::new(MemoryJournal::new("desktop"))));
}

#[then(expr = "the library's OCFL snapshot should be at version {string}")]
fn snapshot_version(w: &mut CatalogWorld, version: String) {
    let lib = w.library("Family archive");
    let ocfl = library_ocfl_root(w.root.as_ref().expect("root"), &lib).expect("root");
    let backend = FsBackend::with_default_staging(ocfl).expect("backend");
    let snapshot = complete_immediately(read_snapshot(&backend, &OwnerReadable))
        .expect("drive")
        .expect("read")
        .expect("snapshot");
    assert_eq!(snapshot.version.to_string(), version);
}

#[then(expr = "replaying {string} should answer its first result without a new version")]
fn replay_answers(w: &mut CatalogWorld, operation: String) {
    let id = w.library("Family archive");
    let before = w.fx().revision(&id);
    let command = LibraryCommand::CreateCollection {
        library_id: id.clone(),
        title: "Summer".into(),
        folder_id: None,
        sort: Default::default(),
    };
    let op = OperationId::new(operation).expect("op");
    let replay = w.fx().run_as(op, "1", command).expect("replay");
    assert_eq!(replay.revision.as_deref(), Some(before.as_str()));
    snapshot_version(w, "v2".to_string());
}

#[then(expr = "no intent should be pending")]
fn none_pending(w: &mut CatalogWorld) {
    let lib = w.library("Family archive");
    assert!(w
        .fx()
        .catalog
        .pending_intents(&lib, 10)
        .expect("intents")
        .is_empty());
}

#[then(expr = "the intent of {string} should be pending")]
fn pending(w: &mut CatalogWorld, operation: String) {
    let lib = w.library("Family archive");
    let intents = w.fx().catalog.pending_intents(&lib, 10).expect("intents");
    assert!(
        intents.iter().any(|i| i.operation_id.as_str() == operation),
        "{intents:?}"
    );
}

#[then(expr = "the collection {string} should be listed")]
fn listed(w: &mut CatalogWorld, title: String) {
    let lib = w.library("Family archive");
    let titles: Vec<String> = w
        .fx()
        .catalog
        .collections(&lib, None, 100)
        .expect("collections")
        .into_iter()
        .map(|k| k.item.title().to_string())
        .collect();
    assert_eq!(titles, [title]);
}

/// A monitor over an in-memory journal, for the media worker scenarios.
fn media_monitor() -> filesquid_api::monitor::Monitor {
    filesquid_api::monitor::Monitor::start(
        Arc::new(MemoryJournal::new("desktop")),
        Arc::new(filesquid_api::monitor::StaticHostChecks::healthy()),
        filesquid_api::monitor::MonitorConfig::new("desktop"),
        filesquid_api::EventBus::new(),
    )
}

impl MediaRig {
    /// Start the scenario with a pool running `program` with `args` (filesystem I/O).
    fn start(&mut self, program: &str, args: &[&str]) {
        let dir = temp_root("media");
        let monitor = media_monitor();
        let pool = MediaWorkers::new(PathBuf::from(program), monitor.registry().clone())
            .with_args(args.iter().map(|a| a.to_string()).collect());
        std::fs::write(dir.join("still.ppm"), b"P6\n1 1\n255\n\0\0\0").expect("still");
        self.dir = Some(dir);
        self.pool = Some(Arc::new(pool));
        self.monitor = Some(monitor);
    }

    /// Start a scenario that runs the worker logic in this process (filesystem I/O).
    fn start_direct(&mut self) {
        self.dir = Some(temp_root("media"));
        self.direct = true;
    }

    /// Ask for a thumbnail of the scenario's still: from the pool, or from the worker logic
    /// directly.
    fn ask(&self, max_edge: u32) -> Result<Rendered, MediaError> {
        let dir = self.dir.as_ref().expect("dir");
        let request = ThumbnailRequest {
            source: self.source.clone().unwrap_or_else(|| dir.join("still.ppm")),
            dest: dir.join("thumb.png"),
            max_edge,
        };
        match self.direct {
            true => serve_directly(&request),
            false => self.pool.as_ref().expect("pool").thumbnail(&request),
        }
    }

    /// Stop the monitor and remove the directory.
    fn finish(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            monitor.shutdown();
        }
        if let Some(dir) = self.dir.take() {
            std::fs::remove_dir_all(dir).expect("cleanup");
        }
    }
}

/// The worker logic's answer to `request`, as the pool would map it.
fn serve_directly(request: &ThumbnailRequest) -> Result<Rendered, MediaError> {
    let json = serde_json::json!({
        "op": "thumbnail",
        "source": request.source,
        "dest": request.dest,
        "max_edge": request.max_edge,
    });
    let answer: serde_json::Value =
        serde_json::from_str(&vips::serve(&json.to_string())).expect("answer");
    if answer["ok"] != true {
        let code = answer["error_code"].as_str().unwrap_or_default();
        return Err(MediaError::Refused(code.to_string()));
    }
    let size = |key: &str| u32::try_from(answer[key].as_u64().unwrap_or(0)).unwrap_or(0);
    Ok(Rendered {
        width: size("width"),
        height: size("height"),
    })
}

#[given(expr = "a still of {int} by {int} pixels")]
fn still(w: &mut CatalogWorld, width: usize, height: usize) {
    w.media.start_direct();
    let path = w.media.dir.as_ref().expect("dir").join("still.ppm");
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    bytes.resize(bytes.len() + width * height * 3, 90);
    std::fs::write(&path, bytes).expect("ppm");
    w.media.source = Some(path);
}

#[given(expr = "a still that only ImageMagick could decode")]
fn magick_only_still(w: &mut CatalogWorld) {
    w.media.start_direct();
    let path = w.media.dir.as_ref().expect("dir").join("still.bmp");
    let mut bytes = b"BM".to_vec();
    for word in [70u32, 0, 54, 40, 2, 2] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&24u16.to_le_bytes());
    bytes.resize(70, 200);
    std::fs::write(&path, bytes).expect("bmp");
    w.media.source = Some(path);
}

#[given(expr = "workers that each take a moment")]
fn slow_workers(w: &mut CatalogWorld) {
    let script = r#"cat >/dev/null; sleep 0.2; echo '{"ok":true,"width":1,"height":1}'"#;
    w.media.start("sh", &["-c", script]);
}

#[given(expr = "a worker that crashes")]
fn crashing_worker(w: &mut CatalogWorld) {
    let marker = std::env::temp_dir().join(format!("fs-media-crashed-{}", std::process::id()));
    let _ = std::fs::remove_file(&marker);
    let script = format!(
        r#"if [ -e '{0}' ]; then cat >/dev/null; echo '{{"ok":true,"width":1,"height":1}}'; else touch '{0}'; kill -9 $$; fi"#,
        marker.display()
    );
    w.media.start("sh", &["-c", &script]);
}

#[when(expr = "the host asks for a thumbnail of at most {int} pixels")]
fn ask_thumbnail(w: &mut CatalogWorld, max_edge: u32) {
    w.media.answer = Some(w.media.ask(max_edge));
}

#[when(expr = "six thumbnails are asked for at once")]
fn ask_six(w: &mut CatalogWorld) {
    let pool = w.media.pool.clone().expect("pool");
    let dir = w.media.dir.clone().expect("dir");
    let runs: Vec<_> = (0..6)
        .map(|n| {
            let (pool, dir) = (pool.clone(), dir.clone());
            std::thread::spawn(move || {
                pool.thumbnail(&ThumbnailRequest {
                    source: dir.join("still.ppm"),
                    dest: dir.join(format!("out-{n}.png")),
                    max_edge: 8,
                })
            })
        })
        .collect();
    for run in runs {
        run.join().expect("thread").expect("rendered");
    }
}

/// Whether the scenario's answer is the refusal of a machine without libvips, which the
/// libvips scenarios accept in place of their outcome.
fn libvips_missing(w: &CatalogWorld) -> bool {
    matches!(w.media.answer, Some(Err(MediaError::Refused(ref code))) if code == "libvips_unavailable")
}

#[then(expr = "the thumbnail should be {int} by {int} pixels")]
fn thumbnail_size(w: &mut CatalogWorld, width: u32, height: u32) {
    if !libvips_missing(w) {
        let rendered = w.media.answer.clone().expect("asked").expect("rendered");
        assert_eq!((rendered.width, rendered.height), (width, height));
    }
    w.media.finish();
}

#[then(expr = "the request should be refused as {string}")]
fn refused_as(w: &mut CatalogWorld, code: String) {
    if !libvips_missing(w) {
        let answer = w.media.answer.clone().expect("asked");
        assert_eq!(answer.err().map(|e| e.error_code().to_string()), Some(code));
    }
    if w.media.source.is_some() {
        w.media.finish();
    }
}

#[then(expr = "the next run through the same workers should succeed")]
fn next_succeeds(w: &mut CatalogWorld) {
    assert!(
        w.media.ask(16).is_ok(),
        "the same pool runs the next worker"
    );
    w.media.finish();
}

#[then(expr = "no more than {int} workers should have run at the same time")]
fn peak_at_most(w: &mut CatalogWorld, most: usize) {
    let peak = w.media.pool.as_ref().expect("pool").peak_running();
    assert!(peak <= most && peak > 0, "{peak}");
    w.media.finish();
}

#[tokio::main]
async fn main() {
    let results = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    std::fs::create_dir_all(&results).expect("allure-results");
    let junit_path = results.join("catalog-cucumber-junit.xml");
    let junit = std::fs::File::create(junit_path).expect("junit file");
    CatalogWorld::cucumber()
        .max_concurrent_scenarios(1)
        .with_writer(
            writer::Basic::stdout()
                .summarized()
                .tee::<CatalogWorld, _>(writer::JUnit::for_tee(junit, 0))
                .normalized(),
        )
        .fail_on_skipped()
        // Scenarios tagged @unix use `sh` stand-in workers; other platforms skip them (the
        // desktop crate runs the real worker out of process on every platform).
        .filter_run_and_exit(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/catalog_features"),
            |_, _, scenario| cfg!(unix) || !scenario.tags.iter().any(|tag| tag == "unix"),
        )
        .await;
}
