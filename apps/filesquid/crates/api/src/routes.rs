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
pub fn router(state: ApiState) -> Router {
    let api = route_table()
        .into_iter()
        .fold(Router::new(), |api, (path, handlers)| {
            api.route(path, handlers)
        })
        .with_state(state);
    Router::new().nest(API_PREFIX, api)
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
