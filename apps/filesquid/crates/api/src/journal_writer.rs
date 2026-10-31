//! The journal writer: a dedicated thread that drains a bounded queue into the
//! [`crate::journal::JournalStore`] in batches.
//!
//! Producers **wait** when the queue is full (backpressure), so entries are never dropped
//! (FileSquid T-055). If the store keeps failing after bounded retries, entries are written
//! to stdout as JSON with `"journal_write_failed": true` so they are still not lost, and the
//! failure is counted.

use crate::events::topic;
use crate::journal::{
    JournalEntry, JournalError, JournalPage, JournalQuery, JournalStore, Level, RetentionPolicy,
};
use crate::metrics::Metrics;
use crate::{ApiEvent, EventBus};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Entries written per store transaction at most.
const MAX_BATCH: usize = 256;
/// Store write attempts before falling back to stdout.
const MAX_ATTEMPTS: u32 = 5;

/// Writer configuration.
#[derive(Clone, Copy, Debug)]
pub struct WriterConfig {
    /// Queue capacity; producers wait when it is full.
    pub capacity: usize,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self { capacity: 1_024 }
    }
}

/// Messages to the writer thread.
enum Msg {
    Entry(Box<JournalEntry>),
    Flush(SyncSender<()>),
    Shutdown,
}

/// Cheap, cloneable handle for sending entries to the journal.
#[derive(Clone)]
pub struct JournalSender {
    tx: SyncSender<Msg>,
    depth: Arc<AtomicU64>,
    capacity: usize,
    metrics: Arc<Metrics>,
}

impl JournalSender {
    /// Queue an entry. Blocks while the queue is full; never drops the entry.
    pub fn send(&self, entry: JournalEntry) {
        let entry = entry.redacted();
        let depth = self.depth.fetch_add(1, Ordering::SeqCst) + 1;
        self.metrics.set_queue_depth(depth);
        if let Err(err) = self.tx.send(Msg::Entry(Box::new(entry))) {
            self.depth.fetch_sub(1, Ordering::SeqCst);
            if let Msg::Entry(entry) = err.0 {
                report_rejected(
                    &mut std::io::stdout().lock(),
                    "journal writer stopped",
                    &[*entry],
                );
            }
        }
    }

    /// Wait until every entry queued before this call has been written.
    pub fn flush(&self) {
        let (ack_tx, ack_rx) = sync_channel(1);
        if self.tx.send(Msg::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv();
        }
    }

    /// Queue fill ratio in `0.0..=1.0`.
    pub fn queue_fill(&self) -> f64 {
        // Blocked producers count toward depth before the channel accepts them, so clamp.
        (self.depth.load(Ordering::SeqCst) as f64 / self.capacity.max(1) as f64).min(1.0)
    }
}

/// Owner of the writer thread.
pub struct JournalWriter {
    sender: JournalSender,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl JournalWriter {
    /// Start the writer thread for `store`. Written entries are published on `events` as
    /// `journal.entry` and counted in `metrics`.
    pub fn start(
        store: Arc<dyn JournalStore>,
        config: WriterConfig,
        events: EventBus,
        metrics: Arc<Metrics>,
    ) -> Self {
        let (tx, rx) = sync_channel(config.capacity.max(1));
        let depth = Arc::new(AtomicU64::new(0));
        let sender = JournalSender {
            tx,
            depth: depth.clone(),
            capacity: config.capacity.max(1),
            metrics: metrics.clone(),
        };
        let handle = std::thread::Builder::new()
            .name("filesquid-journal".into())
            .spawn(move || run(rx, store, events, metrics, depth))
            .expect("spawn journal writer thread");
        Self {
            sender,
            handle: Mutex::new(Some(handle)),
        }
    }

    /// A sender for this writer.
    pub fn sender(&self) -> JournalSender {
        self.sender.clone()
    }

    /// Write everything queued, then stop the thread. Later sends go to the stdout fallback.
    pub fn shutdown(&self) {
        let _ = self.sender.tx.send(Msg::Shutdown);
        if let Some(handle) = self.handle.lock().ok().and_then(|mut h| h.take()) {
            let _ = handle.join();
        }
    }
}

/// Writer loop: block for one message, drain more without blocking, write, acknowledge.
fn run(
    rx: Receiver<Msg>,
    store: Arc<dyn JournalStore>,
    events: EventBus,
    metrics: Arc<Metrics>,
    depth: Arc<AtomicU64>,
) {
    let mut batch = Vec::with_capacity(MAX_BATCH);
    let mut acks = Vec::new();
    while let Ok(first) = rx.recv() {
        let mut stop = collect(first, &mut batch, &mut acks);
        while !stop && batch.len() < MAX_BATCH {
            match rx.try_recv() {
                Ok(msg) => stop = collect(msg, &mut batch, &mut acks),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        write_batch(store.as_ref(), &mut batch, &events, &metrics, &depth);
        acks.drain(..).for_each(|ack: SyncSender<()>| {
            let _ = ack.send(());
        });
        if stop {
            drain_remaining(&rx, store.as_ref(), &events, &metrics, &depth);
            return;
        }
    }
}

/// Add one message to the batch; returns true for shutdown.
fn collect(msg: Msg, batch: &mut Vec<JournalEntry>, acks: &mut Vec<SyncSender<()>>) -> bool {
    match msg {
        Msg::Entry(e) => batch.push(*e),
        Msg::Flush(ack) => acks.push(ack),
        Msg::Shutdown => return true,
    }
    false
}

/// Write whatever is still queued at shutdown (bounded by the queue capacity), then
/// acknowledge flushes, so a flush never returns before the entries queued ahead of it.
fn drain_remaining(
    rx: &Receiver<Msg>,
    store: &dyn JournalStore,
    events: &EventBus,
    metrics: &Metrics,
    depth: &AtomicU64,
) {
    let mut batch = Vec::new();
    let mut acks = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        collect(msg, &mut batch, &mut acks);
    }
    write_batch(store, &mut batch, events, metrics, depth);
    acks.drain(..).for_each(|ack| {
        let _ = ack.send(());
    });
}

/// Store a batch with bounded retries; publish and count the entries stored now. An entry
/// replayed under an ID the store already holds (DE-S-011) is neither announced nor counted
/// again.
fn write_batch(
    store: &dyn JournalStore,
    batch: &mut Vec<JournalEntry>,
    events: &EventBus,
    metrics: &Metrics,
    depth: &AtomicU64,
) {
    if batch.is_empty() {
        return;
    }
    let n = batch.len() as u64;
    let mut result = store.append_new(batch);
    for attempt in 1..MAX_ATTEMPTS {
        if result.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50 * u64::from(attempt)));
        result = store.append_new(batch);
    }
    match result {
        Ok(stored) => announce_new(batch, &stored, store.host(), events, metrics),
        Err(err) => {
            metrics.journal_write_failed(n);
            report_rejected(&mut std::io::stdout().lock(), &err.to_string(), batch);
        }
    }
    let left = depth.fetch_sub(n, Ordering::SeqCst).saturating_sub(n);
    metrics.set_queue_depth(left);
    batch.clear();
}

/// Announce entries a store accepted under `ids`: count them, count their
/// warnings and errors, and publish each, with its id and host, as a
/// `journal.entry` event. The writer and [`AnnouncingJournal`] share it, so
/// every entry that reaches the store is announced the same way.
fn announce(
    entries: &mut [JournalEntry],
    ids: &[i64],
    host: &str,
    events: &EventBus,
    metrics: &Metrics,
) {
    metrics.journal_entry_written(entries.len() as u64);
    for (entry, id) in entries.iter_mut().zip(ids) {
        entry.id = Some(*id);
        if entry.host.is_empty() {
            entry.host = host.to_string();
        }
        match entry.level {
            Level::Warn => metrics.warning(),
            Level::Error => metrics.error(),
            Level::Info => {}
        }
        if let Ok(data) = serde_json::to_value(&*entry) {
            events.publish(ApiEvent::new(topic::JOURNAL_ENTRY, data));
        }
    }
}

/// Announce the entries of `entries` that `stored` reports stored now (see [`announce`]).
fn announce_new(
    entries: &mut [JournalEntry],
    stored: &[(i64, bool)],
    host: &str,
    events: &EventBus,
    metrics: &Metrics,
) {
    let (mut fresh, ids): (Vec<JournalEntry>, Vec<i64>) = entries
        .iter()
        .zip(stored)
        .filter(|(_, (_, new))| *new)
        .map(|(entry, (id, _))| (entry.clone(), *id))
        .unzip();
    announce(&mut fresh, &ids, host, events, metrics);
}

/// A [`JournalStore`] that announces what it stores: entries appended to
/// it directly, outside the writer's queue (such as the catalog outbox a
/// host forwards after its own transaction, DE-0.10), raise the same
/// `journal.entry` events and journal counters as the writer's entries.
/// Every other call goes to the inner store unchanged.
pub struct AnnouncingJournal {
    inner: Arc<dyn JournalStore>,
    events: EventBus,
    metrics: Arc<Metrics>,
}

impl AnnouncingJournal {
    /// Announce appends to `inner` on `events` and in `metrics`.
    pub fn new(inner: Arc<dyn JournalStore>, events: EventBus, metrics: Arc<Metrics>) -> Self {
        Self {
            inner,
            events,
            metrics,
        }
    }
}

impl JournalStore for AnnouncingJournal {
    fn host(&self) -> &str {
        self.inner.host()
    }

    /// Append through the inner store, then announce the entries it stored
    /// now, redacted as the store holds them: a replayed entry the store
    /// already held is neither announced nor counted again. A refused append
    /// announces nothing.
    fn append(&self, entries: &[JournalEntry]) -> Result<Vec<i64>, JournalError> {
        let stored = self.append_new(entries)?;
        Ok(stored.into_iter().map(|(id, _)| id).collect())
    }

    fn append_new(&self, entries: &[JournalEntry]) -> Result<Vec<(i64, bool)>, JournalError> {
        let stored = self.inner.append_new(entries)?;
        let (mut fresh, ids): (Vec<JournalEntry>, Vec<i64>) = entries
            .iter()
            .zip(&stored)
            .filter(|(_, (_, new))| *new)
            .map(|(entry, (id, _))| (entry.clone().redacted(), *id))
            .unzip();
        announce(
            &mut fresh,
            &ids,
            self.inner.host(),
            &self.events,
            &self.metrics,
        );
        Ok(stored)
    }

    fn allocate_ids(&self, count: usize) -> Result<Vec<i64>, JournalError> {
        self.inner.allocate_ids(count)
    }

    fn query(&self, query: &JournalQuery) -> Result<JournalPage, JournalError> {
        self.inner.query(query)
    }

    fn count(&self, query: &JournalQuery) -> Result<u64, JournalError> {
        self.inner.count(query)
    }

    fn apply_retention(&self, policy: RetentionPolicy, now_ms: i64) -> Result<u64, JournalError> {
        self.inner.apply_retention(policy, now_ms)
    }

    fn ping(&self) -> Result<(), JournalError> {
        self.inner.ping()
    }
}

/// Report a write the journal store rejected on `out` (stdout): one `journal write failed`
/// error line, then each entry marked `"journal_write_failed": true`, so none is lost. A
/// failure to write is reported on stderr, since stdout itself failed. I/O.
fn report_rejected(out: &mut dyn Write, error: &str, entries: &[JournalEntry]) {
    if let Err(err) = write_rejected(out, error, entries) {
        eprintln!("journal fallback line not written: {err}");
    }
}

/// Write the lines of [`report_rejected`], one JSON object per line, each flushed so it is
/// not held in a buffer when stdout is a pipe. An entry that cannot be serialized is
/// reported on stderr and skipped. I/O.
fn write_rejected(
    out: &mut dyn Write,
    error: &str,
    entries: &[JournalEntry],
) -> std::io::Result<()> {
    let head =
        serde_json::json!({ "level": "ERROR", "message": "journal write failed", "error": error });
    writeln!(out, "{head}")?;
    out.flush()?;
    for entry in entries {
        match serde_json::to_value(entry) {
            Ok(mut value) => {
                value["journal_write_failed"] = serde_json::Value::Bool(true);
                writeln!(out, "{value}")?;
                out.flush()?;
            }
            Err(err) => eprintln!("journal entry not printable: {err}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::Outcome;

    #[test]
    fn a_rejected_write_is_one_json_line_per_record_on_stdout() {
        let entries = [
            JournalEntry::new("library.create", Outcome::Succeeded),
            JournalEntry::new("host.started", Outcome::Succeeded),
        ];
        let mut out = Vec::new();
        report_rejected(&mut out, "store down", &entries);
        let text = String::from_utf8(out).expect("UTF-8");
        assert!(text.ends_with('\n'), "{text}");
        let lines: Vec<serde_json::Value> = text
            .split_terminator('\n')
            .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
            .collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["message"], "journal write failed");
        assert_eq!(lines[0]["error"], "store down");
        for (line, entry) in lines[1..].iter().zip(&entries) {
            assert_eq!(line["journal_write_failed"], true);
            assert_eq!(line["operation"], entry.operation.as_str());
        }
    }

    /// DE-S-011: an entry replayed under an ID the store already holds (a commit's
    /// `ocfl.commit` journaled again after a crash) is stored once, and announced and counted
    /// once, not once per replay.
    #[test]
    fn a_replayed_entry_the_store_already_holds_is_announced_once() {
        let store = Arc::new(crate::journal::MemoryJournal::new("desktop"));
        let events = EventBus::new();
        let mut received = events.subscribe();
        let metrics = Arc::new(Metrics::default());
        let writer = JournalWriter::start(
            store.clone(),
            WriterConfig { capacity: 8 },
            events,
            metrics.clone(),
        );
        let id = store.allocate_ids(1).expect("an id")[0];
        let mut entry = JournalEntry::new("ocfl.commit", Outcome::Succeeded);
        entry.id = Some(id);
        for _ in 0..3 {
            writer.sender().send(entry.clone());
            writer.sender().flush();
        }
        writer.shutdown();
        let held = store
            .query(&crate::journal::JournalQuery::default())
            .expect("query");
        assert_eq!(held.entries.len(), 1, "{:?}", held.entries);
        assert_eq!(
            metrics.snapshot()["filesquid_journal_entries_total"],
            1,
            "counted once"
        );
        let mut announced = 0;
        while let Ok(event) = received.try_recv() {
            announced += usize::from(event.topic == crate::events::topic::JOURNAL_ENTRY);
        }
        assert_eq!(announced, 1, "announced once");
    }
}
