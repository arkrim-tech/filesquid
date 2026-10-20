//! `POST /api/v1/client-events` (API-S-030). The auth layer has authenticated and authorised
//! the caller for [`Action::RecordClientEvents`] before the handler runs. The handler then:
//!
//! 1. reads at most [`MAX_BODY_BYTES`] (`payload_too_large` past it);
//! 2. parses and checks the batch ([`parse_batch`]; `invalid_request` on any fault);
//! 3. applies the rate bound (`rate_limited`, `429` with `Retry-After`) to every request,
//!    refused ones included, except a lone overflow event that names no other principal;
//! 4. refuses the whole batch when an event names another principal than the credential's
//!    (journaled as an authorisation refusal, after the rate bound, so refusals are bounded);
//! 5. writes one journal entry per new client event ID through the journal store, and only
//!    then acknowledges every ID of the batch.
//!
//! Refusals by bound, schema or rate are not journaled: they are not authorisation
//! refusals, the client keeps and resends the events, and journaling them would let a
//! client flood the journal.

use axum::body::{to_bytes, Body};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use super::{parse_batch, Batch, BatchError, RateDecision, MAX_BODY_BYTES};
use crate::admin::blocking;
use crate::auth::{deny, presented_credential, token_digest, Action, Credential, Principal};
use crate::error::{ApiError, DenialReason, ErrorCode};
use crate::journal::{now_ms, JournalEntry};
use crate::monitor::Monitor;
use crate::ApiState;

/// A refusal: the response to send instead of an acknowledgement (boxed, it is large).
type Refusal = Box<Response>;

/// The acknowledgement of a delivered batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientEventsAck {
    /// Every client event ID of the batch, in request order without repeats: each is
    /// journaled now or was journaled before, so the client may forget it.
    pub acknowledged: Vec<String>,
    /// How many entries this request added (IDs already journaled add none).
    pub journaled: usize,
    /// The opaque principal of the credential, which the client stores with the events it
    /// keeps, so it sends them only under the same principal again.
    pub principal: String,
}

/// The route table entry.
pub(crate) fn route() -> crate::routes::Route {
    (
        "/client-events",
        axum::routing::post(record),
        Action::RecordClientEvents,
    )
}

/// The handler: bounds, schema, rate, principal, then one durable write.
async fn record(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    match accept(&state, &principal, &headers, body).await {
        Ok(ack) => Json(ack).into_response(),
        Err(refusal) => *refusal,
    }
}

/// Run every step of the route; a refusal is the response to send.
async fn accept(
    state: &ApiState,
    principal: &Principal,
    headers: &HeaderMap,
    body: Body,
) -> Result<ClientEventsAck, Refusal> {
    let Ok(bytes) = to_bytes(body, MAX_BODY_BYTES).await else {
        return Err(counted(state, headers, too_large()));
    };
    let batch = match parse_batch(&bytes, now_ms()) {
        Ok(batch) => batch,
        Err(e) => return Err(counted(state, headers, invalid(e))),
    };
    let foreign = names_foreign_principal(principal, &batch);
    if foreign || !batch.is_lone_overflow() {
        check_rate(state, headers)?;
    }
    if foreign {
        return Err(refuse_foreign(state, principal).await);
    }
    let monitor = installed_monitor(state).map_err(refusal)?;
    let entries = entries_of(&batch, principal.actor(), now_ms());
    let journaled = write(monitor, entries).await.map_err(refusal)?;
    Ok(ClientEventsAck {
        acknowledged: unique_ids(&batch),
        journaled,
        principal: principal.actor().to_string(),
    })
}

/// `error` as a [`Refusal`].
fn refusal(error: ApiError) -> Refusal {
    Box::new(error.into_response())
}

/// The installed monitor (`unavailable` before it starts, so the client retries).
fn installed_monitor(state: &ApiState) -> Result<Monitor, ApiError> {
    state.monitor().ok_or_else(|| {
        ApiError::new(
            ErrorCode::Unavailable,
            "The journal is starting. Try again in a moment.",
        )
    })
}

/// `payload_too_large`, naming the bound.
fn too_large() -> ApiError {
    ApiError::new(
        ErrorCode::PayloadTooLarge,
        format!("A request body carries at most {MAX_BODY_BYTES} bytes."),
    )
}

/// `invalid_request` for a batch the schema refuses.
fn invalid(error: BatchError) -> ApiError {
    ApiError::new(ErrorCode::InvalidRequest, error.0)
}

/// Count a request refused before it was parsed (too large or malformed) against its
/// credential, so refused requests are bounded too: `rate_limited` when over the bound, else
/// `error`.
fn counted(state: &ApiState, headers: &HeaderMap, error: ApiError) -> Refusal {
    match check_rate(state, headers) {
        Ok(()) => refusal(error),
        Err(limited) => limited,
    }
}

/// Count one request against its credential; `rate_limited` when over the bound. Only a
/// parsed lone overflow event that names no other principal skips this.
fn check_rate(state: &ApiState, headers: &HeaderMap) -> Result<(), Refusal> {
    let Credential::Bearer(token) = presented_credential(headers) else {
        return Ok(());
    };
    match state.client_event_limiter().admit(token_digest(token)) {
        RateDecision::Admitted => Ok(()),
        RateDecision::Limited { retry_after_s } => Err(Box::new(rate_limited(retry_after_s))),
    }
}

/// `429 rate_limited` with `Retry-After` in whole seconds.
fn rate_limited(retry_after_s: u64) -> Response {
    let error = ApiError::new(
        ErrorCode::RateLimited,
        "Too many client-event requests. Try again later.",
    );
    let mut response = error.into_response();
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from(retry_after_s));
    response
}

/// True when an event of `batch` names another principal than `principal`. Pure.
fn names_foreign_principal(principal: &Principal, batch: &Batch) -> bool {
    batch
        .events
        .iter()
        .filter_map(|e| e.principal())
        .any(|raised| raised != principal.actor())
}

/// Journal the refusal of a batch that names another principal, and answer it.
async fn refuse_foreign(state: &ApiState, principal: &Principal) -> Refusal {
    let denial = deny(
        state,
        DenialReason::NotPermitted,
        Some(principal),
        Some(Action::RecordClientEvents),
    )
    .await;
    refusal(denial)
}

/// One entry per event, in request order. Pure.
fn entries_of(batch: &Batch, actor: &str, now_ms: i64) -> Vec<JournalEntry> {
    batch
        .events
        .iter()
        .map(|e| e.to_entry(actor, now_ms))
        .collect()
}

/// The batch's client event IDs in request order, each once. Pure.
fn unique_ids(batch: &Batch) -> Vec<String> {
    let mut ids: Vec<String> = Vec::with_capacity(batch.events.len());
    for event in &batch.events {
        if !ids.iter().any(|id| id == event.id()) {
            ids.push(event.id().to_string());
        }
    }
    ids
}

/// Write `entries` through the announcing journal store in one append, off the executor;
/// answer how many it stored now. Journal-store I/O.
async fn write(monitor: Monitor, entries: Vec<JournalEntry>) -> Result<usize, ApiError> {
    let store = monitor.announcing_store();
    let stored = blocking("api.client_events", move || store.append_new(&entries)).await?;
    let stored = stored.map_err(|e| ApiError::internal("api.client_events", &e.to_string()))?;
    Ok(stored.iter().filter(|(_, new)| *new).count())
}
