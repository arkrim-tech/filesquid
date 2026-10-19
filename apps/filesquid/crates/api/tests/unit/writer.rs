//! Unit tests for the journal writer: bounded, backpressured, never dropping (K3, K5).

use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::metrics::Metrics;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::EventBus;
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;

#[test]
fn a_burst_of_ten_times_capacity_loses_nothing() {
    let store = Arc::new(MemoryJournal::new("host-a"));
    let writer = JournalWriter::start(
        store.clone(),
        WriterConfig { capacity: 8 },
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let threads: Vec<_> = (0..4)
        .map(|t| {
            let sender = writer.sender();
            std::thread::spawn(move || {
                for i in 0..20 {
                    sender.send(
                        JournalEntry::new("process.progress", Outcome::Info)
                            .process(format!("p{t}-{i}")),
                    );
                }
            })
        })
        .collect();
    threads.into_iter().for_each(|t| t.join().unwrap());
    writer.sender().flush();
    assert_eq!(store.count(&JournalQuery::default()).unwrap(), 80);
    writer.shutdown();
}

#[test]
fn written_entries_are_published_on_the_event_bus_and_counted() {
    let store = Arc::new(MemoryJournal::new("host-a"));
    let events = EventBus::new();
    let mut rx = events.subscribe();
    let metrics = Arc::new(Metrics::default());
    let writer = JournalWriter::start(store, WriterConfig { capacity: 4 }, events, metrics.clone());
    writer
        .sender()
        .send(JournalEntry::new("import", Outcome::Succeeded));
    writer.sender().flush();
    let event = rx.try_recv().expect("journal.entry published");
    assert_eq!(event.topic, "journal.entry");
    assert_eq!(event.data["operation"], "import");
    assert_eq!(metrics.snapshot()["filesquid_journal_entries_total"], 1);
    writer.shutdown();
}

#[test]
fn trace_layer_journals_warnings_without_free_text_or_unlisted_fields() {
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
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(
            operation = "ignored.info",
            "info events are not journaled by the layer"
        );
        tracing::warn!(
            operation = "ocfl.commit",
            outcome = "failed",
            error_code = "EIO",
            target_id = "obj-9",
            title = "Grandma's birthday",
            gps = "52.37,4.89",
            "could not commit Grandma's birthday"
        );
    });
    writer.sender().flush();
    let entries = store.query(&JournalQuery::default()).unwrap().entries;
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.operation, "ocfl.commit");
    assert_eq!(e.outcome, Outcome::Failed);
    assert_eq!(e.error_code.as_deref(), Some("EIO"));
    assert_eq!(e.target_ids, ["obj-9"]);
    let json = serde_json::to_string(e).unwrap();
    for secret in ["Grandma", "52.37", "birthday"] {
        assert!(
            !json.contains(secret),
            "redaction failed for {secret}: {json}"
        );
    }
    writer.shutdown();
}

#[test]
fn trace_layer_keeps_display_formatted_whitelisted_fields() {
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
    let operation = String::from("ocfl.commit");
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(operation = %operation, target_id = %"obj-3", "commit slow");
    });
    writer.sender().flush();
    let entries = store.query(&JournalQuery::default()).unwrap().entries;
    assert_eq!(entries[0].operation, "ocfl.commit");
    assert_eq!(entries[0].target_ids, ["obj-3"]);
    writer.shutdown();
}

#[test]
fn live_journal_events_carry_the_redacted_entry() {
    let events = EventBus::new();
    let mut rx = events.subscribe();
    let writer = JournalWriter::start(
        Arc::new(MemoryJournal::new("host-a")),
        WriterConfig { capacity: 4 },
        events,
        Arc::new(Metrics::default()),
    );
    writer
        .sender()
        .send(JournalEntry::new("import", Outcome::Succeeded).target("Holiday photo.jpg"));
    writer.sender().flush();
    let event = rx.try_recv().expect("journal.entry event");
    assert_eq!(event.data["target_ids"], serde_json::json!(["redacted"]));
    writer.shutdown();
}

/// Entries a host appends straight to the store (the catalog outbox,
/// DE-0.10) are announced like the writer's own: one `journal.entry` event
/// each, the redacted entry with its id, and the journal counters.
#[test]
fn an_announcing_store_publishes_and_counts_what_the_store_accepted() {
    use filesquid_api::journal_writer::AnnouncingJournal;
    let store = Arc::new(MemoryJournal::new("host-a"));
    let events = EventBus::new();
    let mut rx = events.subscribe();
    let metrics = Arc::new(Metrics::default());
    let announcing = AnnouncingJournal::new(store.clone(), events, metrics.clone());
    let ids = announcing
        .append(&[
            JournalEntry::new("library.file.hide", Outcome::Succeeded).target("Holiday photo.jpg"),
            JournalEntry::new("library.file.hide", Outcome::Failed),
        ])
        .expect("append");
    assert_eq!(ids.len(), 2);
    let first = rx.try_recv().expect("first event");
    assert_eq!(first.topic, "journal.entry");
    assert_eq!(first.data["id"], ids[0]);
    assert_eq!(first.data["host"], "host-a");
    assert_eq!(first.data["target_ids"], serde_json::json!(["redacted"]));
    rx.try_recv().expect("second event");
    assert!(rx.try_recv().is_err(), "one event per entry");
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot["filesquid_journal_entries_total"], 2);
    assert_eq!(snapshot["filesquid_errors_total"], 1);
    assert_eq!(announcing.host(), "host-a");
    assert_eq!(store.count(&JournalQuery::default()).unwrap(), 2);
}
