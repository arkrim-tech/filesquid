//! Shared fixture for the library command and query tests (Task CL-0.2): a
//! `LibraryService` over a `MemoryCatalog` with a fixed clock, one repository and a bounded
//! failure journal over an in-memory journal store.

#![allow(dead_code)]

use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::library::{
    CommandRequest, CommandResult, FileScope, LibraryCommand, LibraryError, LibraryQuery,
    LibraryService, MemoryCatalog, QueryAnswer, QueryRequest, VisibilityFilter,
    NEW_LIBRARY_REVISION,
};
use filesquid_api::metrics::Metrics;
use filesquid_api::operation::{
    CommandEnvelope, ExpectedRevision, OperationId, OperationScope, PageSize, QueryEnvelope,
};
use filesquid_api::EventBus;
use filesquid_core::{
    CollectionId, File, FileId, Folder, FolderId, LibraryId, RepositoryId, VirtualCollectionId,
};
use std::cell::{Cell, RefCell};
use std::sync::Arc;

/// The fixed clock: 2026-10-18T00:00:00Z in Unix milliseconds.
pub const NOW_MS: i64 = 1_792_281_600_000;

/// One day in milliseconds.
pub const DAY_MS: i64 = 86_400_000;

/// Upper bound on pages read by one listing helper.
const MAX_PAGES: usize = 2_000;

/// A service, its catalog, a repository, its failure journal and (optionally) one current
/// library. Dropping the fixture stops the journal writer thread.
pub struct Fixture {
    /// The reference catalog behind the service.
    pub catalog: Arc<MemoryCatalog>,
    /// The service under test.
    pub service: LibraryService,
    /// The repository every library is created on.
    pub repository: RepositoryId,
    /// The journal store that receives refused commands and failed queries.
    pub journal: Arc<MemoryJournal>,
    writer: JournalWriter,
    library: RefCell<Option<LibraryId>>,
    next_op: Cell<u64>,
}

impl Drop for Fixture {
    /// Write what is queued and stop the journal writer thread.
    fn drop(&mut self) {
        self.writer.shutdown();
    }
}

impl Fixture {
    /// A fixture with no library.
    pub fn new() -> Self {
        Self::build()
    }

    /// A fixture whose current library "Family archive" exists.
    pub fn with_library() -> Self {
        Self::seeded(Self::build())
    }

    /// Start the failure journal and build the service over an empty catalog.
    fn build() -> Self {
        let catalog = Arc::new(MemoryCatalog::new());
        let journal = Arc::new(MemoryJournal::new("desktop"));
        let writer = JournalWriter::start(
            journal.clone(),
            WriterConfig::default(),
            EventBus::new(),
            Arc::new(Metrics::default()),
        );
        let service =
            LibraryService::new(catalog.clone(), writer.sender()).with_clock(Arc::new(|| NOW_MS));
        Self {
            catalog,
            service,
            repository: RepositoryId::new("repo-1").expect("repository"),
            journal,
            writer,
            library: RefCell::new(None),
            next_op: Cell::new(0),
        }
    }

    /// Every entry of the failure journal, oldest first, once the queue is written.
    pub fn failure_journal(&self) -> Vec<JournalEntry> {
        self.writer.sender().flush();
        self.journal
            .query(&JournalQuery::default())
            .expect("journal query")
            .entries
    }

    fn seeded(self) -> Self {
        let created = self.create_library("Family archive");
        let id = LibraryId::new(created.created_id.expect("id")).expect("library id");
        self.library.replace(Some(id));
        self
    }

    /// The current library.
    pub fn library(&self) -> LibraryId {
        self.library.borrow().clone().expect("fixture library")
    }

    /// The current library's revision.
    pub fn revision(&self) -> String {
        self.revision_of(&self.library())
    }

    /// The revision of `library_id`.
    pub fn revision_of(&self, library_id: &LibraryId) -> String {
        let answer = self
            .service
            .query(self.query(
                1,
                None,
                LibraryQuery::OpenLibrary {
                    library_id: library_id.clone(),
                },
            ))
            .expect("open library");
        match answer {
            QueryAnswer::Library(state) => state.revision,
            other => panic!("unexpected answer {other:?}"),
        }
    }

    /// A command request with a fresh operation ID and `revision`.
    pub fn request(&self, revision: &str, command: LibraryCommand) -> CommandRequest {
        let n = self.next_op.get() + 1;
        self.next_op.set(n);
        CommandRequest {
            envelope: CommandEnvelope::new(
                OperationId::new(format!("op-{n}")).expect("op"),
                ExpectedRevision::new(revision).expect("revision"),
                OperationScope::new("library").expect("scope"),
            ),
            command,
        }
    }

    /// A query request.
    pub fn query(
        &self,
        page_size: u32,
        cursor: Option<filesquid_api::library::Cursor>,
        query: LibraryQuery,
    ) -> QueryRequest {
        QueryRequest {
            envelope: QueryEnvelope::new(
                PageSize::new(page_size).expect("page size"),
                OperationScope::new("library").expect("scope"),
            ),
            cursor,
            query,
        }
    }

    /// Create a library named `name` on the fixture repository.
    pub fn create_library(&self, name: &str) -> CommandResult {
        let command = LibraryCommand::CreateLibrary {
            repository_id: self.repository.clone(),
            name: name.to_string(),
        };
        self.service
            .execute("user:ana", self.request(NEW_LIBRARY_REVISION, command))
            .expect("create library")
    }

    /// Run `command` on the current library at its current revision.
    pub fn try_run(&self, command: LibraryCommand) -> Result<CommandResult, LibraryError> {
        let request = self.request(&self.revision(), command);
        self.service.execute("user:ana", request)
    }

    /// Run `command` on the current library and expect success.
    pub fn run(&self, command: LibraryCommand) -> CommandResult {
        self.try_run(command).expect("command")
    }

    /// Run `command` at the current revision of `library_id` and expect success.
    pub fn run_in(&self, library_id: &LibraryId, command: LibraryCommand) -> CommandResult {
        let request = self.request(&self.revision_of(library_id), command);
        self.service.execute("user:ana", request).expect("command")
    }

    /// Create a collection titled `title` in the current library.
    pub fn create_collection(&self, title: &str) -> CollectionId {
        let result = self.run(LibraryCommand::CreateCollection {
            library_id: self.library(),
            title: title.to_string(),
            folder_id: None,
            sort: Default::default(),
        });
        CollectionId::new(result.created_id.expect("id")).expect("collection id")
    }

    /// Create a virtual collection, rule-based when `expression` is given.
    pub fn create_virtual_collection(
        &self,
        title: &str,
        expression: Option<&str>,
    ) -> VirtualCollectionId {
        let result = self.run(LibraryCommand::CreateVirtualCollection {
            library_id: self.library(),
            title: title.to_string(),
            expression: expression.map(str::to_string),
        });
        VirtualCollectionId::new(result.created_id.expect("id")).expect("vc id")
    }

    /// Store a file whose home collection does not exist (an unfiled file), as an import would.
    pub fn add_file(&self, id: &str) -> FileId {
        self.add_file_in(id, &CollectionId::new("removed").expect("collection"))
    }

    /// Store a file whose bytes live in `collection`, as an import would.
    pub fn add_file_in(&self, id: &str, collection: &CollectionId) -> FileId {
        let file_id = FileId::new(id).expect("file id");
        let file = File::new(file_id.clone(), self.library(), collection.clone());
        self.catalog.insert_file(file, 0).expect("insert file");
        file_id
    }

    /// Store a folder `id` of `library_id`, as a host would.
    pub fn add_folder(&self, id: &str, library_id: &LibraryId) -> FolderId {
        let folder_id = FolderId::new(id).expect("folder id");
        let folder = Folder::new(folder_id.clone(), library_id.clone(), "Folder").expect("folder");
        self.catalog.insert_folder(folder).expect("insert folder");
        folder_id
    }

    /// Every file of `scope` and `visibility`, read in pages of 2.
    pub fn list_files(&self, scope: FileScope, visibility: VisibilityFilter) -> Vec<FileId> {
        self.list_files_paged(scope, visibility, 2)
    }

    /// Every file of `scope` and `visibility`, read in pages of `page_size`.
    pub fn list_files_paged(
        &self,
        scope: FileScope,
        visibility: VisibilityFilter,
        page_size: u32,
    ) -> Vec<FileId> {
        let make = || LibraryQuery::ListFiles {
            library_id: self.library(),
            scope: scope.clone(),
            visibility,
        };
        self.read_all(page_size, make, |answer| match answer {
            QueryAnswer::Files(page) => (
                page.items
                    .into_iter()
                    .map(|e| e.file.id().clone())
                    .collect(),
                page.next_cursor,
            ),
            other => panic!("unexpected answer {other:?}"),
        })
    }

    /// Every collection of the current library.
    pub fn list_collections(&self) -> Vec<CollectionId> {
        let make = || LibraryQuery::ListCollections {
            library_id: self.library(),
        };
        self.read_all(2, make, |answer| match answer {
            QueryAnswer::Collections(page) => (
                page.items.into_iter().map(|c| c.id().clone()).collect(),
                page.next_cursor,
            ),
            other => panic!("unexpected answer {other:?}"),
        })
    }

    /// Every virtual collection of the current library.
    pub fn list_virtual_collections(&self) -> Vec<VirtualCollectionId> {
        let make = || LibraryQuery::ListVirtualCollections {
            library_id: self.library(),
        };
        self.read_all(2, make, |answer| match answer {
            QueryAnswer::VirtualCollections(page) => (
                page.items.into_iter().map(|c| c.id().clone()).collect(),
                page.next_cursor,
            ),
            other => panic!("unexpected answer {other:?}"),
        })
    }

    /// Follow cursors until the last page, at most [`MAX_PAGES`] pages.
    fn read_all<T>(
        &self,
        page_size: u32,
        make: impl Fn() -> LibraryQuery,
        split: impl Fn(QueryAnswer) -> (Vec<T>, Option<filesquid_api::library::Cursor>),
    ) -> Vec<T> {
        let mut out = Vec::new();
        let mut cursor = None;
        for _ in 0..MAX_PAGES {
            let answer = self
                .service
                .query(self.query(page_size, cursor.take(), make()))
                .expect("query");
            let (items, next) = split(answer);
            out.extend(items);
            cursor = next;
            if cursor.is_none() {
                return out;
            }
        }
        panic!("listing did not finish within {MAX_PAGES} pages");
    }
}
