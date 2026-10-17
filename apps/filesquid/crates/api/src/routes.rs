//! HTTP-shaped routes of the FileSquid API. Paths are relative to [`crate::API_PREFIX`].
//!
//! The router is built from one route table, so [`routed_paths`] lists exactly what
//! [`router`] serves. The contract drift test compares that list with the paths documented
//! in the OpenAPI contract in both directions (T-048).

use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::routing::{get, MethodRouter};
use axum::{Json, Router};
use filesquid_core::{health, HealthStatus};

use crate::error::{ApiError, ErrorCode};
use crate::{ApiState, API_PREFIX, CONTRACT};

/// One route: path relative to [`API_PREFIX`] and its method handlers.
pub(crate) type Route = (&'static str, MethodRouter<ApiState>);

/// Every route the API serves, relative to [`API_PREFIX`].
fn route_table() -> Vec<Route> {
    let mut table: Vec<Route> = vec![
        ("/health", get(health_handler)),
        ("/openapi.json", get(contract_handler)),
    ];
    table.extend(crate::admin::routes());
    table
}

/// Build the FileSquid API router. Hosts mount it unchanged.
///
/// Under [`API_PREFIX`], an unknown path answers a `not_found` [`ApiError`] and a known path
/// with the wrong method answers a `method_not_allowed` [`ApiError`]. The fallbacks sit on the
/// nested router, so a host's own fallback (the server's static UI) still answers every path
/// outside [`API_PREFIX`].
pub fn router(state: ApiState) -> Router {
    let api = route_table()
        .into_iter()
        .fold(Router::new(), |api, (path, handlers)| {
            api.route(path, handlers)
        })
        // After the routes: axum applies this to the method routers registered so far.
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .with_state(state);
    Router::new().nest(API_PREFIX, api)
}

/// Fallback for an unknown path under [`API_PREFIX`].
async fn not_found() -> ApiError {
    ApiError::new(ErrorCode::NotFound, "This API path does not exist.")
}

/// Fallback for a known path called with a method it does not accept.
async fn method_not_allowed() -> ApiError {
    ApiError::new(
        ErrorCode::MethodNotAllowed,
        "This API path does not accept that method.",
    )
}

/// Full paths (with [`API_PREFIX`]) of every route [`router`] serves, in table order.
///
/// Host-provided routes, such as the server's WebSocket event channel, are not included;
/// the contract marks those with `x-filesquid-host-route: true`.
pub fn routed_paths() -> Vec<String> {
    route_table()
        .into_iter()
        .map(|(path, _)| format!("{API_PREFIX}{path}"))
        .collect()
}

/// `GET /api/v1/health`: process health from `filesquid-core`.
async fn health_handler() -> Json<HealthStatus> {
    Json(health())
}

/// `GET /api/v1/openapi.json`: the API contract this build serves.
async fn contract_handler() -> impl IntoResponse {
    ([(CONTENT_TYPE, "application/json")], CONTRACT)
}
