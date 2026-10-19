//! The desktop's stdout event stream (Task DE-0.6; DE-S-003, NFR-009,
//! [ADR 014](../../../../../resources/architecture/014-stdout-observability.md)).
//!
//! Every `tracing` event the host's stdout layer keeps is one JSON object on one line of
//! stdout: a `timestamp` in RFC 3339 UTC with milliseconds (the activity journal's own time
//! format), its `level`, `target` and fields. Rust's stdout flushes each completed line, so
//! the stream is unbuffered event by event. The host writes no log file and links no Tauri
//! log plugin (the build policy refuses it, DE-S-002), and runs no telemetry server.

use filesquid_api::journal::{format_rfc3339_ms, now_ms, parse_rfc3339_ms};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;

/// The stdout timestamp: RFC 3339 UTC with millisecond precision, for example
/// `2026-10-19T00:00:00.123Z`.
#[derive(Clone, Copy, Debug, Default)]
pub struct UtcMillis;

impl FormatTime for UtcMillis {
    /// Write the current time (reads the system clock).
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        w.write_str(&rfc3339_utc_now())
    }
}

/// The current time as RFC 3339 UTC with milliseconds (reads the system clock).
pub fn rfc3339_utc_now() -> String {
    format_rfc3339_ms(now_ms())
}

/// Whether `text` is an RFC 3339 UTC time with exactly three fractional digits and a `Z`
/// offset, the only form the stdout stream writes. Pure.
pub fn is_rfc3339_utc_ms(text: &str) -> bool {
    let shape = text.len() == 24
        && text.ends_with('Z')
        && text.as_bytes().get(10) == Some(&b'T')
        && text.as_bytes().get(19) == Some(&b'.');
    shape && parse_rfc3339_ms(text).is_some()
}
