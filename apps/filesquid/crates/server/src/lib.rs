//! `filesquid-server`: the FileSquid server host.
//!
//! Serves the single FileSquid API ([`filesquid_api::router`]) over HTTP, pushes API events
//! to WebSocket clients at `GET /api/v1/events`, exposes Prometheus metrics, and serves one
//! distribution's built UI as static files ([ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md),
//! [ADR 032](../../../../../resources/architecture/032-hosting-and-privacy-model.md)).
//!
//! This host is for servers that the owner or their organisation operates: its operator
//! can read the files it stores. It MUST NOT be used as the ciphertext-only hosted service.
//!
//! Every API route, the event channel and `/metrics` are authorised deny-by-default by the
//! [`filesquid_api::auth`] layer with the administrator token
//! ([`install_admin_authoriser`]) until server accounts arrive (API-F1.9). The static UI is
//! public: it is a client-rendered build with no private content (ADR 031).

pub mod admin_token;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::middleware::from_fn_with_state;
use axum::response::Response;
use axum::routing::{get, MethodRouter};
use axum::Router;
use filesquid_api::auth::{self, Action, Authoriser, Guard, Principal, EVENTS_SUBPROTOCOL};
use filesquid_api::monitor::HostMonitor;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::{ApiError, ApiState, ErrorCode, API_PREFIX};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::sync::broadcast::error::RecvError;
use tower_http::services::{ServeDir, ServeFile};

/// Default listen address: loopback only, so a fresh install is not exposed on the network.
pub const DEFAULT_BIND: &str = "127.0.0.1:8083";

/// Default data directory (operational store and admin token), relative to the working directory.
pub const DEFAULT_DATA_DIR: &str = "filesquid-data";

/// Runtime configuration for the server host. `Debug` output redacts the admin token.
#[derive(Clone)]
pub struct ServerConfig {
    /// Address to listen on (`FILESQUID_BIND`).
    pub bind: SocketAddr,
    /// Directory of a built distribution UI to serve at `/` (`FILESQUID_UI_DIR`). `None` serves the API only.
    pub ui_dir: Option<PathBuf>,
    /// Directory for the operational store and the admin token (`FILESQUID_DATA_DIR`).
    pub data_dir: PathBuf,
    /// Administrator token required by every API call (`FILESQUID_ADMIN_TOKEN`); generated into the data dir when absent.
    pub admin_token: Option<String>,
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("bind", &self.bind)
            .field("ui_dir", &self.ui_dir)
            .field("data_dir", &self.data_dir)
            .field(
                "admin_token",
                &self.admin_token.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND
                .parse()
                .expect("DEFAULT_BIND is a valid socket address"),
            ui_dir: None,
            data_dir: PathBuf::from(DEFAULT_DATA_DIR),
            admin_token: None,
        }
    }
}

impl ServerConfig {
    /// Build configuration from a variable lookup (for example `std::env::var`).
    ///
    /// Returns an error message naming the variable when `FILESQUID_BIND` is not `host:port`.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let bind_text = lookup("FILESQUID_BIND").unwrap_or_else(|| DEFAULT_BIND.to_string());
        let bind = bind_text
            .parse()
            .map_err(|err| format!("FILESQUID_BIND must be host:port, got {bind_text:?}: {err}"))?;
        let ui_dir = lookup("FILESQUID_UI_DIR").map(PathBuf::from);
        let data_dir = lookup("FILESQUID_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));
        let admin_token = lookup("FILESQUID_ADMIN_TOKEN");
        Ok(Self {
            bind,
            ui_dir,
            data_dir,
            admin_token,
        })
    }
}

/// Resolve the administrator token (configured, stored, or generated into
/// `<data_dir>/admin-token`) and install the administrator authoriser in `state`. Until this
/// runs, every call is refused. Returns whether a new token was issued (generated) by this
/// call. Performs filesystem I/O. The token itself is never logged.
pub fn install_admin_authoriser(config: &ServerConfig, state: &ApiState) -> Result<bool, String> {
    let configured = config.admin_token.as_deref();
    let (token, created) = admin_token::load_or_create_token(&config.data_dir, configured)?;
    if created {
        let path = config.data_dir.join(admin_token::TOKEN_FILE);
        tracing::info!(path = %path.display(), "generated the administrator token");
    }
    if !state.install_authoriser(Authoriser::server_administrator(token)) {
        return Err("an authoriser was already installed".to_string());
    }
    Ok(created)
}

/// Start the server's authorisation and monitor in order: install the administrator
/// authoriser, open the monitor, then journal `api.auth.token_issued` for the administrator
/// credential this run accepts, whether it was generated now or loaded (API-S-017).
/// Performs filesystem and database I/O.
pub fn start_server(
    config: &ServerConfig,
    state: &ApiState,
    layer: Option<&JournalLayer>,
) -> Result<HostMonitor, String> {
    install_admin_authoriser(config, state)?;
    let host = start_host_monitor(config, state, layer)?;
    let entry = auth::token_issued_entry(&Principal::Administrator);
    host.monitor().journal().send(entry);
    Ok(host)
}

/// Start the activity journal, process registry and admin routes for this server (FileSquid
/// F-061): opens `<data_dir>/filesquid.sqlite` and installs the monitor in `state`.
/// Performs filesystem and database I/O.
pub fn start_host_monitor(
    config: &ServerConfig,
    state: &ApiState,
    layer: Option<&JournalLayer>,
) -> Result<HostMonitor, String> {
    HostMonitor::open(&config.data_dir, "filesquid-server", state, layer)
}

/// Wrap a host route so it runs only after the caller is authorised for `action`.
///
/// The guard is a `layer`, not a `route_layer`, so it also covers the method router's
/// fallback: an unsupported method is refused like any other call, and only an authorised
/// caller learns that the method is not allowed.
fn guarded(
    state: &ApiState,
    route: MethodRouter<ApiState>,
    action: Action,
) -> MethodRouter<ApiState> {
    let guard = Guard::new(state.clone(), action);
    route
        .fallback(method_not_allowed)
        .layer(from_fn_with_state(guard, auth::require))
}

/// The typed `method_not_allowed` answer for a host route called with another method.
async fn method_not_allowed() -> ApiError {
    ApiError::new(
        ErrorCode::MethodNotAllowed,
        "This path does not accept that method.",
    )
}

/// Compose the server router: API, events, metrics, then the optional static UI.
///
/// The event channel and `/metrics` are guarded like every API route; the static UI is not.
pub fn app(config: &ServerConfig, state: ApiState) -> Router {
    let events_route = guarded(&state, get(events_handler), Action::SubscribeEvents);
    let metrics_route = guarded(&state, get(metrics_handler), Action::ReadMonitor);
    let events = Router::new()
        .route(&format!("{API_PREFIX}/events"), events_route)
        .route("/metrics", metrics_route)
        .with_state(state.clone());
    let router = filesquid_api::router(state).merge(events);
    match &config.ui_dir {
        Some(dir) => router.fallback_service(static_ui(dir)),
        None => router,
    }
}

/// Serve a built UI directory; unknown paths fall back to `index.html` for client-side routing.
fn static_ui(dir: &std::path::Path) -> ServeDir<ServeFile> {
    ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")))
}

/// `GET /api/v1/events`: upgrade to WebSocket and forward every API event as a JSON text frame.
///
/// Runs only after the guard authorised the caller. When the client offered the
/// `filesquid.events.v1` subprotocol (browsers do, next to their `filesquid.bearer.<token>`
/// credential), it is selected; the credential entry is never echoed.
async fn events_handler(ws: WebSocketUpgrade, State(state): State<ApiState>) -> Response {
    ws.protocols([EVENTS_SUBPROTOCOL])
        .on_upgrade(move |socket| forward_events(socket, state))
}

/// Forward bus events to one WebSocket until the client disconnects or the bus closes.
///
/// The socket is polled alongside the bus, so a client that leaves while no events flow
/// still ends this task and drops its bus subscription.
async fn forward_events(mut socket: WebSocket, state: ApiState) {
    let mut events = state.events.subscribe();
    loop {
        let next = tokio::select! {
            incoming = socket.recv() => {
                if client_gone(incoming) {
                    tracing::debug!("event client disconnected");
                    return;
                }
                continue;
            }
            next = events.recv() => next,
        };
        let event = match next {
            Ok(event) => event,
            Err(RecvError::Lagged(skipped)) => {
                tracing::warn!(skipped, "event subscriber lagged; events dropped");
                continue;
            }
            Err(RecvError::Closed) => return,
        };
        let text = match serde_json::to_string(&event) {
            Ok(text) => text,
            Err(err) => {
                tracing::error!(%err, "event serialization failed");
                continue;
            }
        };
        if socket.send(Message::Text(text.into())).await.is_err() {
            tracing::debug!("event client disconnected");
            return;
        }
    }
}

/// True when a WebSocket read shows the client has left: stream ended, read error, or a Close frame.
/// Other client frames are ignored; the event channel is push-only.
fn client_gone(incoming: Option<Result<Message, axum::Error>>) -> bool {
    matches!(incoming, None | Some(Err(_)) | Some(Ok(Message::Close(_))))
}

/// `GET /metrics`: Prometheus text from the same counters as `GET /api/v1/admin/metrics`.
async fn metrics_handler(State(state): State<ApiState>) -> String {
    match state.monitor() {
        Some(monitor) => monitor.metrics().to_prometheus(),
        None => "# HELP filesquid_up 1 while the FileSquid host is running\n# TYPE filesquid_up gauge\nfilesquid_up 1\n".to_string(),
    }
}
