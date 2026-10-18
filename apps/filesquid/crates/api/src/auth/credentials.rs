//! Where a call carries its credential. Pure header parsing; no I/O.
//!
//! - Every request MAY send `Authorization: Bearer <token>` (scheme name case-insensitive).
//! - A WebSocket upgrade MAY instead offer the subprotocol `filesquid.bearer.<token>` next to
//!   [`EVENTS_SUBPROTOCOL`], because browsers cannot set headers on a WebSocket. The server
//!   selects [`EVENTS_SUBPROTOCOL`], never the token entry.

use axum::http::header::{AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL, UPGRADE};
use axum::http::HeaderMap;

/// Subprotocol prefix that carries a bearer token on a WebSocket upgrade.
pub const BEARER_SUBPROTOCOL_PREFIX: &str = "filesquid.bearer.";

/// Subprotocol of the event channel; the server selects it when a client offers it.
pub const EVENTS_SUBPROTOCOL: &str = "filesquid.events.v1";

/// The bearer token a request presents, if any. The `Authorization` header wins; the
/// subprotocol carrier counts only on a WebSocket upgrade. An empty bearer value is returned
/// as presented, so it is refused as an invalid credential rather than a missing one.
pub fn presented_token(headers: &HeaderMap) -> Option<&str> {
    bearer_header(headers).or_else(|| bearer_subprotocol(headers))
}

/// The token of the first `Authorization: Bearer` header.
fn bearer_header(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// The token of the first `filesquid.bearer.` subprotocol offered on a WebSocket upgrade, and
/// only when the client also offers [`EVENTS_SUBPROTOCOL`], the protocol the server selects.
fn bearer_subprotocol(headers: &HeaderMap) -> Option<&str> {
    let upgrade = headers.get(UPGRADE)?.to_str().ok()?;
    if !upgrade.eq_ignore_ascii_case("websocket") {
        return None;
    }
    let offered: Vec<&str> = headers
        .get_all(SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .collect();
    if !offered.contains(&EVENTS_SUBPROTOCOL) {
        return None;
    }
    offered
        .into_iter()
        .find_map(|entry| entry.strip_prefix(BEARER_SUBPROTOCOL_PREFIX))
}
