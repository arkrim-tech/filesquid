//! HTTP-shaped routes of the FileSquid API. Paths are relative to [`crate::API_PREFIX`].
//!
//! The router is built from one route table, so [`routed_paths`] lists exactly what
//! [`router`] serves. The contract drift test compares that list with the paths documented
//! in the OpenAPI contract in both directions (T-048).
//!
//! Every table entry names the [`Action`] it performs, and [`router`] wraps every entry in
//! [`auth::require`], so a route cannot be served without being authorised first
//! (API-S-004). Two routes are outside the Bearer guard: the public liveness probe
//! `GET /api/v1/health/live` ([`Action::Public`], API-S-015), and the content route
//! `/api/v1/content/{handle}` ([`Action::ReadContent`], API-S-018), whose handler accepts the
//! content handle in its path, and checks its minting credential's permission, before
//! anything else. Building the router panics if any other path declares either action. [`route_actions`] lists the actions for the
//! contract's `x-filesquid-action`.

use axum::extract::{OriginalUri, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, Method};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, MethodRouter};
use axum::{Json, Router};
use filesquid_core::{health, HealthStatus};

use crate::auth::{self, Action, Guard};
use crate::content;
use crate::error::{ApiError, ErrorCode};
use crate::{ApiState, API_PREFIX, CONTRACT};

/// One route: path relative to [`API_PREFIX`], its method handlers and the action they perform.
pub(crate) type Route = (&'static str, MethodRouter<ApiState>, Action);

/// Every route the API serves, relative to [`API_PREFIX`].
fn route_table() -> Vec<Route> {
    let mut table: Vec<Route> = vec![
        ("/health", get(health_handler), Action::ReadHealth),
        // Public on purpose: a liveness probe that says only that the process is up.
        (LIVENESS_PATH, get(liveness_handler), Action::Public),
        ("/openapi.json", get(contract_handler), Action::ReadContract),
        ("/host", get(host_handler), Action::ReadHost),
    ];
    table.extend(crate::admin::routes());
    table.extend(crate::files::routes());
    table.push(crate::content::mint_route());
    table.push(crate::content::content_routes());
    table.extend(crate::history::routes::routes());
    table
}

/// The one path, relative to [`API_PREFIX`], that may be public: the liveness probe.
pub(crate) const LIVENESS_PATH: &str = "/health/live";

/// Wrap `handlers` so they run only after the caller is authorised for `action`.
///
/// The one exception is the liveness probe: its entry carries [`Action::Public`] and is left
/// unguarded on purpose. Any other path declaring [`Action::Public`] panics while the router
/// is built, so the exception cannot spread by mistake; [`auth::require`] itself never
/// bypasses authentication.
fn guarded(
    state: &ApiState,
    path: &str,
    handlers: MethodRouter<ApiState>,
    action: Action,
) -> MethodRouter<ApiState> {
    if action == Action::Public {
        assert!(
            path == LIVENESS_PATH,
            "only the liveness probe may be public, not {path}"
        );
        return handlers;
    }
    if action == Action::ReadContent {
        // The handle in the path is this route's only credential: its handlers accept the
        // handle before anything else (API-S-018), and no Bearer credential is asked for.
        assert!(
            path == crate::content::CONTENT_PATH_TEMPLATE,
            "only the content route is guarded by a content handle, not {path}"
        );
        return handlers;
    }
    let guard = Guard::new(state.clone(), action);
    handlers.route_layer(from_fn_with_state(guard, auth::require))
}

/// Build the FileSquid API router. Hosts mount it unchanged.
///
/// Every route except the public liveness probe (`GET /api/v1/health/live`, API-S-015) is
/// authorised by [`auth::require`] before its handler runs, with the authoriser installed in
/// `state` (deny-by-default: none installed refuses every guarded call).
/// Under [`API_PREFIX`], an unknown path answers a `not_found` [`ApiError`] and a known path
/// with the wrong method answers a `method_not_allowed` [`ApiError`], but only to an
/// authenticated caller; anyone else gets the `unauthorized` or `forbidden` denial. The
/// fallbacks sit on the nested router, so a host's own fallback (the server's static UI)
/// still answers every path outside [`API_PREFIX`].
pub fn router(state: ApiState) -> Router {
    let api = route_table()
        .into_iter()
        .fold(Router::new(), |api, (path, handlers, action)| {
            api.route(path, guarded(&state, path, handlers, action))
        })
        // After the routes: axum applies this to the method routers registered so far.
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .with_state(state);
    Router::new().nest(API_PREFIX, api)
}

/// Fallback for an unknown path under [`API_PREFIX`]: `not_found` for a known caller. A path
/// under the content route gets the content-handle denial instead, whoever calls
/// (API-S-018).
async fn not_found(
    State(state): State<ApiState>,
    method: Method,
    uri: OriginalUri,
    headers: HeaderMap,
) -> Response {
    if content::is_content_path(uri.path()) {
        return content::refuse_content(&state, method == Method::HEAD).await;
    }
    if let Err(denial) = auth::check_caller(&state, &headers).await {
        return denial.into_response();
    }
    ApiError::new(ErrorCode::NotFound, "This API path does not exist.").into_response()
}

/// Fallback for a known path called with a method it does not accept: `method_not_allowed`
/// for a known caller, or the content-handle denial under the content route.
async fn method_not_allowed(
    State(state): State<ApiState>,
    method: Method,
    uri: OriginalUri,
    headers: HeaderMap,
) -> Response {
    if content::is_content_path(uri.path()) {
        return content::refuse_content(&state, method == Method::HEAD).await;
    }
    if let Err(denial) = auth::check_caller(&state, &headers).await {
        return denial.into_response();
    }
    let message = "This API path does not accept that method.";
    ApiError::new(ErrorCode::MethodNotAllowed, message).into_response()
}

/// Full paths (with [`API_PREFIX`]) of every route [`router`] serves, in table order.
///
/// Host-provided routes, such as the server's WebSocket event channel, are not included;
/// the contract marks those with `x-filesquid-host-route: true`.
pub fn routed_paths() -> Vec<String> {
    route_actions().into_iter().map(|(path, _)| path).collect()
}

/// Full paths of every route [`router`] serves, with the [`Action`] each one is authorised
/// for, in table order. The contract names the same action in `x-filesquid-action`.
pub fn route_actions() -> Vec<(String, Action)> {
    route_table()
        .into_iter()
        .map(|(path, _, action)| (format!("{API_PREFIX}{path}"), action))
        .collect()
}

/// Body of the public liveness probe: only that the process is up. No version, component
/// detail or journal, so an anonymous caller learns nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Liveness {
    status: &'static str,
}

impl Liveness {
    /// The only answer: `{"status":"up"}`.
    pub fn up() -> Self {
        Self { status: "up" }
    }
}

/// `GET /api/v1/health/live`: public liveness probe (no credential needed).
async fn liveness_handler() -> Json<Liveness> {
    Json(Liveness::up())
}

/// `GET /api/v1/health`: process health from `filesquid-core`.
async fn health_handler() -> Json<HealthStatus> {
    Json(health())
}

/// `GET /api/v1/host`: the host descriptor (API-S-020), or a retryable `unavailable` error
/// until the host has described itself. A successful read is not journaled (API-S-017).
async fn host_handler(State(state): State<ApiState>) -> Response {
    match state.host() {
        Some(host) => Json(host.descriptor()).into_response(),
        None => {
            let message = "The host has not described itself yet; try again shortly.";
            ApiError::new(ErrorCode::Unavailable, message).into_response()
        }
    }
}

/// `GET /api/v1/openapi.json`: the API contract this build serves.
async fn contract_handler() -> impl IntoResponse {
    ([(CONTENT_TYPE, "application/json")], CONTRACT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "only the liveness probe may be public")]
    fn a_route_other_than_the_liveness_probe_cannot_be_public() {
        let _ = guarded(
            &ApiState::new(),
            "/health",
            get(health_handler),
            Action::Public,
        );
    }

    #[test]
    fn the_liveness_probe_is_the_one_public_path() {
        let _ = guarded(
            &ApiState::new(),
            LIVENESS_PATH,
            get(liveness_handler),
            Action::Public,
        );
        assert_eq!(LIVENESS_PATH, "/health/live");
    }
}
