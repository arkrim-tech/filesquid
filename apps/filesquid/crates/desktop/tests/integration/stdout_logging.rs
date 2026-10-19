//! Task DE-0.6 (DE-S-003, NFR-009, ADR 014): the desktop's stdout event stream. Every
//! `tracing` event, and every `log` record (Tauri's own), is one JSON object per line with
//! an RFC 3339 UTC timestamp, its severity, target and fields; installing the stack twice
//! is reported, never swallowed. The stack is installed globally once, in this binary only.

use filesquid_api::trace_layer::JournalLayer;
use filesquid_desktop_lib::install_tracing;
use filesquid_desktop_lib::ipc_observer::TauriIpcObserver;
use filesquid_desktop_lib::stdout_log::{
    is_rfc3339_utc_ms, logging_failure_entry, report_logging_failure, rfc3339_utc_now, Flushing,
    LOGGING_OPERATION,
};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

/// Captured stdout of the host subscriber, counting flushes.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>, Arc<Mutex<usize>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("captured").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        *self.1.lock().expect("flushes") += 1;
        Ok(())
    }
}

impl Captured {
    /// Number of flushes so far.
    fn flushes(&self) -> usize {
        *self.1.lock().expect("flushes")
    }
}

/// Every captured line, parsed as JSON. The text must end with a newline and hold no empty
/// line, so every record is complete and framed by exactly one newline.
fn lines(captured: &Captured) -> Vec<serde_json::Value> {
    let text = String::from_utf8(captured.0.lock().expect("captured").clone()).expect("UTF-8");
    assert!(
        text.is_empty() || text.ends_with('\n'),
        "unterminated record: {text}"
    );
    text.split_terminator('\n')
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line:?}")))
        .collect()
}

#[test]
fn the_timestamp_format_is_rfc3339_utc_with_milliseconds() {
    assert!(is_rfc3339_utc_ms("2026-10-19T00:00:00.123Z"));
    assert!(is_rfc3339_utc_ms(&rfc3339_utc_now()));
    for bad in [
        "2026-10-19T00:00:00Z",
        "2026-10-19T00:00:00.123+02:00",
        "2026-10-19 00:00:00.123Z",
        "",
    ] {
        assert!(!is_rfc3339_utc_ms(bad), "{bad}");
    }
}

#[test]
fn every_event_and_log_record_is_one_json_line_and_a_second_install_is_refused() {
    let stdout = Captured::default();
    let sink = stdout.clone();
    let filter = tracing_subscriber::EnvFilter::new("info");
    install_tracing(
        JournalLayer::new(),
        TauriIpcObserver::new(),
        Flushing(move || sink.clone()),
        filter,
    )
    .expect("first install");
    tracing::warn!(target: "filesquid::test", job_id = "job-1", outcome = "failed", "a job failed");
    log::info!(target: "tauri::test", "a record from the log crate");
    let parsed = lines(&stdout);
    assert_eq!(parsed.len(), 2, "{parsed:?}");
    assert!(stdout.flushes() >= 2, "each record is flushed");
    for line in &parsed {
        let time = line["timestamp"].as_str().expect("timestamp");
        assert!(is_rfc3339_utc_ms(time), "{time}");
        assert!(line["level"].is_string(), "{line}");
        assert!(line["target"].is_string(), "{line}");
    }
    assert_eq!(parsed[0]["level"], "WARN");
    assert_eq!(parsed[0]["fields"]["job_id"], "job-1");
    assert_eq!(parsed[0]["fields"]["outcome"], "failed");
    assert_eq!(parsed[1]["level"], "INFO");
    assert_eq!(
        parsed[1]["fields"]["message"],
        "a record from the log crate"
    );
    let again = Captured::default();
    let err = install_tracing(
        JournalLayer::new(),
        TauriIpcObserver::new(),
        move || again.clone(),
        tracing_subscriber::EnvFilter::new("info"),
    )
    .expect_err("a second install is refused");
    assert!(!err.is_empty());
}

#[test]
fn a_flushing_writer_flushes_every_record_it_writes() {
    let captured = Captured::default();
    let inner = captured.clone();
    let make = Flushing(move || inner.clone());
    for n in 0..3 {
        let mut writer = make.make_writer();
        writeln!(writer, "{{\"n\":{n}}}").expect("write");
    }
    assert_eq!(captured.flushes(), 3);
    assert_eq!(lines(&captured).len(), 3);
}

#[test]
fn a_logging_stack_that_cannot_be_installed_is_reported_on_stdout_and_to_the_journal() {
    let captured = Captured::default();
    let mut out = captured.clone();
    report_logging_failure(&mut out, "tracing not installed: already set");
    let parsed = lines(&captured);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["operation"], LOGGING_OPERATION);
    assert_eq!(parsed[0]["error_code"], "logging_unavailable");
    assert_eq!(parsed[0]["journal_unavailable"], true);
    assert!(
        !parsed[0].to_string().contains("already set"),
        "detail stays on stderr"
    );
    assert!(captured.flushes() >= 1);
    let entry = logging_failure_entry();
    assert_eq!(entry.operation, LOGGING_OPERATION);
    assert_eq!(entry.outcome.as_str(), "failed");
    assert_eq!(entry.error_code.as_deref(), Some("logging_unavailable"));
}
