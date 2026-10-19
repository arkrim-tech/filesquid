//! The desktop's stdout event stream (Task DE-0.6; DE-S-003, NFR-009,
//! [ADR 014](../../../../../resources/architecture/014-stdout-observability.md)).
//!
//! Every `tracing` event the host's stdout layer keeps is one JSON object on one line of
//! stdout: a `timestamp` in RFC 3339 UTC with milliseconds (the activity journal's own time
//! format), its `level`, `target` and fields. Each record is flushed when it is written
//! ([`Flushing`]), so the stream is unbuffered event by event even through a pipe. Records a
//! host writes before its journal opens (`journal_unavailable`, see
//! [`crate::panic_journal`]) use the journal entry's shape instead. The host writes no log file and links no Tauri
//! log plugin (the build policy refuses it, DE-S-002), and runs no telemetry server.

use crate::panic_journal;
use filesquid_api::journal::{format_rfc3339_ms, now_ms, parse_rfc3339_ms, JournalEntry, Outcome};
use std::io::Write;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::MakeWriter;

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

/// Journal operation of a host logging stack that could not be installed.
pub const LOGGING_OPERATION: &str = "desktop.logging";

/// Error code of a logging stack that could not be installed.
const LOGGING_UNAVAILABLE: &str = "logging_unavailable";

/// A [`MakeWriter`] whose writers flush when dropped, so every record the stdout layer
/// writes is flushed when it is complete. Rust guarantees line buffering only when stdout is
/// a terminal; through a pipe (a supervisor or a log collector) a record could otherwise sit
/// in the buffer.
#[derive(Clone, Copy, Debug, Default)]
pub struct Flushing<M>(pub M);

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for Flushing<M> {
    type Writer = FlushOnDrop<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        FlushOnDrop(self.0.make_writer())
    }
}

/// A writer that flushes its inner writer when dropped. A flush error is reported on stderr,
/// since the stdout stream itself failed.
#[derive(Debug)]
pub struct FlushOnDrop<W: Write>(W);

impl<W: Write> Write for FlushOnDrop<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl<W: Write> Drop for FlushOnDrop<W> {
    /// Flush what the record wrote (I/O).
    fn drop(&mut self) {
        if let Err(err) = self.0.flush() {
            eprintln!("stdout record not flushed: {err}");
        }
    }
}

/// The host's stdout, flushed after every record (I/O when written).
pub fn stdout() -> Flushing<fn() -> std::io::Stdout> {
    Flushing(std::io::stdout)
}

/// The journal entry of a logging stack that could not be installed, recorded once the
/// journal opens (redacted: operation and error code only). Pure.
pub fn logging_failure_entry() -> JournalEntry {
    JournalEntry::new(LOGGING_OPERATION, Outcome::Failed).error_code(LOGGING_UNAVAILABLE)
}

/// Report a logging stack that could not be installed before the journal exists: its
/// `detail` to stderr, and a redacted `journal_unavailable` record (operation and error code
/// only) to `stdout`, flushed. Writes to stderr and `stdout`.
pub fn report_logging_failure(stdout: &mut dyn Write, detail: &str) {
    eprintln!("FileSquid desktop logging: {detail}");
    let line = panic_journal::unjournaled_line(LOGGING_OPERATION, LOGGING_UNAVAILABLE);
    panic_journal::write_line(stdout, &line);
}
