//! The single FileSquid API.
//!
//! Every FileSquid feature is reachable through this API, and every host serves it
//! ([ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)):
//!
//! - `filesquid-desktop` mounts [`router`] behind a Tauri custom URI scheme (no TCP port);
//! - `filesquid-server` mounts it on Axum over HTTP and adds a WebSocket event route.
//!
//! The crate owns the versioned contract ([`CONTRACT`], an OpenAPI 3.1 document with the
//! [`operation`] schemas and the typed [`ApiError`] body) and the
//! in-process [`EventBus`] that hosts forward to UIs ([`events`]: documented topics, bounded
//! subscriptions and resync hints). It holds no transport code of its own, so the same router
//! answers on every host.
//!
//! Every routed operation except the public liveness probe (`GET /api/v1/health/live`) is
//! authenticated and authorised by the [`auth`] layer before its handler runs. The 404 and 405
//! fallbacks are authentication-only: they answer only a caller with an accepted credential
//! and apply no policy, because no operation runs. All of it is deny-by-default: a host
//! installs its [`auth::Authoriser`] with [`ApiState::install_authoriser`] (the desktop
//! per-launch token, or the server administrator token), and without one every guarded call
//! is refused.

pub mod admin;
pub mod auth;
pub mod error;
pub mod events;
pub mod health;
pub mod journal;
pub mod journal_writer;
pub mod metrics;
pub mod monitor;
pub mod operation;
pub mod registry;
mod routes;
pub mod trace_layer;

pub use error::{ApiError, DenialReason, ErrorCode};
pub use events::{ApiEvent, EventBus, EventSubscription};
pub use routes::{route_actions, routed_paths, router, Liveness};

use std::sync::{Arc, OnceLock};

/// Version of the FileSquid API contract. Bumped when the contract changes.
pub const API_VERSION: &str = "0.5.0";

/// URL prefix shared by every API route.
pub const API_PREFIX: &str = "/api/v1";

/// The checked-in OpenAPI 3.1 contract. TypeScript types are generated from this file.
pub const CONTRACT: &str = include_str!("../openapi.json");

/// Shared state handed to every request handler.
///
/// Cloning is cheap: the event bus and the monitor and authoriser slots are
/// reference-counted.
#[derive(Clone)]
pub struct ApiState {
    /// Bus for push events (job progress, change notifications, journal entries).
    pub events: EventBus,
    /// The host's monitor, installed once at startup (admin routes answer 503 until then).
    monitor: Arc<OnceLock<monitor::Monitor>>,
    /// The host's authoriser, installed once at startup (every guarded call is refused until then).
    authoriser: Arc<OnceLock<auth::Authoriser>>,
}

impl std::fmt::Debug for ApiState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiState")
            .field("monitor_installed", &self.monitor.get().is_some())
            .field("authoriser_installed", &self.authoriser.get().is_some())
            .finish()
    }
}

impl ApiState {
    /// Create state with a fresh event bus, no monitor and no authoriser (every guarded call is refused; the public liveness
    /// probe still answers).
    pub fn new() -> Self {
        Self {
            events: EventBus::new(),
            monitor: Arc::new(OnceLock::new()),
            authoriser: Arc::new(OnceLock::new()),
        }
    }

    /// Install the host's authoriser. Returns false, keeping the first, if one was already
    /// installed, so a credential cannot be swapped while the host runs.
    pub fn install_authoriser(&self, authoriser: auth::Authoriser) -> bool {
        self.authoriser.set(authoriser).is_ok()
    }

    /// The installed authoriser, or one that refuses every guarded call when none is installed.
    pub fn authoriser(&self) -> auth::Authoriser {
        self.authoriser
            .get()
            .cloned()
            .unwrap_or_else(auth::Authoriser::deny_all)
    }

    /// Install the host's monitor. Returns false if one was already installed.
    pub fn install_monitor(&self, monitor: monitor::Monitor) -> bool {
        self.monitor.set(monitor).is_ok()
    }

    /// The installed monitor, if any.
    pub fn monitor(&self) -> Option<monitor::Monitor> {
        self.monitor.get().cloned()
    }
}

impl Default for ApiState {
    fn default() -> Self {
        Self::new()
    }
}
