//! Shared fixture for the per-library SQLite catalog tests (Task DE-0.2): a
//! `LibraryService` over a `SqliteCatalog` in a fresh temporary directory, with the host's
//! journal store and a bounded failure journal.

#![allow(dead_code)]

use filesquid_api::journal::{
    JournalEntry, JournalError, JournalPage, JournalQuery, JournalStore, MemoryJournal,
    RetentionPolicy, MAX_PAGE,
};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::library::{CommandResult, LibraryCommand, LibraryService, NEW_LIBRARY_REVISION};
use filesquid_api::metrics::Metrics;
use filesquid_api::operation::{CommandEnvelope, ExpectedRevision, OperationId, OperationScope};
use filesquid_api::EventBus;
use filesquid_core::{CollectionId, LibraryId, RepositoryId, VirtualCollectionId};
use filesquid_native::{CommitProbe, SqliteCatalog};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Upper bound on journal pages read by [`all_entries`] (AGENTS.md §22).
const MAX_JOURNAL_PAGES: usize = 1_000;

/// A fresh, empty temporary directory named after `name` (filesystem I/O).
pub fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-catalog-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// A journal store whose writes fail while `down` is set, standing in for an unreachable
/// host journal.
pub struct FlakyJournal {
    /// The store that receives entries while the journal is up.
    pub inner: MemoryJournal,
    /// While set, every append fails.
    pub down: AtomicBool,
}

impl FlakyJournal {
    /// A store that starts down.
    pub fn down() -> Arc<Self> {
        Arc::new(Self {
            inner: MemoryJournal::new("desktop"),
            down: AtomicBool::new(true),
        })
    }
}

impl JournalStore for FlakyJournal {
    fn host(&self) -> &str {
        self.inner.host()
    }
    fn append(&self, entries: &[JournalEntry]) -> Result<Vec<i64>, JournalError> {
        if self.down.load(Ordering::SeqCst) {
            return Err(JournalError("journal down".into()));
        }
        self.inner.append(entries)
    }
    fn query(&self, query: &JournalQuery) -> Result<JournalPage, JournalError> {
        self.inner.query(query)
    }
    fn count(&self, query: &JournalQuery) -> Result<u64, JournalError> {
        self.inner.count(query)
    }
    fn apply_retention(&self, policy: RetentionPolicy, now_ms: i64) -> Result<u64, JournalError> {
        self.inner.apply_retention(policy, now_ms)
    }
    fn ping(&self) -> Result<(), JournalError> {
        self.inner.ping()
    }
}

/// Every entry of `store`, oldest first, read page by page (bounded).
pub fn all_entries(store: &dyn JournalStore) -> Vec<JournalEntry> {
    let mut entries = Vec::new();
    let mut after_id = None;
    for _ in 0..MAX_JOURNAL_PAGES {
        let query = JournalQuery {
            after_id,
            limit: MAX_PAGE,
            ..JournalQuery::default()
        };
        let page = store.query(&query).expect("journal query");
        entries.extend(page.entries);
        after_id = page.next_cursor;
        if after_id.is_none() {
            return entries;
        }
    }
    panic!("journal exceeds {MAX_JOURNAL_PAGES} pages");
}

/// A service over an on-disk catalog. Dropping it stops the failure journal writer.
pub struct Fixture {
    /// Root directory of the catalog files.
    pub root: PathBuf,
    /// The host journal store the catalog forwards commit entries to.
    pub journal: Arc<dyn JournalStore>,
    /// The catalog under test.
    pub catalog: Arc<SqliteCatalog>,
    /// The service over the catalog.
    pub service: LibraryService,
    /// The repository every library is created on.
    pub repository: RepositoryId,
    writer: JournalWriter,
    next_op: AtomicU64,
    owns_root: bool,
}

impl Drop for Fixture {
    /// Stop the failure journal writer thread and remove the catalog directory, unless
    /// [`Fixture::release`] kept it for a reopen (filesystem I/O).
    fn drop(&mut self) {
        self.writer.shutdown();
        if self.owns_root {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

impl Fixture {
    /// A fixture over a fresh directory named after `name`, journaling to memory.
    pub fn new(name: &str) -> Self {
        Self::over(temp_root(name), Arc::new(MemoryJournal::new("desktop")))
    }

    /// A fixture over the catalog files in `root`, forwarding to `journal` (opens SQLite). The
    /// fixture owns `root` and removes it when dropped.
    pub fn over(root: PathBuf, journal: Arc<dyn JournalStore>) -> Self {
        let catalog = SqliteCatalog::open(&root, journal.clone()).expect("open");
        Self::around(root, journal, catalog)
    }

    /// A fixture over `root` whose commits stop where `probe` says, as a crash would.
    pub fn crashing(
        root: PathBuf,
        journal: Arc<dyn JournalStore>,
        probe: Arc<dyn CommitProbe>,
    ) -> Self {
        let catalog = SqliteCatalog::open_with_probe(&root, journal.clone(), probe).expect("open");
        Self::around(root, journal, catalog)
    }

    /// A fixture around an opened `catalog` (starts the failure journal writer).
    fn around(root: PathBuf, journal: Arc<dyn JournalStore>, catalog: SqliteCatalog) -> Self {
        let catalog = Arc::new(catalog);
        let writer = JournalWriter::start(
            journal.clone(),
            WriterConfig::default(),
            EventBus::new(),
            Arc::new(Metrics::default()),
        );
        let service = LibraryService::new(catalog.clone(), writer.sender());
        Self {
            root,
            journal,
            catalog,
            service,
            repository: RepositoryId::new("repo-1").expect("repository"),
            writer,
            next_op: AtomicU64::new(0),
            owns_root: true,
        }
    }

    /// Close the catalog but keep its directory, answering it for a reopen with
    /// [`Fixture::over`] (which owns it again).
    pub fn release(mut self) -> PathBuf {
        self.owns_root = false;
        self.root.clone()
    }

    /// A fresh operation ID, unique across fixtures over the same directory (a reopened
    /// catalog keeps every receipt, so a reused ID would replay).
    pub fn op(&self) -> OperationId {
        let n = self.next_op.fetch_add(1, Ordering::SeqCst) + 1;
        let unique = filesquid_api::library::mint_uuid_v4().expect("uuid");
        OperationId::new(format!("op-{n}-{unique}")).expect("op")
    }

    /// Run `command` under `operation_id` at `revision`.
    pub fn run_as(
        &self,
        operation_id: OperationId,
        revision: &str,
        command: LibraryCommand,
    ) -> Result<CommandResult, filesquid_api::library::LibraryError> {
        let envelope = CommandEnvelope::new(
            operation_id,
            ExpectedRevision::new(revision).expect("revision"),
            OperationScope::new("library").expect("scope"),
        );
        let request = filesquid_api::library::CommandRequest { envelope, command };
        self.service.execute("user:ana", request)
    }

    /// Run `command` at `revision` with a fresh operation ID and expect success.
    pub fn run(&self, revision: &str, command: LibraryCommand) -> CommandResult {
        self.run_as(self.op(), revision, command).expect("command")
    }

    /// Create a library named `name`; answers its ID.
    pub fn create_library(&self, name: &str) -> LibraryId {
        let result = self.run(
            NEW_LIBRARY_REVISION,
            LibraryCommand::CreateLibrary {
                repository_id: self.repository.clone(),
                name: name.to_string(),
            },
        );
        LibraryId::new(result.created_id.expect("id")).expect("library id")
    }

    /// The current revision of `library_id`.
    pub fn revision(&self, library_id: &LibraryId) -> String {
        use filesquid_api::library::LibraryCatalog;
        self.catalog
            .library(library_id)
            .expect("library")
            .expect("present")
            .revision
    }

    /// Create a collection titled `title` in `library_id`.
    pub fn create_collection(&self, library_id: &LibraryId, title: &str) -> CollectionId {
        let result = self.run(
            &self.revision(library_id),
            LibraryCommand::CreateCollection {
                library_id: library_id.clone(),
                title: title.to_string(),
                folder_id: None,
                sort: Default::default(),
            },
        );
        CollectionId::new(result.created_id.expect("id")).expect("collection id")
    }

    /// Create a hand-picked virtual collection titled `title` in `library_id`.
    pub fn create_virtual(&self, library_id: &LibraryId, title: &str) -> VirtualCollectionId {
        let result = self.run(
            &self.revision(library_id),
            LibraryCommand::CreateVirtualCollection {
                library_id: library_id.clone(),
                title: title.to_string(),
                expression: None,
            },
        );
        VirtualCollectionId::new(result.created_id.expect("id")).expect("vc id")
    }

    /// The catalog file of `library_id` in this fixture's root.
    pub fn file_of(&self, library_id: &LibraryId) -> PathBuf {
        catalog_file(&self.root, library_id)
    }
}

/// The catalog file of `library_id` under `root`.
pub fn catalog_file(root: &Path, library_id: &LibraryId) -> PathBuf {
    root.join(format!("{}.sqlite", library_id.as_str()))
}
