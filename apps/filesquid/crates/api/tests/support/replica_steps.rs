//! Step definitions for `replica_cache_identity_and_pending_changes_API-S-024.feature`
//! (Task CL-0.3): the replica cache identity, coverage and recovery states, and the pending
//! change ledger, driven through the public `filesquid_api::replica` surface.

use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::host::HostType;
use filesquid_api::library::{
    CommandRequest, DeletionPolicy, LibraryCommand, LibraryQuery, QueryAnswer, QueryRequest,
};
use filesquid_api::operation::{
    CommandEnvelope, ExpectedRevision, OperationId, OperationScope, PageSize, QueryEnvelope,
};
use filesquid_api::replica::{
    answer_from_cache, CacheIdentity, CachedPage, Coverage, GrantDigest, HeadPublication,
    MemoryReplicaCache, PendingLedger, ReplicaAnswer, ReplicaCache, ReplicaError, ReplicaRequest,
    ViewerId, MAX_ATTEMPTS,
};
use filesquid_core::{FileId, LibraryId};

const QUEUED_AT: &str = "2026-10-18T00:00:00Z";

/// Per-scenario state of the replica steps.
#[derive(Default)]
pub struct ReplicaWorld {
    viewer: String,
    library: String,
    grants: Vec<String>,
    cache: Option<MemoryReplicaCache>,
    ledger: PendingLedger,
    answer: Option<Result<ReplicaAnswer, ReplicaError>>,
    refused: Option<ReplicaError>,
    next_op: u32,
}

impl std::fmt::Debug for ReplicaWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplicaWorld")
            .field("library", &self.library)
            .finish()
    }
}

/// The identity of `viewer` on the scenario's library under `grants`.
fn identity(viewer: &str, library: &str, grants: &[String]) -> CacheIdentity {
    let scopes: Vec<&str> = grants.iter().map(String::as_str).collect();
    CacheIdentity::new(
        HostType::Browser,
        ViewerId::new(viewer).expect("viewer"),
        LibraryId::new(library).expect("library"),
        GrantDigest::of(&scopes).expect("grants"),
    )
}

/// The one query every scenario reads.
fn query() -> QueryRequest {
    QueryRequest {
        envelope: QueryEnvelope::new(
            PageSize::new(10).expect("page size"),
            OperationScope::new("library").expect("scope"),
        ),
        cursor: None,
        query: LibraryQuery::DeletionPolicy,
    }
}

/// A cached page for `identity` with `coverage`.
fn page(identity: CacheIdentity, coverage: Coverage) -> CachedPage {
    CachedPage {
        identity,
        coverage,
        answer: QueryAnswer::DeletionPolicy(DeletionPolicy::default()),
    }
}

/// A hide command on `library` under operation `op`.
fn hide(library: &str, op: &str) -> CommandRequest {
    CommandRequest {
        envelope: CommandEnvelope::new(
            OperationId::new(op).expect("op"),
            ExpectedRevision::new("3").expect("revision"),
            OperationScope::new("library").expect("scope"),
        ),
        command: LibraryCommand::HideFiles {
            library_id: LibraryId::new(library).expect("library"),
            file_ids: vec![FileId::new("f1").expect("file")],
        },
    }
}

impl ReplicaWorld {
    fn cache(&self) -> &MemoryReplicaCache {
        self.cache.as_ref().expect("replica cache")
    }

    fn alice(&self) -> CacheIdentity {
        identity(&self.viewer, &self.library, &self.grants)
    }

    fn store(&self, stored_for: CacheIdentity, coverage: Coverage) {
        let key = self.alice().query_key(&query()).expect("key");
        self.cache()
            .store(key, page(stored_for, coverage))
            .expect("store");
    }

    fn read(&mut self, identity: CacheIdentity) {
        let request = ReplicaRequest {
            identity,
            request: query(),
        };
        self.answer = Some(answer_from_cache(self.cache(), &request));
    }

    fn queue(&mut self) -> OperationId {
        self.next_op += 1;
        let op = format!("op-{}", self.next_op);
        let request = hide(&self.library, &op);
        self.ledger.enqueue(request, QUEUED_AT).expect("enqueue");
        OperationId::new(op).expect("op")
    }

    fn state_of(&self, n: u32) -> String {
        let op = OperationId::new(format!("op-{n}")).expect("op");
        let change = self.ledger.get(&op).expect("change");
        serde_json::to_value(change.state)
            .expect("state")
            .as_str()
            .expect("text")
            .to_string()
    }
}

#[given(
    expr = "a replica for viewer {string} on library {string} with grants {string} and {string}"
)]
fn replica(w: &mut MonitorWorld, viewer: String, library: String, a: String, b: String) {
    w.replica.viewer = viewer;
    w.replica.library = library;
    w.replica.grants = vec![a, b];
    w.replica.cache = Some(MemoryReplicaCache::new(8).expect("cache"));
    w.replica.ledger = PendingLedger::new();
}

#[given(expr = "the replica stored a complete view for alice")]
fn stored_complete(w: &mut MonitorWorld) {
    w.replica.store(w.replica.alice(), Coverage::Complete);
}

#[given(expr = "the replica stored a view with {int} of {int} index parts loaded")]
fn stored_partial(w: &mut MonitorWorld, loaded: u32, total: u32) {
    let coverage = Coverage::from_shards(loaded, total).expect("coverage");
    w.replica.store(w.replica.alice(), coverage);
}

#[given(expr = "the replica holds a view made for bob under alice's key")]
fn stored_for_bob(w: &mut MonitorWorld) {
    let bob = identity("bob", &w.replica.library, &w.replica.grants);
    w.replica.store(bob, Coverage::Complete);
}

#[given(expr = "the device evicts that view")]
fn evict(w: &mut MonitorWorld) {
    let key = w.replica.alice().query_key(&query()).expect("key");
    assert!(w.replica.cache().evict(&key).expect("evict"));
}

#[when(expr = "alice reads the view from the replica")]
fn alice_reads(w: &mut MonitorWorld) {
    let id = w.replica.alice();
    w.replica.read(id);
}

#[when(expr = "bob reads the same view from the replica")]
fn bob_reads(w: &mut MonitorWorld) {
    let id = identity("bob", &w.replica.library, &w.replica.grants);
    w.replica.read(id);
}

#[when(expr = "alice's grants shrink to {string} and she reads the same view")]
fn narrowed(w: &mut MonitorWorld, grant: String) {
    let id = identity(&w.replica.viewer, &w.replica.library, &[grant]);
    w.replica.read(id);
}

#[then(expr = "the answer should say the coverage is complete")]
fn coverage_complete(w: &mut MonitorWorld) {
    let answer = w.replica.answer.take().expect("read").expect("hit");
    assert_eq!(answer.coverage, Coverage::Complete);
}

#[then(expr = "the answer should say {int} of {int} parts are loaded")]
fn coverage_partial(w: &mut MonitorWorld, loaded: u32, total: u32) {
    let answer = w.replica.answer.take().expect("read").expect("hit");
    assert_eq!(answer.coverage, Coverage::Incomplete { loaded, total });
}

#[then(expr = "the replica should report {string}")]
fn reports(w: &mut MonitorWorld, code: String) {
    let err = w
        .replica
        .answer
        .as_ref()
        .expect("read")
        .as_ref()
        .expect_err("refused");
    assert_eq!(err.error_code(), code);
}

#[then(expr = "the answer should not be an empty page")]
fn not_empty_page(w: &mut MonitorWorld) {
    assert!(w.replica.answer.as_ref().expect("read").is_err());
}

#[when(expr = "alice hides a file while offline")]
fn hides_offline(w: &mut MonitorWorld) {
    w.replica.queue();
}

#[given(expr = "alice has queued a change")]
fn queued_one(w: &mut MonitorWorld) {
    w.replica.queue();
}

#[given(expr = "alice has queued two changes")]
fn queued_two(w: &mut MonitorWorld) {
    w.replica.queue();
    w.replica.queue();
}

#[when(
    expr = "the change begins publishing and the head publication succeeds at revision {string}"
)]
fn published(w: &mut MonitorWorld, revision: String) {
    let op = OperationId::new("op-1").expect("op");
    w.replica.ledger.begin_publish(&op).expect("begin");
    let head = HeadPublication::new(revision).expect("head");
    w.replica
        .ledger
        .commit_published(&op, head)
        .expect("commit");
}

#[when(expr = "the first change begins publishing and loses the race")]
fn lost_race(w: &mut MonitorWorld) {
    let op = OperationId::new("op-1").expect("op");
    w.replica.ledger.begin_publish(&op).expect("begin");
    w.replica.ledger.report_conflict(&op).expect("conflict");
}

#[when(expr = "the change fails to publish {int} times in a row")]
fn fails_repeatedly(w: &mut MonitorWorld, times: u32) {
    assert_eq!(times, MAX_ATTEMPTS);
    let op = OperationId::new("op-1").expect("op");
    for _ in 0..times {
        w.replica.ledger.begin_publish(&op).expect("begin");
        w.replica.ledger.report_retry(&op).expect("retry");
    }
}

#[when(expr = "the device reloads from the saved list of pending changes")]
fn reloads(w: &mut MonitorWorld) {
    let json = w.replica.ledger.snapshot().expect("snapshot");
    w.replica.ledger = PendingLedger::restore(&json).expect("restore");
}

#[when(expr = "the device reloads from a damaged saved list")]
fn reload_damaged(w: &mut MonitorWorld) {
    w.replica.refused = PendingLedger::restore("[{\"library_id\":").err();
}

#[then(expr = "the change should be {string}")]
fn change_state(w: &mut MonitorWorld, state: String) {
    assert_eq!(w.replica.state_of(1), state);
}

#[then(expr = "the first change should be {string}")]
fn first_state(w: &mut MonitorWorld, state: String) {
    assert_eq!(w.replica.state_of(1), state);
}

#[then(expr = "library {string} should have {int} pending change(s)")]
fn pending_count(w: &mut MonitorWorld, library: String, count: usize) {
    let library = LibraryId::new(library).expect("library");
    assert_eq!(w.replica.ledger.pending_count(&library), count);
}

#[then(expr = "the second change should not be allowed to publish")]
fn second_blocked(w: &mut MonitorWorld) {
    let op = OperationId::new("op-2").expect("op");
    let err = w.replica.ledger.begin_publish(&op).expect_err("blocked");
    assert_eq!(err.error_code(), "not_next");
}

#[then(expr = "the saved list should be refused as {string}")]
fn list_refused(w: &mut MonitorWorld, code: String) {
    let err = w.replica.refused.as_ref().expect("refused");
    assert_eq!(err.error_code(), code);
}
