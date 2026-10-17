//! The authorisation layer: Axum middleware that runs before every handler, and the journal
//! records of its decisions.
//!
//! [`require`] guards one route with one [`Action`]. The API router applies it to every route
//! in its route table, and hosts apply it to the routes they serve themselves (the server's
//! event channel and `/metrics`). The router's 404 and 405 fallbacks call [`check_caller`]
//! first, so an anonymous caller cannot probe which paths exist.

use super::credentials::presented_token;
use super::policy::{Action, Principal, Resource};
use crate::error::{ApiError, DenialReason};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::ApiState;
use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Journal operation of every refused call.
pub const DENIED_OPERATION: &str = "api.auth.denied";

/// Journal operation recorded when a host issues a credential at startup.
pub const TOKEN_ISSUED_OPERATION: &str = "api.auth.token_issued";

/// Journal actor of a call that presented no accepted credential.
pub const ANONYMOUS_ACTOR: &str = "anonymous";

/// Middleware state for [`require`]: the API state and the action the guarded route performs.
#[derive(Clone, Debug)]
pub struct Guard {
    state: ApiState,
    action: Action,
}

impl Guard {
    /// Guard a route that performs `action`, using the authoriser installed in `state`.
    pub fn new(state: ApiState, action: Action) -> Self {
        Self { state, action }
    }
}

/// Axum middleware: authenticate the caller, authorise [`Guard`]'s action on the host, and
/// only then run the route. A refused call answers the typed [`ApiError`] denial and is
/// journaled; the route never runs. On success the [`Principal`] is added to the request
/// extensions for the handler.
///
/// Use with `axum::middleware::from_fn_with_state(Guard::new(state, action), require)` as a
/// `route_layer`.
pub async fn require(State(guard): State<Guard>, mut request: Request, next: Next) -> Response {
    let checked = authorise_call(&guard.state, request.headers(), guard.action).await;
    match checked {
        Ok(principal) => {
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        Err(denial) => denial.into_response(),
    }
}

/// Authenticate and authorise one call for `action` on the host. Journals a refusal.
pub async fn authorise_call(
    state: &ApiState,
    headers: &HeaderMap,
    action: Action,
) -> Result<Principal, ApiError> {
    let authoriser = state.authoriser();
    let principal = match authoriser.authenticate(presented_token(headers)) {
        Ok(principal) => principal,
        Err(reason) => return Err(deny(state, reason, None, Some(action)).await),
    };
    match authoriser.authorise(&principal, action, &Resource::Host) {
        Ok(()) => Ok(principal),
        Err(reason) => Err(deny(state, reason, Some(&principal), Some(action)).await),
    }
}

/// Authenticate a call that matched no route (the router's 404 and 405 fallbacks), so those
/// answers are given only to a known caller. Journals a refusal.
pub async fn check_caller(state: &ApiState, headers: &HeaderMap) -> Result<Principal, ApiError> {
    let authenticated = state.authoriser().authenticate(presented_token(headers));
    match authenticated {
        Ok(principal) => Ok(principal),
        Err(reason) => Err(deny(state, reason, None, None).await),
    }
}

/// Log a refusal to stdout, journal it, and return the typed denial.
async fn deny(
    state: &ApiState,
    reason: DenialReason,
    principal: Option<&Principal>,
    action: Option<Action>,
) -> ApiError {
    let actor = principal.map_or(ANONYMOUS_ACTOR, Principal::actor);
    let action_name = action.map_or("none", Action::as_str);
    // INFO under a non-activity target: stdout only, so the journal gets exactly one record.
    tracing::info!(
        target: "filesquid::auth",
        reason = reason.as_str(),
        action = action_name,
        actor,
        "API call refused"
    );
    journal_entry(state, denial_entry(reason, actor, action)).await;
    ApiError::denied(reason)
}

/// The journal record of one refusal: actor, reason as error code, action as target. It has
/// no field that could carry the presented credential.
pub fn denial_entry(reason: DenialReason, actor: &str, action: Option<Action>) -> JournalEntry {
    let entry = JournalEntry::new(DENIED_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .actor(actor)
        .error_code(reason.as_str());
    match action {
        Some(action) => entry.target(action.as_str()),
        None => entry,
    }
}

/// The journal record of a host issuing a credential for `principal` at startup (the desktop
/// per-launch token). It names the principal only, never the secret.
pub fn token_issued_entry(principal: &Principal) -> JournalEntry {
    JournalEntry::new(TOKEN_ISSUED_OPERATION, Outcome::Succeeded).target(principal.actor())
}

/// Queue `entry` on the monitor's journal without blocking the async executor. Before the
/// monitor starts there is no journal and the stdout line is the only record.
async fn journal_entry(state: &ApiState, entry: JournalEntry) {
    let Some(monitor) = state.monitor() else {
        return;
    };
    let sender = monitor.journal();
    if let Err(err) = tokio::task::spawn_blocking(move || sender.send(entry)).await {
        tracing::error!(%err, operation = DENIED_OPERATION, "journal write panicked");
    }
}
