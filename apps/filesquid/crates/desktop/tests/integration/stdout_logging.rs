//! Task DE-0.6 (DE-S-003, NFR-009, ADR 014): the desktop's stdout event stream. Every
//! `tracing` event, and every `log` record (Tauri's own), is one JSON object per line with
//! an RFC 3339 UTC timestamp, its severity, target and fields; installing the stack twice
//! is reported, never swallowed. The stack is installed globally once, in this binary only.

use filesquid_api::trace_layer::JournalLayer;
use filesquid_desktop_lib::install_tracing;
use filesquid_desktop_lib::ipc_observer::TauriIpcObserver;
use filesquid_desktop_lib::stdout_log::{is_rfc3339_utc_ms, rfc3339_utc_now};
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Captured stdout of the host subscriber.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("captured").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Every captured line, parsed as JSON.
fn lines(captured: &Captured) -> Vec<serde_json::Value> {
    let text = String::from_utf8(captured.0.lock().expect("captured").clone()).expect("UTF-8");
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
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
        move || sink.clone(),
        filter,
    )
    .expect("first install");
    tracing::warn!(target: "filesquid::test", job_id = "job-1", outcome = "failed", "a job failed");
    log::info!(target: "tauri::test", "a record from the log crate");
    let parsed = lines(&stdout);
    assert_eq!(parsed.len(), 2, "{parsed:?}");
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
