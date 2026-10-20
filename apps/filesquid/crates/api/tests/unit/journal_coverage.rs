//! Unit tests for the journal coverage of API-S-031: OCFL engine warnings journaled with an
//! operation and object reference through the activity `tracing` target, and the optional,
//! default-off `metrics.snapshot` entries.

use filesquid_api::journal::{JournalQuery, JournalStore, Level, MemoryJournal};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::metrics::Metrics;
use filesquid_api::monitor::{
    Monitor, MonitorConfig, StaticHostChecks, METRICS_SNAPSHOT_OPERATION,
};
use filesquid_api::trace_layer::{activity_span, JournalLayer};
use filesquid_api::EventBus;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;

/// Run `emit` under a subscriber whose journal layer writes to a fresh store; return it.
fn journaled(emit: impl FnOnce()) -> Arc<MemoryJournal> {
    let store = Arc::new(MemoryJournal::new("host-a"));
    let writer = JournalWriter::start(
        store.clone(),
        WriterConfig::default(),
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let layer = JournalLayer::new();
    layer.attach(writer.sender());
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, emit);
    writer.sender().flush();
    writer.shutdown();
    store
}

#[test]
fn an_engine_lease_warning_names_the_engine_operation_and_the_object_version() {
    let store = journaled(|| {
        let span = activity_span("op-7", None);
        let _entered = span.enter();
        tracing::warn!(
            target: "filesquid_ocfl::write::lease",
            object = "urn:uuid:5f0c3e44-2a7b-4c1d-9e8f-0123456789ab",
            version = "v3",
            previous_holder = "/Users/me/.filesquid/lease",
            "took over an expired reservation"
        );
    });
    let entries = store
        .query(&JournalQuery::default())
        .expect("query")
        .entries;
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.operation, "ocfl.engine.write.lease");
    assert_eq!(entry.level, Level::Warn);
    assert_eq!(entry.operation_id.as_deref(), Some("op-7"));
    assert_eq!(
        entry.object_ref.as_deref(),
        Some("urn:uuid:5f0c3e44-2a7b-4c1d-9e8f-0123456789ab@v3")
    );
    assert_eq!(
        entry.target_ids,
        ["urn:uuid:5f0c3e44-2a7b-4c1d-9e8f-0123456789ab"]
    );
    let text = serde_json::to_string(entry).expect("json");
    assert!(
        !text.contains("/Users"),
        "engine paths never reach the journal"
    );
}

#[test]
fn an_engine_guard_warning_inherits_the_object_reference_of_its_activity_span() {
    let store = journaled(|| {
        let span = activity_span("op-8", Some("urn:uuid:aa@v2"));
        let _entered = span.enter();
        tracing::warn!(
            target: "filesquid_ocfl::read::copy_guard",
            destination = "/Volumes/Backup/ocfl-root/photo.jpg",
            root = "/Volumes/Backup/ocfl-root",
            "refused a copy into an OCFL root"
        );
    });
    let entries = store
        .query(&JournalQuery::default())
        .expect("query")
        .entries;
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.operation, "ocfl.engine.read.copy_guard");
    assert_eq!(entry.operation_id.as_deref(), Some("op-8"));
    assert_eq!(entry.object_ref.as_deref(), Some("urn:uuid:aa@v2"));
    let text = serde_json::to_string(entry).expect("json");
    assert!(!text.contains("Volumes") && !text.contains("photo.jpg"));
    assert_eq!(entry, &entry.clone().redacted());
}

#[test]
fn an_event_field_wins_over_its_span_field() {
    let store = journaled(|| {
        let span = activity_span("op-outer", Some("urn:uuid:aa@v1"));
        let _entered = span.enter();
        tracing::warn!(
            operation = "library.check",
            operation_id = "op-inner",
            "checked"
        );
    });
    let entries = store
        .query(&JournalQuery::default())
        .expect("query")
        .entries;
    assert_eq!(entries[0].operation_id.as_deref(), Some("op-inner"));
    assert_eq!(entries[0].object_ref.as_deref(), Some("urn:uuid:aa@v1"));
}

#[test]
fn a_warning_outside_any_activity_span_carries_no_operation_id() {
    let store = journaled(|| {
        tracing::warn!(target: "filesquid_ocfl::write::commit", "could not release");
    });
    let entries = store
        .query(&JournalQuery::default())
        .expect("query")
        .entries;
    assert_eq!(entries[0].operation, "ocfl.engine.write.commit");
    assert!(entries[0].operation_id.is_none());
    assert!(entries[0].object_ref.is_none());
}

fn monitor(config: MonitorConfig) -> Monitor {
    Monitor::start(
        Arc::new(MemoryJournal::new("host-a")),
        Arc::new(StaticHostChecks::healthy()),
        config,
        EventBus::new(),
    )
}

fn snapshots(monitor: &Monitor) -> Vec<filesquid_api::journal::JournalEntry> {
    monitor.journal().flush();
    let query = JournalQuery {
        operation: Some(METRICS_SNAPSHOT_OPERATION.to_string()),
        ..JournalQuery::default()
    };
    monitor.store().query(&query).expect("query").entries
}

#[test]
fn metrics_snapshots_are_off_by_default() {
    let config = MonitorConfig::new("host-a");
    assert_eq!(config.metrics_snapshot_every_ms, None);
    let monitor = monitor(config);
    monitor.tick();
    monitor.tick();
    assert!(snapshots(&monitor).is_empty());
    monitor.shutdown();
}

#[test]
fn enabled_metrics_snapshots_are_journaled_once_per_interval_with_opaque_values() {
    let now = Arc::new(AtomicI64::new(1_000_000));
    let clock = now.clone();
    let mut config = MonitorConfig::new("host-a");
    config.clock = Arc::new(move || clock.load(Ordering::SeqCst));
    config.metrics_snapshot_every_ms = Some(60_000);
    let monitor = monitor(config);
    monitor.tick();
    monitor.tick();
    assert_eq!(snapshots(&monitor).len(), 1, "one per interval");
    now.fetch_add(60_000, Ordering::SeqCst);
    monitor.tick();
    let taken = snapshots(&monitor);
    assert_eq!(taken.len(), 2);
    let entry = &taken[0];
    assert!(entry
        .target_ids
        .iter()
        .any(|t| t.starts_with("filesquid_journal_entries_total:")));
    assert_eq!(
        entry,
        &entry.clone().redacted(),
        "metrics.snapshot is redacted"
    );
    monitor.shutdown();
}
