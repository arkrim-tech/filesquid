//! PROV-S-005: the in-memory history index answers object, entity, operation,
//! agent and time-range queries in bounded, cursor-linked pages.

#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use filesquid_prov::{
    build_record, entity_id, Cursor, HistoryIndex, HistoryQuery, PageRequest, Record, Timestamp,
    MAX_PAGE_SIZE,
};
use fixtures::*;

const OBJ_A: &str = "urn:uuid:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const OBJ_B: &str = "urn:uuid:bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

/// Record of version `version` of `object` (base = previous version) ending at `ended`.
fn record(object: &str, version: u32, operation: &str, ended: &str) -> Record {
    let mut facts = first_version_facts(object, operation, ended);
    facts.version = format!("v{version}");
    if version > 1 {
        facts.base_version = Some(format!("v{}", version - 1));
        facts.base_state = state(&[("a.txt", 'a')]);
        facts.new_state = state(&[("a.txt", char::from_digit(version, 16).unwrap())]);
    }
    build_record(&facts).expect("build")
}

/// Five activities over two objects; two share one end time, and one
/// operation (`op-shared`) commits a version of both objects.
fn sample_index() -> HistoryIndex {
    let mut index = HistoryIndex::new();
    for rec in [
        record(OBJ_A, 1, "op-1", "2026-10-19T10:00:00Z"),
        record(OBJ_B, 1, "op-2", "2026-10-19T10:00:01Z"),
        record(OBJ_A, 2, "op-shared", "2026-10-19T10:00:02Z"),
        record(OBJ_B, 2, "op-shared", "2026-10-19T10:00:02Z"),
        record(OBJ_A, 3, "op-3", "2026-10-19T10:00:03.5Z"),
    ] {
        index.insert(rec).expect("insert");
    }
    index
}

/// Collect every activity id that `query` yields, page by page.
fn all_pages(index: &HistoryIndex, query: &HistoryQuery, size: usize) -> Vec<String> {
    let mut ids = Vec::new();
    let mut cursor: Option<Cursor> = None;
    for _ in 0..100 {
        let request = PageRequest::new(size, cursor.clone()).expect("size");
        let page = index.query(query, &request);
        assert!(page.items().len() <= size);
        ids.extend(page.items().iter().map(|a| a.id().to_string()));
        cursor = page.next().cloned();
        if cursor.is_none() {
            return ids;
        }
    }
    panic!("paging did not end");
}

#[test]
fn page_size_must_be_between_one_and_a_thousand() {
    assert_eq!(MAX_PAGE_SIZE, 1000);
    for bad in [0, 1001, usize::MAX] {
        let err = PageRequest::new(bad, None).expect_err("refused");
        assert_eq!(err.code(), "PROV_PAGE_SIZE");
    }
    for good in [1, 1000] {
        assert_eq!(PageRequest::new(good, None).expect("ok").limit(), good);
    }
}

#[test]
fn activities_of_an_object_are_ordered_by_end_time() {
    let index = sample_index();
    let ids = all_pages(&index, &HistoryQuery::Object(OBJ_A.to_string()), 2);
    assert_eq!(
        ids,
        [
            filesquid_prov::activity_id("op-1", OBJ_A, "v1"),
            filesquid_prov::activity_id("op-shared", OBJ_A, "v2"),
            filesquid_prov::activity_id("op-3", OBJ_A, "v3"),
        ]
    );
}

#[test]
fn equal_end_times_are_ordered_by_activity_id() {
    let index = sample_index();
    let ids = all_pages(&index, &HistoryQuery::Operation("op-shared".to_string()), 1);
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
    assert_eq!(ids.len(), 2);
}

#[test]
fn every_page_size_yields_the_same_sequence_without_gaps() {
    let index = sample_index();
    let query = HistoryQuery::TimeRange {
        from: Timestamp::parse("2026-10-19T00:00:00Z").unwrap(),
        to: Timestamp::parse("2026-10-20T00:00:00Z").unwrap(),
    };
    let full = all_pages(&index, &query, 1000);
    assert_eq!(full.len(), 5);
    for size in 1..=5 {
        assert_eq!(all_pages(&index, &query, size), full, "page size {size}");
    }
}

#[test]
fn time_range_includes_from_and_excludes_to() {
    let index = sample_index();
    let query = HistoryQuery::TimeRange {
        from: Timestamp::parse("2026-10-19T10:00:01Z").unwrap(),
        to: Timestamp::parse("2026-10-19T10:00:03.5Z").unwrap(),
    };
    assert_eq!(all_pages(&index, &query, 10).len(), 3);
}

#[test]
fn activities_of_an_entity_include_its_generation_and_its_use() {
    let index = sample_index();
    let v1 = entity_id(OBJ_A, "v1", "a.txt");
    let ids = all_pages(&index, &HistoryQuery::Entity(v1), 10);
    assert_eq!(
        ids,
        [
            filesquid_prov::activity_id("op-1", OBJ_A, "v1"),
            filesquid_prov::activity_id("op-shared", OBJ_A, "v2"),
        ]
    );
}

#[test]
fn activities_of_an_agent() {
    let index = sample_index();
    let agent = HistoryQuery::Agent(format!("urn:uuid:{INSTALLATION}"));
    assert_eq!(all_pages(&index, &agent, 3).len(), 5);
    let nobody = HistoryQuery::Agent("urn:uuid:00000000-0000-4000-8000-000000000000".into());
    assert!(all_pages(&index, &nobody, 3).is_empty());
}

#[test]
fn a_full_last_page_has_no_next_cursor() {
    let index = sample_index();
    let request = PageRequest::new(3, None).unwrap();
    let page = index.query(&HistoryQuery::Object(OBJ_A.to_string()), &request);
    assert_eq!(page.items().len(), 3);
    assert!(page.next().is_none());
}

#[test]
fn cursors_travel_as_tokens() {
    let index = sample_index();
    let request = PageRequest::new(2, None).unwrap();
    let page = index.query(&HistoryQuery::Object(OBJ_A.to_string()), &request);
    let token = page.next().expect("more").to_token();
    let cursor = Cursor::from_token(&token).expect("token");
    assert_eq!(&cursor, page.next().unwrap());
    for bad in [
        "",
        "no-separator",
        "2026-10-19T10:00:00+01:00 urn:x",
        "2026-10-19T10:00:00Z urn:x",
        "2026-10-19T10:00:00Z urn:filesquid:activity:op-1",
        "2026-10-19T10:00:00Z urn:filesquid:activity:op-1:obj:v0",
        "2026-10-19T10:00:00Z urn:filesquid:activity:op%3a1:obj:v1",
        "2026-10-19T10:00:00Z urn:filesquid:activity:op%41:obj:v1",
    ] {
        assert_eq!(
            Cursor::from_token(bad).expect_err(bad).code(),
            "PROV_INVALID_CURSOR"
        );
    }
}

#[test]
fn records_are_kept_and_conflicting_duplicates_refused() {
    let mut index = sample_index();
    assert_eq!(index.len(), 5);
    let same = record(OBJ_A, 1, "op-1", "2026-10-19T10:00:00Z");
    index
        .insert(same.clone())
        .expect("identical insert is a no-op");
    assert_eq!(index.len(), 5);
    assert_eq!(index.record(same.activity().id()), Some(&same));
    let changed = record(OBJ_A, 1, "op-1", "2026-10-19T10:00:09Z");
    let err = index.insert(changed).expect_err("conflict");
    assert_eq!(err.code(), "PROV_CONFLICTING_RECORD");
}

#[test]
fn a_cursor_after_an_activity_identifier_longer_than_one_part_round_trips() {
    let long_object = format!("urn:example:{}", "o".repeat(1000));
    let mut index = HistoryIndex::new();
    index
        .insert(record(&long_object, 1, "op-1", "2026-10-19T10:00:00Z"))
        .expect("first");
    index
        .insert(record(&long_object, 2, "op-2", "2026-10-19T10:00:01Z"))
        .expect("second");
    let query = HistoryQuery::Object(long_object);
    let page = index.query(&query, &PageRequest::new(1, None).unwrap());
    let next = page.next().expect("more");
    assert!(next.to_token().len() > 1024);
    let cursor = Cursor::from_token(&next.to_token()).expect("a token the index made");
    let rest = index.query(&query, &PageRequest::new(1, Some(cursor)).unwrap());
    assert_eq!(rest.items()[0].operation(), "op-2");
}

#[test]
fn a_cursor_after_an_operation_with_a_colon_round_trips() {
    let mut index = HistoryIndex::new();
    for (version, operation, ended) in [
        (1, "op:1", "2026-10-19T10:00:00Z"),
        (2, "op:2", "2026-10-19T10:00:01Z"),
    ] {
        index
            .insert(record(OBJ_A, version, operation, ended))
            .expect("insert");
    }
    let query = HistoryQuery::Object(OBJ_A.to_string());
    let page = index.query(&query, &PageRequest::new(1, None).unwrap());
    let token = page.next().expect("more").to_token();
    let cursor = Cursor::from_token(&token).expect("token");
    let rest = index.query(&query, &PageRequest::new(1, Some(cursor)).unwrap());
    assert_eq!(rest.items()[0].operation(), "op:2");
}
