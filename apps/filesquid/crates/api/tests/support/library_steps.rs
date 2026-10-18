//! Step definitions for `library_domain_commands_and_queries_API-S-019.feature`
//! (Task CL-0.2): the library commands and queries of the application layer, driven through
//! `LibraryService` over the reference `MemoryCatalog`.

use crate::library_fixture::{Fixture, DAY_MS};
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::library::{
    is_uuid_v4, CommandResult, FileScope, LibraryCommand, LibraryError, LibraryQuery,
    PurgeConfirmation, QueryAnswer, QueryRequest, VisibilityFilter,
};
use filesquid_api::operation::{CommandEnvelope, ExpectedRevision, OperationId, OperationScope};
use filesquid_core::{CollectionId, FileId, VirtualCollectionId};
use std::collections::BTreeMap;

/// Per-scenario state of the library steps.
#[derive(Default)]
pub struct LibraryWorld {
    fx: Option<Fixture>,
    collections: BTreeMap<String, CollectionId>,
    virtuals: BTreeMap<String, VirtualCollectionId>,
    files: Vec<FileId>,
    created: Option<CommandResult>,
    results: Vec<Result<CommandResult, LibraryError>>,
    commits_before: u64,
    listed: Vec<FileId>,
}

impl std::fmt::Debug for LibraryWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryWorld")
            .field("files", &self.files)
            .finish()
    }
}

impl LibraryWorld {
    fn fx(&self) -> &Fixture {
        self.fx.as_ref().expect("library fixture")
    }

    fn collection(&self, title: &str) -> CollectionId {
        self.collections
            .get(title)
            .cloned()
            .expect("known collection")
    }

    fn virtual_collection(&self, title: &str) -> VirtualCollectionId {
        self.virtuals
            .get(title)
            .cloned()
            .expect("known virtual collection")
    }

    fn count(&self, scope: FileScope, visibility: VisibilityFilter) -> usize {
        self.fx().list_files(scope, visibility).len()
    }
}

#[given(expr = "a FileSquid application layer with an empty catalog")]
fn empty_catalog(w: &mut MonitorWorld) {
    w.library.fx = Some(Fixture::new());
}

#[when(expr = "the owner creates a library named {string}")]
fn create_library(w: &mut MonitorWorld, name: String) {
    w.library.created = Some(w.library.fx().create_library(&name));
}

#[then(expr = "the library should have a UUID version 4 identifier")]
fn library_has_uuid(w: &mut MonitorWorld) {
    let created = w.library.created.as_ref().expect("created");
    assert!(is_uuid_v4(created.created_id.as_deref().expect("id")));
}

#[then(expr = "opening the library should answer the name {string} at revision {string}")]
fn open_library(w: &mut MonitorWorld, name: String, revision: String) {
    let lib = w
        .library
        .created
        .as_ref()
        .and_then(|c| c.library_id.clone());
    let fx = w.library.fx();
    let request = fx.query(
        1,
        None,
        LibraryQuery::OpenLibrary {
            library_id: lib.expect("library id"),
        },
    );
    let QueryAnswer::Library(state) = fx.service.query(request).expect("open") else {
        panic!("expected a library");
    };
    assert_eq!(state.record.name, name);
    assert_eq!(state.revision, revision);
}

#[given(expr = "a library with a collection {string} holding {int} file(s)")]
fn library_with_files(w: &mut MonitorWorld, title: String, count: usize) {
    let fx = Fixture::with_library();
    let collection = fx.create_collection(&title);
    w.library.files = (0..count)
        .map(|n| fx.add_file_in(&format!("f-{n}"), &collection))
        .collect();
    w.library.collections.insert(title, collection);
    w.library.fx = Some(fx);
}

#[when(expr = "the owner deletes the collection {string}")]
fn delete_collection(w: &mut MonitorWorld, title: String) {
    let w = &mut w.library;
    w.fx().run(LibraryCommand::DeleteCollection {
        library_id: w.fx().library(),
        collection_id: w.collection(&title),
    });
}

#[then(expr = "the library should still list {int} files")]
fn library_lists(w: &mut MonitorWorld, count: usize) {
    assert_eq!(
        w.library
            .count(FileScope::Library, VisibilityFilter::Ordinary),
        count
    );
}

#[then(expr = "the unfiled listing should show {int} files")]
fn unfiled_lists(w: &mut MonitorWorld, count: usize) {
    assert_eq!(
        w.library
            .count(FileScope::Unfiled, VisibilityFilter::Ordinary),
        count
    );
}

#[given(expr = "the file is a member of the virtual collections {string} and {string}")]
fn member_of_two(w: &mut MonitorWorld, first: String, second: String) {
    let w = &mut w.library;
    for title in [first, second] {
        let vc = w.fx().create_virtual_collection(&title, None);
        w.fx().run(LibraryCommand::AddToVirtualCollection {
            library_id: w.fx().library(),
            virtual_collection_id: vc.clone(),
            file_ids: w.files.clone(),
        });
        w.virtuals.insert(title, vc);
    }
}

#[when(expr = "the owner removes the file from {string}")]
fn remove_member(w: &mut MonitorWorld, title: String) {
    let w = &mut w.library;
    w.fx().run(LibraryCommand::RemoveFromVirtualCollection {
        library_id: w.fx().library(),
        virtual_collection_id: w.virtual_collection(&title),
        file_ids: w.files.clone(),
    });
}

#[then(expr = "the collection {string} should still list the file")]
fn collection_lists_file(w: &mut MonitorWorld, title: String) {
    let scope = FileScope::Collection {
        collection_id: w.library.collection(&title),
    };
    let listed = w.library.fx().list_files(scope, VisibilityFilter::Ordinary);
    assert_eq!(listed, w.library.files);
}

#[then(expr = "the virtual collection {string} should still list the file")]
fn virtual_lists_file(w: &mut MonitorWorld, title: String) {
    let scope = FileScope::VirtualCollection {
        virtual_collection_id: w.library.virtual_collection(&title),
    };
    let listed = w.library.fx().list_files(scope, VisibilityFilter::Ordinary);
    assert_eq!(listed, w.library.files);
}

#[then(expr = "the virtual collection {string} should list no file")]
fn virtual_lists_nothing(w: &mut MonitorWorld, title: String) {
    let scope = FileScope::VirtualCollection {
        virtual_collection_id: w.library.virtual_collection(&title),
    };
    assert!(w
        .library
        .fx()
        .list_files(scope, VisibilityFilter::Ordinary)
        .is_empty());
}

#[given(expr = "a rule-based virtual collection {string}")]
fn rule_based(w: &mut MonitorWorld, title: String) {
    let vc = w
        .library
        .fx()
        .create_virtual_collection(&title, Some("after:2026-01-01"));
    w.library.virtuals.insert(title, vc);
}

#[when(expr = "the owner adds the file to {string}")]
fn add_member(w: &mut MonitorWorld, title: String) {
    let w = &mut w.library;
    let result = w.fx().try_run(LibraryCommand::AddToVirtualCollection {
        library_id: w.fx().library(),
        virtual_collection_id: w.virtual_collection(&title),
        file_ids: w.files.clone(),
    });
    w.results.push(result);
}

#[then(expr = "the command should be refused as {string}")]
fn refused_as(w: &mut MonitorWorld, code: String) {
    let last = w.library.results.last().expect("a command ran");
    let err = last.as_ref().expect_err("refused");
    assert_eq!(
        serde_json::to_value(err.api_code()).expect("code"),
        serde_json::Value::String(code)
    );
}

#[when(expr = "the owner hides the first file and deletes the second")]
fn hide_and_delete(w: &mut MonitorWorld) {
    let w = &mut w.library;
    w.fx().run(LibraryCommand::HideFiles {
        library_id: w.fx().library(),
        file_ids: vec![w.files[0].clone()],
    });
    w.fx().run(LibraryCommand::DeleteFiles {
        library_id: w.fx().library(),
        file_ids: vec![w.files[1].clone()],
    });
}

#[then(expr = "the ordinary listing should show {int} file(s)")]
fn ordinary_shows(w: &mut MonitorWorld, count: usize) {
    assert_eq!(
        w.library
            .count(FileScope::Library, VisibilityFilter::Ordinary),
        count
    );
}

#[then(expr = "the hidden view should show {int} file(s)")]
fn hidden_shows(w: &mut MonitorWorld, count: usize) {
    assert_eq!(
        w.library
            .count(FileScope::Library, VisibilityFilter::Hidden),
        count
    );
}

#[then(expr = "the deleted view should show {int} file(s)")]
fn deleted_shows(w: &mut MonitorWorld, count: usize) {
    assert_eq!(
        w.library
            .count(FileScope::Library, VisibilityFilter::Deleted),
        count
    );
}

#[then(expr = "the deleted view should show {int} file to be purged {int} days after deletion")]
fn deleted_with_purge(w: &mut MonitorWorld, count: usize, days: i64) {
    let fx = w.library.fx();
    let request = fx.query(
        10,
        None,
        LibraryQuery::ListFiles {
            library_id: fx.library(),
            scope: FileScope::Library,
            visibility: VisibilityFilter::Deleted,
        },
    );
    let QueryAnswer::Files(page) = fx.service.query(request).expect("list") else {
        panic!("expected files");
    };
    assert_eq!(page.items.len(), count);
    for entry in &page.items {
        let deletion = entry.state.deletion.as_ref().expect("deletion");
        assert_eq!(
            deletion.purge_after_ms - deletion.deleted_at_ms,
            days * DAY_MS
        );
    }
}

#[given(expr = "the owner has deleted every file")]
fn deleted_every(w: &mut MonitorWorld) {
    let w = &mut w.library;
    w.fx().run(LibraryCommand::DeleteFiles {
        library_id: w.fx().library(),
        file_ids: w.files.clone(),
    });
}

#[when(expr = "the owner permanently deletes every file confirming {int} file")]
fn purge_confirming(w: &mut MonitorWorld, count: u32) {
    let w = &mut w.library;
    let result = w.fx().try_run(LibraryCommand::PurgeFiles {
        library_id: w.fx().library(),
        file_ids: w.files.clone(),
        confirmation: PurgeConfirmation {
            library_id: w.fx().library(),
            file_count: count,
        },
    });
    w.results.push(result);
}

#[when(expr = "the owner renames the collection {string} with a stale revision")]
fn rename_stale(w: &mut MonitorWorld, title: String) {
    let w = &mut w.library;
    w.commits_before = w.fx().catalog.commit_count();
    let command = LibraryCommand::RenameCollection {
        library_id: w.fx().library(),
        collection_id: w.collection(&title),
        title: "Renamed".into(),
    };
    let request = w.fx().request("1", command);
    let result = w.fx().service.execute("user:ana", request);
    w.results.push(result);
}

#[then(expr = "nothing should have been committed")]
fn nothing_committed(w: &mut MonitorWorld) {
    assert_eq!(
        w.library.fx().catalog.commit_count(),
        w.library.commits_before
    );
}

#[when(expr = "the owner sends the same create-collection command twice")]
fn same_command_twice(w: &mut MonitorWorld) {
    let w = &mut w.library;
    let request = filesquid_api::library::CommandRequest {
        envelope: CommandEnvelope::new(
            OperationId::new("op-retry").expect("op"),
            ExpectedRevision::new(w.fx().revision()).expect("revision"),
            OperationScope::new("library").expect("scope"),
        ),
        command: LibraryCommand::CreateCollection {
            library_id: w.fx().library(),
            title: "Again".into(),
            folder_id: None,
            sort: Default::default(),
        },
    };
    for _ in 0..2 {
        let result = w.fx().service.execute("user:ana", request.clone());
        w.results.push(result);
    }
}

#[then(expr = "both answers should name the same collection")]
fn same_collection(w: &mut MonitorWorld) {
    let ids: Vec<_> = w
        .library
        .results
        .iter()
        .map(|r| r.as_ref().expect("committed").created_id.clone())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids[0].is_some());
    assert_eq!(ids[0], ids[1]);
}

#[then(expr = "the library should list {int} collections")]
fn lists_collections(w: &mut MonitorWorld, count: usize) {
    assert_eq!(w.library.fx().list_collections().len(), count);
}

#[when(expr = "the owner lists the library {int} files per page")]
fn list_paged(w: &mut MonitorWorld, per_page: u32) {
    let fx = w.library.fx();
    w.library.listed =
        fx.list_files_paged(FileScope::Library, VisibilityFilter::Ordinary, per_page);
}

#[then(expr = "every file should appear exactly once in identifier order")]
fn every_file_once(w: &mut MonitorWorld) {
    let mut expected = w.library.files.clone();
    expected.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(w.library.listed, expected);
}

#[then(expr = "a query asking for {int} items per page should be refused")]
fn oversized_page(_w: &mut MonitorWorld, size: u32) {
    let json = serde_json::json!({
        "envelope": {"schema_version": 1, "page_size": size, "scope": "library"},
        "query": {"query": "deletion_policy"}
    });
    assert!(serde_json::from_value::<QueryRequest>(json).is_err());
}

#[when(expr = "the owner renames the collection {string} to {string}")]
fn rename_to(w: &mut MonitorWorld, title: String, new_title: String) {
    let w = &mut w.library;
    w.fx().run(LibraryCommand::RenameCollection {
        library_id: w.fx().library(),
        collection_id: w.collection(&title),
        title: new_title,
    });
}

#[then(expr = "the catalog should hold one journal entry per commit")]
fn one_entry_per_commit(w: &mut MonitorWorld) {
    let catalog = &w.library.fx().catalog;
    assert_eq!(
        catalog.journal_entries().len() as u64,
        catalog.commit_count()
    );
}

#[then(expr = "no journal entry should contain {string} or {string}")]
fn journal_redacted(w: &mut MonitorWorld, first: String, second: String) {
    let text = serde_json::to_string(&w.library.fx().catalog.journal_entries()).expect("json");
    assert!(!text.contains(&first), "{text}");
    assert!(!text.contains(&second), "{text}");
}
