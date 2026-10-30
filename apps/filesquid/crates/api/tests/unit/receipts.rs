//! Unit tests of the file command receipts (Task API-F2.13; API-S-002, API-S-019): a stored
//! answer is replayed for the same command, refused for another, an ID in flight is busy, a
//! dropped claim frees its ID, and the oldest receipt is forgotten at the cap.

use filesquid_api::files::receipts::{Claim, CommandReceipts, Fingerprint, MAX_RECEIPTS};
use serde_json::json;
use std::sync::Arc;

/// The fingerprint of a move of `from` in library `lib`.
fn moving(from: &str) -> Fingerprint {
    Fingerprint {
        route: "file.move",
        library_id: "lib".to_string(),
        input: json!({ "from": from, "to": "b.txt" }),
    }
}

/// Claim `id` for `fingerprint`, expecting a fresh claim, and store `answer`.
fn store(receipts: &Arc<CommandReceipts>, id: &str, fingerprint: Fingerprint, answer: i64) {
    match receipts.claim(id, fingerprint) {
        Claim::Fresh(guard) => guard.store(json!(answer)),
        other => panic!("a fresh claim, not {other:?}"),
    }
}

#[test]
fn a_stored_answer_is_replayed_for_the_same_command_only() {
    let receipts = Arc::new(CommandReceipts::new());
    store(&receipts, "op-1", moving("a.txt"), 7);
    assert!(matches!(receipts.claim("op-1", moving("a.txt")), Claim::Replay(v) if v == json!(7)));
    assert!(matches!(
        receipts.claim("op-1", moving("c.txt")),
        Claim::Reused
    ));
    assert_eq!(receipts.len(), 1);
}

#[test]
fn a_second_claim_while_one_is_in_flight_is_busy() {
    let receipts = Arc::new(CommandReceipts::new());
    let first = receipts.claim("op-1", moving("a.txt"));
    assert!(matches!(first, Claim::Fresh(_)));
    assert!(matches!(
        receipts.claim("op-1", moving("a.txt")),
        Claim::Busy
    ));
    drop(first);
}

#[test]
fn a_dropped_claim_frees_its_id_and_stores_nothing() {
    let receipts = Arc::new(CommandReceipts::new());
    let claim = receipts.claim("op-1", moving("a.txt"));
    drop(claim);
    assert!(receipts.is_empty());
    assert!(matches!(
        receipts.claim("op-1", moving("a.txt")),
        Claim::Fresh(_)
    ));
}

#[test]
fn the_oldest_receipt_is_forgotten_at_the_cap() {
    let receipts = Arc::new(CommandReceipts::new());
    for n in 0..MAX_RECEIPTS {
        store(&receipts, &format!("op-{n}"), moving("a.txt"), 1);
    }
    assert_eq!(receipts.len(), MAX_RECEIPTS);
    store(&receipts, "op-new", moving("a.txt"), 2);
    assert_eq!(receipts.len(), MAX_RECEIPTS);
    assert!(
        matches!(receipts.claim("op-0", moving("a.txt")), Claim::Fresh(_)),
        "forgotten"
    );
    assert!(
        matches!(receipts.claim("op-1", moving("a.txt")), Claim::Replay(_)),
        "kept"
    );
}
