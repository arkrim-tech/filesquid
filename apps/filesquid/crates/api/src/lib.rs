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
//! in-process [`EventBus`] that hosts forward to UIs. It holds no transport code of its own,
//! so the same router answers on every host.

pub mod admin;
pub mod error;
mod events;
pub mod health;
pub mod journal;
pub mod journal_writer;
pub mod metrics;
pub mod monitor;
pub mod operation;
pub mod registry;
mod routes;
pub mod trace_layer;

pub use error::{ApiError, ErrorCode};
pub use events::{ApiEvent, EventBus};
pub use routes::{routed_paths, router};

use std::sync::{Arc, OnceLock};

/// Version of the FileSquid API contract. Bumped when the contract changes.
pub const API_VERSION: &str = "0.3.0";

/// URL prefix shared by every API route.
pub const API_PREFIX: &str = "/api/v1";

/// The checked-in OpenAPI 3.1 contract. TypeScript types are generated from this file.
pub const CONTRACT: &str = include_str!("../openapi.json");

/// Shared state handed to every request handler.
///
/// Cloning is cheap: the event bus and the monitor slot are reference-counted.
#[derive(Clone)]
pub struct ApiState {
    /// Bus for push events (job progress, change notifications, journal entries).
    pub events: EventBus,
    /// The host's monitor, installed once at startup (admin routes answer 503 until then).
    monitor: Arc<OnceLock<monitor::Monitor>>,
}

impl std::fmt::Debug for ApiState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiState")
            .field("monitor_installed", &self.monitor.get().is_some())
            .finish()
    }
}

impl ApiState {
    /// Create state with a fresh event bus and no monitor.
    pub fn new() -> Self {
        Self {
            events: EventBus::new(),
            monitor: Arc::new(OnceLock::new()),
        }
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
