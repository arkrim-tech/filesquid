//! Task CL-0.3 / API-S-024: the replica's permission-aware cache identity, visible coverage
//! and recovery states, and the pending-change ledger whose changes become canonical only
//! through a fenced head publication.

use filesquid_api::host::HostType;
use filesquid_api::library::{
    CommandRequest, DeletionPolicy, LibraryCommand, LibraryQuery, Page, QueryAnswer, QueryRequest,
};
use filesquid_api::operation::{
    CommandEnvelope, ExpectedRevision, OperationId, OperationScope, PageSize, QueryEnvelope,
};
use filesquid_api::replica::{
    answer_from_cache, CacheIdentity, CachedPage, Coverage, GrantDigest, HeadPublication,
    MemoryReplicaCache, PendingLedger, PendingState, ReplicaCache, ReplicaError, ReplicaRequest,
    ViewerId, DEFAULT_CACHE_CAPACITY, MAX_ATTEMPTS, MAX_LIBRARIES, MAX_PENDING_PER_LIBRARY,
};
use filesquid_api::ErrorCode;
use filesquid_core::{FileId, LibraryId, RepositoryId};

const QUEUED_AT: &str = "2026-10-18T00:00:00Z";

/// An identity for `viewer` on library `lib` with `grants`.
fn identity(viewer: &str, lib: &str, grants: &[&str]) -> CacheIdentity {
    CacheIdentity::new(
        HostType::Browser,
        ViewerId::new(viewer).expect("viewer"),
        LibraryId::new(lib).expect("library"),
        GrantDigest::of(grants).expect("grants"),
    )
}

/// A query request for the deletion policy, with `page_size`.
fn query(page_size: u32) -> QueryRequest {
    QueryRequest {
        envelope: QueryEnvelope::new(
            PageSize::new(page_size).expect("page size"),
            OperationScope::new("library").expect("scope"),
        ),
        cursor: None,
        query: LibraryQuery::DeletionPolicy,
    }
}

/// A cached page for `identity`.
fn page(identity: &CacheIdentity, coverage: Coverage) -> CachedPage {
    CachedPage {
        identity: identity.clone(),
        coverage,
        answer: QueryAnswer::DeletionPolicy(DeletionPolicy::default()),
    }
}

/// A hide command on library `lib` under operation `op`.
fn hide(lib: &str, op: &str, revision: &str) -> CommandRequest {
    CommandRequest {
        envelope: CommandEnvelope::new(
            OperationId::new(op).expect("op"),
            ExpectedRevision::new(revision).expect("revision"),
            OperationScope::new("library").expect("scope"),
        ),
        command: LibraryCommand::HideFiles {
            library_id: LibraryId::new(lib).expect("library"),
            file_ids: vec![FileId::new("f1").expect("file")],
        },
    }
}

fn head(revision: &str) -> HeadPublication {
    HeadPublication::new(revision).expect("head")
}

fn op(name: &str) -> OperationId {
    OperationId::new(name).expect("op")
}

// ---- C1, C2: identity and grant digest -------------------------------------------------

#[test]
fn viewer_ids_are_bounded_and_free_of_control_characters() {
    assert!(ViewerId::new("a").is_ok());
    assert!(ViewerId::new("v".repeat(256)).is_ok());
    for bad in [
        "".to_string(),
        "v".repeat(257),
        "a\nb".to_string(),
        "a\u{7f}".to_string(),
    ] {
        let err = ViewerId::new(bad).expect_err("refused");
        assert_eq!(err.error_code(), "invalid_viewer");
        assert_eq!(err.api_code(), ErrorCode::InvalidRequest);
    }
}

#[test]
fn grant_digest_ignores_order_and_repeats_but_not_membership() {
    let a = GrantDigest::of(&["library.read", "file.read"]).expect("a");
    let b = GrantDigest::of(&["file.read", "library.read", "file.read"]).expect("b");
    let c = GrantDigest::of(&["file.read"]).expect("c");
    let d = GrantDigest::of(&["file.read", "library.read", "library.write"]).expect("d");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, d);
    assert_eq!(a.as_str().len(), 64);
    assert!(a
        .as_str()
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
}

#[test]
fn grant_digest_bounds_the_scope_list() {
    let many: Vec<String> = (0..64).map(|i| format!("scope.{i}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    assert!(GrantDigest::of(&refs).is_ok());
    let too_many: Vec<String> = (0..65).map(|i| format!("scope.{i}")).collect();
    let refs: Vec<&str> = too_many.iter().map(String::as_str).collect();
    assert_eq!(
        GrantDigest::of(&refs).expect_err("65").error_code(),
        "invalid_grants"
    );
    assert_eq!(
        GrantDigest::of(&[]).expect_err("none").error_code(),
        "invalid_grants"
    );
    assert_eq!(
        GrantDigest::of(&[""]).expect_err("blank").error_code(),
        "invalid_grants"
    );
    let long = "s".repeat(129);
    assert_eq!(
        GrantDigest::of(&[long.as_str()])
            .expect_err("long")
            .error_code(),
        "invalid_grants"
    );
    let edge = "s".repeat(128);
    assert!(GrantDigest::of(&[edge.as_str()]).is_ok());
}

// ---- C3, C4: keys ----------------------------------------------------------------------

#[test]
fn equal_identities_give_equal_keys_and_every_component_changes_the_key() {
    let base = identity("alice", "lib-1", &["file.read"]);
    assert_eq!(base.key(), identity("alice", "lib-1", &["file.read"]).key());
    assert_eq!(base.key().as_str().len(), 64);
    let others = [
        identity("bob", "lib-1", &["file.read"]),
        identity("alice", "lib-2", &["file.read"]),
        identity("alice", "lib-1", &["file.read", "file.write"]),
        CacheIdentity::new(
            HostType::Desktop,
            ViewerId::new("alice").expect("viewer"),
            LibraryId::new("lib-1").expect("library"),
            GrantDigest::of(&["file.read"]).expect("grants"),
        ),
    ];
    for other in &others {
        assert_ne!(base.key(), other.key());
    }
}

#[test]
fn the_key_encoding_is_not_ambiguous_across_field_boundaries() {
    let a = identity("ab", "c", &["file.read"]);
    let b = identity("a", "bc", &["file.read"]);
    assert_ne!(a.key(), b.key());
}

#[test]
fn keys_and_identities_carry_no_name_or_path() {
    let id = identity("alice", "lib-1", &["file.read"]);
    let json = serde_json::to_string(&id).expect("json");
    assert!(!json.contains('/') && !json.contains('\\'));
    assert!(!id.key().as_str().contains("alice"));
    let back: CacheIdentity = serde_json::from_str(&json).expect("round trip");
    assert_eq!(back, id);
}

#[test]
fn query_keys_differ_by_identity_page_size_cursor_and_query() {
    let id = identity("alice", "lib-1", &["file.read"]);
    let key = id.query_key(&query(10)).expect("key");
    assert_eq!(key, id.query_key(&query(10)).expect("again"));
    assert_ne!(key, id.query_key(&query(11)).expect("size"));
    let other = identity("bob", "lib-1", &["file.read"]);
    assert_ne!(key, other.query_key(&query(10)).expect("bob"));
    let mut open = query(10);
    open.query = LibraryQuery::OpenLibrary {
        library_id: LibraryId::new("lib-1").expect("library"),
    };
    assert_ne!(key, id.query_key(&open).expect("open"));
    assert_ne!(key, id.key(), "a query key is never the identity key");
}

#[test]
fn a_changed_grant_supersedes_the_old_identity_only_for_the_same_viewer_and_library() {
    let old = identity("alice", "lib-1", &["file.read", "file.write"]);
    let new = identity("alice", "lib-1", &["file.read"]);
    assert!(old.is_superseded_by(&new));
    assert!(!old.is_superseded_by(&old));
    assert!(!old.is_superseded_by(&identity("bob", "lib-1", &["file.read"])));
    assert!(!old.is_superseded_by(&identity("alice", "lib-2", &["file.read"])));
}

// ---- C5, C6, C7: cache, coverage, recovery ----------------------------------------------

#[test]
fn a_cached_page_answers_with_its_coverage_and_key() {
    let cache = MemoryReplicaCache::new(8).expect("cache");
    let id = identity("alice", "lib-1", &["file.read"]);
    let request = ReplicaRequest {
        identity: id.clone(),
        request: query(10),
    };
    let key = id.query_key(&request.request).expect("key");
    cache
        .store(key.clone(), page(&id, Coverage::Complete))
        .expect("store");
    let answer = answer_from_cache(&cache, &request).expect("hit");
    assert_eq!(answer.cache_key, key);
    assert_eq!(answer.coverage, Coverage::Complete);
    assert_eq!(
        answer.answer,
        QueryAnswer::DeletionPolicy(DeletionPolicy::default())
    );
}

#[test]
fn incomplete_coverage_is_visible_on_the_answer() {
    let cache = MemoryReplicaCache::new(8).expect("cache");
    let id = identity("alice", "lib-1", &["file.read"]);
    let request = ReplicaRequest {
        identity: id.clone(),
        request: query(10),
    };
    let key = id.query_key(&request.request).expect("key");
    let partial = Coverage::from_shards(3, 8).expect("coverage");
    cache.store(key, page(&id, partial)).expect("store");
    let answer = answer_from_cache(&cache, &request).expect("hit");
    assert_eq!(
        answer.coverage,
        Coverage::Incomplete {
            loaded: 3,
            total: 8
        }
    );
}

#[test]
fn coverage_is_complete_only_when_every_shard_is_loaded() {
    assert_eq!(Coverage::from_shards(8, 8), Ok(Coverage::Complete));
    assert_eq!(
        Coverage::from_shards(0, 8),
        Ok(Coverage::Incomplete {
            loaded: 0,
            total: 8
        })
    );
    for (loaded, total) in [(9, 8), (0, 0), (1, 4_097)] {
        let err = Coverage::from_shards(loaded, total).expect_err("refused");
        assert_eq!(err.error_code(), "invalid_coverage", "{loaded}/{total}");
    }
    assert_eq!(Coverage::from_shards(4_096, 4_096), Ok(Coverage::Complete));
}

#[test]
fn a_miss_is_not_cached_and_never_an_empty_page() {
    let cache = MemoryReplicaCache::new(8).expect("cache");
    let request = ReplicaRequest {
        identity: identity("alice", "lib-1", &["file.read"]),
        request: query(10),
    };
    let err = answer_from_cache(&cache, &request).expect_err("miss");
    assert_eq!(err.error_code(), "not_cached");
    assert_eq!(err.api_code(), ErrorCode::Unavailable);
    assert!(err.api_code().retryable());
}

#[test]
fn an_evicted_entry_says_evicted() {
    let cache = MemoryReplicaCache::new(8).expect("cache");
    let id = identity("alice", "lib-1", &["file.read"]);
    let request = ReplicaRequest {
        identity: id.clone(),
        request: query(10),
    };
    let key = id.query_key(&request.request).expect("key");
    cache
        .store(key.clone(), page(&id, Coverage::Complete))
        .expect("store");
    assert!(cache.evict(&key).expect("evict"));
    assert!(!cache.evict(&key).expect("again"));
    let err = answer_from_cache(&cache, &request).expect_err("evicted");
    assert_eq!(err.error_code(), "evicted");
    assert_eq!(err.api_code(), ErrorCode::Unavailable);
}

#[test]
fn storing_again_after_eviction_recovers_the_entry() {
    let cache = MemoryReplicaCache::new(8).expect("cache");
    let id = identity("alice", "lib-1", &["file.read"]);
    let request = ReplicaRequest {
        identity: id.clone(),
        request: query(10),
    };
    let key = id.query_key(&request.request).expect("key");
    cache
        .store(key.clone(), page(&id, Coverage::Complete))
        .expect("store");
    cache.evict(&key).expect("evict");
    cache
        .store(key, page(&id, Coverage::Complete))
        .expect("restore");
    assert!(answer_from_cache(&cache, &request).is_ok());
}

#[test]
fn the_cache_is_bounded_evicts_oldest_first_and_remembers_it() {
    let cache = MemoryReplicaCache::new(2).expect("cache");
    let id = identity("alice", "lib-1", &["file.read"]);
    let requests: Vec<ReplicaRequest> = [1, 2, 3]
        .into_iter()
        .map(|n| ReplicaRequest {
            identity: id.clone(),
            request: query(n),
        })
        .collect();
    for request in &requests {
        let key = id.query_key(&request.request).expect("key");
        cache
            .store(key, page(&id, Coverage::Complete))
            .expect("store");
    }
    assert_eq!(cache.len().expect("len"), 2);
    let oldest = answer_from_cache(&cache, &requests[0]).expect_err("oldest gone");
    assert_eq!(oldest.error_code(), "evicted");
    assert!(answer_from_cache(&cache, &requests[1]).is_ok());
    assert!(answer_from_cache(&cache, &requests[2]).is_ok());
}

#[test]
fn the_eviction_memory_is_bounded_too() {
    let cache = MemoryReplicaCache::new(1).expect("cache");
    let id = identity("alice", "lib-1", &["file.read"]);
    let first = ReplicaRequest {
        identity: id.clone(),
        request: query(1),
    };
    for n in 1..=3 {
        let request = ReplicaRequest {
            identity: id.clone(),
            request: query(n),
        };
        let key = id.query_key(&request.request).expect("key");
        cache
            .store(key, page(&id, Coverage::Complete))
            .expect("store");
    }
    // Query 1 was evicted first and has since been forgotten: it reads as not cached.
    let err = answer_from_cache(&cache, &first).expect_err("forgotten");
    assert_eq!(err.error_code(), "not_cached");
}

#[test]
fn cache_capacity_must_be_positive_and_has_a_default() {
    assert_eq!(
        MemoryReplicaCache::new(0).expect_err("zero").error_code(),
        "invalid_capacity"
    );
    assert_eq!(DEFAULT_CACHE_CAPACITY, 1_024);
    assert!(MemoryReplicaCache::with_default_capacity()
        .is_empty()
        .expect("empty"));
}

#[test]
fn a_page_stored_for_another_identity_is_never_served() {
    let cache = MemoryReplicaCache::new(8).expect("cache");
    let alice = identity("alice", "lib-1", &["file.read"]);
    let bob = identity("bob", "lib-1", &["file.read"]);
    let request = ReplicaRequest {
        identity: alice.clone(),
        request: query(10),
    };
    let key = alice.query_key(&request.request).expect("key");
    cache
        .store(key, page(&bob, Coverage::Complete))
        .expect("store");
    let err = answer_from_cache(&cache, &request).expect_err("mismatch");
    assert_eq!(err.error_code(), "identity_mismatch");
    assert_eq!(err.api_code(), ErrorCode::Forbidden);
}

#[test]
fn a_grant_change_makes_the_old_entries_unreachable() {
    let cache = MemoryReplicaCache::new(8).expect("cache");
    let wide = identity("alice", "lib-1", &["file.read", "file.write"]);
    let narrow = identity("alice", "lib-1", &["file.read"]);
    let wide_request = ReplicaRequest {
        identity: wide.clone(),
        request: query(10),
    };
    let key = wide.query_key(&wide_request.request).expect("key");
    cache
        .store(key, page(&wide, Coverage::Complete))
        .expect("store");
    let narrow_request = ReplicaRequest {
        identity: narrow.clone(),
        request: query(10),
    };
    let err = answer_from_cache(&cache, &narrow_request).expect_err("not for narrow grants");
    assert_eq!(err.error_code(), "not_cached");
}

// ---- C8 to C14: pending changes ---------------------------------------------------------

#[test]
fn a_new_change_is_queued_with_its_command_unchanged() {
    let mut ledger = PendingLedger::new();
    let request = hide("lib-1", "op-1", "3");
    let change = ledger.enqueue(request.clone(), QUEUED_AT).expect("enqueue");
    assert_eq!(change.state, PendingState::Queued);
    assert_eq!(change.attempts, 0);
    assert_eq!(change.request, request);
    assert_eq!(change.library_id, LibraryId::new("lib-1").expect("library"));
    assert_eq!(change.queued_at, QUEUED_AT);
    assert_eq!(ledger.pending_count(&change.library_id), 1);
}

#[test]
fn queued_at_must_be_rfc3339_utc() {
    let mut ledger = PendingLedger::new();
    for bad in ["yesterday", "2026-10-18T00:00:00+02:00", "2026-10-18", ""] {
        let err = ledger
            .enqueue(hide("lib-1", "op-1", "3"), bad)
            .expect_err("refused");
        assert_eq!(err.error_code(), "invalid_time", "{bad}");
    }
}

#[test]
fn create_library_cannot_be_pending_because_it_names_no_library() {
    let mut ledger = PendingLedger::new();
    let request = CommandRequest {
        envelope: CommandEnvelope::new(
            op("op-1"),
            ExpectedRevision::new("0").expect("revision"),
            OperationScope::new("library").expect("scope"),
        ),
        command: LibraryCommand::CreateLibrary {
            repository_id: RepositoryId::new("repo").expect("repo"),
            name: "Family".to_string(),
        },
    };
    let err = ledger.enqueue(request, QUEUED_AT).expect_err("refused");
    assert_eq!(err.error_code(), "pending_needs_library");
}

#[test]
fn a_repeated_operation_id_is_the_same_change() {
    let mut ledger = PendingLedger::new();
    let first = ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("first");
    let again = ledger
        .enqueue(hide("lib-1", "op-1", "3"), "2026-10-18T01:00:00Z")
        .expect("again");
    assert_eq!(first, again);
    assert_eq!(ledger.pending_count(&first.library_id), 1);
    let other = ledger
        .enqueue(hide("lib-1", "op-1", "9"), QUEUED_AT)
        .expect_err("different");
    assert_eq!(other.error_code(), "operation_reused");
    assert_eq!(other.api_code(), ErrorCode::Conflict);
}

#[test]
fn pending_changes_are_bounded_per_library() {
    let mut ledger = PendingLedger::new();
    for n in 0..MAX_PENDING_PER_LIBRARY {
        ledger
            .enqueue(hide("lib-1", &format!("op-{n}"), "3"), QUEUED_AT)
            .expect("room");
    }
    let err = ledger
        .enqueue(hide("lib-1", "op-over", "3"), QUEUED_AT)
        .expect_err("full");
    assert_eq!(err.error_code(), "pending_full");
    assert_eq!(err.api_code(), ErrorCode::Conflict);
    assert!(ledger
        .enqueue(hide("lib-2", "op-other", "3"), QUEUED_AT)
        .is_ok());
}

#[test]
fn the_number_of_libraries_with_pending_changes_is_bounded() {
    let mut ledger = PendingLedger::new();
    for n in 0..MAX_LIBRARIES {
        ledger
            .enqueue(
                hide(&format!("lib-{n}"), &format!("op-{n}"), "3"),
                QUEUED_AT,
            )
            .expect("room");
    }
    let err = ledger
        .enqueue(hide("lib-extra", "op-extra", "3"), QUEUED_AT)
        .expect_err("full");
    assert_eq!(err.error_code(), "pending_full");
}

#[test]
fn only_the_oldest_unfinished_change_may_begin_publishing() {
    let mut ledger = PendingLedger::new();
    ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("one");
    ledger
        .enqueue(hide("lib-1", "op-2", "3"), QUEUED_AT)
        .expect("two");
    let library = LibraryId::new("lib-1").expect("library");
    assert_eq!(
        ledger
            .next_to_publish(&library)
            .expect("next")
            .request
            .envelope
            .operation_id(),
        &op("op-1")
    );
    let err = ledger.begin_publish(&op("op-2")).expect_err("out of order");
    assert_eq!(err.error_code(), "not_next");
    ledger.begin_publish(&op("op-1")).expect("first");
    assert_eq!(
        ledger.get(&op("op-1")).expect("op-1").state,
        PendingState::Publishing
    );
    assert_eq!(ledger.get(&op("op-1")).expect("op-1").attempts, 1);
}

#[test]
fn a_change_is_committed_only_by_a_fenced_head_publication() {
    let mut ledger = PendingLedger::new();
    ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("one");
    let early = ledger
        .commit_published(&op("op-1"), head("4"))
        .expect_err("not publishing");
    assert_eq!(early.error_code(), "wrong_state");
    ledger.begin_publish(&op("op-1")).expect("begin");
    let done = ledger
        .commit_published(&op("op-1"), head("4"))
        .expect("commit");
    assert_eq!(done.state, PendingState::Committed);
    assert_eq!(done.published_revision.as_deref(), Some("4"));
    let library = LibraryId::new("lib-1").expect("library");
    assert_eq!(ledger.pending_count(&library), 0);
    let discard = ledger
        .discard(&op("op-1"))
        .expect_err("committed is canonical");
    assert_eq!(discard.error_code(), "wrong_state");
}

#[test]
fn a_head_publication_names_a_revision() {
    assert_eq!(
        HeadPublication::new("").expect_err("blank").error_code(),
        "invalid_publication"
    );
}

#[test]
fn a_conflict_blocks_later_changes_until_rebased_or_discarded() {
    let mut ledger = PendingLedger::new();
    ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("one");
    ledger
        .enqueue(hide("lib-1", "op-2", "3"), QUEUED_AT)
        .expect("two");
    let library = LibraryId::new("lib-1").expect("library");
    ledger.begin_publish(&op("op-1")).expect("begin");
    let lost = ledger.report_conflict(&op("op-1")).expect("conflict");
    assert_eq!(lost.state, PendingState::Conflict);
    assert!(
        ledger.next_to_publish(&library).is_none(),
        "the conflict blocks the queue"
    );
    assert_eq!(
        ledger
            .begin_publish(&op("op-2"))
            .expect_err("blocked")
            .error_code(),
        "not_next"
    );

    let rebased = ledger
        .rebase(&op("op-1"), ExpectedRevision::new("4").expect("revision"))
        .expect("rebase");
    assert_eq!(rebased.state, PendingState::Queued);
    assert_eq!(rebased.attempts, 0);
    assert_eq!(rebased.request.envelope.expected_revision().as_str(), "4");
    assert_eq!(
        ledger
            .next_to_publish(&library)
            .expect("next")
            .request
            .envelope
            .operation_id(),
        &op("op-1")
    );

    ledger.begin_publish(&op("op-1")).expect("again");
    ledger.report_conflict(&op("op-1")).expect("conflict again");
    ledger.discard(&op("op-1")).expect("discard");
    assert!(ledger.get(&op("op-1")).is_none());
    assert_eq!(
        ledger
            .next_to_publish(&library)
            .expect("op-2 is next")
            .request
            .envelope
            .operation_id(),
        &op("op-2")
    );
}

#[test]
fn a_publishing_change_can_be_neither_discarded_nor_rebased() {
    let mut ledger = PendingLedger::new();
    ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("one");
    ledger.begin_publish(&op("op-1")).expect("begin");
    assert_eq!(
        ledger
            .discard(&op("op-1"))
            .expect_err("publishing")
            .error_code(),
        "wrong_state"
    );
    let revision = ExpectedRevision::new("4").expect("revision");
    assert_eq!(
        ledger
            .rebase(&op("op-1"), revision)
            .expect_err("publishing")
            .error_code(),
        "wrong_state"
    );
}

#[test]
fn a_retry_waits_then_publishes_again_and_stalls_after_the_attempt_bound() {
    let mut ledger = PendingLedger::new();
    ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("one");
    let library = LibraryId::new("lib-1").expect("library");
    for attempt in 1..MAX_ATTEMPTS {
        ledger.begin_publish(&op("op-1")).expect("begin");
        let waiting = ledger.report_retry(&op("op-1")).expect("retry");
        assert_eq!(waiting.state, PendingState::RetryWait, "attempt {attempt}");
        assert!(ledger.next_to_publish(&library).is_some());
    }
    ledger.begin_publish(&op("op-1")).expect("last attempt");
    let stalled = ledger.report_retry(&op("op-1")).expect("stall");
    assert_eq!(stalled.state, PendingState::Stalled);
    assert_eq!(stalled.attempts, MAX_ATTEMPTS);
    assert!(
        ledger.next_to_publish(&library).is_none(),
        "a stalled head blocks the queue"
    );
    assert_eq!(
        ledger
            .begin_publish(&op("op-1"))
            .expect_err("stalled")
            .error_code(),
        "wrong_state"
    );
    let rebased = ledger
        .rebase(&op("op-1"), ExpectedRevision::new("5").expect("revision"))
        .expect("owner rebases");
    assert_eq!(rebased.state, PendingState::Queued);
}

#[test]
fn unknown_operations_are_not_found() {
    let mut ledger = PendingLedger::new();
    for err in [
        ledger.begin_publish(&op("nope")).expect_err("begin"),
        ledger.report_retry(&op("nope")).expect_err("retry"),
        ledger.report_conflict(&op("nope")).expect_err("conflict"),
        ledger.discard(&op("nope")).expect_err("discard"),
    ] {
        assert_eq!(err.error_code(), "not_found");
        assert_eq!(err.api_code(), ErrorCode::NotFound);
    }
}

#[test]
fn the_snapshot_restores_the_same_ledger_and_requeues_an_interrupted_publication() {
    let mut ledger = PendingLedger::new();
    ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("one");
    ledger
        .enqueue(hide("lib-1", "op-2", "3"), QUEUED_AT)
        .expect("two");
    ledger.begin_publish(&op("op-1")).expect("begin");
    let json = ledger.snapshot().expect("snapshot");
    let restored = PendingLedger::restore(&json).expect("restore");
    let first = restored.get(&op("op-1")).expect("op-1");
    assert_eq!(
        first.state,
        PendingState::RetryWait,
        "an interrupted publication retries"
    );
    assert_eq!(first.attempts, 1);
    assert_eq!(
        restored.get(&op("op-2")).expect("op-2").state,
        PendingState::Queued
    );
    assert!(!json.contains("/"), "no path in a snapshot");
}

#[test]
fn restore_refuses_duplicates_malformed_input_and_over_bound_snapshots() {
    let mut ledger = PendingLedger::new();
    ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("one");
    let json = ledger.snapshot().expect("snapshot");
    let mut value: serde_json::Value = serde_json::from_str(&json).expect("json");
    let first = value.as_array().expect("array")[0].clone();
    value.as_array_mut().expect("array").push(first);
    let dup = PendingLedger::restore(&value.to_string()).expect_err("duplicate");
    assert_eq!(dup.error_code(), "invalid_snapshot");
    assert_eq!(
        PendingLedger::restore("{")
            .expect_err("malformed")
            .error_code(),
        "invalid_snapshot"
    );

    let one = ledger.get(&op("op-1")).expect("op-1").clone();
    let over: Vec<_> = (0..=MAX_PENDING_PER_LIBRARY)
        .map(|n| {
            let mut change = one.clone();
            change.request = hide("lib-1", &format!("op-{n}"), "3");
            change
        })
        .collect();
    let over = serde_json::to_string(&over).expect("json");
    assert_eq!(
        PendingLedger::restore(&over)
            .expect_err("over")
            .error_code(),
        "invalid_snapshot"
    );
}

#[test]
fn restore_refuses_a_committed_change_without_a_published_revision() {
    let mut ledger = PendingLedger::new();
    ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("one");
    let json = ledger.snapshot().expect("snapshot");
    let forged = json.replace("\"queued\"", "\"committed\"");
    assert_eq!(
        PendingLedger::restore(&forged)
            .expect_err("forged")
            .error_code(),
        "invalid_snapshot"
    );
}

#[test]
fn every_error_has_a_stable_code_and_a_message_without_names() {
    let errors: Vec<ReplicaError> = vec![
        ViewerId::new("").expect_err("viewer"),
        GrantDigest::of(&[]).expect_err("grants"),
        Coverage::from_shards(0, 0).expect_err("coverage"),
    ];
    for err in errors {
        assert!(!err.message().is_empty());
        assert!(!err.error_code().is_empty());
        assert!(!err.message().contains('/'));
        let api: filesquid_api::ApiError = err.into();
        assert_eq!(api.code(), ErrorCode::InvalidRequest);
    }
}

#[test]
fn a_page_answer_type_round_trips_as_json() {
    let answer = QueryAnswer::Collections(Page {
        items: vec![],
        next_cursor: None,
    });
    let json = serde_json::to_string(&answer).expect("json");
    let back: QueryAnswer = serde_json::from_str(&json).expect("back");
    assert_eq!(back, answer);
}

#[test]
fn only_a_committed_change_can_be_acknowledged() {
    let mut ledger = PendingLedger::new();
    ledger
        .enqueue(hide("lib-1", "op-1", "3"), QUEUED_AT)
        .expect("one");
    assert_eq!(
        ledger
            .acknowledge(&op("op-1"))
            .expect_err("queued")
            .error_code(),
        "wrong_state"
    );
    ledger.begin_publish(&op("op-1")).expect("begin");
    ledger
        .commit_published(&op("op-1"), head("4"))
        .expect("commit");
    ledger.acknowledge(&op("op-1")).expect("acknowledge");
    assert!(ledger.get(&op("op-1")).is_none());
    assert_eq!(
        ledger
            .acknowledge(&op("op-1"))
            .expect_err("gone")
            .error_code(),
        "not_found"
    );
}
