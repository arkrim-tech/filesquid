//! Journal and commit linkage for PROV-O history (API-F2.8; API-S-028,
//! API-S-017; PRD F-067, F-061; ADR 040 Decision 6): journal entries carry
//! the operation ID and the object@version they committed, both opaque and
//! redacted like every other identifier, and every OCFL commit is journaled
//! as `ocfl.commit`.

use std::sync::Arc;

use filesquid_api::journal::commit::{
    ocfl_commit_entries, CommitFacts, LEASE_TAKEOVER, OCFL_COMMIT,
};
use filesquid_api::journal::{
    JournalEntry, JournalQuery, JournalStore, Level, MemoryJournal, Outcome, REDACTED,
};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::metrics::Metrics;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::EventBus;
use filesquid_core::{OperationId, VersionRef};
use tracing_subscriber::layer::SubscriberExt;

/// A FileSquid object ID.
const OBJECT: &str = "urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

fn op(id: &str) -> OperationId {
    OperationId::new(id).expect("operation id")
}

fn version(name: &str) -> VersionRef {
    VersionRef::new(OBJECT, name).expect("version ref")
}

#[test]
fn an_entry_carries_its_operation_id_and_object_reference() {
    let entry = JournalEntry::new(OCFL_COMMIT, Outcome::Succeeded)
        .operation_id("op-1")
        .object_ref(format!("{OBJECT}@v3"));
    let json = serde_json::to_value(entry.clone().redacted()).unwrap();
    assert_eq!(json["operation_id"], "op-1");
    assert_eq!(json["object_ref"], format!("{OBJECT}@v3"));
    let plain = serde_json::to_value(JournalEntry::new("import", Outcome::Succeeded)).unwrap();
    assert!(
        plain.get("operation_id").is_none(),
        "absent fields are left out"
    );
    assert!(plain.get("object_ref").is_none());
    let back: JournalEntry = serde_json::from_value(json).unwrap();
    assert_eq!(back.operation_id.as_deref(), Some("op-1"));
}

#[test]
fn identifying_values_in_the_new_fields_are_redacted() {
    let entry = JournalEntry::new(OCFL_COMMIT, Outcome::Succeeded)
        .operation_id("Grandma's birthday")
        .object_ref("photos/grandma.jpg@v2")
        .actor("someone@example.org")
        .redacted();
    assert_eq!(entry.operation_id.as_deref(), Some(REDACTED));
    assert_eq!(entry.object_ref.as_deref(), Some(REDACTED));
    assert_eq!(
        entry.actor, REDACTED,
        "'@' stays a redacted character elsewhere"
    );
    for bad in ["obj@", "@v1", "obj@v0", "obj@v1@v2", "a b@v1"] {
        let entry = JournalEntry::new(OCFL_COMMIT, Outcome::Succeeded)
            .object_ref(bad)
            .redacted();
        assert_eq!(entry.object_ref.as_deref(), Some(REDACTED), "{bad}");
    }
}

#[test]
fn a_commit_is_journaled_as_ocfl_commit_with_operation_and_object_version() {
    let facts = CommitFacts {
        operation_id: &op("op-1"),
        version: &version("v3"),
        took_over: false,
    };
    let entries = ocfl_commit_entries(&facts);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.operation, OCFL_COMMIT);
    assert_eq!(entry.outcome, Outcome::Succeeded);
    assert_eq!(entry.level, Level::Info);
    assert_eq!(entry.operation_id.as_deref(), Some("op-1"));
    assert_eq!(
        entry.object_ref.as_deref(),
        Some(format!("{OBJECT}@v3").as_str())
    );
    assert_eq!(entry.target_ids, [OBJECT]);
}

#[test]
fn a_lease_takeover_is_journaled_naming_the_object_and_version() {
    let facts = CommitFacts {
        operation_id: &op("op-2"),
        version: &version("v4"),
        took_over: true,
    };
    let entries = ocfl_commit_entries(&facts);
    assert_eq!(entries.len(), 2);
    let takeover = &entries[1];
    assert_eq!(takeover.operation, LEASE_TAKEOVER);
    assert_eq!(takeover.level, Level::Warn);
    assert_eq!(takeover.operation_id.as_deref(), Some("op-2"));
    assert_eq!(
        takeover.object_ref.as_deref(),
        Some(format!("{OBJECT}@v4").as_str())
    );
}

#[test]
fn both_fields_are_stored_and_published_by_the_writer() {
    let store = Arc::new(MemoryJournal::new("host-a"));
    let events = EventBus::new();
    let mut rx = events.subscribe();
    let writer = JournalWriter::start(
        store.clone(),
        WriterConfig { capacity: 4 },
        events,
        Arc::new(Metrics::default()),
    );
    let facts = CommitFacts {
        operation_id: &op("op-3"),
        version: &version("v1"),
        took_over: false,
    };
    for entry in ocfl_commit_entries(&facts) {
        writer.sender().send(entry);
    }
    writer.sender().flush();
    let stored = &store.query(&JournalQuery::default()).unwrap().entries[0];
    assert_eq!(stored.operation_id.as_deref(), Some("op-3"));
    let event = rx.try_recv().expect("journal.entry event");
    assert_eq!(event.data["object_ref"], format!("{OBJECT}@v1"));
    writer.shutdown();
}

#[test]
fn the_trace_layer_keeps_the_operation_id_and_object_reference() {
    let store = Arc::new(MemoryJournal::new("host-a"));
    let writer = JournalWriter::start(
        store.clone(),
        WriterConfig { capacity: 4 },
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let layer = JournalLayer::new();
    layer.attach(writer.sender());
    let subscriber = tracing_subscriber::registry().with(layer);
    let object_ref = format!("{OBJECT}@v2");
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(
            operation = "ocfl.commit",
            operation_id = "op-4",
            object_ref = %object_ref,
            "commit took over a lease"
        );
    });
    writer.sender().flush();
    let entry = &store.query(&JournalQuery::default()).unwrap().entries[0];
    assert_eq!(entry.operation_id.as_deref(), Some("op-4"));
    assert_eq!(entry.object_ref.as_deref(), Some(object_ref.as_str()));
    writer.shutdown();
}
