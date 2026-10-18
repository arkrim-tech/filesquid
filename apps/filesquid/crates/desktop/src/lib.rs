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
//! The host describes itself to the API as `desktop` at every launch
//! ([`describe_desktop_host`]), so `GET /api/v1/host` tells the UI where it runs.
//!
//! Every launch generates a new per-launch token ([`install_launch_token`]). The API refuses
//! any request without it (deny-by-default, API-S-004), and the host hands it only to its
//! own pages through a webview initialization script ([`api_protocol::launch_token_script`]).
//! The same plugin keeps every webview on the app's own origins, so the token and the events
//! reach app pages only.
//!
//! The webview's Tauri capabilities are deny-by-default ([`capability_policy`]): the main
//! window may only listen for the API's events. The host defines no `#[tauri::command]`, and
//! the app ACL manifest (`permissions/app-acl.toml`) makes Tauri refuse any custom command no
//! capability grants. `build.rs` fails the build when a capability file widens that set.
//!
//! Every refusal in the shell is journaled (Task DE-0.9): the host owns Tauri's IPC scheme and
//! journals every IPC call it or Tauri refuses ([`ipc_gate`]); a `tracing` layer journals the
//! refusals Tauri reports only through its own trace ([`ipc_observer`]); the host builds the
//! main window itself and refuses and journals new windows, permission requests and foreign
//! downloads ([`app_window`]); navigation off the app's origins is refused and journaled by the
//! app-pages plugin; and a panic on any host thread is journaled ([`panic_journal`]).
//!
//! The test-instrumented build (`--features test-webdriver`, never shipped) adds the embedded
//! WebDriver server for the packaged-window tests and a 100 MB range fixture (`test_fixture`).

pub mod api_protocol;
pub mod app_window;
pub mod capability_policy;
pub mod event_channel;
pub mod ipc_gate;
pub mod ipc_observer;
pub mod panic_journal;
#[cfg(feature = "test-webdriver")]
pub mod test_fixture;

pub use event_channel::EVENT_NAME;

use event_channel::{Forwarding, TauriSink, FORWARDER_HEARTBEAT};
use filesquid_api::auth::{record_token_issue, Authoriser, LaunchToken, Principal};
use filesquid_api::host::{HostIdentity, NotEnrolled};
use filesquid_api::journal_writer::JournalSender;
use filesquid_api::registry::ProcessHandle;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::ApiState;
use filesquid_native::HostMonitor;
use ipc_observer::TauriIpcObserver;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{Manager, RunEvent};
use tracing::Subscriber;
use tracing_subscriber::filter::{filter_fn, EnvFilter, FilterExt};
use tracing_subscriber::fmt::MakeWriter;
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

/// The journal sinks the host attaches once the journal opens: the journal layer, the Tauri
/// IPC observer and the journal the panic hook flushes.
#[derive(Clone)]
struct HostJournal {
    layer: JournalLayer,
    observer: TauriIpcObserver,
    panic_flush: Arc<OnceLock<JournalSender>>,
}

impl HostJournal {
    /// Sinks with no journal yet.
    fn new() -> Self {
        Self {
            layer: JournalLayer::new(),
            observer: TauriIpcObserver::new(),
            panic_flush: Arc::new(OnceLock::new()),
        }
    }

    /// Attach the observer and the panic hook to the opened journal (`HostMonitor::open`
    /// attaches the layer itself).
    fn attach(&self, sender: JournalSender) {
        self.observer.attach(sender.clone());
        let _ = self.panic_flush.set(sender);
    }
}

/// The host's `tracing` stack: JSON lines to `stdout` filtered by `filter`, the journal
/// layer, and the Tauri IPC observer ([`ipc_observer`]). The stdout and journal layers
/// skip Tauri's IPC trace and webview errors, which only the observer handles, so neither a
/// request body nor the invoke key Tauri records there reaches either. Pure: builds the
/// subscriber without installing it.
pub fn host_subscriber<W>(
    journal: JournalLayer,
    observer: TauriIpcObserver,
    stdout: W,
    filter: EnvFilter,
) -> impl Subscriber + Send + Sync + 'static
where
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    let kept = filter_fn(ipc_observer::outside_observed);
    let stdout = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(stdout)
        .with_filter(filter.and(kept.clone()));
    tracing_subscriber::registry()
        .with(stdout)
        .with(journal.with_filter(kept))
        .with(observer.with_filter(filter_fn(ipc_observer::observed)))
}

/// Install the host's `tracing` stack ([`host_subscriber`]) with stdout filtered by `RUST_LOG`
/// (default `info`). It also routes `log` records (Tauri's own) into `tracing`.
fn init_tracing(journal: &HostJournal) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    host_subscriber(
        journal.layer.clone(),
        journal.observer.clone(),
        std::io::stdout,
        filter,
    )
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

/// Describe this host to the API (API-S-020): host type `desktop`, no operator name, and not
/// enrolled with the hosted service until enrolment exists (API-F4.1). Fails when the state
/// already describes a host.
pub fn describe_desktop_host(state: &ApiState) -> Result<(), String> {
    if !state.describe_host(HostIdentity::desktop(), Arc::new(NotEnrolled)) {
        return Err("the host was already described".to_string());
    }
    Ok(())
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
    let journal = HostJournal::new();
    init_tracing(&journal);
    panic_journal::install_panic_hook(journal.panic_flush.clone());
    let state = ApiState::new();
    let token = install_launch_token(&state)?;
    describe_desktop_host(&state)?;
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
        .register_asynchronous_uri_scheme_protocol(
            ipc_gate::IPC_SCHEME,
            |ctx, request, responder| {
                let label = ctx.webview_label().to_string();
                let reply: ipc_gate::Reply = Box::new(move |answer| responder.respond(answer));
                ipc_gate::handle(ctx.app_handle(), &label, request, reply);
            },
        )
        .setup(move |app| {
            start_monitor(app.handle(), &journal)?;
            forward_events(app.handle());
            open_main_window(app.handle())
        });
    Ok(builder)
}

/// Build the main window ([`app_window::create_main_window`]). When it cannot be built, the
/// failure is already journaled; stop the monitor so the journal is flushed before the
/// process exits. Performs I/O.
fn open_main_window(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    match app_window::create_main_window(app) {
        Ok(_) => Ok(()),
        Err(err) => {
            stop_monitor(app);
            Err(Box::new(err))
        }
    }
}

/// The desktop's launch journaling: open the journal in `data_dir`, install the monitor in
/// `state` (attaching `layer` when given, so warnings and errors are journaled), journal
/// `host.started`, and journal `api.auth.token_issued` for the owner, naming the principal
/// only, never the secret. Performs filesystem and database I/O.
///
/// # Errors
/// The journal could not be opened in `data_dir`.
pub fn open_host_monitor(
    state: &ApiState,
    data_dir: &std::path::Path,
    layer: Option<&JournalLayer>,
) -> Result<HostMonitor, String> {
    let host = HostMonitor::open(data_dir, "filesquid-desktop", state, layer)?;
    record_token_issue(host.monitor(), &Principal::Owner);
    Ok(host)
}

/// Open the journal in the app data directory, attach every journal sink, install the
/// monitor, register the event forwarder's process and start the ticker. Performs filesystem
/// and database I/O.
fn start_monitor(
    app: &tauri::AppHandle,
    journal: &HostJournal,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    let state = app.state::<ApiState>();
    let host = Arc::new(open_host_monitor(&state, &data_dir, Some(&journal.layer))?);
    journal.attach(host.monitor().journal());
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

/// Run the desktop event loop. Every configured window is built by the host
/// ([`app_window::host_built_windows`]) so it carries the host's guards.
pub fn run() {
    let builder = match builder() {
        Ok(builder) => builder,
        Err(message) => {
            tracing::error!(%message, "FileSquid desktop could not secure its API");
            std::process::exit(1);
        }
    };
    let mut context = tauri::generate_context!();
    app_window::host_built_windows(context.config_mut());
    let app = match builder.build(context) {
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
