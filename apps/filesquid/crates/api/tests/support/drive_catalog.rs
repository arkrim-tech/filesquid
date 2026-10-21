//! A library catalog seeded for the view operation and storage use per collection (Task
//! API-F2.2): one library with a chosen identifier, one collection and one virtual
//! collection, and files placed in them as an import would, read by a navigation service
//! over the same catalog.
// Each test crate that includes this fixture uses a different part of it.
#![allow(dead_code)]

use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::library::{
    CatalogChange, CatalogCommit, Deletion, FileState, LibraryCatalog, LibraryRecord,
    MemoryCatalog, NEW_LIBRARY_REVISION,
};
use filesquid_api::navigation::{
    HostAccess, MemoryFolderIndex, NavigationService, NodeRef, RepositoryInfo,
};
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_core::{
    Collection, CollectionId, File, FileId, Library, LibraryId, RepositoryId, VirtualCollection,
    VirtualCollectionId, VirtualMembership,
};
use std::sync::Arc;

/// The repository of the seeded catalog.
pub const REPOSITORY: &str = "repository";

/// The seeded collection.
pub const COLLECTION: &str = "2c0d1e2f-3a4b-4c5d-8e6f-7a8b9c0d1e2f";

/// The seeded virtual collection.
pub const VIRTUAL: &str = "3d1e2f3a-4b5c-4d6e-9f7a-8b9c0d1e2f3a";

/// A catalog with library `library` holding collection [`COLLECTION`] ("Holidays") and
/// virtual collection [`VIRTUAL`] ("Favourites"), and the navigation service over it.
pub struct DriveCatalog {
    /// The catalog.
    pub catalog: Arc<MemoryCatalog>,
    /// The navigation service over it.
    pub navigation: Arc<NavigationService>,
    library: LibraryId,
    next_member: std::cell::Cell<i64>,
}

impl DriveCatalog {
    /// Seed the catalog for `library`.
    pub fn new(library: &str) -> Self {
        let catalog = Arc::new(MemoryCatalog::new());
        let library_id = LibraryId::new(library).expect("library id");
        let repository = RepositoryId::new(REPOSITORY).expect("repository");
        let record = LibraryRecord {
            library: Library::new(library_id.clone(), repository.clone()),
            name: "Family archive".into(),
        };
        let collection =
            Collection::new(collection_id(), library_id.clone(), "Holidays").expect("collection");
        let virtual_collection =
            VirtualCollection::new(virtual_id(), library_id.clone(), "Favourites")
                .expect("virtual collection");
        let changes = vec![
            CatalogChange::CreateLibrary(record),
            CatalogChange::PutCollection(collection),
            CatalogChange::PutVirtualCollection(virtual_collection),
        ];
        commit(&catalog, &library_id, NEW_LIBRARY_REVISION, changes);
        let navigation = Arc::new(NavigationService::new(
            catalog.clone(),
            Arc::new(MemoryFolderIndex::new()),
            Arc::new(HostAccess),
            RepositoryInfo::new(repository, None),
        ));
        Self {
            catalog,
            navigation,
            library: library_id,
            next_member: std::cell::Cell::new(0),
        }
    }

    /// Place `file` in [`COLLECTION`].
    pub fn file_in_collection(&self, file: &str) {
        self.insert(file, collection_id());
    }

    /// Place `file` in no existing collection (an unfiled file).
    pub fn unfiled_file(&self, file: &str) {
        self.insert(file, CollectionId::new("removed").expect("collection"));
    }

    /// Add `file` to [`VIRTUAL`].
    pub fn add_to_virtual(&self, file: &str) {
        let order = self.next_member.get() + 1;
        self.next_member.set(order);
        let member = VirtualMembership::new(virtual_id(), file_id(file), order);
        let revision = self.revision();
        let changes = vec![CatalogChange::AddVirtualMember(member)];
        commit(&self.catalog, &self.library, &revision, changes);
    }

    /// Mark `file` recoverably deleted.
    pub fn delete(&self, file: &str) {
        let state = FileState {
            hidden: false,
            deletion: Some(Deletion {
                deleted_at_ms: 1_000,
                purge_after_ms: 2_000,
            }),
        };
        let change = CatalogChange::SetFileState {
            file_id: file_id(file),
            state,
        };
        let revision = self.revision();
        commit(&self.catalog, &self.library, &revision, vec![change]);
    }

    /// The handle of the library node.
    pub fn library_node(&self) -> String {
        NodeRef::Library(self.library.clone())
            .to_handle()
            .expect("handle")
    }

    /// The handle of the collection node.
    pub fn collection_node(&self) -> String {
        NodeRef::Collection(collection_id())
            .to_handle()
            .expect("handle")
    }

    /// The handle of the virtual-collection node.
    pub fn virtual_node(&self) -> String {
        NodeRef::VirtualCollection(virtual_id())
            .to_handle()
            .expect("handle")
    }

    /// Store `file` with home `collection`, in insertion order.
    fn insert(&self, file: &str, collection: CollectionId) {
        let order = self.next_member.get() + 1;
        self.next_member.set(order);
        let file = File::new(file_id(file), self.library.clone(), collection);
        self.catalog.insert_file(file, order).expect("insert file");
    }

    /// The library's current revision.
    fn revision(&self) -> String {
        self.catalog
            .library(&self.library)
            .expect("catalog")
            .expect("library")
            .revision
    }
}

/// Commit `changes` to `library` at `revision`.
fn commit(
    catalog: &MemoryCatalog,
    library: &LibraryId,
    revision: &str,
    changes: Vec<CatalogChange>,
) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let commit = CatalogCommit {
        operation_id: OperationId::new(format!("seed-{n}")).expect("operation"),
        library_id: library.clone(),
        expected_revision: ExpectedRevision::new(revision).expect("revision"),
        changes,
        journal: JournalEntry::new("library.seed", Outcome::Succeeded),
        created_id: None,
        deletion: None,
    };
    catalog.commit(commit).expect("seed commit");
}

/// The seeded collection's identifier.
fn collection_id() -> CollectionId {
    CollectionId::new(COLLECTION).expect("collection id")
}

/// The seeded virtual collection's identifier.
fn virtual_id() -> VirtualCollectionId {
    VirtualCollectionId::new(VIRTUAL).expect("virtual collection id")
}

/// The identifier of `file`.
fn file_id(file: &str) -> FileId {
    FileId::new(file).expect("file id")
}
