//! `filesquid-desktop`: the FileSquid desktop host (Tauri 2).
//!
//! Serves the single FileSquid API to the webview through the `filesquid://` custom URI
//! scheme ([`api_protocol`]) and forwards API events to the UI as Tauri events named
//! [`EVENT_NAME`]. There is no TCP listener and no per-feature IPC command
//! ([ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)).
//! Activity is recorded in the activity journal in the app data directory and the host's
//! health is tracked by the process registry
//! ([ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
//! Logs go to stdout as JSON; no log files.
//!
//! Every launch generates a new per-launch token ([`install_launch_token`]). The API refuses
//! any request without it (deny-by-default, API-S-004), and the host hands it only to its
//! own pages through a webview initialization script ([`api_protocol::launch_token_script`]).

pub mod api_protocol;

use filesquid_api::auth::{record_token_issue, Authoriser, LaunchToken, Principal};
use filesquid_api::monitor::HostMonitor;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::ApiState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager, RunEvent};
use tokio::sync::broadcast::error::RecvError;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

/// Tauri event name that carries every [`filesquid_api::ApiEvent`] to the UI.
pub const EVENT_NAME: &str = "filesquid-event";

/// Interval between monitor ticks (host heartbeat, stalled checks, health).
const TICK: Duration = Duration::from_secs(1);

/// Desktop monitor plus the flag that ends its ticker.
struct DesktopMonitor {
    host: Arc<HostMonitor>,
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

/// Name of the plugin that carries the per-launch token's initialization script.
const TOKEN_PLUGIN: &str = "filesquid-launch-token";

/// Generate this launch's token and install it as the only credential `state` accepts.
/// Fails when the random source fails or an authoriser is already installed.
pub fn install_launch_token(state: &ApiState) -> Result<LaunchToken, String> {
    let token = LaunchToken::generate()?;
    if !state.install_authoriser(Authoriser::desktop_owner(&token)) {
        return Err("an authoriser was already installed".to_string());
    }
    Ok(token)
}

/// A plugin with no commands whose only job is the initialization script that gives app
/// pages the per-launch token.
fn launch_token_plugin(token: &LaunchToken) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new(TOKEN_PLUGIN)
        .js_init_script(api_protocol::launch_token_script(token))
        .build()
}

/// Build the Tauri application with the per-launch token, the API scheme, event forwarding
/// and the monitor. Fails when the per-launch token cannot be generated.
pub fn builder() -> Result<tauri::Builder<tauri::Wry>, String> {
    let layer = JournalLayer::new();
    init_tracing(layer.clone());
    let state = ApiState::new();
    let token = install_launch_token(&state)?;
    let scheme_state = state.clone();
    let builder = tauri::Builder::default()
        .plugin(launch_token_plugin(&token))
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
            forward_events(app.handle().clone());
            Ok(())
        });
    Ok(builder)
}

/// Open the journal in the app data directory, install the monitor and start its ticker.
/// Performs filesystem and database I/O.
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
    let stop = Arc::new(AtomicBool::new(false));
    spawn_ticker(host.clone(), stop.clone());
    app.manage(DesktopMonitor { host, stop });
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

/// Forward API bus events to every webview until the bus closes.
///
/// A lagging receiver logs the dropped count and keeps forwarding; only a closed bus ends the task.
fn forward_events(app: tauri::AppHandle) {
    let mut events = app.state::<ApiState>().events.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "event forwarder lagged; events dropped");
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            if let Err(err) = app.emit(EVENT_NAME, &event) {
                tracing::error!(%err, "event forwarding failed");
            }
        }
    });
}

/// Stop the ticker, journal `host.stopped` and flush the journal.
fn stop_monitor(app: &tauri::AppHandle) {
    if let Some(monitor) = app.try_state::<DesktopMonitor>() {
        monitor.stop.store(true, Ordering::SeqCst);
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
