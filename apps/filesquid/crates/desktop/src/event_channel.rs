//! The desktop event channel: API bus events pushed to the app's own webviews.
//!
//! ADR 030 Decision 4 gives the desktop one event channel behind the same `FileSquidClient`
//! interface as the server's WebSocket (DE-S-005). Transport spike S1 (b) adopted Tauri
//! events named [`EVENT_NAME`], which `@filesquid/ui` subscribes to, so no loopback port
//! exists (ADR 002, T-049).
//!
//! The host opens **one** counted [`EventBus::subscription`] for its webviews and forwards
//! every event it yields, `events.resync` hints included, so a window that fell behind or
//! missed an oversized event is told to reread (the limits are the bus's: 256 buffered
//! events, 65,536 bytes per event, 64 subscriptions). The forwarder registers in the process
//! registry as [`FORWARDER_KIND`] and heartbeats every [`FORWARDER_HEARTBEAT`].
//!
//! **Only the app's own webviews.** Tauri delivers an event to every webview that listens
//! for it. The host therefore keeps every webview on the app's own origins: navigation
//! anywhere else is refused ([`navigation_allowed`]), so every listener is an app page. The
//! default capability grants event listening to the main window's app pages only.

use crate::api_protocol::app_origins;
use filesquid_api::registry::{ProcessHandle, ProcessRegistry, ProcessSpec};
use filesquid_api::{ApiEvent, EventBus, EventSubscription};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::Url;

/// Tauri event name that carries every [`ApiEvent`] to the UI (`DESKTOP_EVENT` in
/// `@filesquid/ui`).
pub const EVENT_NAME: &str = "filesquid-event";

/// Process-registry kind of the event forwarder.
pub const FORWARDER_KIND: &str = "event-forwarder";

/// Interval between the forwarder's heartbeats and stop-flag checks.
pub const FORWARDER_HEARTBEAT: Duration = Duration::from_secs(5);

/// The forwarder's stall timeout in milliseconds (the host process uses the same).
pub const FORWARDER_TIMEOUT_MS: i64 = 30_000;

/// Journal operation when the bus refuses the forwarder's subscription.
const SUBSCRIPTION_OPERATION: &str = "desktop.events.subscription";

/// Journal operation when an event cannot be handed to the webviews.
const FORWARD_OPERATION: &str = "desktop.events.forward";

/// Journal operation when a webview is kept from leaving the app's origins.
const NAVIGATION_OPERATION: &str = "desktop.navigation.refused";

/// Where forwarded events go: the app's webviews in the running app, a recorder in tests.
pub trait EventSink {
    /// Hand one event to the webviews. Performs I/O (a webview script evaluation in the app).
    ///
    /// # Errors
    /// A description of why the event could not be delivered; the forwarder journals the
    /// failure and goes on with the next event.
    fn deliver(&self, event: &ApiEvent) -> Result<(), String>;
}

/// The running app's sink: emits each event as the Tauri event [`EVENT_NAME`].
pub struct TauriSink(pub tauri::AppHandle);

impl EventSink for TauriSink {
    fn deliver(&self, event: &ApiEvent) -> Result<(), String> {
        use tauri::Emitter;
        self.0
            .emit(EVENT_NAME, event)
            .map_err(|err| err.to_string())
    }
}

/// How a [`pump`] runs: when it stops, how often it heartbeats, and its registry entry.
pub struct Forwarding {
    /// Set at app exit; the pump checks it at every heartbeat and then ends.
    pub stop: Arc<AtomicBool>,
    /// Interval between heartbeats and stop checks ([`FORWARDER_HEARTBEAT`] in the app).
    pub heartbeat: Duration,
    /// The forwarder's process, heartbeated while it runs and completed when it ends.
    pub process: Option<Arc<ProcessHandle>>,
}

/// Why a [`pump`] ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PumpEnd {
    /// The event bus was dropped; no event can follow.
    BusClosed,
    /// The stop flag was set at app exit.
    Stopped,
}

/// What a [`pump`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PumpReport {
    /// Events handed to the sink.
    pub delivered: u64,
    /// Events the sink could not deliver (each journaled as `desktop.events.forward`).
    pub failed: u64,
    /// Why the pump ended.
    pub end: PumpEnd,
}

/// Open the forwarder's counted subscription on `bus`. When the bus already serves its
/// maximum number of subscriptions, journal the refusal as `desktop.events.subscription`
/// (error code `unavailable`) and return `None`; the API keeps working without push.
pub fn open_subscription(bus: &EventBus) -> Option<EventSubscription> {
    match bus.subscription() {
        Ok(subscription) => Some(subscription),
        Err(error) => {
            // `EventBus::subscription` refuses only with the retryable `unavailable` code.
            tracing::error!(
                operation = SUBSCRIPTION_OPERATION,
                error_code = "unavailable",
                detail = error.message(),
                "desktop event channel could not subscribe"
            );
            None
        }
    }
}

/// Register the forwarder in `registry` as [`FORWARDER_KIND`] with [`FORWARDER_TIMEOUT_MS`].
pub fn register_forwarder(registry: &ProcessRegistry) -> ProcessHandle {
    registry.register(ProcessSpec::new(FORWARDER_KIND, FORWARDER_TIMEOUT_MS))
}

/// Forward every event `subscription` yields to `sink`, in order, until the bus closes or the
/// stop flag is set. Bounded by the bus's and the app's lifetimes: each wait ends with an
/// event, a closed bus, or a heartbeat at which the stop flag is checked. Completes the
/// forwarder's process when it ends.
pub async fn pump<S: EventSink>(
    mut subscription: EventSubscription,
    sink: &S,
    forwarding: Forwarding,
) -> PumpReport {
    let mut report = PumpReport {
        delivered: 0,
        failed: 0,
        end: PumpEnd::Stopped,
    };
    let mut beat = tokio::time::interval(forwarding.heartbeat);
    while !forwarding.stop.load(Ordering::SeqCst) {
        tokio::select! {
            next = subscription.next() => match next {
                Some(event) => forward_one(sink, &event, &mut report),
                None => {
                    report.end = PumpEnd::BusClosed;
                    break;
                }
            },
            _ = beat.tick() => heartbeat(forwarding.process.as_ref()),
        }
    }
    if let Some(process) = &forwarding.process {
        process.complete();
    }
    report
}

/// Deliver one event and count the outcome; a failure is journaled without its detail.
fn forward_one<S: EventSink>(sink: &S, event: &ApiEvent, report: &mut PumpReport) {
    match sink.deliver(event) {
        Ok(()) => report.delivered += 1,
        Err(detail) => {
            report.failed += 1;
            tracing::error!(
                operation = FORWARD_OPERATION,
                error_code = "emit_failed",
                detail,
                "desktop event could not reach the webviews"
            );
        }
    }
}

/// Report that the forwarder is alive.
fn heartbeat(process: Option<&Arc<ProcessHandle>>) {
    if let Some(process) = process {
        process.heartbeat();
    }
}

/// True when `url` is a page of one of the app's own origins ([`app_origins`]). Pure.
pub fn is_app_url(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let origin = match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    };
    app_origins().contains(&origin.as_str())
}

/// Decide whether app webview `label` may navigate to `url`: app pages and blank documents
/// (`about:blank`, `about:srcdoc`) only. A refusal is journaled as
/// `desktop.navigation.refused` with the actor `webview:<label>` and error code
/// `foreign_origin`, never with the address.
pub fn navigation_allowed(label: &str, url: &Url) -> bool {
    let blank = url.scheme() == "about" && matches!(url.path(), "blank" | "srcdoc");
    if blank || is_app_url(url) {
        return true;
    }
    tracing::warn!(
        operation = NAVIGATION_OPERATION,
        outcome = "failed",
        actor = format!("webview:{label}").as_str(),
        error_code = "foreign_origin",
        "app webview kept from leaving the app's origins"
    );
    false
}
