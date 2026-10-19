//! Cucumber harness for the per-library SQLite catalog Gherkin under
//! `tests/catalog_features/` (Task DE-0.2: F-001, F-009, DE-S-009). Needs the `desktop`
//! feature. Writes JUnit to `tests/allure-results/catalog-cucumber-junit.xml`.

#[path = "support/catalog_fixture.rs"]
mod catalog_fixture;

use catalog_fixture::{all_entries, catalog_file, temp_root, Fixture, FlakyJournal};
use cucumber::{given, then, when, writer, World, WriterExt as _};
use filesquid_api::journal::{JournalEntry, JournalStore, MemoryJournal, Outcome};
use filesquid_api::library::{
    CatalogChange, CatalogCommit, CommandResult, LibraryCatalog, LibraryCommand,
};
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_core::{CollectionId, FileId, LibraryId, VirtualCollectionId, VirtualMembership};
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

#[then(expr = "the commit should fail")]
fn commit_failed(w: &mut CatalogWorld) {
    assert!(w.failed);
}

#[then(expr = "the backup should open as a catalog at revision {int}")]
fn backup_opens(w: &mut CatalogWorld, revision: u32) {
    let dir = w.backup.clone().expect("backup");
    let copy = Fixture::over(dir, Arc::new(MemoryJournal::new("desktop")));
    assert_eq!(
        copy.revision(&w.library("Family archive")),
        revision.to_string()
    );
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
        .run_and_exit(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/catalog_features"))
        .await;
}
