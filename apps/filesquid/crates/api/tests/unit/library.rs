//! Task CL-0.2 / API-S-019: library domain commands. Every test drives `LibraryService`
//! against the in-memory `MemoryCatalog`, the reference implementation of the injected
//! `LibraryCatalog` port, so no filesystem or SQL is involved.

#[path = "../support/library_fixture.rs"]
mod library_fixture;

use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::library::{
    is_uuid_v4, mint_uuid_v4, CatalogChange, CatalogCommit, CatalogError, CommandRequest,
    CommitReceipt, FileEntry, FileListing, FileScope, Keyed, LibraryCatalog, LibraryCommand,
    LibraryError, LibraryQuery, LibraryRecord, LibraryService, LibraryState, PageKey,
    PurgeConfirmation, QueryRequest, VisibilityFilter, MAX_FILES_PER_COMMAND, MAX_NAME_BYTES,
    NEW_LIBRARY_REVISION, RECOVERABLE_DAYS,
};
use filesquid_api::metrics::Metrics;
use filesquid_api::operation::{
    CommandEnvelope, CommandOutcomeKind, ExpectedRevision, OperationId, OperationScope, PageSize,
    QueryEnvelope,
};
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::{ApiError, ErrorCode, EventBus};
use filesquid_core::{
    Collection, CollectionId, FileId, Folder, FolderId, Library, LibraryId, RepositoryId,
    VirtualCollection, VirtualCollectionId,
};
use library_fixture::{Fixture, DAY_MS, NOW_MS};
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;

#[test]
fn minted_identifiers_are_canonical_uuid_v4() {
    let ids: Vec<String> = (0..64).map(|_| mint_uuid_v4().expect("mint")).collect();
    for id in &ids {
        assert!(is_uuid_v4(id), "{id}");
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"), "{id}");
        assert_eq!(id.to_ascii_lowercase(), *id);
    }
    let unique: std::collections::BTreeSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
}

#[test]
fn uuid_check_rejects_other_shapes() {
    for bad in [
        "",
        "not-a-uuid",
        "123e4567-e89b-12d3-a456-426614174000",
        "123E4567-E89B-42D3-A456-426614174000",
        "123e4567-e89b-42d3-c456-426614174000",
        "123e4567e89b42d3a456426614174000",
        "/Users/ana/Pictures",
    ] {
        assert!(!is_uuid_v4(bad), "{bad}");
    }
    assert!(is_uuid_v4("123e4567-e89b-42d3-a456-426614174000"));
}

#[test]
fn create_library_mints_an_id_and_starts_at_revision_one() {
    let fx = Fixture::new();
    let result = fx.create_library("Family archive");
    assert_eq!(result.outcome.kind(), CommandOutcomeKind::Committed);
    let id = result.created_id.clone().expect("created id");
    assert!(is_uuid_v4(&id));
    assert_eq!(
        result.library_id.as_ref().map(|l| l.as_str()),
        Some(id.as_str())
    );
    assert_eq!(result.revision.as_deref(), Some("1"));
}

#[test]
fn create_library_needs_the_new_library_revision() {
    let fx = Fixture::new();
    let request = fx.request(
        "1",
        LibraryCommand::CreateLibrary {
            repository_id: fx.repository.clone(),
            name: "Archive".into(),
        },
    );
    let err = fx
        .service
        .execute("user:ana", request)
        .expect_err("refused");
    assert_eq!(err.api_code(), ErrorCode::InvalidRequest);
    assert_eq!(NEW_LIBRARY_REVISION, "0");
}

#[test]
fn names_must_be_non_empty_and_bounded() {
    let fx = Fixture::new();
    for name in [String::new(), "x".repeat(MAX_NAME_BYTES + 1)] {
        let request = fx.request(
            NEW_LIBRARY_REVISION,
            LibraryCommand::CreateLibrary {
                repository_id: fx.repository.clone(),
                name,
            },
        );
        let err = fx
            .service
            .execute("user:ana", request)
            .expect_err("refused");
        assert_eq!(err.api_code(), ErrorCode::InvalidRequest);
    }
    fx.create_library(&"x".repeat(MAX_NAME_BYTES));
}

#[test]
fn a_stale_expected_revision_is_a_conflict_without_change() {
    let fx = Fixture::with_library();
    let request = fx.request(
        "0",
        LibraryCommand::CreateCollection {
            library_id: fx.library(),
            title: "Trips".into(),
            folder_id: None,
            sort: Default::default(),
        },
    );
    let commits = fx.catalog.commit_count();
    let err = fx
        .service
        .execute("user:ana", request)
        .expect_err("conflict");
    assert_eq!(err, LibraryError::Conflict);
    assert_eq!(err.api_code(), ErrorCode::Conflict);
    let op = OperationId::new("op-x").expect("op");
    assert_eq!(err.outcome(&op).kind(), CommandOutcomeKind::Conflict);
    assert_eq!(fx.catalog.commit_count(), commits);
}

#[test]
fn a_repeated_operation_id_returns_the_first_outcome() {
    let fx = Fixture::with_library();
    let command = LibraryCommand::CreateCollection {
        library_id: fx.library(),
        title: "Trips".into(),
        folder_id: None,
        sort: Default::default(),
    };
    let envelope = CommandEnvelope::new(
        OperationId::new("op-repeat").expect("op"),
        ExpectedRevision::new(fx.revision()).expect("rev"),
        OperationScope::new("library").expect("scope"),
    );
    let first = fx
        .service
        .execute(
            "user:ana",
            CommandRequest {
                envelope: envelope.clone(),
                command: command.clone(),
            },
        )
        .expect("first");
    let commits = fx.catalog.commit_count();
    let journal = fx.catalog.journal_entries().len();
    let again = fx
        .service
        .execute("user:ana", CommandRequest { envelope, command })
        .expect("replay");
    assert_eq!(again.created_id, first.created_id);
    assert_eq!(again.revision, first.revision);
    assert_eq!(again.outcome.kind(), CommandOutcomeKind::Committed);
    assert_eq!(fx.catalog.commit_count(), commits);
    assert_eq!(fx.catalog.journal_entries().len(), journal);
}

#[test]
fn references_to_another_library_are_not_found() {
    let fx = Fixture::with_library();
    let collection = fx.create_collection("Trips");
    let other = fx.create_library("Other");
    let other_id = LibraryId::new(other.created_id.expect("id")).expect("id");
    let request = fx.request(
        &fx.revision_of(&other_id),
        LibraryCommand::RenameCollection {
            library_id: other_id,
            collection_id: collection,
            title: "Stolen".into(),
        },
    );
    let err = fx
        .service
        .execute("user:ana", request)
        .expect_err("refused");
    assert_eq!(err.api_code(), ErrorCode::NotFound);
}

#[test]
fn file_batches_hold_one_to_one_thousand_distinct_files() {
    let fx = Fixture::with_library();
    let file = fx.add_file("f-1");
    let too_many: Vec<FileId> = (0..1001)
        .map(|n| FileId::new(format!("f-{n}")).expect("id"))
        .collect();
    for file_ids in [vec![], vec![file.clone(), file.clone()], too_many] {
        let err = fx
            .service
            .execute(
                "user:ana",
                fx.request(
                    &fx.revision(),
                    LibraryCommand::HideFiles {
                        library_id: fx.library(),
                        file_ids,
                    },
                ),
            )
            .expect_err("refused");
        assert_eq!(err.api_code(), ErrorCode::InvalidRequest);
    }
    let exactly: Vec<FileId> = (0..MAX_FILES_PER_COMMAND)
        .map(|n| fx.add_file(&format!("g-{n:04}")))
        .collect();
    assert_eq!(exactly.len(), 1_000);
    let hidden = fx.run(LibraryCommand::HideFiles {
        library_id: fx.library(),
        file_ids: exactly.clone(),
    });
    assert_eq!(hidden.outcome.kind(), CommandOutcomeKind::Committed);
    assert_eq!(
        fx.list_files_paged(FileScope::Library, VisibilityFilter::Hidden, 1_000),
        exactly
    );
}

#[test]
fn deleting_a_collection_retains_its_files_as_unfiled() {
    let fx = Fixture::with_library();
    let collection = fx.create_collection("Trips");
    let file = fx.add_file_in("f-1", &collection);
    fx.run(LibraryCommand::DeleteCollection {
        library_id: fx.library(),
        collection_id: collection,
    });
    assert_eq!(
        fx.list_files(FileScope::Library, VisibilityFilter::Ordinary),
        vec![file.clone()]
    );
    assert_eq!(
        fx.list_files(FileScope::Unfiled, VisibilityFilter::Ordinary),
        vec![file]
    );
}

#[test]
fn virtual_collection_membership_leaves_the_home_collection_alone() {
    let fx = Fixture::with_library();
    let collection = fx.create_collection("Trips");
    let file = fx.add_file_in("f-1", &collection);
    let first = fx.create_virtual_collection("Best", None);
    let second = fx.create_virtual_collection("Family", None);
    for vc in [&first, &second] {
        fx.run(LibraryCommand::AddToVirtualCollection {
            library_id: fx.library(),
            virtual_collection_id: vc.clone(),
            file_ids: vec![file.clone()],
        });
    }
    fx.run(LibraryCommand::RemoveFromVirtualCollection {
        library_id: fx.library(),
        virtual_collection_id: first.clone(),
        file_ids: vec![file.clone()],
    });
    let in_collection = FileScope::Collection {
        collection_id: collection,
    };
    assert_eq!(
        fx.list_files(in_collection, VisibilityFilter::Ordinary),
        vec![file.clone()]
    );
    let in_first = FileScope::VirtualCollection {
        virtual_collection_id: first,
    };
    assert!(fx
        .list_files(in_first, VisibilityFilter::Ordinary)
        .is_empty());
    let in_second = FileScope::VirtualCollection {
        virtual_collection_id: second,
    };
    assert_eq!(
        fx.list_files(in_second, VisibilityFilter::Ordinary),
        vec![file]
    );
}

#[test]
fn deleting_a_virtual_collection_retains_its_members() {
    let fx = Fixture::with_library();
    let collection = fx.create_collection("Trips");
    let file = fx.add_file_in("f-1", &collection);
    let vc = fx.create_virtual_collection("Best", None);
    fx.run(LibraryCommand::AddToVirtualCollection {
        library_id: fx.library(),
        virtual_collection_id: vc.clone(),
        file_ids: vec![file.clone()],
    });
    fx.run(LibraryCommand::DeleteVirtualCollection {
        library_id: fx.library(),
        virtual_collection_id: vc.clone(),
    });
    let scope = FileScope::Collection {
        collection_id: collection,
    };
    assert_eq!(
        fx.list_files(scope, VisibilityFilter::Ordinary),
        vec![file.clone()]
    );
    assert!(fx.list_virtual_collections().is_empty());
    assert_eq!(fx.catalog.virtual_collection(&vc).expect("read"), None);
    assert!(!fx.catalog.is_virtual_member(&vc, &file).expect("read"));
    let err = fx
        .service
        .query(fx.query(
            10,
            None,
            LibraryQuery::ListFiles {
                library_id: fx.library(),
                scope: FileScope::VirtualCollection {
                    virtual_collection_id: vc,
                },
                visibility: VisibilityFilter::Ordinary,
            },
        ))
        .expect_err("deleted scope");
    assert_eq!(err.api_code(), ErrorCode::NotFound);
}

#[test]
fn adding_a_present_member_is_a_no_op_that_commits_nothing() {
    let fx = Fixture::with_library();
    let file = fx.add_file("f-1");
    let vc = fx.create_virtual_collection("Best", None);
    let add = LibraryCommand::AddToVirtualCollection {
        library_id: fx.library(),
        virtual_collection_id: vc,
        file_ids: vec![file],
    };
    fx.run(add.clone());
    let commits = fx.catalog.commit_count();
    let revision = fx.revision();
    let result = fx.run(add);
    assert_eq!(result.outcome.kind(), CommandOutcomeKind::Committed);
    assert_eq!(result.revision.as_deref(), Some(revision.as_str()));
    assert_eq!(fx.catalog.commit_count(), commits);
}

#[test]
fn rule_based_virtual_collections_refuse_hand_picked_members() {
    let fx = Fixture::with_library();
    let file = fx.add_file("f-1");
    let vc = fx.create_virtual_collection("Recent", Some("after:2026-01-01"));
    let err = fx
        .service
        .execute(
            "user:ana",
            fx.request(
                &fx.revision(),
                LibraryCommand::AddToVirtualCollection {
                    library_id: fx.library(),
                    virtual_collection_id: vc,
                    file_ids: vec![file],
                },
            ),
        )
        .expect_err("refused");
    assert_eq!(err.api_code(), ErrorCode::InvalidRequest);
    assert_eq!(err.error_code(), "rule_based_virtual_collection");
}

#[test]
fn deletion_is_recoverable_for_thirty_days_and_restore_keeps_hidden() {
    let fx = Fixture::with_library();
    let file = fx.add_file("f-1");
    let files = vec![file.clone()];
    fx.run(LibraryCommand::HideFiles {
        library_id: fx.library(),
        file_ids: files.clone(),
    });
    let deleted = fx.run(LibraryCommand::DeleteFiles {
        library_id: fx.library(),
        file_ids: files.clone(),
    });
    assert_eq!(RECOVERABLE_DAYS, 30);
    let deletion = deleted.deletion.expect("deletion");
    assert_eq!(deletion.deleted_at_ms, NOW_MS);
    assert_eq!(deletion.purge_after_ms, NOW_MS + 30 * DAY_MS);
    let json = serde_json::to_value(deletion).expect("json");
    assert_eq!(json["purge_after"], "2026-11-17T00:00:00.000Z");
    assert_eq!(
        fx.list_files(FileScope::Library, VisibilityFilter::Deleted),
        files
    );
    fx.run(LibraryCommand::RestoreFiles {
        library_id: fx.library(),
        file_ids: files.clone(),
    });
    assert!(fx
        .list_files(FileScope::Library, VisibilityFilter::Deleted)
        .is_empty());
    assert_eq!(
        fx.list_files(FileScope::Library, VisibilityFilter::Hidden),
        files
    );
}

#[test]
fn permanent_deletion_needs_deleted_files_and_an_exact_confirmation() {
    let fx = Fixture::with_library();
    let file = fx.add_file("f-1");
    let files = vec![file.clone()];
    let vc = fx.create_virtual_collection("Best", None);
    fx.run(LibraryCommand::AddToVirtualCollection {
        library_id: fx.library(),
        virtual_collection_id: vc.clone(),
        file_ids: files.clone(),
    });
    let purge = |count: u32, library: LibraryId| LibraryCommand::PurgeFiles {
        library_id: fx.library(),
        file_ids: files.clone(),
        confirmation: PurgeConfirmation {
            library_id: library,
            file_count: count,
        },
    };
    let not_deleted = fx.try_run(purge(1, fx.library())).expect_err("not deleted");
    assert_eq!(not_deleted.error_code(), "not_recoverably_deleted");
    fx.run(LibraryCommand::DeleteFiles {
        library_id: fx.library(),
        file_ids: files.clone(),
    });
    let other = LibraryId::new("other").expect("id");
    for bad in [purge(2, fx.library()), purge(1, other)] {
        let err = fx.try_run(bad).expect_err("confirmation");
        assert_eq!(err.error_code(), "purge_confirmation_mismatch");
    }
    assert_eq!(
        fx.list_files(FileScope::Library, VisibilityFilter::Deleted),
        files
    );
    fx.run(purge(1, fx.library()));
    for view in [
        VisibilityFilter::Ordinary,
        VisibilityFilter::Hidden,
        VisibilityFilter::Deleted,
    ] {
        assert!(fx.list_files(FileScope::Library, view).is_empty());
    }
    assert_eq!(fx.catalog.file(&file).expect("read"), None);
    assert!(!fx.catalog.is_virtual_member(&vc, &file).expect("read"));
}

#[test]
fn every_commit_carries_one_redacted_journal_entry() {
    let fx = Fixture::with_library();
    let collection = fx.create_collection("Holiday in Lisbon 2026");
    fx.run(LibraryCommand::RenameCollection {
        library_id: fx.library(),
        collection_id: collection.clone(),
        title: "Secret title".into(),
    });
    let entries = fx.catalog.journal_entries();
    assert_eq!(entries.len(), fx.catalog.operation_count() as usize);
    let rename = entries.last().expect("entry");
    assert_eq!(rename.operation, "library.collection.rename");
    assert_eq!(rename.outcome, Outcome::Succeeded);
    assert_eq!(rename.actor, "user:ana");
    assert!(rename.target_ids.contains(&collection.as_str().to_string()));
    let text = serde_json::to_string(&entries).expect("json");
    for secret in ["Family archive", "Holiday", "Lisbon", "Secret title"] {
        assert!(!text.contains(secret), "{secret} leaked: {text}");
    }
}

#[test]
fn refused_commands_are_journaled_as_failed() {
    let fx = Fixture::with_library();
    let missing = CollectionId::new("missing").expect("id");
    let err = fx
        .try_run(LibraryCommand::DeleteCollection {
            library_id: fx.library(),
            collection_id: missing,
        })
        .expect_err("not found");
    let entries = fx.failure_journal();
    assert_eq!(entries.len(), 1);
    let failed = &entries[0];
    assert_eq!(failed.operation, "library.collection.delete");
    assert_eq!(failed.outcome, Outcome::Failed);
    assert_eq!(failed.error_code.as_deref(), Some(err.error_code()));
}

#[test]
fn errors_map_to_api_and_port_errors() {
    let cases = [
        (LibraryError::Conflict, ErrorCode::Conflict, false),
        (
            LibraryError::NotFound("collection"),
            ErrorCode::NotFound,
            false,
        ),
        (LibraryError::Unavailable, ErrorCode::Unavailable, true),
        (LibraryError::Internal, ErrorCode::Internal, false),
    ];
    for (err, code, retryable) in cases {
        let api: ApiError = err.clone().into();
        assert_eq!(api.code(), code);
        assert_eq!(api.retryable(), retryable);
        assert!(!api.message().is_empty());
        assert_eq!(err.port_error().retryable(), retryable);
        assert_eq!(err.port_error().code(), err.error_code());
    }
}

#[test]
fn commands_round_trip_as_versioned_json() {
    let fx = Fixture::with_library();
    let request = fx.request(
        &fx.revision(),
        LibraryCommand::HideFiles {
            library_id: fx.library(),
            file_ids: vec![FileId::new("f-1").expect("id")],
        },
    );
    let json = serde_json::to_value(&request).expect("json");
    assert_eq!(json["command"]["command"], "hide_files");
    assert_eq!(json["envelope"]["schema_version"], 1);
    let back: CommandRequest = serde_json::from_value(json).expect("parse");
    assert_eq!(back, request);
}

/// A catalog whose store is unreachable for every call.
struct DownCatalog;

/// The error every [`DownCatalog`] call answers.
fn down<T>() -> Result<T, CatalogError> {
    Err(CatalogError::Unavailable("store offline".into()))
}

impl LibraryCatalog for DownCatalog {
    fn receipt(&self, _: &OperationId) -> Result<Option<CommitReceipt>, CatalogError> {
        down()
    }
    fn library(&self, _: &LibraryId) -> Result<Option<LibraryState>, CatalogError> {
        down()
    }
    fn libraries(
        &self,
        _: &RepositoryId,
        _: Option<&PageKey>,
        _: usize,
    ) -> Result<Vec<Keyed<LibraryRecord>>, CatalogError> {
        down()
    }
    fn collection(&self, _: &CollectionId) -> Result<Option<Collection>, CatalogError> {
        down()
    }
    fn collections(
        &self,
        _: &LibraryId,
        _: Option<&PageKey>,
        _: usize,
    ) -> Result<Vec<Keyed<Collection>>, CatalogError> {
        down()
    }
    fn virtual_collection(
        &self,
        _: &VirtualCollectionId,
    ) -> Result<Option<VirtualCollection>, CatalogError> {
        down()
    }
    fn virtual_collections(
        &self,
        _: &LibraryId,
        _: Option<&PageKey>,
        _: usize,
    ) -> Result<Vec<Keyed<VirtualCollection>>, CatalogError> {
        down()
    }
    fn is_virtual_member(&self, _: &VirtualCollectionId, _: &FileId) -> Result<bool, CatalogError> {
        down()
    }
    fn last_member_order(&self, _: &VirtualCollectionId) -> Result<Option<i64>, CatalogError> {
        down()
    }
    fn folder(&self, _: &FolderId) -> Result<Option<Folder>, CatalogError> {
        down()
    }
    fn file(&self, _: &FileId) -> Result<Option<FileEntry>, CatalogError> {
        down()
    }
    fn files(
        &self,
        _: &FileListing,
        _: Option<&PageKey>,
        _: usize,
    ) -> Result<Vec<Keyed<FileEntry>>, CatalogError> {
        down()
    }
    fn commit(&self, _: CatalogCommit) -> Result<CommitReceipt, CatalogError> {
        down()
    }
}

#[test]
fn catalog_failures_are_retryable_and_journaled_for_queries_too() {
    let store = Arc::new(MemoryJournal::new("desktop"));
    let writer = start_writer(&store);
    let service = LibraryService::new(Arc::new(DownCatalog), writer.sender());
    let request = QueryRequest {
        envelope: QueryEnvelope::new(
            PageSize::new(10).expect("page"),
            OperationScope::new("library").expect("scope"),
        ),
        cursor: None,
        query: LibraryQuery::OpenLibrary {
            library_id: LibraryId::new("lib-1").expect("id"),
        },
    };
    let err = service.query(request).expect_err("store down");
    assert_eq!(err, LibraryError::Unavailable);
    writer.sender().flush();
    let page = store.query(&JournalQuery::default()).expect("query");
    let failed = page.entries.last().expect("journaled");
    assert_eq!(failed.operation, "library.query");
    assert_eq!(failed.outcome, Outcome::Failed);
    assert_eq!(failed.error_code.as_deref(), Some("unavailable"));
    writer.shutdown();
}

/// Start a journal writer over `store`.
fn start_writer(store: &Arc<MemoryJournal>) -> JournalWriter {
    JournalWriter::start(
        store.clone(),
        WriterConfig::default(),
        EventBus::new(),
        Arc::new(Metrics::default()),
    )
}

#[test]
fn catalog_failures_are_journaled_exactly_once_with_the_journal_layer_installed() {
    let store = Arc::new(MemoryJournal::new("desktop"));
    let writer = start_writer(&store);
    let layer = JournalLayer::new();
    layer.attach(writer.sender());
    let subscriber = tracing_subscriber::registry().with(layer);
    let service = LibraryService::new(Arc::new(DownCatalog), writer.sender());
    let request = CommandRequest {
        envelope: CommandEnvelope::new(
            OperationId::new("op-down").expect("op"),
            ExpectedRevision::new("1").expect("rev"),
            OperationScope::new("library").expect("scope"),
        ),
        command: LibraryCommand::DeleteCollection {
            library_id: LibraryId::new("lib-1").expect("id"),
            collection_id: CollectionId::new("c-1").expect("id"),
        },
    };
    let err = tracing::subscriber::with_default(subscriber, || {
        service
            .execute("user:ana", request)
            .expect_err("store down")
    });
    assert_eq!(err, LibraryError::Unavailable);
    writer.sender().flush();
    let entries = store
        .query(&JournalQuery::default())
        .expect("query")
        .entries;
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].operation, "library.collection.delete");
    assert_eq!(entries[0].error_code.as_deref(), Some("unavailable"));
    writer.shutdown();
}

#[test]
fn renaming_to_the_same_title_commits_nothing() {
    let fx = Fixture::with_library();
    let collection = fx.create_collection("Trips");
    let vc = fx.create_virtual_collection("Best", None);
    let commits = fx.catalog.commit_count();
    let revision = fx.revision();
    let same = [
        LibraryCommand::RenameCollection {
            library_id: fx.library(),
            collection_id: collection,
            title: "Trips".into(),
        },
        LibraryCommand::RenameVirtualCollection {
            library_id: fx.library(),
            virtual_collection_id: vc,
            title: "Best".into(),
        },
    ];
    for command in same {
        let result = fx.run(command);
        assert_eq!(result.outcome.kind(), CommandOutcomeKind::Committed);
        assert_eq!(result.revision.as_deref(), Some(revision.as_str()));
    }
    assert_eq!(fx.catalog.commit_count(), commits);
}

#[test]
fn a_no_op_is_journaled_and_replays_under_its_operation_id() {
    let fx = Fixture::with_library();
    let file = fx.add_file("f-1");
    let vc = fx.create_virtual_collection("Best", None);
    let add = LibraryCommand::AddToVirtualCollection {
        library_id: fx.library(),
        virtual_collection_id: vc.clone(),
        file_ids: vec![file.clone()],
    };
    fx.run(add.clone());
    let journal = fx.catalog.journal_entries().len();
    let noop = fx.request(&fx.revision(), add);
    let first = fx.service.execute("user:ana", noop.clone()).expect("no-op");
    assert_eq!(fx.catalog.journal_entries().len(), journal + 1);
    fx.run(LibraryCommand::RemoveFromVirtualCollection {
        library_id: fx.library(),
        virtual_collection_id: vc.clone(),
        file_ids: vec![file.clone()],
    });
    let again = fx.service.execute("user:ana", noop).expect("replay");
    assert_eq!(again, first);
    assert!(!fx.catalog.is_virtual_member(&vc, &file).expect("read"));
}

#[test]
fn a_replayed_delete_answers_its_first_deletion() {
    let fx = Fixture::with_library();
    let file = fx.add_file("f-1");
    let request = fx.request(
        &fx.revision(),
        LibraryCommand::DeleteFiles {
            library_id: fx.library(),
            file_ids: vec![file],
        },
    );
    let first = fx
        .service
        .execute("user:ana", request.clone())
        .expect("delete");
    assert!(first.deletion.is_some());
    let again = fx.service.execute("user:ana", request).expect("replay");
    assert_eq!(again.deletion, first.deletion);
    assert_eq!(again, first);
}

#[test]
fn create_collection_needs_a_folder_of_its_library() {
    let fx = Fixture::with_library();
    let other = fx.create_library("Other");
    let other_id = LibraryId::new(other.created_id.expect("id")).expect("id");
    let own = fx.add_folder("folder-own", &fx.library());
    let foreign = fx.add_folder("folder-foreign", &other_id);
    let unknown = FolderId::new("folder-unknown").expect("id");
    let create = |folder: FolderId| LibraryCommand::CreateCollection {
        library_id: fx.library(),
        title: "Trips".into(),
        folder_id: Some(folder),
        sort: Default::default(),
    };
    for folder in [foreign, unknown] {
        let err = fx.try_run(create(folder)).expect_err("refused");
        assert_eq!(err, LibraryError::NotFound("folder"));
    }
    let created = fx.run(create(own.clone()));
    let id = CollectionId::new(created.created_id.expect("id")).expect("id");
    let stored = fx.catalog.collection(&id).expect("read").expect("stored");
    assert_eq!(stored.folder_id(), Some(&own));
}

#[test]
fn membership_batches_keep_their_order_within_one_millisecond() {
    let fx = Fixture::with_library();
    let vc = fx.create_virtual_collection("Best", None);
    let added_first = fx.add_file("f-b");
    let added_second = fx.add_file("f-a");
    for file in [&added_first, &added_second] {
        fx.run(LibraryCommand::AddToVirtualCollection {
            library_id: fx.library(),
            virtual_collection_id: vc.clone(),
            file_ids: vec![file.clone()],
        });
    }
    let scope = FileScope::VirtualCollection {
        virtual_collection_id: vc,
    };
    assert_eq!(
        fx.list_files(scope, VisibilityFilter::Ordinary),
        vec![added_first, added_second]
    );
}

/// A raw catalog commit of `changes` to `library_id` at `revision`, as operation `op`.
fn raw_commit(
    op: &str,
    library_id: &LibraryId,
    revision: &str,
    changes: Vec<CatalogChange>,
) -> CatalogCommit {
    CatalogCommit {
        operation_id: OperationId::new(op).expect("op"),
        library_id: library_id.clone(),
        expected_revision: ExpectedRevision::new(revision).expect("rev"),
        changes,
        journal: JournalEntry::new("library.test", Outcome::Succeeded),
        created_id: None,
        deletion: None,
    }
}

/// A library record of `library_id` on `fx`'s repository, named `name`.
fn record(fx: &Fixture, library_id: &LibraryId, name: &str) -> LibraryRecord {
    LibraryRecord {
        library: Library::new(library_id.clone(), fx.repository.clone()),
        name: name.into(),
    }
}

#[test]
fn a_failed_commit_leaves_the_memory_catalog_unchanged() {
    let fx = Fixture::with_library();
    let stray = LibraryId::new("lib-stray").expect("id");
    let named = LibraryId::new("lib-named").expect("id");
    let create = CatalogChange::CreateLibrary(record(&fx, &stray, "Stray"));
    let commits = fx.catalog.commit_count();
    let journal = fx.catalog.journal_entries().len();
    let result = fx
        .catalog
        .commit(raw_commit("op-stray", &named, "0", vec![create]));
    assert!(result.is_err(), "{result:?}");
    assert_eq!(fx.catalog.library(&stray).expect("read"), None);
    assert_eq!(fx.catalog.library(&named).expect("read"), None);
    assert_eq!(fx.catalog.commit_count(), commits);
    assert_eq!(fx.catalog.journal_entries().len(), journal);
    let op = OperationId::new("op-stray").expect("op");
    assert_eq!(fx.catalog.receipt(&op).expect("read"), None);
}

#[test]
fn a_later_create_library_change_cannot_replace_a_library() {
    let fx = Fixture::with_library();
    let library = fx.library();
    let before = fx
        .catalog
        .library(&library)
        .expect("read")
        .expect("library");
    let collection = Collection::new(
        CollectionId::new("c-sneaky").expect("id"),
        library.clone(),
        "Sneaky",
    )
    .expect("collection");
    let changes = vec![
        CatalogChange::PutCollection(collection.clone()),
        CatalogChange::CreateLibrary(record(&fx, &library, "Replaced")),
    ];
    let result = fx.catalog.commit(raw_commit(
        "op-replace",
        &library,
        &before.revision,
        changes,
    ));
    assert!(result.is_err(), "{result:?}");
    assert_eq!(fx.catalog.library(&library).expect("read"), Some(before));
    assert_eq!(fx.catalog.collection(collection.id()).expect("read"), None);
}
