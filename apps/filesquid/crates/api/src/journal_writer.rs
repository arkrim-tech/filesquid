//! The journal writer: a dedicated thread that drains a bounded queue into the
//! [`crate::journal::JournalStore`] in batches.
//!
//! Producers **wait** when the queue is full (backpressure), so entries are never dropped
//! (FileSquid T-055). If the store keeps failing after bounded retries, entries are written
//! to stdout as JSON with `"journal_write_failed": true` so they are still not lost, and the
//! failure is counted.

use crate::events::topic;
use crate::journal::{JournalEntry, JournalStore, Level};
use crate::metrics::Metrics;
use crate::{ApiEvent, EventBus};
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
                stdout_fallback(&[*entry]);
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

/// Store a batch with bounded retries; publish and count written entries.
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
    let mut result = store.append(batch);
    for attempt in 1..MAX_ATTEMPTS {
        if result.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50 * u64::from(attempt)));
        result = store.append(batch);
    }
    match result {
        Ok(ids) => {
            metrics.journal_entry_written(n);
            for (entry, id) in batch.iter_mut().zip(ids) {
                entry.id = Some(id);
                if entry.host.is_empty() {
                    entry.host = store.host().to_string();
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
        Err(err) => {
            metrics.journal_write_failed(n);
            print_flushed(
                &serde_json::json!({ "level": "ERROR", "message": "journal write failed", "error": err.to_string() })
                    .to_string(),
            );
            stdout_fallback(batch);
        }
    }
    let left = depth.fetch_sub(n, Ordering::SeqCst).saturating_sub(n);
    metrics.set_queue_depth(left);
    batch.clear();
}

/// Print entries that could not be stored to stdout so they are not lost.
fn stdout_fallback(entries: &[JournalEntry]) {
    for entry in entries {
        if let Ok(mut value) = serde_json::to_value(entry) {
            value["journal_write_failed"] = serde_json::Value::Bool(true);
            print_flushed(&value.to_string());
        }
    }
}

/// Write `line` to stdout and flush it, so a fallback record is not held in a buffer when
/// stdout is a pipe. A failure is reported on stderr, since stdout itself failed. I/O.
fn print_flushed(line: &str) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = writeln!(stdout, "{line}").and_then(|()| stdout.flush()) {
        eprintln!("journal fallback line not written: {err}");
    }
}
