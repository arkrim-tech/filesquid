//! Cucumber harness for the `@server-start` scenarios of the API's
//! `host_descriptor_API-S-020.feature` (API-S-020): how `filesquid-server` reads
//! `FILESQUID_OPERATOR_NAME` when it starts. An unset or blank value starts the server with no
//! operator name and one `server.operator_unnamed` warning on stdout (whatever the log filter)
//! and in the journal; an
//! unusable value stops the real binary with an error that names the variable and omits the
//! value. The API harness runs the feature's other scenarios. Writes a JUnit report to
//! `tests/allure-results/`, cleared before each run.

use axum::body::Body;
use axum::http::{header, Request};
use cucumber::{given, then, when, writer, World, WriterExt as _};
use filesquid_api::journal::{JournalEntry, JournalQuery, Level};
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::ApiState;
use filesquid_native::HostMonitor;
use filesquid_server::{app, start_server_writing_to, ServerConfig, OPERATOR_NAME_VARIABLE};
use http_body_util::BodyExt;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

/// The feature file, owned by the API crate.
const FEATURE: &str = "../api/tests/features/host_descriptor_API-S-020.feature";

/// Tag of the scenarios this harness runs.
const SERVER_START_TAG: &str = "server-start";

/// Administrator token of the test server.
const TOKEN: &str = "5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed";

/// Marker inside every unusable value, so a leak is easy to find.
const VALUE_MARKER: &str = "Leakmarker";

/// Distinguishes the data directories of scenarios in one run.
static SCENARIO: AtomicU32 = AtomicU32::new(0);

/// State shared by the steps of one scenario.
#[derive(Default, World)]
struct ServerWorld {
    data_dir: PathBuf,
    operator: Option<String>,
    started: Option<(ApiState, HostMonitor, ServerConfig)>,
    stdout: Captured,
    descriptor: serde_json::Value,
    refusal: String,
}

impl std::fmt::Debug for ServerWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerWorld")
            .field("data_dir", &self.data_dir)
            .finish_non_exhaustive()
    }
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
        String::from_utf8_lossy(&bytes)
            .lines()
            .filter(|line| line.contains(needle))
            .map(str::to_string)
            .collect()
    }
}

/// A fresh data directory for one scenario.
fn fresh_data_dir() -> PathBuf {
    let n = SCENARIO.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("fs-server-features-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The configured value for an example `setting` of the outline.
fn setting_value(setting: &str) -> Option<String> {
    match setting {
        "unset" => None,
        "set to an empty value" => Some(String::new()),
        "set to only spaces and tabs" => Some(" \t \t ".to_string()),
        other => panic!("unknown setting {other:?}"),
    }
}

/// The configured value for an example `value` of the refusal outline.
fn unusable_value(value: &str) -> String {
    match value {
        "a name of 257 characters" => format!("{VALUE_MARKER}{}", "n".repeat(257)),
        "a name with a control character" => format!("{VALUE_MARKER}\u{7}Team"),
        other => panic!("unknown value {other:?}"),
    }
}

/// Start the server as `main` does (JSON stdout plus the journal layer), with the stdout log
/// filter turned off (`RUST_LOG=off`) and the server's unfiltered startup lines captured.
fn start_captured(w: &mut ServerWorld, config: ServerConfig) {
    let layer = JournalLayer::new();
    let filtered = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(w.stdout.clone())
        .with_filter(tracing_subscriber::filter::LevelFilter::OFF);
    let subscriber = tracing_subscriber::registry()
        .with(filtered)
        .with(layer.clone());
    let state = ApiState::new();
    let mut stdout = w.stdout.clone();
    let host = tracing::subscriber::with_default(subscriber, || {
        start_server_writing_to(&config, &state, Some(&layer), &mut stdout)
            .expect("the server starts")
    });
    host.monitor().journal().flush();
    w.started = Some((state, host, config));
}

#[given(expr = "a FileSquid server host started with FILESQUID_OPERATOR_NAME {}")]
fn server_started(w: &mut ServerWorld, setting: String) {
    w.data_dir = fresh_data_dir();
    let operator = setting_value(&setting);
    let dir = w.data_dir.to_string_lossy().to_string();
    let config = ServerConfig::from_lookup(|key| match key {
        "FILESQUID_ADMIN_TOKEN" => Some(TOKEN.to_string()),
        "FILESQUID_DATA_DIR" => Some(dir.clone()),
        "FILESQUID_OPERATOR_NAME" => operator.clone(),
        _ => None,
    })
    .expect("the configuration is accepted");
    start_captured(w, config);
}

/// Until server accounts arrive (API-F1.9), the administrator credential is the only one the
/// server accepts.
#[when(expr = "a caller signed in to the server reads the host descriptor")]
async fn caller_reads(w: &mut ServerWorld) {
    let (state, _, config) = w.started.as_ref().expect("started");
    let request = Request::get("/api/v1/host")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let response = app(config, state.clone())
        .oneshot(request)
        .await
        .expect("answer");
    assert_eq!(response.status().as_u16(), 200);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    w.descriptor = serde_json::from_slice(&body).expect("JSON");
}

#[then(expr = "the descriptor should say host type {string} with no operator name")]
fn no_operator(w: &mut ServerWorld, host_type: String) {
    assert_eq!(w.descriptor["host_type"], host_type.as_str());
    assert_eq!(w.descriptor["operator_name"], serde_json::Value::Null);
}

#[then(expr = "stdout should carry one {string} warning that names the variable")]
fn stdout_warning(w: &mut ServerWorld, operation: String) {
    let lines = w.stdout.lines_with(&operation);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("\"level\":\"WARN\""), "{}", lines[0]);
    assert!(lines[0].contains(OPERATOR_NAME_VARIABLE), "{}", lines[0]);
}

/// Entries of `operation` in the started server's journal.
fn journaled(w: &ServerWorld, operation: &str) -> Vec<JournalEntry> {
    let (_, host, _) = w.started.as_ref().expect("started");
    host.monitor().journal().flush();
    let query = JournalQuery {
        operation: Some(operation.to_string()),
        ..JournalQuery::default()
    };
    host.monitor()
        .store()
        .query(&query)
        .expect("journal")
        .entries
}

#[then(expr = "the activity journal should hold one {string} warning that names the variable")]
fn journal_warning(w: &mut ServerWorld, operation: String) {
    let entries = journaled(w, &operation);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].level, Level::Warn);
    assert_eq!(
        entries[0].target_ids,
        vec![OPERATOR_NAME_VARIABLE.to_string()]
    );
}

#[then(
    expr = "neither warning should contain a token, key, account or device identifier, path or file name"
)]
fn warnings_redacted(w: &mut ServerWorld) {
    let entries = journaled(w, "server.operator_unnamed");
    let mut texts = w.stdout.lines_with("server.operator_unnamed");
    texts.push(serde_json::to_string(&entries[0]).expect("JSON"));
    let data_dir = w.data_dir.to_string_lossy().to_string();
    for text in &texts {
        assert!(!text.contains(TOKEN), "{text}");
        assert!(!text.contains(&data_dir), "{text}");
        assert!(!text.contains('@'), "{text}");
    }
    assert_eq!(
        entries[0].clone().redacted(),
        entries[0],
        "only opaque codes"
    );
    assert!(entries[0].process_id.is_none() && entries[0].error_code.is_none());
    let (_, host, _) = w.started.take().expect("started");
    host.stop();
}

#[given(expr = "FILESQUID_OPERATOR_NAME is set to {}")]
fn unusable_operator(w: &mut ServerWorld, value: String) {
    w.data_dir = fresh_data_dir();
    w.operator = Some(unusable_value(&value));
}

/// The real binary, so the refusal is the one an operator sees.
#[when(expr = "filesquid-server starts")]
fn binary_starts(w: &mut ServerWorld) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_filesquid-server"))
        .env(
            OPERATOR_NAME_VARIABLE,
            w.operator.as_deref().expect("value"),
        )
        .env("FILESQUID_ADMIN_TOKEN", TOKEN)
        .env("FILESQUID_DATA_DIR", &w.data_dir)
        .env("FILESQUID_BIND", "127.0.0.1:0")
        .output()
        .expect("the server binary runs");
    assert!(!output.status.success(), "the server must not start");
    w.refusal = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[then(expr = "it should refuse to start with an error that names FILESQUID_OPERATOR_NAME")]
fn refusal_names_variable(w: &mut ServerWorld) {
    assert!(w.refusal.contains(OPERATOR_NAME_VARIABLE), "{}", w.refusal);
    assert!(
        !w.data_dir.join("filesquid.sqlite").exists(),
        "refused before opening"
    );
}

#[then(expr = "the error should not contain the value")]
fn refusal_omits_value(w: &mut ServerWorld) {
    assert!(!w.refusal.contains(VALUE_MARKER), "{}", w.refusal);
}

#[tokio::main]
async fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let results = root.join("tests/allure-results");
    let _ = std::fs::remove_dir_all(&results);
    std::fs::create_dir_all(&results).expect("allure-results");
    let junit = std::fs::File::create(results.join("cucumber-junit.xml")).expect("junit file");
    ServerWorld::cucumber()
        .with_writer(
            writer::Basic::stdout()
                .summarized()
                .tee::<ServerWorld, _>(writer::JUnit::for_tee(junit, 0))
                .normalized(),
        )
        .fail_on_skipped()
        .filter_run_and_exit(root.join(FEATURE), |_, _, scenario| {
            scenario.tags.iter().any(|tag| tag == SERVER_START_TAG)
        })
        .await;
}
