//! Steps of `desktop_installation_F-067.feature` (Task DE-0.10; DE-S-012, DE-S-011,
//! SRS-F-067): the desktop's installation UUID read through the real launch path
//! (`load_installation` over the host monitor's operational store), and the replay of an
//! import whose commit was published but never journaled, through the desktop's own file
//! provenance (`install_file_provenance`). Runs in a scratch data directory that is removed
//! when the scenario ends.

use super::DesktopWorld;
use cucumber::{given, then, when};
use filesquid_api::journal::{parse_rfc3339_ms, JournalEntry, Outcome};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::ApiState;
use filesquid_core::{InstallationId, LibraryId};
use filesquid_desktop_lib::{install_file_provenance, load_installation, open_host_monitor};
use filesquid_metadata::import::ImportMethod;
use filesquid_native::import::file::{import_file, FileImport};
use filesquid_native::import::host::Build;
use filesquid_native::installation::INSTALLATION_FILE;
use filesquid_native::{open_fs_library, HostFiles, SqliteJournal, JOURNAL_FILE};
use filesquid_ocfl::storage::FsBackend;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Scenario counter, so concurrent scenarios get separate scratch directories.
static SCENARIOS: AtomicU32 = AtomicU32::new(0);

/// The library of the replay scenario.
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// The file the replay scenario imports.
const FILE_ID: &str = "0b1c2d3e-4f5a-4b6c-8d7e-9f0a1b2c3d4e";

/// The import job's operation in the replay scenario.
const OPERATION: &str = "op-replay";

/// The operational store and its write-ahead files.
const STORE_FILES: [&str; 3] = [JOURNAL_FILE, "filesquid.sqlite-wal", "filesquid.sqlite-shm"];

/// What one installation scenario set up and saw.
#[derive(Debug, Default)]
pub struct InstallationScenario {
    dir: Option<PathBuf>,
    launches: Vec<Result<InstallationId, String>>,
    file_after_first: Option<serde_json::Value>,
    recorded: Vec<JournalEntry>,
}

impl Drop for InstallationScenario {
    fn drop(&mut self) {
        let Some(dir) = &self.dir else { return };
        if let Err(err) = std::fs::remove_dir_all(dir) {
            eprintln!("scenario directory not removed: {err}");
        }
    }
}

/// The scenario's state.
fn scenario(world: &mut DesktopWorld) -> &mut InstallationScenario {
    world
        .installation
        .get_or_insert_with(InstallationScenario::default)
}

/// The scenario's scratch directory; its data directory is `<dir>/data`.
fn dir(world: &mut DesktopWorld) -> PathBuf {
    let state = scenario(world);
    state
        .dir
        .get_or_insert_with(|| {
            let n = SCENARIOS.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "fs-desktop-installation-{}-{n}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("data")).expect("data dir");
            dir
        })
        .clone()
}

/// The data directory of `dir`.
fn data(dir: &Path) -> PathBuf {
    dir.join("data")
}

/// One launch: open the host monitor, read the installation, stop. Filesystem and database
/// I/O.
fn launch(world: &mut DesktopWorld) {
    let data = data(&dir(world));
    let host = open_host_monitor(&ApiState::new(), &data, None).expect("monitor");
    let found = load_installation(&data, &host);
    host.stop();
    scenario(world).launches.push(found);
}

/// Every entry of the host's journal store. Database I/O.
fn journal(world: &mut DesktopWorld) -> Vec<JournalEntry> {
    let store =
        SqliteJournal::open(&data(&dir(world)).join(JOURNAL_FILE), "desktop").expect("journal");
    crate::journal::all_entries(&store)
}

/// The installation file, parsed.
fn file_json(world: &mut DesktopWorld) -> serde_json::Value {
    let text = std::fs::read_to_string(data(&dir(world)).join(INSTALLATION_FILE))
        .expect("installation file");
    serde_json::from_str(&text).expect("JSON")
}

/// The UUID the launch `n` read.
fn launched(world: &mut DesktopWorld, n: usize) -> InstallationId {
    scenario(world).launches[n].clone().expect("a launch")
}

/// Remove the operational store (filesystem I/O).
fn lose_store(world: &mut DesktopWorld) {
    let data = data(&dir(world));
    for name in STORE_FILES {
        let _ = std::fs::remove_file(data.join(name));
    }
}

#[given("a desktop data directory with no installation UUID")]
fn no_installation(world: &mut DesktopWorld) {
    let data = data(&dir(world));
    assert!(!data.join(INSTALLATION_FILE).exists());
}

#[given("a desktop that has launched once")]
fn launched_once(world: &mut DesktopWorld) {
    launch(world);
    let json = file_json(world);
    scenario(world).file_after_first = Some(json);
}

#[given("a desktop data directory whose installation file holds no UUID v4")]
fn malformed_file(world: &mut DesktopWorld) {
    let data = data(&dir(world));
    std::fs::write(
        data.join(INSTALLATION_FILE),
        r#"{"schema":1,"installation_id":"not-a-uuid","created":"2026-10-31T00:00:00.000Z"}"#,
    )
    .expect("write");
}

#[when("the desktop launches twice")]
fn launches_twice(world: &mut DesktopWorld) {
    launch(world);
    launch(world);
}

#[when("the desktop launches")]
fn launches(world: &mut DesktopWorld) {
    launch(world);
}

#[when(expr = "its {word} {word} is lost and the desktop launches again")]
fn copy_lost(world: &mut DesktopWorld, first: String, second: String) {
    match format!("{first} {second}").as_str() {
        "installation file" => {
            let path = data(&dir(world)).join(INSTALLATION_FILE);
            std::fs::remove_file(path).expect("remove the file");
        }
        "operational store" => lose_store(world),
        other => panic!("unknown copy {other}"),
    }
    launch(world);
}

#[when("both copies are lost and the desktop launches again")]
fn both_lost(world: &mut DesktopWorld) {
    let path = data(&dir(world)).join(INSTALLATION_FILE);
    std::fs::remove_file(path).expect("remove the file");
    lose_store(world);
    launch(world);
}

#[then("both launches should read the same UUID v4")]
fn same_uuid_v4(world: &mut DesktopWorld) {
    let (first, second) = (launched(world, 0), launched(world, 1));
    assert_eq!(first, second);
    assert_eq!(&first.as_str()[14..15], "4", "version 4");
}

#[then("the launch should read the same UUID as before")]
fn same_as_before(world: &mut DesktopWorld) {
    assert_eq!(launched(world, 0), launched(world, 1));
}

#[then("the launch should read a new UUID v4")]
fn new_uuid(world: &mut DesktopWorld) {
    let (first, second) = (launched(world, 0), launched(world, 1));
    assert_ne!(first, second);
    assert_eq!(&second.as_str()[14..15], "4", "version 4");
}

#[then("the installation file should hold schema 1, the UUID and an RFC 3339 UTC creation time")]
fn file_schema(world: &mut DesktopWorld) {
    let json = file_json(world);
    let id = launched(world, 0);
    assert_eq!(json["schema"], 1, "{json}");
    assert_eq!(json["installation_id"], id.as_str(), "{json}");
    let created = json["created"].as_str().expect("created");
    assert!(
        created.ends_with('Z') && parse_rfc3339_ms(created).is_some(),
        "{created}"
    );
}

#[then("the installation file should be as it was after the first launch")]
fn file_unchanged(world: &mut DesktopWorld) {
    let now = file_json(world);
    assert_eq!(scenario(world).file_after_first.as_ref(), Some(&now));
}

#[then(expr = "the journal should record {string} once")]
fn recorded_once(world: &mut DesktopWorld, operation: String) {
    let entries = journal(world);
    let found: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == operation)
        .collect();
    assert_eq!(found.len(), 1, "{entries:?}");
    assert_eq!(found[0].outcome, Outcome::Succeeded);
}

#[then("no journal entry should name the installation UUID or the data directory")]
fn names_nothing(world: &mut DesktopWorld) {
    let id = launched(world, 0);
    let dir = dir(world);
    let text = serde_json::to_string(&journal(world)).expect("json");
    assert!(!text.contains(id.as_str()), "{text}");
    assert!(!text.contains(dir.to_string_lossy().as_ref()), "{text}");
}

#[then("the launch should be refused with an error that names no path")]
fn refused(world: &mut DesktopWorld) {
    let dir = dir(world);
    let err = scenario(world).launches[0].clone().expect_err("refused");
    assert!(!err.contains(dir.to_string_lossy().as_ref()), "{err}");
    assert!(!err.contains("not-a-uuid"), "{err}");
}

#[then(expr = "the journal should record {string} failed with the error code {string}")]
fn recorded_failed(world: &mut DesktopWorld, operation: String, code: String) {
    let entries = journal(world);
    let found: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == operation)
        .collect();
    assert_eq!(found.len(), 1, "{entries:?}");
    assert_eq!(found[0].outcome, Outcome::Failed);
    assert_eq!(found[0].error_code.as_deref(), Some(code.as_str()));
}

/// This test build.
fn build() -> Build {
    Build {
        version: "0.1.0".into(),
        build: "test".into(),
        distribution: "filesquid".into(),
    }
}

/// An empty library storage root at `<dir>/root`, opened in a new file service whose
/// commits record their history through the desktop's own provenance wiring, with journal
/// IDs from `host`'s store. Filesystem and database I/O.
async fn recording_library(dir: &Path, host: &filesquid_native::HostMonitor) -> HostFiles {
    let root = dir.join("root");
    std::fs::create_dir_all(&root).expect("root");
    let backend = FsBackend::new(&root, dir.join("root-parts")).expect("backend");
    let id = LibraryId::new(LIBRARY).expect("library");
    LocalLibraryService::new(
        host.monitor().journal(),
        Arc::new(MemoryRepairLedger::new()),
    )
    .create_root("owner", &id, &backend)
    .await
    .expect("create root");
    let files = HostFiles::new();
    open_fs_library(&files, &id, &root, &data(dir)).expect("opened");
    let installation = load_installation(&data(dir), host).expect("installation");
    install_file_provenance(&files, host, &installation).expect("provenance");
    files
}

/// The import of `path` from its folder, under [`OPERATION`].
fn request<'a>(path: &'a Path, build: &'a Build) -> FileImport<'a> {
    FileImport {
        library_id: LIBRARY,
        file_id: FILE_ID,
        path,
        import_root: path.parent().expect("folder"),
        method: ImportMethod::Folder,
        job_id: Some("job-replay".into()),
        operation_id: Some(OPERATION.into()),
        build,
        actor: "owner",
        progress: None,
    }
}

/// A source file `<dir>/source/a.txt` (filesystem I/O).
fn source_file(dir: &Path) -> PathBuf {
    let folder = dir.join("source");
    std::fs::create_dir_all(&folder).expect("source");
    let path = folder.join("a.txt");
    std::fs::write(&path, b"alpha").expect("write");
    path
}

#[given("a desktop with an open library whose commits record their history")]
fn recording_desktop(world: &mut DesktopWorld) {
    let _ = dir(world);
}

#[given("a file was imported but the app stopped before its commit reached the journal")]
async fn crashed_import(world: &mut DesktopWorld) {
    let dir = dir(world);
    let host = open_host_monitor(&ApiState::new(), &data(&dir), None).expect("monitor");
    let files = recording_library(&dir, &host).await;
    let build = build();
    let path = source_file(&dir);
    let committed = import_file(&files, &request(&path, &build))
        .await
        .expect("imported");
    host.stop();
    scenario(world).recorded = committed.entries;
}

#[when("the import is replayed twice after the restart")]
async fn replayed(world: &mut DesktopWorld) {
    let dir = dir(world);
    for _ in 0..2 {
        let host = open_host_monitor(&ApiState::new(), &data(&dir), None).expect("monitor");
        let files = HostFiles::new();
        let id = LibraryId::new(LIBRARY).expect("library");
        open_fs_library(&files, &id, &dir.join("root"), &data(&dir)).expect("opened");
        let installation = load_installation(&data(&dir), &host).expect("installation");
        install_file_provenance(&files, &host, &installation).expect("provenance");
        let build = build();
        let path = dir.join("source").join("a.txt");
        let again = import_file(&files, &request(&path, &build))
            .await
            .expect("replayed");
        let journal = host.monitor().journal();
        again.entries.into_iter().for_each(|e| journal.send(e));
        host.stop();
    }
}

#[then(
    expr = "the journal should hold that import's {string} entry once, under the ID its version recorded"
)]
fn held_once(world: &mut DesktopWorld, operation: String) {
    let recorded = scenario(world)
        .recorded
        .iter()
        .find(|e| e.operation == operation)
        .cloned()
        .expect("the commit recorded its entry");
    let entries = journal(world);
    let held: Vec<_> = entries
        .iter()
        .filter(|e| e.operation == operation)
        .collect();
    assert_eq!(held.len(), 1, "{entries:?}");
    assert_eq!(held[0].id, recorded.id);
}

#[then("that entry should name the import's operation and object version and no path")]
fn names_ids_only(world: &mut DesktopWorld) {
    let dir = dir(world);
    let entries = journal(world);
    let entry = entries
        .iter()
        .find(|e| e.operation == "ocfl.commit")
        .expect("the commit entry");
    assert_eq!(entry.operation_id.as_deref(), Some(OPERATION));
    let reference = format!("urn:uuid:{FILE_ID}@v1");
    assert_eq!(entry.object_ref.as_deref(), Some(reference.as_str()));
    let text = serde_json::to_string(entry).expect("json");
    assert!(!text.contains(dir.to_string_lossy().as_ref()), "{text}");
    assert!(!text.contains("a.txt"), "{text}");
}
