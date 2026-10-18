//! The API event channel (API-S-003, [ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)).
//!
//! One in-process bus carries every push event. Hosts forward it to their UIs: a WebSocket at
//! `GET /api/v1/events` on `filesquid-server`, Tauri events named `filesquid-event` on
//! `filesquid-desktop`. This module holds no transport code.
//!
//! Events are **hints**, not state. A client that misses events, because it was slow, an event
//! was too large to carry, or it disconnected, rereads durable state through the request
//! routes. A client-facing [`EventSubscription`] therefore turns every gap into an explicit
//! [`topic::RESYNC`] event instead of dropping events silently.
//!
//! Memory is bounded three ways: each subscriber lags at most [`EVENT_BUFFER`] events, each
//! event encodes to at most [`MAX_EVENT_BYTES`], and at most [`MAX_SUBSCRIBERS`] client
//! subscriptions are open at once.

use crate::error::{ApiError, ErrorCode};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

/// Events buffered for each subscriber. A subscriber that falls further behind receives one
/// [`topic::RESYNC`] hint and then the newest events. A power of two, so the broadcast channel
/// holds exactly this many.
pub const EVENT_BUFFER: usize = 256;

/// Largest JSON encoding of one [`ApiEvent`], in bytes. A larger event is replaced by a
/// [`topic::RESYNC`] hint. With [`EVENT_BUFFER`] this bounds the buffer to 256 events of at
/// most 64 KiB encoded each, however slow a subscriber is.
pub const MAX_EVENT_BYTES: usize = 64 * 1024;

/// Most client subscriptions open at once on one bus (for example WebSocket sessions).
pub const MAX_SUBSCRIBERS: usize = 64;

/// The documented event topics. The contract's `EventTopic` enum lists exactly these.
pub mod topic {
    /// A process or job changed state or progress; payload `ProcessView`.
    pub const PROCESS_UPDATE: &str = "process.update";
    /// An activity journal entry was written; payload `JournalEntry`.
    pub const JOURNAL_ENTRY: &str = "journal.entry";
    /// The host's health status changed; payload `HealthReport`.
    pub const HEALTH_CHANGE: &str = "health.change";
    /// The subscriber missed events and MUST reread the state it shows; payload `EventResync`.
    pub const RESYNC: &str = "events.resync";
    /// A field of the host descriptor changed; payload the whole new `HostDescriptor`.
    pub const HOST_CHANGE: &str = "host.change";
    /// Every documented topic, in contract order.
    pub const ALL: [&str; 5] = [
        PROCESS_UPDATE,
        JOURNAL_ENTRY,
        HEALTH_CHANGE,
        RESYNC,
        HOST_CHANGE,
    ];
}

/// One push event: a topic and that topic's payload.
///
/// Publishers MUST use a topic from [`topic::ALL`], the only topics the contract documents;
/// every publisher in this crate does, and the contract drift test checks their payloads.
/// [`EventBus::publish`] does not reject other topics, so hosts' tests can send markers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiEvent {
    /// Dotted event name.
    pub topic: String,
    /// Event payload; its shape is defined per topic in the API contract.
    pub data: serde_json::Value,
}

/// Payload of a [`topic::RESYNC`] hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventResync {
    /// How many events this subscriber did not receive (at least 1).
    pub missed: u64,
}

impl ApiEvent {
    /// Build an event from a topic and a JSON payload.
    pub fn new(topic: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            topic: topic.into(),
            data,
        }
    }

    /// A hint that the subscriber missed `missed` events and must reread durable state.
    pub fn resync(missed: u64) -> Self {
        Self::new(topic::RESYNC, serde_json::json!({ "missed": missed }))
    }
}

/// Broadcast bus for [`ApiEvent`]s. Cloning shares the same bus.
///
/// The bus is process-wide and unscoped: every subscriber (each WebSocket session on
/// `filesquid-server`, each desktop webview) receives every event. Publishers MUST NOT emit
/// account-, permission- or collection-specific payloads until events are filtered per
/// authorized session (API-F1.7); no host has accounts yet.
#[derive(Clone, Debug)]
pub struct EventBus {
    sender: broadcast::Sender<ApiEvent>,
    open: Arc<AtomicUsize>,
    oversized: Arc<AtomicU64>,
}

impl EventBus {
    /// Create an empty bus.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_BUFFER);
        Self {
            sender,
            open: Arc::new(AtomicUsize::new(0)),
            oversized: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Publish an event to every current subscriber; never blocks.
    ///
    /// An event whose JSON encoding exceeds [`MAX_EVENT_BYTES`] is replaced by a
    /// [`topic::RESYNC`] hint and counted in [`oversized_events`](Self::oversized_events).
    /// Returns the number of subscribers that received it. Zero subscribers is not an error.
    pub fn publish(&self, event: ApiEvent) -> usize {
        let event = if fits(&event) {
            event
        } else {
            self.oversized.fetch_add(1, Ordering::Relaxed);
            // DEBUG only: the journal writer thread publishes here, and a WARN or ERROR line
            // would be journaled, so it could wait on its own full queue.
            tracing::debug!(topic = %event.topic, "event over MAX_EVENT_BYTES replaced by resync");
            ApiEvent::resync(1)
        };
        self.sender.send(event).unwrap_or(0)
    }

    /// Open a client subscription (one per WebSocket session or webview channel).
    ///
    /// # Errors
    /// A retryable `unavailable` [`ApiError`] when [`MAX_SUBSCRIBERS`] are already open.
    pub fn subscription(&self) -> Result<EventSubscription, ApiError> {
        let slot = SubscriberSlot::claim(&self.open).ok_or_else(|| {
            ApiError::new(
                ErrorCode::Unavailable,
                "Too many event subscribers are connected; try again later.",
            )
        })?;
        Ok(EventSubscription {
            receiver: self.sender.subscribe(),
            _slot: slot,
        })
    }

    /// Raw receiver for a trusted in-process consumer. It is not counted against
    /// [`MAX_SUBSCRIBERS`] and reports gaps as [`RecvError::Lagged`]; anything that serves a
    /// client MUST use [`subscription`](Self::subscription) instead.
    pub fn subscribe(&self) -> broadcast::Receiver<ApiEvent> {
        self.sender.subscribe()
    }

    /// Number of live receivers, raw and client subscriptions together.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Number of open client subscriptions (at most [`MAX_SUBSCRIBERS`]).
    pub fn open_subscriptions(&self) -> usize {
        self.open.load(Ordering::SeqCst)
    }

    /// Number of events replaced by a resync hint because they exceeded [`MAX_EVENT_BYTES`].
    pub fn oversized_events(&self) -> u64 {
        self.oversized.load(Ordering::Relaxed)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// True when `event` encodes to at most [`MAX_EVENT_BYTES`]. Counting stops at the cap, so an
/// oversized payload is never encoded in full. An event that cannot be encoded does not fit.
fn fits(event: &ApiEvent) -> bool {
    let mut counter = ByteCounter { bytes: 0 };
    serde_json::to_writer(&mut counter, event).is_ok()
}

/// A writer that counts bytes and fails once more than [`MAX_EVENT_BYTES`] are written.
struct ByteCounter {
    bytes: usize,
}

impl std::io::Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes += buf.len();
        if self.bytes > MAX_EVENT_BYTES {
            return Err(std::io::Error::other("event exceeds MAX_EVENT_BYTES"));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// One counted place under [`MAX_SUBSCRIBERS`]; released when dropped.
#[derive(Debug)]
struct SubscriberSlot {
    open: Arc<AtomicUsize>,
}

impl SubscriberSlot {
    /// Take a slot if fewer than [`MAX_SUBSCRIBERS`] are taken. Check and increment are one
    /// atomic step, so concurrent callers never exceed the cap.
    fn claim(open: &Arc<AtomicUsize>) -> Option<Self> {
        open.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
            (n < MAX_SUBSCRIBERS).then_some(n + 1)
        })
        .ok()?;
        Some(Self { open: open.clone() })
    }
}

impl Drop for SubscriberSlot {
    fn drop(&mut self) {
        self.open.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A client's view of the bus. Dropping it (for example when the client disconnects)
/// releases its receiver and its [`MAX_SUBSCRIBERS`] slot.
#[derive(Debug)]
pub struct EventSubscription {
    receiver: broadcast::Receiver<ApiEvent>,
    _slot: SubscriberSlot,
}

impl EventSubscription {
    /// The next event, or `None` once the bus is gone.
    ///
    /// If the subscriber fell more than [`EVENT_BUFFER`] events behind, this returns one
    /// [`topic::RESYNC`] hint with the number missed, then continues with the oldest event
    /// still buffered. Cancel-safe: it can be raced in `tokio::select!` against a socket read.
    pub async fn next(&mut self) -> Option<ApiEvent> {
        match self.receiver.recv().await {
            Ok(event) => Some(event),
            Err(RecvError::Lagged(missed)) => Some(ApiEvent::resync(missed)),
            Err(RecvError::Closed) => None,
        }
    }
}
