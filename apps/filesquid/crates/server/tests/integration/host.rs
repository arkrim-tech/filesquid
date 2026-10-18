//! Integration tests: the host descriptor on `filesquid-server` (API-S-020). The operator
//! name comes from `FILESQUID_OPERATOR_NAME` (trimmed, 1 to 256 characters, no control
//! characters); an unset or blank value starts the server with a `null` operator and one
//! `server.operator_unnamed` warning on stdout and in the journal at every start; an unusable
//! value stops the server with an error that names the variable and omits the value.

use axum::body::Body;
use axum::http::{header, Request};
use filesquid_api::journal::{JournalQuery, Level};
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::ApiState;
use filesquid_native::HostMonitor;
use filesquid_server::{
    app, operator_unnamed_entry, operator_unnamed_line, start_server, start_server_writing_to,
    unicode_value, ServerConfig, OPERATOR_NAME_VARIABLE, OPERATOR_UNNAMED_OPERATION,
};
use http_body_util::BodyExt;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

/// Administrator token configured for the test server.
const TOKEN: &str = "0dd5eed50dd5eed50dd5eed50dd5eed50dd5eed50dd5eed50dd5eed50dd5eed5";

/// A fresh, empty data directory for one test.
fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-server-host-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Configuration read from [`TOKEN`], `dir` and an optional operator setting.
fn config(dir: &std::path::Path, operator: Option<&str>) -> Result<ServerConfig, String> {
    let dir = dir.to_string_lossy().to_string();
    ServerConfig::from_lookup(|key| match key {
        "FILESQUID_ADMIN_TOKEN" => Some(TOKEN.to_string()),
        "FILESQUID_DATA_DIR" => Some(dir.clone()),
        "FILESQUID_OPERATOR_NAME" => operator.map(str::to_string),
        _ => None,
    })
}

/// `GET /api/v1/host` with the administrator token: status and JSON body.
async fn read_host(state: ApiState, config: &ServerConfig) -> (u16, serde_json::Value) {
    let request = Request::get("/api/v1/host")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let response = app(config, state).oneshot(request).await.expect("answer");
    let status = response.status().as_u16();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (status, serde_json::from_slice(&body).expect("JSON"))
}

/// A `MakeWriter` that keeps everything written, standing in for stdout.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("buffer").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Captured;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Captured {
    /// Captured lines that mention `needle`.
    fn lines_with(&self, needle: &str) -> Vec<String> {
        let bytes = self.0.lock().expect("buffer").clone();
        String::from_utf8(bytes)
            .expect("UTF-8")
            .lines()
            .filter(|line| line.contains(needle))
            .map(str::to_string)
            .collect()
    }
}

/// Start the server as `main` does (JSON stdout plus the journal layer), but with the
/// stdout log filter turned off entirely (`RUST_LOG=off`), and its unfiltered startup lines
/// captured. Returns the state, the running monitor and the captured stdout.
fn start_captured(config: &ServerConfig) -> (ApiState, HostMonitor, Captured) {
    let mut stdout = Captured::default();
    let layer = JournalLayer::new();
    let filtered = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(stdout.clone())
        .with_filter(tracing_subscriber::filter::LevelFilter::OFF);
    let subscriber = tracing_subscriber::registry()
        .with(filtered)
        .with(layer.clone());
    let state = ApiState::new();
    let host = tracing::subscriber::with_default(subscriber, || {
        start_server_writing_to(config, &state, Some(&layer), &mut stdout).expect("server starts")
    });
    host.monitor().journal().flush();
    (state, host, stdout)
}

/// Journal entries of `operation` in the server's journal.
fn journaled(host: &HostMonitor, operation: &str) -> Vec<filesquid_api::journal::JournalEntry> {
    let query = JournalQuery {
        operation: Some(operation.to_string()),
        ..JournalQuery::default()
    };
    host.monitor().journal().flush();
    host.monitor()
        .store()
        .query(&query)
        .expect("journal")
        .entries
}

#[test]
fn the_operator_name_is_read_trimmed_from_the_environment() {
    let dir = data_dir("trim");
    let named = config(&dir, Some("  Garderen Family\t")).expect("valid");
    assert_eq!(
        named.operator_name.as_ref().map(|n| n.as_str()),
        Some("Garderen Family")
    );
    for blank in [None, Some(""), Some(" \t \t")] {
        assert_eq!(config(&dir, blank).expect("starts").operator_name, None);
    }
    let longest = "é".repeat(256);
    assert!(config(&dir, Some(&longest)).is_ok());
}

#[test]
fn an_unusable_operator_name_refuses_to_start_without_echoing_the_value() {
    let dir = data_dir("refuse");
    let too_long = format!("Operatorvalue{}", "z".repeat(250));
    for value in [
        too_long.as_str(),
        "Operatorvalue\u{7}bell",
        "Operatorvalue\nline",
    ] {
        let message = config(&dir, Some(value)).expect_err("refused");
        assert!(message.contains(OPERATOR_NAME_VARIABLE), "{message}");
        assert!(!message.contains("Operatorvalue"), "{message}");
    }
}

#[tokio::test]
async fn a_named_server_describes_itself_and_writes_no_warning() {
    let dir = data_dir("named");
    let config = config(&dir, Some("Garderen Family")).expect("valid");
    let (state, host, stdout) = start_captured(&config);
    let (status, body) = read_host(state, &config).await;
    assert_eq!(status, 200);
    assert_eq!(body["host_type"], "server");
    assert_eq!(body["operator_name"], "Garderen Family");
    assert_eq!(body["hosted_service"]["enrolled"], false);
    assert!(stdout.lines_with(OPERATOR_UNNAMED_OPERATION).is_empty());
    assert!(journaled(&host, OPERATOR_UNNAMED_OPERATION).is_empty());
    host.stop();
}

#[tokio::test]
async fn an_unnamed_server_starts_with_one_warning_on_stdout_and_in_the_journal() {
    let dir = data_dir("unnamed");
    let config = config(&dir, None).expect("starts");
    let (state, host, stdout) = start_captured(&config);
    let (status, body) = read_host(state, &config).await;
    assert_eq!(status, 200);
    assert_eq!(body["host_type"], "server");
    assert_eq!(body["operator_name"], serde_json::Value::Null);
    let lines = stdout.lines_with(OPERATOR_UNNAMED_OPERATION);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("\"level\":\"WARN\""), "{}", lines[0]);
    assert!(lines[0].contains(OPERATOR_NAME_VARIABLE), "{}", lines[0]);
    assert!(!lines[0].contains(TOKEN));
    assert!(!lines[0].contains(&dir.to_string_lossy().to_string()));
    let entries = journaled(&host, OPERATOR_UNNAMED_OPERATION);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].level, Level::Warn);
    assert_eq!(
        entries[0].target_ids,
        vec![OPERATOR_NAME_VARIABLE.to_string()]
    );
    host.stop();
}

#[test]
fn the_warning_is_written_at_every_start() {
    let dir = data_dir("restart");
    let config = config(&dir, Some("   ")).expect("starts");
    for start in 1..=2 {
        let (_, host, stdout) = start_captured(&config);
        assert_eq!(stdout.lines_with(OPERATOR_UNNAMED_OPERATION).len(), 1);
        assert_eq!(journaled(&host, OPERATOR_UNNAMED_OPERATION).len(), start);
        host.stop();
    }
}

#[test]
fn without_a_journal_layer_the_warning_is_journaled_directly_once() {
    let dir = data_dir("no-layer");
    let config = config(&dir, None).expect("starts");
    let state = ApiState::new();
    let host = start_server(&config, &state, None).expect("server starts");
    assert_eq!(journaled(&host, OPERATOR_UNNAMED_OPERATION).len(), 1);
    host.stop();
}

/// Redaction (API-S-020, T-055): the entry carries the operation, a level, the system actor
/// and the variable's name, and nothing that could hold a value, token, path or identifier.
#[test]
fn the_operator_unnamed_entry_carries_nothing_private() {
    let entry = operator_unnamed_entry();
    assert_eq!(
        entry.clone().redacted(),
        entry,
        "every field is an opaque code"
    );
    assert_eq!(entry.operation, OPERATOR_UNNAMED_OPERATION);
    assert_eq!(entry.actor, "system");
    assert_eq!(entry.target_ids, vec![OPERATOR_NAME_VARIABLE.to_string()]);
    assert_eq!(entry.process_id, None);
    assert_eq!(entry.error_code, None);
    let text = serde_json::to_string(&entry).expect("JSON");
    for forbidden in ['/', '\\', '@', ' '] {
        assert!(!text.contains(forbidden), "{text}");
    }
    let line: serde_json::Value =
        serde_json::from_str(&operator_unnamed_line("2026-10-18T00:00:00.000Z")).expect("JSON");
    let fields = line["fields"].as_object().expect("fields");
    let mut keys: Vec<&str> = fields.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["message", "operation", "variable"]);
    assert_eq!(line["level"], "WARN");
    assert!(!line.to_string().contains('/') && !line.to_string().contains('@'));
}

/// The real binary with `RUST_LOG=off` still prints the warning: it does not go through the
/// log filter. The server is stopped as soon as the line appears (bounded wait).
#[test]
fn the_binary_prints_the_warning_even_with_logging_off() {
    let dir = data_dir("binary-off");
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_filesquid-server"))
        .env("RUST_LOG", "off")
        .env("FILESQUID_ADMIN_TOKEN", TOKEN)
        .env("FILESQUID_DATA_DIR", &dir)
        .env("FILESQUID_BIND", "127.0.0.1:0")
        .env_remove(OPERATOR_NAME_VARIABLE)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("server binary runs");
    let stdout = child.stdout.take().expect("stdout");
    let (found, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let warning = std::io::BufRead::lines(std::io::BufReader::new(stdout))
            .map_while(Result::ok)
            .find(|line| line.contains(OPERATOR_UNNAMED_OPERATION));
        let _ = found.send(warning);
    });
    let warning = lines.recv_timeout(std::time::Duration::from_secs(30));
    let _ = child.kill();
    let _ = child.wait();
    let line = warning.expect("a line in time").expect("the warning line");
    assert!(line.contains("\"level\":\"WARN\""), "{line}");
    assert!(line.contains(OPERATOR_NAME_VARIABLE), "{line}");
    assert!(!line.contains(TOKEN), "{line}");
}

/// A non-Unicode value is refused, not read as unset, for the operator name and for every
/// other variable the server reads (here the administrator token), and the message names the
/// variable without the value.
#[cfg(unix)]
#[test]
fn a_non_unicode_variable_is_refused_without_echoing_it() {
    use std::os::unix::ffi::OsStrExt;
    let value = std::ffi::OsStr::from_bytes(b"Leakybytes\xff\xfe");
    for variable in [OPERATOR_NAME_VARIABLE, "FILESQUID_ADMIN_TOKEN"] {
        let message = unicode_value(variable, Some(value)).expect_err("refused");
        assert!(message.contains(variable), "{message}");
        assert!(!message.contains("Leakybytes"), "{message}");
    }
    assert_eq!(unicode_value(OPERATOR_NAME_VARIABLE, None), Ok(None));
    let unicode = std::ffi::OsStr::new("Garderen Family");
    assert_eq!(
        unicode_value(OPERATOR_NAME_VARIABLE, Some(unicode)),
        Ok(Some("Garderen Family".to_string()))
    );
    assert_eq!(filesquid_server::ENV_VARIABLES.len(), 5);
}

/// The real binary refuses to start with a non-Unicode `FILESQUID_OPERATOR_NAME`.
#[cfg(unix)]
#[test]
fn the_binary_refuses_a_non_unicode_operator_name() {
    use std::os::unix::ffi::OsStrExt;
    let dir = data_dir("binary-non-unicode");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_filesquid-server"))
        .env(
            OPERATOR_NAME_VARIABLE,
            std::ffi::OsStr::from_bytes(b"Leakybytes\xff"),
        )
        .env("FILESQUID_ADMIN_TOKEN", TOKEN)
        .env("FILESQUID_DATA_DIR", &dir)
        .env("FILESQUID_BIND", "127.0.0.1:0")
        .env("RUST_LOG", "info")
        .output()
        .expect("server binary runs");
    assert!(!output.status.success(), "the server must not start");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(text.contains(OPERATOR_NAME_VARIABLE), "{text}");
    assert!(!text.contains("Leakybytes"), "{text}");
    assert!(
        !dir.join("filesquid.sqlite").exists(),
        "refused before opening"
    );
}
