//! HTTP-shaped routes of the FileSquid API. Paths are relative to [`crate::API_PREFIX`].
//!
//! The router is built from one route table, so [`routed_paths`] lists exactly what
//! [`router`] serves. The contract drift test compares that list with the paths documented
//! in the OpenAPI contract in both directions (T-048).
//!
//! Every table entry names the [`Action`] it performs, and [`router`] wraps every entry in
//! [`auth::require`], so a route cannot be served without being authorised first
//! (API-S-004). [`route_actions`] lists the actions for the contract's `x-filesquid-action`.

use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderMap;
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, MethodRouter};
use axum::{Json, Router};
use filesquid_core::{health, HealthStatus};

use crate::auth::{self, Action, Guard};
use crate::error::{ApiError, ErrorCode};
use crate::{ApiState, API_PREFIX, CONTRACT};

/// One route: path relative to [`API_PREFIX`], its method handlers and the action they perform.
pub(crate) type Route = (&'static str, MethodRouter<ApiState>, Action);

/// Every route the API serves, relative to [`API_PREFIX`].
fn route_table() -> Vec<Route> {
    let mut table: Vec<Route> = vec![
        ("/health", get(health_handler), Action::ReadHealth),
        // Public on purpose: a liveness probe that says only that the process is up.
        ("/health/live", get(liveness_handler), Action::Public),
        ("/openapi.json", get(contract_handler), Action::ReadContract),
    ];
    table.extend(crate::admin::routes());
    table
}

/// Wrap `handlers` so they run only after the caller is authorised for `action`.
fn guarded(
    state: &ApiState,
    handlers: MethodRouter<ApiState>,
    action: Action,
) -> MethodRouter<ApiState> {
    let guard = Guard::new(state.clone(), action);
    handlers.route_layer(from_fn_with_state(guard, auth::require))
}

/// Build the FileSquid API router. Hosts mount it unchanged.
///
/// Every route is authorised by [`auth::require`] before its handler runs, with the
/// authoriser installed in `state` (deny-by-default: none installed refuses every call).
/// Under [`API_PREFIX`], an unknown path answers a `not_found` [`ApiError`] and a known path
/// with the wrong method answers a `method_not_allowed` [`ApiError`], but only to an
/// authenticated caller; anyone else gets the `unauthorized` or `forbidden` denial. The
/// fallbacks sit on the nested router, so a host's own fallback (the server's static UI)
/// still answers every path outside [`API_PREFIX`].
pub fn router(state: ApiState) -> Router {
    let api = route_table()
        .into_iter()
        .fold(Router::new(), |api, (path, handlers, action)| {
            api.route(path, guarded(&state, handlers, action))
        })
        // After the routes: axum applies this to the method routers registered so far.
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .with_state(state);
    Router::new().nest(API_PREFIX, api)
}

/// Fallback for an unknown path under [`API_PREFIX`]: `not_found` for a known caller.
async fn not_found(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    if let Err(denial) = auth::check_caller(&state, &headers).await {
        return denial.into_response();
    }
    ApiError::new(ErrorCode::NotFound, "This API path does not exist.").into_response()
}

/// Fallback for a known path called with a method it does not accept: `method_not_allowed`
/// for a known caller.
async fn method_not_allowed(State(state): State<ApiState>, headers: HeaderMap) -> Response {
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

/// `GET /api/v1/openapi.json`: the API contract this build serves.
async fn contract_handler() -> impl IntoResponse {
    ([(CONTENT_TYPE, "application/json")], CONTRACT)
}
