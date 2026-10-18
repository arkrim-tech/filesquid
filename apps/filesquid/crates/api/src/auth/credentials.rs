//! Where a call carries its credential. Pure header parsing; no I/O.
//!
//! - Every request MAY send `Authorization: Bearer <token>` (scheme name case-insensitive),
//!   WebSocket upgrades included.
//! - A browser cannot set headers on a WebSocket, so an upgrade MAY instead offer the
//!   subprotocol `filesquid.bearer.<token>` next to [`EVENTS_SUBPROTOCOL`]. The server selects
//!   [`EVENTS_SUBPROTOCOL`], never the token entry.
//! - An `Authorization` header that is present but not a usable `Bearer` credential is
//!   [`Credential::Unusable`] (refused as `invalid_credentials`); it never falls back to the
//!   subprotocol. Only a request with neither carrier is [`Credential::Absent`].

use axum::http::header::{AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL, UPGRADE};
use axum::http::HeaderMap;

/// Subprotocol prefix that carries a bearer token on a WebSocket upgrade.
pub const BEARER_SUBPROTOCOL_PREFIX: &str = "filesquid.bearer.";

/// Subprotocol of the event channel; the server selects it when a client offers it.
pub const EVENTS_SUBPROTOCOL: &str = "filesquid.events.v1";

/// The credential a request presents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Credential<'a> {
    /// No `Authorization` header and no usable subprotocol carrier (`missing_credentials`).
    Absent,
    /// An `Authorization` header that is not `Bearer <token>`, or not readable text
    /// (`invalid_credentials`).
    Unusable,
    /// A bearer token, possibly empty (an empty one is never accepted).
    Bearer(&'a str),
}

/// The credential a request presents. A present `Authorization` header decides alone; the
/// subprotocol carrier counts only on a WebSocket upgrade that has no `Authorization` header.
pub fn presented_credential(headers: &HeaderMap) -> Credential<'_> {
    match headers.get(AUTHORIZATION) {
        Some(value) => bearer_header(value.to_str().ok()),
        None => bearer_subprotocol(headers).map_or(Credential::Absent, Credential::Bearer),
    }
}

/// Parse an `Authorization` header value: `Bearer <token>` (scheme case-insensitive) or
/// [`Credential::Unusable`].
fn bearer_header(value: Option<&str>) -> Credential<'_> {
    let Some((scheme, token)) = value.and_then(|v| v.split_once(' ')) else {
        return Credential::Unusable;
    };
    if scheme.eq_ignore_ascii_case("bearer") {
        Credential::Bearer(token.trim())
    } else {
        Credential::Unusable
    }
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
