//! The fenced per-object lease that reserves `vN` (IO-0.6; OCFL-S-018;
//! ADR 007 exclusive leases, ADR 012 conflict guard, ADR 016 write step 2).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use filesquid_ocfl::storage::{complete_immediately, MemoryBackend, StorageBackend};
use filesquid_ocfl::write::{
    rfc3339_utc, Clock, LeaseConfig, LeaseStore, Reservation, ReservationError, VersionReservations,
};
use filesquid_ocfl::VersionNum;

/// A clock the test moves by hand.
#[derive(Debug, Clone, Default)]
struct ManualClock(Arc<AtomicU64>);

impl ManualClock {
    /// Move the clock forward by `millis`.
    fn advance(&self, millis: u64) {
        self.0.fetch_add(millis, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_millis(&self) -> Option<u64> {
        Some(self.0.load(Ordering::SeqCst))
    }
}

/// TTL 60 s, margin 10 s.
const CONFIG: LeaseConfig = LeaseConfig {
    ttl_ms: 60_000,
    margin_ms: 10_000,
};

/// Drive a backend future that never waits.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("lease futures complete at once")
}

/// A lease store on a fresh memory backend, and its clock.
fn store() -> (LeaseStore<MemoryBackend, ManualClock>, ManualClock) {
    let clock = ManualClock::default();
    let store = LeaseStore::new(MemoryBackend::new(), "leases", clock.clone(), CONFIG)
        .expect("valid config");
    (store, clock)
}

/// A version number from its name.
fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

/// Reserve `version` of `object` for `holder`.
fn reserve(
    store: &LeaseStore<MemoryBackend, ManualClock>,
    object: &str,
    version: &str,
    holder: &str,
) -> Result<Reservation, ReservationError> {
    ready(store.reserve(object, &v(version), holder))
}

#[test]
fn a_reservation_names_its_object_version_holder_and_first_token() {
    let (store, _clock) = store();
    let held = reserve(&store, "urn:example:a", "v1", "writer-a").expect("reserve");
    assert_eq!(held.object_id(), "urn:example:a");
    assert_eq!(held.version(), &v("v1"));
    assert_eq!(held.holder(), "writer-a");
    assert_eq!(held.token(), 1);
    assert_eq!(ready(store.confirm(&held)), Ok(()));
}

#[test]
fn a_held_lease_refuses_every_other_reservation() {
    let (store, _clock) = store();
    reserve(&store, "urn:example:a", "v1", "writer-a").expect("reserve");
    for holder in ["writer-b", "writer-a"] {
        let err = reserve(&store, "urn:example:a", "v1", holder).expect_err("held");
        assert!(matches!(err, ReservationError::Held { .. }), "{err:?}");
    }
}

#[test]
fn objects_are_reserved_independently() {
    let (store, _clock) = store();
    reserve(&store, "urn:example:a", "v1", "writer-a").expect("reserve a");
    let other = reserve(&store, "urn:example:b", "v1", "writer-b").expect("reserve b");
    assert_eq!(other.token(), 1);
}

#[test]
fn an_expired_lease_is_taken_over_with_a_higher_token() {
    let (store, clock) = store();
    let stale = reserve(&store, "urn:example:a", "v2", "writer-a").expect("reserve");
    clock.advance(CONFIG.ttl_ms);
    let fresh = reserve(&store, "urn:example:a", "v2", "writer-b").expect("takeover");
    assert_eq!(fresh.token(), stale.token() + 1);
    let err = ready(store.confirm(&stale)).expect_err("stale");
    assert!(matches!(err, ReservationError::Lost { .. }), "{err:?}");
    assert_eq!(ready(store.confirm(&fresh)), Ok(()));
}

#[test]
fn an_expired_lease_fails_its_fence_check_even_before_takeover() {
    let (store, clock) = store();
    let held = reserve(&store, "urn:example:a", "v1", "writer-a").expect("reserve");
    clock.advance(CONFIG.ttl_ms);
    let err = ready(store.confirm(&held)).expect_err("expired");
    assert!(matches!(err, ReservationError::Lost { .. }), "{err:?}");
}

#[test]
fn a_lease_within_its_safety_margin_fails_its_fence_check() {
    let (store, clock) = store();
    let held = reserve(&store, "urn:example:a", "v1", "writer-a").expect("reserve");
    clock.advance(CONFIG.ttl_ms - CONFIG.margin_ms);
    let err = ready(store.confirm(&held)).expect_err("inside the margin");
    assert!(matches!(err, ReservationError::Lost { .. }), "{err:?}");
}

#[test]
fn confirming_renews_a_lease_past_half_its_ttl() {
    let (store, clock) = store();
    let held = reserve(&store, "urn:example:a", "v1", "writer-a").expect("reserve");
    for _ in 0..10 {
        clock.advance(CONFIG.ttl_ms / 2 + 1);
        assert_eq!(ready(store.confirm(&held)), Ok(()), "renewed each time");
    }
    let err = reserve(&store, "urn:example:a", "v1", "writer-b").expect_err("still held");
    assert!(matches!(err, ReservationError::Held { .. }), "{err:?}");
}

#[test]
fn a_released_lease_is_lost_and_its_token_keeps_growing() {
    let (store, _clock) = store();
    let first = reserve(&store, "urn:example:a", "v1", "writer-a").expect("reserve");
    ready(store.release(&first)).expect("release");
    let err = ready(store.confirm(&first)).expect_err("released");
    assert!(matches!(err, ReservationError::Lost { .. }), "{err:?}");
    let second = reserve(&store, "urn:example:a", "v2", "writer-b").expect("reserve");
    assert_eq!(second.token(), first.token() + 1);
}

#[test]
fn releasing_a_lost_reservation_is_refused_and_changes_nothing() {
    let (store, clock) = store();
    let stale = reserve(&store, "urn:example:a", "v1", "writer-a").expect("reserve");
    clock.advance(CONFIG.ttl_ms);
    let fresh = reserve(&store, "urn:example:a", "v1", "writer-b").expect("takeover");
    let err = ready(store.release(&stale)).expect_err("not ours");
    assert!(matches!(err, ReservationError::Lost { .. }), "{err:?}");
    assert_eq!(ready(store.confirm(&fresh)), Ok(()));
}

#[test]
fn a_corrupt_lease_record_is_reported() {
    let (store, _clock) = store();
    let key = store.lease_key("urn:example:a").expect("key");
    ready(store.backend().put(&key, b"not json")).expect("put");
    let err = reserve(&store, "urn:example:a", "v1", "writer-a").expect_err("corrupt");
    assert!(matches!(err, ReservationError::Corrupt { .. }), "{err:?}");
}

#[test]
fn lease_records_live_under_the_store_prefix() {
    let (store, _clock) = store();
    let key = store.lease_key("urn:example:a").expect("key");
    assert!(key.as_str().starts_with("leases/"), "{key}");
    assert!(key.as_str().ends_with(".json"), "{key}");
}

#[test]
fn a_margin_of_half_the_ttl_or_more_is_refused() {
    let bad = LeaseConfig {
        ttl_ms: 1_000,
        margin_ms: 500,
    };
    let result = LeaseStore::new(MemoryBackend::new(), "leases", ManualClock::default(), bad);
    assert!(matches!(
        result,
        Err(ReservationError::InvalidConfig { .. })
    ));
}

#[test]
fn an_expired_lease_cannot_be_released() {
    let (store, clock) = store();
    let held = reserve(&store, "urn:example:a", "v1", "writer-a").expect("reserve");
    clock.advance(CONFIG.ttl_ms);
    let err = ready(store.release(&held)).expect_err("expired");
    assert!(matches!(err, ReservationError::Lost { .. }), "{err:?}");
}

#[test]
fn a_record_at_the_largest_token_is_not_wrapped() {
    let (store, _clock) = store();
    let key = store.lease_key("urn:example:a").expect("key");
    let record = format!(
        r#"{{"objectId":"urn:example:a","version":"v1","holder":"w","token":{},"expires":0,"released":true}}"#,
        u64::MAX
    );
    ready(store.backend().put(&key, record.as_bytes())).expect("put");
    let err = reserve(&store, "urn:example:a", "v2", "writer-a").expect_err("no next token");
    assert!(matches!(err, ReservationError::Corrupt { .. }), "{err:?}");
}

#[test]
fn a_record_too_large_to_read_back_is_not_written() {
    let (store, _clock) = store();
    let holder = "h".repeat(5_000);
    let err = reserve(&store, "urn:example:a", "v1", &holder).expect_err("too large");
    assert!(matches!(err, ReservationError::Storage(_)), "{err:?}");
    let key = store.lease_key("urn:example:a").expect("key");
    assert!(!ready(store.backend().exists(&key)).expect("exists"));
}

/// A clock that cannot tell the time.
#[derive(Debug, Clone, Copy)]
struct BrokenClock;

impl Clock for BrokenClock {
    fn now_millis(&self) -> Option<u64> {
        None
    }
}

#[test]
fn a_clock_that_cannot_tell_the_time_fails_closed() {
    let store =
        LeaseStore::new(MemoryBackend::new(), "leases", BrokenClock, CONFIG).expect("valid config");
    let err = ready(store.reserve("urn:example:a", &v("v1"), "writer-a")).expect_err("no time");
    assert_eq!(err, ReservationError::ClockUnavailable);
}

#[test]
fn rfc3339_utc_formats_unix_milliseconds() {
    assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339_utc(951_782_400_000), "2000-02-29T00:00:00Z");
    assert_eq!(rfc3339_utc(1_791_590_399_999), "2026-10-09T23:59:59Z");
}
