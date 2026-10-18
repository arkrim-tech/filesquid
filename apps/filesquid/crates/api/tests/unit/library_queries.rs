//! Task CL-0.2 / API-S-019: library domain queries — open, paged listing with a stable
//! order and an opaque cursor, and the hidden and deleted views.

#[path = "../support/library_fixture.rs"]
mod library_fixture;

use filesquid_api::library::{
    Cursor, FileScope, LibraryCommand, LibraryQuery, PageKey, QueryAnswer, QueryRequest,
    VisibilityFilter, RECOVERABLE_DAYS,
};
use filesquid_api::operation::{OperationScope, PageSize, QueryEnvelope};
use filesquid_api::ErrorCode;
use filesquid_core::{FileId, LibraryId};
use library_fixture::Fixture;

/// A query request for `query` with `page_size` and `cursor`.
fn query(page_size: u32, cursor: Option<Cursor>, query: LibraryQuery) -> QueryRequest {
    QueryRequest {
        envelope: QueryEnvelope::new(
            PageSize::new(page_size).expect("page size"),
            OperationScope::new("library").expect("scope"),
        ),
        cursor,
        query,
    }
}

#[test]
fn open_library_answers_its_record_and_revision() {
    let fx = Fixture::with_library();
    let answer = fx
        .service
        .query(query(
            10,
            None,
            LibraryQuery::OpenLibrary {
                library_id: fx.library(),
            },
        ))
        .expect("open");
    let QueryAnswer::Library(state) = answer else {
        panic!("expected a library answer");
    };
    assert_eq!(state.record.library.id(), &fx.library());
    assert_eq!(state.record.name, "Family archive");
    assert_eq!(state.revision, fx.revision());
}

#[test]
fn opening_an_unknown_library_is_not_found() {
    let fx = Fixture::new();
    let missing = LibraryId::new("missing").expect("id");
    let err = fx
        .service
        .query(query(
            10,
            None,
            LibraryQuery::OpenLibrary {
                library_id: missing,
            },
        ))
        .expect_err("missing");
    assert_eq!(err.api_code(), ErrorCode::NotFound);
}

#[test]
fn paging_is_stable_with_no_repeats_or_gaps() {
    let fx = Fixture::with_library();
    let mut expected: Vec<FileId> = (0..7).map(|n| fx.add_file(&format!("f-{n}"))).collect();
    expected.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    for page_size in [1, 2, 3, 7, 1000] {
        let listed = fx.list_files_paged(FileScope::Library, VisibilityFilter::Ordinary, page_size);
        assert_eq!(listed, expected, "page size {page_size}");
    }
}

#[test]
fn the_last_page_has_no_cursor() {
    let fx = Fixture::with_library();
    fx.add_file("f-1");
    let answer = fx
        .service
        .query(query(
            10,
            None,
            LibraryQuery::ListFiles {
                library_id: fx.library(),
                scope: FileScope::Library,
                visibility: VisibilityFilter::Ordinary,
            },
        ))
        .expect("list");
    let QueryAnswer::Files(page) = answer else {
        panic!("expected files");
    };
    assert_eq!(page.items.len(), 1);
    assert!(page.next_cursor.is_none());
}

#[test]
fn cursors_are_opaque_and_malformed_ones_are_refused() {
    let key = PageKey {
        order: 3,
        id: "f-1".into(),
    };
    let cursor = Cursor::encode(&key);
    assert!(!cursor.as_str().contains("f-1"));
    assert!(cursor.as_str().starts_with("v1."));
    assert_eq!(cursor.decode().expect("decode"), key);
    let fx = Fixture::with_library();
    for bad in ["", "v1.zz", "v2.00", "v1.6131", "../etc"] {
        let err = fx
            .service
            .query(query(
                10,
                Some(serde_json::from_value(bad.into()).expect("cursor")),
                LibraryQuery::ListCollections {
                    library_id: fx.library(),
                },
            ))
            .expect_err(bad);
        assert_eq!(err.api_code(), ErrorCode::InvalidRequest, "{bad}");
    }
}

#[test]
fn ordinary_listing_excludes_hidden_and_deleted_files() {
    let fx = Fixture::with_library();
    let shown = fx.add_file("f-shown");
    let hidden = fx.add_file("f-hidden");
    let deleted = fx.add_file("f-deleted");
    fx.run(LibraryCommand::HideFiles {
        library_id: fx.library(),
        file_ids: vec![hidden.clone()],
    });
    fx.run(LibraryCommand::DeleteFiles {
        library_id: fx.library(),
        file_ids: vec![deleted.clone()],
    });
    assert_eq!(
        fx.list_files(FileScope::Library, VisibilityFilter::Ordinary),
        vec![shown]
    );
    assert_eq!(
        fx.list_files(FileScope::Library, VisibilityFilter::Hidden),
        vec![hidden]
    );
    assert_eq!(
        fx.list_files(FileScope::Library, VisibilityFilter::Deleted),
        vec![deleted]
    );
}

#[test]
fn collections_and_virtual_collections_list_by_library() {
    let fx = Fixture::with_library();
    let a = fx.create_collection("A");
    let b = fx.create_collection("B");
    let vc = fx.create_virtual_collection("V", None);
    let other = fx.create_library("Other");
    let other_id = LibraryId::new(other.created_id.expect("id")).expect("id");
    fx.run_in(
        &other_id,
        LibraryCommand::CreateCollection {
            library_id: other_id.clone(),
            title: "Elsewhere".into(),
            folder_id: None,
            sort: Default::default(),
        },
    );
    let mut expected = vec![a, b];
    expected.sort_by(|x, y| x.as_str().cmp(y.as_str()));
    assert_eq!(fx.list_collections(), expected);
    assert_eq!(fx.list_virtual_collections(), vec![vc]);
}

#[test]
fn listing_libraries_pages_by_repository() {
    let fx = Fixture::new();
    let ids: Vec<String> = ["A", "B", "C"]
        .iter()
        .map(|name| fx.create_library(name).created_id.expect("id"))
        .collect();
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..10 {
        let answer = fx
            .service
            .query(query(
                2,
                cursor.take(),
                LibraryQuery::ListLibraries {
                    repository_id: fx.repository.clone(),
                },
            ))
            .expect("list");
        let QueryAnswer::Libraries(page) = answer else {
            panic!("expected libraries");
        };
        seen.extend(
            page.items
                .iter()
                .map(|r| r.library.id().as_str().to_string()),
        );
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, ids);
}

#[test]
fn deletion_policy_query_reports_thirty_days() {
    let fx = Fixture::new();
    let answer = fx
        .service
        .query(query(1, None, LibraryQuery::DeletionPolicy))
        .expect("policy");
    let QueryAnswer::DeletionPolicy(policy) = answer else {
        panic!("expected policy");
    };
    assert_eq!(policy.recoverable_days, RECOVERABLE_DAYS);
}

#[test]
fn listing_a_scope_of_another_library_is_not_found() {
    let fx = Fixture::with_library();
    let collection = fx.create_collection("Trips");
    let other = fx.create_library("Other");
    let other_id = LibraryId::new(other.created_id.expect("id")).expect("id");
    let err = fx
        .service
        .query(query(
            10,
            None,
            LibraryQuery::ListFiles {
                library_id: other_id,
                scope: FileScope::Collection {
                    collection_id: collection,
                },
                visibility: VisibilityFilter::Ordinary,
            },
        ))
        .expect_err("refused");
    assert_eq!(err.api_code(), ErrorCode::NotFound);
}

#[test]
fn answers_carry_no_filesystem_paths() {
    let fx = Fixture::with_library();
    fx.add_file("f-1");
    let answer = fx
        .service
        .query(query(
            10,
            None,
            LibraryQuery::ListFiles {
                library_id: fx.library(),
                scope: FileScope::Library,
                visibility: VisibilityFilter::Ordinary,
            },
        ))
        .expect("list");
    let json = serde_json::to_value(&answer).expect("json");
    assert_eq!(json["answer"], "files");
    let text = json.to_string();
    for marker in ["/Users", "C:\\\\", "SELECT", ".sqlite", "inventory.json"] {
        assert!(!text.contains(marker), "{marker}: {text}");
    }
}
