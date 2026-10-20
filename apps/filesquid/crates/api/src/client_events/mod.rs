//! Client events: browser console warnings and errors journaled through the API
//! (API-S-030; [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)
//! Decision 7).
//!
//! `POST /api/v1/client-events` takes a batch of at most [`MAX_EVENTS`] events in a body of
//! at most [`MAX_BODY_BYTES`]. Each event is journaled as its own entry ([`WARN_OPERATION`],
//! [`ERROR_OPERATION`] or [`OVERFLOW_OPERATION`]), so the bounds only delay events and never
//! merge them. Every event carries a client event ID (UUID v4) that the journal store keeps
//! under a unique index, so a batch resent after a lost acknowledgement, or delivered again
//! on the next open, adds no entry. A credential may send at most [`RATE_LIMIT`] requests per
//! [`RATE_WINDOW_MS`]; a request that carries only one overflow event is exempt.
//!
//! - [`schema`] parses and checks a batch (pure): only the level, a component identifier,
//!   an error code and opaque IDs are read; any other field, such as a message, URL, file
//!   name or stack, is never read and never stored.
//! - [`limiter`] holds the per-credential rate bound in memory.
//! - [`route`] is the handler: bounds, rate, principal check, then one durable write.

pub mod limiter;
pub mod route;
pub mod schema;

pub use limiter::{RateDecision, RateLimiter};
pub use route::ClientEventsAck;
pub use schema::{parse_batch, Batch, BatchError, ClientEvent, ConsoleLevel};

/// Most events in one request.
pub const MAX_EVENTS: usize = 20;
/// Largest request body in bytes (16 KiB).
pub const MAX_BODY_BYTES: usize = 16 * 1024;
/// Requests one credential may send per [`RATE_WINDOW_MS`].
pub const RATE_LIMIT: usize = 60;
/// The rolling window of the rate bound, in milliseconds (one minute).
pub const RATE_WINDOW_MS: i64 = 60_000;
/// Largest overflow count; the smallest is 1.
pub const MAX_OVERFLOW_COUNT: u64 = 1_000_000;
/// Most opaque IDs on one console event.
pub const MAX_EVENT_IDS: usize = 8;
/// How far ahead of the host's clock an event time may be, in milliseconds (five minutes).
pub const MAX_CLOCK_SKEW_MS: i64 = 300_000;

/// Journal operation of a console warning.
pub const WARN_OPERATION: &str = "ui.console.warn";
/// Journal operation of a console error.
pub const ERROR_OPERATION: &str = "ui.console.error";
/// Journal operation of the page-close count of events the client could not keep.
pub const OVERFLOW_OPERATION: &str = "ui.console.overflow";
