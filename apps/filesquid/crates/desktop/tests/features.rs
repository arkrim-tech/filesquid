//! Cucumber harness for the `filesquid-desktop` Gherkin under `tests/features/` (F-057 and
//! F-056: DE-S-001, DE-S-005; F-037: DE-S-001, DE-S-002, DE-S-006). Needs the `test-webdriver`
//! feature for the 100 MB test file.
//! Writes a JUnit report to `tests/allure-results/`, cleared before each run.

use cucumber::{given, then, when, writer, World, WriterExt as _};
use filesquid_api::auth::LaunchToken;
use filesquid_api::events::{topic, EVENT_BUFFER};
use filesquid_api::journal::JournalEntry;
use filesquid_api::{ApiEvent, ApiState, EventSubscription};
use filesquid_desktop_lib::api_protocol::{handle, into_scheme_response};
use filesquid_desktop_lib::capability_policy::{
    check_app_security, check_capability, check_command_placement, check_plugin_dependencies,
    Violation, APP_COMMANDS, GRANTED_PERMISSIONS,
};
use filesquid_desktop_lib::event_channel::{
    navigation_allowed, open_subscription, pump, EventSink, Forwarding,
};
use filesquid_desktop_lib::test_fixture::{fixture_byte, FIXTURE_LEN, FIXTURE_PATH};
use filesquid_desktop_lib::{install_launch_token, open_host_monitor};
use filesquid_native::HostMonitor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// The scenarios run on cucumber's own runtime, so they need `journaled` but not `block_on`.
#[allow(dead_code)]
#[path = "support/journal.rs"]
mod journal;

/// State shared by the steps of one scenario.
#[derive(Debug, Default, World)]
struct DesktopWorld {
    state: Option<ApiState>,
    token: Option<LaunchToken>,
    response: Option<http::Response<Vec<u8>>>,
    requested: Option<(u64, u64)>,
    total_read: u64,
    reads: u32,
    subscription: Option<EventSubscription>,
    received: Vec<ApiEvent>,
    navigation: Option<bool>,
    journal: Vec<JournalEntry>,
    launched: Option<Launched>,
    capabilities: Vec<serde_json::Value>,
    violations: Vec<Violation>,
}

/// The host monitor opened by the real launch path, in a scratch data directory that is
/// removed (after the monitor stops) when the scenario ends.
struct Launched {
    host: HostMonitor,
    dir: PathBuf,
}

impl std::fmt::Debug for Launched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Launched").field("dir", &self.dir).finish()
    }
}

impl Drop for Launched {
    fn drop(&mut self) {
        self.host.stop();
        if let Err(err) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("scenario data directory not removed: {err}");
        }
    }
}

/// Scenario counter, so concurrent scenarios get separate data directories.
static SCENARIOS: AtomicU32 = AtomicU32::new(0);

/// A sink that keeps every event it is given.
#[derive(Default)]
struct KeepingSink(Mutex<Vec<ApiEvent>>);

impl EventSink for KeepingSink {
    fn deliver(&self, event: &ApiEvent) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|e| e.to_string())?
            .push(event.clone());
        Ok(())
    }
}

impl DesktopWorld {
    fn state(&self) -> ApiState {
        self.state.clone().expect("host started")
    }

    /// Read the 100 MB test file with an optional `Range`, as an app page (with the token) or
    /// as any other page (without it).
    async fn read(&mut self, range: Option<String>, with_token: bool) {
        let mut builder = http::Request::get(format!("filesquid://localhost{FIXTURE_PATH}"))
            .header(http::header::ORIGIN, "tauri://localhost");
        if with_token {
            let secret = self.token.as_ref().expect("token").secret().to_string();
            builder = builder.header(http::header::AUTHORIZATION, format!("Bearer {secret}"));
        }
        if let Some(range) = range {
            builder = builder.header(http::header::RANGE, range);
        }
        let request = builder.body(Vec::new()).expect("request");
        self.response = Some(handle(self.state(), request).await);
    }

    fn response(&self) -> &http::Response<Vec<u8>> {
        self.response.as_ref().expect("an answer")
    }

    /// Forward the open subscription to a sink until the bus has nothing more, then keep what
    /// the window received.
    async fn drain(&mut self) {
        let subscription = self.subscription.take().expect("subscribed");
        let stop = Arc::new(AtomicBool::new(false));
        let forwarding = Forwarding {
            stop: stop.clone(),
            heartbeat: Duration::from_millis(10),
            process: None,
        };
        let sink = KeepingSink::default();
        let stopper = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            stop.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        pump(subscription, &sink, forwarding).await;
        stopper.await.expect("stopper");
        self.received = sink.0.into_inner().expect("events");
    }
}

#[given("the desktop host has started with a new per-launch token")]
fn host_started(world: &mut DesktopWorld) {
    let state = ApiState::new();
    world.token = Some(install_launch_token(&state).expect("token"));
    let n = SCENARIOS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let name = format!("filesquid-desktop-features-{}-{n}", std::process::id());
    let dir = std::env::temp_dir().join(name);
    let host = open_host_monitor(&state, &dir, None).expect("launch journaling");
    world.launched = Some(Launched { host, dir });
    world.state = Some(state);
}

#[then("the activity journal should record one token issue for the owner without the secret")]
fn token_issue_journaled(world: &mut DesktopWorld) {
    let launched = world.launched.as_ref().expect("host started");
    let monitor = launched.host.monitor();
    monitor.journal().flush();
    let store = monitor.store();
    let issued: Vec<JournalEntry> = journal::all_entries(store.as_ref())
        .into_iter()
        .filter(|e| e.operation == "api.auth.token_issued")
        .collect();
    assert_eq!(issued.len(), 1, "{issued:?}");
    assert_eq!(issued[0].target_ids, ["owner"]);
    let text = serde_json::to_string(&issued[0]).expect("json");
    let secret = world.token.as_ref().expect("token").secret();
    assert!(!text.contains(secret), "{text}");
}

#[when("an app page reads the last 100 bytes of the 100 MB test file with the token")]
async fn read_last_bytes(world: &mut DesktopWorld) {
    world.requested = Some((FIXTURE_LEN - 100, FIXTURE_LEN - 1));
    world.read(Some("bytes=-100".to_string()), true).await;
}

#[when("a page reads the last 100 bytes of the 100 MB test file without the token")]
async fn read_without_token(world: &mut DesktopWorld) {
    world.read(Some("bytes=-100".to_string()), false).await;
}

#[when("an app page reads the 100 MB test file from its first byte with the token")]
async fn read_open_ended(world: &mut DesktopWorld) {
    world.read(Some("bytes=0-".to_string()), true).await;
}

#[when("an app page reads past the end of the 100 MB test file with the token")]
async fn read_past_end(world: &mut DesktopWorld) {
    world
        .read(Some(format!("bytes={FIXTURE_LEN}-")), true)
        .await;
}

#[when("an app page reads the whole 100 MB test file range by range with the token")]
async fn read_whole_file(world: &mut DesktopWorld) {
    while world.total_read < FIXTURE_LEN && world.reads < 20 {
        let range = format!("bytes={}-", world.total_read);
        world.read(Some(range), true).await;
        assert_eq!(world.response().status(), http::StatusCode::PARTIAL_CONTENT);
        world.total_read += world.response().body().len() as u64;
        world.reads += 1;
    }
}

#[when("the host answers with a body one byte over its buffer cap")]
async fn answer_over_cap(world: &mut DesktopWorld) {
    let body = axum::body::Body::from(vec![0u8; 11]);
    let response = http::Response::builder().body(body).expect("response");
    world.response = Some(into_scheme_response(response, 10).await);
}

#[then("the read should succeed as partial content")]
fn partial_content(world: &mut DesktopWorld) {
    assert_eq!(world.response().status(), http::StatusCode::PARTIAL_CONTENT);
}

#[then(expr = "the answer should name the range {string}")]
fn names_range(world: &mut DesktopWorld, range: String) {
    assert_eq!(
        world.response().headers()[http::header::CONTENT_RANGE],
        range.as_str()
    );
}

#[then("the answer should carry exactly the requested bytes")]
fn exact_bytes(world: &mut DesktopWorld) {
    let (start, end) = world.requested.expect("requested range");
    let expected: Vec<u8> = (start..=end).map(fixture_byte).collect();
    assert_eq!(world.response().body(), &expected);
}

#[then(expr = "it should have read {int} bytes in {int} reads")]
fn read_totals(world: &mut DesktopWorld, bytes: u64, reads: u32) {
    assert_eq!((world.total_read, world.reads), (bytes, reads));
}

#[then(expr = "the answer should be the typed error {string}")]
fn typed_error(world: &mut DesktopWorld, code: String) {
    let json: serde_json::Value = serde_json::from_slice(world.response().body()).expect("json");
    assert_eq!(json["code"], code.as_str());
}

#[then(expr = "the answer should be \"range not satisfiable\" naming the length {int}")]
fn not_satisfiable(world: &mut DesktopWorld, length: u64) {
    let response = world.response();
    assert_eq!(response.status(), http::StatusCode::RANGE_NOT_SATISFIABLE);
    let expected = format!("bytes */{length}");
    assert_eq!(
        response.headers()[http::header::CONTENT_RANGE],
        expected.as_str()
    );
}

#[given("an app window subscribed to the event channel")]
#[given("an app window subscribed to the event channel that is not reading")]
fn subscribed(world: &mut DesktopWorld) {
    world.subscription = Some(open_subscription(&world.state().events).expect("subscription"));
}

#[when("a process update is published")]
async fn process_update(world: &mut DesktopWorld) {
    let data = serde_json::json!({ "id": "import-1", "state": "running" });
    world
        .state()
        .events
        .publish(ApiEvent::new(topic::PROCESS_UPDATE, data));
    world.drain().await;
}

#[when("one more event than the buffer holds is published")]
async fn overflow(world: &mut DesktopWorld) {
    for n in 0..=EVENT_BUFFER {
        world
            .state()
            .events
            .publish(ApiEvent::new("test.marker", serde_json::json!({ "n": n })));
    }
    world.drain().await;
}

#[then(expr = "the window should receive a {string} event")]
fn received_topic(world: &mut DesktopWorld, expected: String) {
    assert!(
        world.received.iter().any(|e| e.topic == expected),
        "{:?}",
        world.received
    );
}

#[then(expr = "the window should first receive an {string} hint for {int} missed event")]
fn received_hint(world: &mut DesktopWorld, expected: String, missed: u64) {
    let first = world.received.first().expect("an event");
    assert_eq!(first.topic, expected);
    assert_eq!(first.data["missed"], missed);
}

#[when(expr = "an app window is asked to navigate to {string}")]
fn navigate(world: &mut DesktopWorld, address: String) {
    let url = tauri::Url::parse(&address).expect("url");
    let mut allowed = None;
    world.journal = journal::journaled(|| allowed = Some(navigation_allowed(&url)));
    world.navigation = allowed;
}

#[then("the navigation should be refused")]
fn navigation_refused(world: &mut DesktopWorld) {
    assert_eq!(world.navigation, Some(false));
}

#[then(expr = "the activity journal should record {string} without the address")]
fn journal_records(world: &mut DesktopWorld, operation: String) {
    let entry = world
        .journal
        .iter()
        .find(|e| e.operation == operation)
        .expect("journaled");
    let text = serde_json::to_string(entry).expect("json");
    assert!(!text.contains("example.com"), "{text}");
}

/// The crate's shipped capability files, read from `capabilities/`. Performs filesystem I/O.
fn read_shipped_capabilities() -> Vec<serde_json::Value> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    std::fs::read_dir(dir)
        .expect("capabilities directory")
        .map(|entry| {
            let text = std::fs::read_to_string(entry.expect("entry").path()).expect("file");
            serde_json::from_str(&text).expect("capability JSON")
        })
        .collect()
}

/// Read one file of the crate as text. Performs filesystem I/O.
fn read_crate_file(name: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name)).expect(name)
}

/// Check a capability for the main window that grants `permissions`.
fn check_main_window_capability(permissions: serde_json::Value) -> Vec<Violation> {
    let doc = serde_json::json!({
        "identifier": "default", "windows": ["main"], "permissions": permissions
    });
    check_capability(&doc)
}

#[given("the desktop host's shipped capability set")]
fn shipped_capability_set(world: &mut DesktopWorld) {
    world.capabilities = read_shipped_capabilities();
}

#[then("the capability set should have no policy violations")]
fn no_policy_violations(world: &mut DesktopWorld) {
    let mut violations: Vec<Violation> = world
        .capabilities
        .iter()
        .flat_map(check_capability)
        .collect();
    let conf = serde_json::from_str(&read_crate_file("tauri.conf.json")).expect("conf");
    violations.extend(check_app_security(&conf));
    violations.extend(check_plugin_dependencies(&read_crate_file("Cargo.toml")));
    violations.extend(check_command_placement(
        "lib.rs",
        &read_crate_file("src/lib.rs"),
    ));
    assert_eq!(violations, Vec::new());
}

#[then(expr = "the main window should be granted exactly {string} and {string}")]
fn granted_exactly(world: &mut DesktopWorld, first: String, second: String) {
    assert_eq!(GRANTED_PERMISSIONS, [first.as_str(), second.as_str()]);
    let granted: Vec<&serde_json::Value> = world
        .capabilities
        .iter()
        .map(|c| &c["permissions"])
        .collect();
    assert_eq!(granted, [&serde_json::json!([first, second])]);
}

#[then("no Tauri command of the app should be defined")]
fn no_app_command(_world: &mut DesktopWorld) {
    assert!(APP_COMMANDS.is_empty());
}

#[when(expr = "a capability grants {string} to the main window")]
fn capability_grants(world: &mut DesktopWorld, permission: String) {
    world.violations = check_main_window_capability(serde_json::json!([permission]));
}

#[when(expr = "a capability names the window {string}")]
fn capability_names_window(world: &mut DesktopWorld, window: String) {
    let doc =
        serde_json::json!({ "identifier": "default", "windows": [window], "permissions": [] });
    world.violations = check_capability(&doc);
}

#[when("a capability grants event listening to remote pages")]
fn capability_for_remote_pages(world: &mut DesktopWorld) {
    let doc = serde_json::json!({
        "identifier": "default",
        "windows": ["main"],
        "remote": { "urls": ["https://*.example.com"] },
        "permissions": ["core:event:allow-listen"]
    });
    world.violations = check_capability(&doc);
}

#[when(expr = "lib.rs defines the Tauri command {string}")]
fn lib_defines_command(world: &mut DesktopWorld, name: String) {
    let source = format!("#[tauri::command]\nfn {name}() {{}}\n");
    world.violations = check_command_placement("lib.rs", &source);
}

#[when(expr = "a submodule defines the Tauri command {string}")]
fn submodule_defines_command(world: &mut DesktopWorld, name: String) {
    let source = format!("#[tauri::command]\npub fn {name}(path: String) {{}}\n");
    world.violations = check_command_placement("files.rs", &source);
}

#[then(expr = "the policy should refuse it as the forbidden plugin permission {string}")]
fn refused_forbidden(world: &mut DesktopWorld, permission: String) {
    assert_eq!(
        world.violations,
        vec![Violation::ForbiddenPlugin(permission)]
    );
}

#[then(expr = "the policy should refuse it as the ungranted permission {string}")]
fn refused_ungranted(world: &mut DesktopWorld, permission: String) {
    assert_eq!(
        world.violations,
        vec![Violation::PermissionNotGranted(permission)]
    );
}

#[then(expr = "the policy should refuse it as the unknown window {string}")]
fn refused_window(world: &mut DesktopWorld, window: String) {
    assert_eq!(world.violations, vec![Violation::UnknownWindow(window)]);
}

#[then("the policy should refuse it for its remote context")]
fn refused_remote(world: &mut DesktopWorld) {
    assert_eq!(world.violations, vec![Violation::RemoteContext]);
}

#[then(expr = "the policy should refuse the command {string} in lib.rs")]
fn refused_in_lib(world: &mut DesktopWorld, name: String) {
    assert_eq!(world.violations, vec![Violation::CommandInLib(name)]);
}

#[then(expr = "the policy should refuse the unlisted command {string}")]
fn refused_unlisted(world: &mut DesktopWorld, name: String) {
    assert_eq!(world.violations, vec![Violation::UnlistedCommand(name)]);
}

#[tokio::main]
async fn main() {
    let results = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    let _ = std::fs::remove_dir_all(&results);
    std::fs::create_dir_all(&results).expect("allure-results");
    let junit = std::fs::File::create(results.join("cucumber-junit.xml")).expect("junit file");
    DesktopWorld::cucumber()
        .with_writer(
            writer::Basic::stdout()
                .summarized()
                .tee::<DesktopWorld, _>(writer::JUnit::for_tee(junit, 0))
                .normalized(),
        )
        .fail_on_skipped()
        .run_and_exit(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/features"))
        .await;
}
