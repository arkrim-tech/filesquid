//! Test support: capture what the activity journal records while a closure runs, so tests can
//! prove that each desktop event type is journaled with whitelisted, redacted fields only.

use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::metrics::Metrics;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::EventBus;
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;

/// Run `body` on this thread with the journal layer attached to an in-memory journal, and
/// return every entry it journaled. Async code inside `body` must run on a current-thread
/// runtime built inside it, so its events reach this thread's subscriber.
pub fn journaled(body: impl FnOnce()) -> Vec<JournalEntry> {
    let store = Arc::new(MemoryJournal::new("desktop"));
    let writer = JournalWriter::start(
        store.clone(),
        WriterConfig { capacity: 16 },
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let layer = JournalLayer::new();
    layer.attach(writer.sender());
    tracing::subscriber::with_default(tracing_subscriber::registry().with(layer), body);
    writer.sender().flush();
    let entries = store
        .query(&JournalQuery::default())
        .expect("query journal")
        .entries;
    writer.shutdown();
    entries
}

/// Block on `future` with a current-thread runtime, so its tracing events stay on this thread.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime")
        .block_on(future)
}
