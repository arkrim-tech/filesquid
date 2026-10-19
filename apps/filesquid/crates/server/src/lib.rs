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
//! Every API route except the public liveness probe (`GET /api/v1/health/live`), plus the
//! event channel and `/metrics`, is authorised deny-by-default by the
//! [`filesquid_api::auth`] layer with the administrator token
//! ([`install_admin_authoriser`]) until server accounts arrive (API-F1.9). The static UI is
//! public: it holds no private content, because private content is only ever served through
//! the authorised API (ADR 032, hosting and privacy model).

pub mod admin_token;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::middleware::from_fn_with_state;
use axum::response::Response;
use axum::routing::{get, MethodRouter};
use axum::Router;
use filesquid_api::auth::{self, Action, Authoriser, Guard, Principal, EVENTS_SUBPROTOCOL};
use filesquid_api::host::{HostIdentity, NotEnrolled, OperatorName};
use filesquid_api::journal::{format_rfc3339_ms, now_ms, JournalEntry, Level, Outcome};
use filesquid_api::ocfl_validation::ValidationService;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::{ApiError, ApiState, ErrorCode, API_PREFIX};
use filesquid_native::HostMonitor;
use filesquid_ocfl::storage::FsBackend;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tower_http::services::{ServeDir, ServeFile};

/// Default listen address: loopback only, so a fresh install is not exposed on the network.
pub const DEFAULT_BIND: &str = "127.0.0.1:8083";

/// Environment variable that names the server's operator in the host descriptor (API-S-020).
pub const OPERATOR_NAME_VARIABLE: &str = "FILESQUID_OPERATOR_NAME";

/// Journal operation of the warning written at every start without an operator name.
pub const OPERATOR_UNNAMED_OPERATION: &str = "server.operator_unnamed";

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
    /// Administrator token required by every API call except the public liveness probe
    /// (`FILESQUID_ADMIN_TOKEN`, 64 hexadecimal characters). When absent, Unix servers load
    /// the token stored in the data dir, or create one there if none exists; other platforms
    /// refuse to start.
    pub admin_token: Option<String>,
    /// The operator's display name for the host descriptor (`FILESQUID_OPERATOR_NAME`,
    /// trimmed, 1 to 256 characters, no control characters). `None` when unset or blank.
    pub operator_name: Option<OperatorName>,
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
            .field("operator_name", &self.operator_name)
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
            operator_name: None,
        }
    }
}

impl ServerConfig {
    /// Build configuration from a variable lookup (for example `std::env::var`).
    ///
    /// Returns an error message naming the variable when `FILESQUID_BIND` is not `host:port`,
    /// or when `FILESQUID_OPERATOR_NAME` is too long or has a control character (the message
    /// never contains that value).
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
        let operator_name = operator_name_from(lookup(OPERATOR_NAME_VARIABLE))?;
        Ok(Self {
            bind,
            ui_dir,
            data_dir,
            admin_token,
            operator_name,
        })
    }
}

/// Every environment variable [`ServerConfig::from_env`] reads.
pub const ENV_VARIABLES: [&str; 5] = [
    "FILESQUID_BIND",
    "FILESQUID_UI_DIR",
    "FILESQUID_DATA_DIR",
    "FILESQUID_ADMIN_TOKEN",
    OPERATOR_NAME_VARIABLE,
];

impl ServerConfig {
    /// Build configuration from the process environment: read each of [`ENV_VARIABLES`]
    /// once, refuse any that is set but not valid Unicode ([`unicode_value`]), then apply
    /// [`from_lookup`](Self::from_lookup). Performs environment reads.
    ///
    /// # Errors
    /// A variable that is not valid Unicode, which would otherwise read as unset, or any
    /// [`from_lookup`](Self::from_lookup) error. No message contains a value.
    pub fn from_env() -> Result<Self, String> {
        let mut values = Vec::with_capacity(ENV_VARIABLES.len());
        for variable in ENV_VARIABLES {
            let value = unicode_value(variable, std::env::var_os(variable).as_deref())?;
            values.push((variable, value));
        }
        Self::from_lookup(|key| {
            values
                .iter()
                .find(|(variable, _)| *variable == key)
                .and_then(|(_, value)| value.clone())
        })
    }
}

/// The value of environment `variable` as text: `None` when unset. Pure.
///
/// # Errors
/// The value is set but not valid Unicode. The message names the variable and never the
/// value.
pub fn unicode_value(
    variable: &str,
    value: Option<&std::ffi::OsStr>,
) -> Result<Option<String>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    raw.to_str().map(|text| Some(text.to_string())).ok_or_else(|| {
        format!("{variable} is not usable: its value is not valid Unicode; set it to valid text or leave it unset")
    })
}

/// Parse the configured `FILESQUID_OPERATOR_NAME`: unset or blank is `None`. Pure.
///
/// # Errors
/// A message that names the variable and the rule broken, never the value.
fn operator_name_from(configured: Option<String>) -> Result<Option<OperatorName>, String> {
    let Some(raw) = configured else {
        return Ok(None);
    };
    OperatorName::parse(&raw).map_err(|err| {
        format!(
            "{OPERATOR_NAME_VARIABLE} is not usable: {err}; set 1 to 256 characters with no control characters, or leave it unset"
        )
    })
}

/// The journal record of a start without an operator name: operation
/// [`OPERATOR_UNNAMED_OPERATION`], level warn, the variable's name as target. It holds no
/// value, token, path or identifier.
pub fn operator_unnamed_entry() -> JournalEntry {
    JournalEntry::new(OPERATOR_UNNAMED_OPERATION, Outcome::Info)
        .level(Level::Warn)
        .target(OPERATOR_NAME_VARIABLE)
}

/// Describe this server to the API (API-S-020): host type `server`, the configured operator
/// name, and not enrolled with the hosted service. Without an operator name, write one
/// [`OPERATOR_UNNAMED_OPERATION`] warning line to `stdout` and queue
/// [`operator_unnamed_entry`] on `host`'s activity journal.
///
/// The warning bypasses `tracing` on purpose: the stdout log filter (`RUST_LOG=error` or
/// `off`) cannot hide it, and the journal layer cannot journal it a second time.
///
/// # Errors
/// The API state already describes a host.
pub fn describe_server_host(
    config: &ServerConfig,
    state: &ApiState,
    host: &HostMonitor,
    stdout: &mut dyn Write,
) -> Result<(), String> {
    let identity = HostIdentity::server(config.operator_name.clone());
    if !state.describe_host(identity, Arc::new(NotEnrolled)) {
        return Err("the host was already described".to_string());
    }
    if config.operator_name.is_none() {
        write_operator_unnamed_warning(stdout);
        host.monitor().journal().send(operator_unnamed_entry());
    }
    Ok(())
}

/// The stdout line of the unnamed-operator warning: one JSON object shaped like the server's
/// other log lines (RFC 3339 UTC timestamp, `WARN`, the operation and the variable's name).
/// It holds no value, token or path. Pure.
pub fn operator_unnamed_line(timestamp: &str) -> String {
    let message = format!(
        "{OPERATOR_NAME_VARIABLE} is unset or blank; the host descriptor reports no operator name"
    );
    serde_json::json!({
        "timestamp": timestamp,
        "level": "WARN",
        "fields": {
            "message": message,
            "operation": OPERATOR_UNNAMED_OPERATION,
            "variable": OPERATOR_NAME_VARIABLE,
        },
        "target": "filesquid_server",
    })
    .to_string()
}

/// Write the unnamed-operator warning line to `stdout` and flush it. A failed write is
/// reported through `tracing`; the journal entry is written either way.
fn write_operator_unnamed_warning(stdout: &mut dyn Write) {
    let timestamp = format_rfc3339_ms(now_ms());
    let line = operator_unnamed_line(&timestamp);
    if let Err(err) = writeln!(stdout, "{line}").and_then(|()| stdout.flush()) {
        tracing::error!(%err, operation = OPERATOR_UNNAMED_OPERATION, "stdout write failed");
    }
}

/// Resolve the administrator token (configured, stored, or generated into
/// `<data_dir>/admin-token`) and install the administrator authoriser in `state`. Until this
/// runs, every guarded call is refused. Returns whether a new token was issued (generated) by this
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
/// authoriser, open the monitor, journal `api.auth.token_issued` for the administrator
/// credential this run accepts, whether it was generated now or loaded (API-S-017), then
/// describe the host ([`describe_server_host`]). Performs filesystem and database I/O.
pub fn start_server(
    config: &ServerConfig,
    state: &ApiState,
    layer: Option<&JournalLayer>,
) -> Result<HostMonitor, String> {
    start_server_writing_to(config, state, layer, &mut std::io::stdout())
}

/// [`start_server`], writing its unfiltered startup warnings to `stdout` instead of the
/// process's standard output (tests capture them this way).
pub fn start_server_writing_to(
    config: &ServerConfig,
    state: &ApiState,
    layer: Option<&JournalLayer>,
    stdout: &mut dyn Write,
) -> Result<HostMonitor, String> {
    install_admin_authoriser(config, state)?;
    let host = start_host_monitor(config, state, layer)?;
    auth::record_token_issue(host.monitor(), &Principal::Administrator);
    if let Err(message) = start_validation(config, state, &host) {
        host.stop();
        return Err(message);
    }
    if let Err(message) = describe_server_host(config, state, &host, stdout) {
        host.stop();
        return Err(message);
    }
    Ok(host)
}

/// Start OCFL validation (API-S-022) over the findings store in the data directory and
/// answer the service. The server has no OCFL library model yet (its libraries arrive with
/// SV-F1.2), so it opens no library: the findings route answers the administrator an empty
/// list with the settings, and the settings route works. A later task keeps the service this
/// answers and opens each server library with
/// [`open_library`](filesquid_api::ocfl_validation::ValidationService::open_library).
/// Performs database I/O.
///
/// # Errors
/// The findings store could not be opened.
pub fn start_validation(
    config: &ServerConfig,
    state: &ApiState,
    host: &HostMonitor,
) -> Result<ValidationService<FsBackend>, String> {
    host.start_validation::<FsBackend>(&config.data_dir, state)
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
