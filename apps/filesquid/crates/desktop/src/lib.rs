//! `filesquid-desktop`: the FileSquid desktop host (Tauri 2).
//!
//! Serves the single FileSquid API to the webview through the `filesquid://` custom URI
//! scheme ([`api_protocol`]), ranged reads included, and pushes API events to the app's own
//! webviews as Tauri events named [`EVENT_NAME`] ([`event_channel`]). There is no TCP listener
//! and no per-feature IPC command
//! ([ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)).
//! Activity is recorded in the activity journal in the app data directory and the host's
//! health is tracked by the process registry
//! ([ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
//! Logs go to stdout as JSON; no log files.
//!
//! Every launch generates a new per-launch token ([`install_launch_token`]). The API refuses
//! any request without it (deny-by-default, API-S-004), and the host hands it only to its
//! own pages through a webview initialization script ([`api_protocol::launch_token_script`]).
//! The same plugin keeps every webview on the app's own origins, so the token and the events
//! reach app pages only.
//!
//! The test-instrumented build (`--features test-webdriver`, never shipped) adds the embedded
//! WebDriver server for the packaged-window tests and a 100 MB range fixture (`test_fixture`).

pub mod api_protocol;
pub mod event_channel;
#[cfg(feature = "test-webdriver")]
pub mod test_fixture;

pub use event_channel::EVENT_NAME;

use event_channel::{Forwarding, TauriSink, FORWARDER_HEARTBEAT};
use filesquid_api::auth::{record_token_issue, Authoriser, LaunchToken, Principal};
use filesquid_api::monitor::HostMonitor;
use filesquid_api::registry::ProcessHandle;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::ApiState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Manager, RunEvent};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

/// Interval between monitor ticks (host heartbeat, stalled checks, health).
const TICK: Duration = Duration::from_secs(1);

/// Desktop monitor, the event forwarder's process, and the flag that ends both loops.
struct DesktopMonitor {
    host: Arc<HostMonitor>,
    forwarder: Arc<ProcessHandle>,
    stop: Arc<AtomicBool>,
}

/// Install stdout JSON logging (filtered by `RUST_LOG`) and the unfiltered journal layer.
fn init_tracing(journal: JournalLayer) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let stdout = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(std::io::stdout)
        .with_filter(filter);
    tracing_subscriber::registry()
        .with(stdout)
        .with(journal)
        .try_init()
        .ok();
}

/// Name of the plugin that gives app pages the per-launch token and keeps every webview on
/// the app's own origins.
const APP_PAGES_PLUGIN: &str = "filesquid-app-pages";

/// Generate this launch's token and install it as the only credential `state` accepts.
/// Fails when the random source fails or an authoriser is already installed.
pub fn install_launch_token(state: &ApiState) -> Result<LaunchToken, String> {
    let token = LaunchToken::generate()?;
    if !state.install_authoriser(Authoriser::desktop_owner(&token)) {
        return Err("an authoriser was already installed".to_string());
    }
    Ok(token)
}

/// A plugin with no commands that guards the app's pages: its initialization script gives
/// app pages the per-launch token, and its navigation hook refuses any navigation off the
/// app's own origins ([`event_channel::navigation_allowed`]), so the token and the event
/// channel reach app pages only.
fn app_pages_plugin(token: &LaunchToken) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new(APP_PAGES_PLUGIN)
        .js_init_script(api_protocol::launch_token_script(token))
        .on_navigation(|_webview, url| event_channel::navigation_allowed(url))
        .build()
}

/// Add the embedded WebDriver server of the packaged-window tests. Test-instrumented builds
/// only: it listens on `127.0.0.1`, on the port in `TAURI_WEBDRIVER_PORT`.
#[cfg(feature = "test-webdriver")]
fn with_test_driver(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.plugin(tauri_plugin_wdio_webdriver::init())
}

/// Shipping builds add no WebDriver server and open no TCP listener.
#[cfg(not(feature = "test-webdriver"))]
fn with_test_driver(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
}

/// Build the Tauri application with the per-launch token, the API scheme, event forwarding
/// and the monitor. Fails when the per-launch token cannot be generated.
pub fn builder() -> Result<tauri::Builder<tauri::Wry>, String> {
    let layer = JournalLayer::new();
    init_tracing(layer.clone());
    let state = ApiState::new();
    let token = install_launch_token(&state)?;
    let scheme_state = state.clone();
    let builder = with_test_driver(tauri::Builder::default())
        .plugin(app_pages_plugin(&token))
        .manage(state)
        .register_asynchronous_uri_scheme_protocol(
            api_protocol::SCHEME,
            move |_ctx, request, responder| {
                let state = scheme_state.clone();
                tauri::async_runtime::spawn(async move {
                    responder.respond(api_protocol::handle(state, request).await);
                });
            },
        )
        .setup(move |app| {
            start_monitor(app.handle(), &layer)?;
            forward_events(app.handle());
            Ok(())
        });
    Ok(builder)
}

/// Open the journal in the app data directory, install the monitor, register the event
/// forwarder's process and start the ticker. Performs filesystem and database I/O.
fn start_monitor(
    app: &tauri::AppHandle,
    layer: &JournalLayer,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    let state = app.state::<ApiState>();
    let host = Arc::new(HostMonitor::open(
        &data_dir,
        "filesquid-desktop",
        &state,
        Some(layer),
    )?);
    record_token_issue(host.monitor(), &Principal::Owner);
    let forwarder = Arc::new(event_channel::register_forwarder(host.monitor().registry()));
    let stop = Arc::new(AtomicBool::new(false));
    spawn_ticker(host.clone(), stop.clone());
    app.manage(DesktopMonitor {
        host,
        forwarder,
        stop,
    });
    Ok(())
}

/// Tick the monitor once per [`TICK`] until `stop` is set at app exit (bounded by the app's lifetime).
fn spawn_ticker(host: Arc<HostMonitor>, stop: Arc<AtomicBool>) {
    tauri::async_runtime::spawn(async move {
        while !stop.load(Ordering::SeqCst) {
            tokio::time::sleep(TICK).await;
            let host = host.clone();
            if tauri::async_runtime::spawn_blocking(move || host.tick())
                .await
                .is_err()
            {
                tracing::error!("monitor tick panicked");
            }
        }
    });
}

/// Start forwarding the API bus to the app's webviews through one counted subscription
/// ([`event_channel::pump`]) until the bus closes or the app exits. When no subscription slot
/// is free, the refusal is journaled, the forwarder's process fails, and the API keeps
/// working without push.
fn forward_events(app: &tauri::AppHandle) {
    let Some(monitor) = app.try_state::<DesktopMonitor>() else {
        tracing::error!(
            operation = "desktop.events.subscription",
            error_code = "internal",
            "event forwarder started before the monitor"
        );
        return;
    };
    let Some(subscription) = event_channel::open_subscription(&app.state::<ApiState>().events)
    else {
        monitor.forwarder.fail("unavailable");
        return;
    };
    let forwarding = Forwarding {
        stop: monitor.stop.clone(),
        heartbeat: FORWARDER_HEARTBEAT,
        process: Some(monitor.forwarder.clone()),
    };
    let sink = TauriSink(app.clone());
    tauri::async_runtime::spawn(async move {
        let report = event_channel::pump(subscription, &sink, forwarding).await;
        tracing::info!(
            delivered = report.delivered,
            failed = report.failed,
            end = ?report.end,
            "desktop event forwarder ended"
        );
    });
}

/// Stop the ticker and the event forwarder, complete the forwarder's process, journal
/// `host.stopped` and flush the journal.
fn stop_monitor(app: &tauri::AppHandle) {
    if let Some(monitor) = app.try_state::<DesktopMonitor>() {
        monitor.stop.store(true, Ordering::SeqCst);
        monitor.forwarder.complete();
        monitor.host.stop();
    }
}

/// Run the desktop event loop.
pub fn run() {
    let builder = match builder() {
        Ok(builder) => builder,
        Err(message) => {
            tracing::error!(%message, "FileSquid desktop could not secure its API");
            std::process::exit(1);
        }
    };
    let app = match builder.build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(err) => {
            tracing::error!(%err, "FileSquid desktop could not start");
            std::process::exit(1);
        }
    };
    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            stop_monitor(handle);
        }
    });
}
