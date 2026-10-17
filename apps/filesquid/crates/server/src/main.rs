//! `filesquid-server` binary.
//!
//! Reads `FILESQUID_BIND` (default `127.0.0.1:8083`), `FILESQUID_UI_DIR` (optional built
//! distribution UI), `FILESQUID_DATA_DIR` (default `./filesquid-data`; operational store and
//! admin token) and `FILESQUID_ADMIN_TOKEN` (optional). Every API call needs the administrator
//! token (deny-by-default, API-S-004). Logs JSON to stdout only; activity is recorded in the
//! activity journal (ADR 033).

mod telemetry;

use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::ApiState;
use filesquid_server::{app, install_admin_authoriser, start_host_monitor, ServerConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::watch;

/// Interval between monitor ticks (heartbeat, stalled checks, health).
const TICK: Duration = Duration::from_secs(1);

#[tokio::main]
async fn main() {
    let layer = JournalLayer::new();
    telemetry::init(layer.clone());
    let config = match ServerConfig::from_lookup(|key| std::env::var(key).ok()) {
        Ok(config) => config,
        Err(message) => {
            tracing::error!(%message, "invalid configuration");
            std::process::exit(2);
        }
    };
    let state = ApiState::new();
    if let Err(message) = install_admin_authoriser(&config, &state) {
        tracing::error!(%message, "cannot set up API authorisation");
        std::process::exit(2);
    }
    let host = match start_host_monitor(&config, &state, Some(&layer)) {
        Ok(host) => Arc::new(host),
        Err(message) => {
            tracing::error!(%message, "cannot start the activity journal");
            std::process::exit(2);
        }
    };
    let listener = match TcpListener::bind(config.bind).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!(%err, addr = %config.bind, "cannot bind");
            host.stop();
            std::process::exit(1);
        }
    };
    let (stop_tx, stop_rx) = watch::channel(false);
    let ticker = tokio::spawn(run_ticker(host.clone(), stop_rx));
    tracing::info!(addr = %config.bind, ui_dir = ?config.ui_dir, data_dir = %config.data_dir.display(), "filesquid-server listening");
    let served = axum::serve(listener, app(&config, state))
        .with_graceful_shutdown(shutdown_signal())
        .await;
    let _ = stop_tx.send(true);
    if ticker.await.is_err() {
        tracing::error!("monitor ticker panicked");
    }
    if let Err(err) = tokio::task::spawn_blocking(move || host.stop()).await {
        tracing::error!(%err, "journal shutdown panicked");
    }
    if let Err(err) = served {
        tracing::error!(%err, "server stopped with an error");
        std::process::exit(1);
    }
}

/// Tick the monitor once per [`TICK`] until `stop` turns true. The loop is bounded by the
/// server's lifetime: it ends as soon as shutdown starts.
async fn run_ticker(
    host: Arc<filesquid_api::monitor::HostMonitor>,
    mut stop: watch::Receiver<bool>,
) {
    let mut interval = tokio::time::interval(TICK);
    while !*stop.borrow() {
        tokio::select! {
            _ = interval.tick() => {
                let host = host.clone();
                if tokio::task::spawn_blocking(move || host.tick()).await.is_err() {
                    tracing::error!("monitor tick panicked");
                }
            }
            _ = stop.changed() => {}
        }
    }
}

/// Resolve on Ctrl-C (all platforms) or SIGTERM (Unix).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(%err, "cannot listen for Ctrl-C");
        }
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(err) => tracing::error!(%err, "cannot listen for SIGTERM"),
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("shutdown requested");
}
