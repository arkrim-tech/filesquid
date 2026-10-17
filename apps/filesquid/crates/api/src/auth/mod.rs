//! Deny-by-default authentication and authorisation for every API call (API-S-004,
//! [ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)
//! Decision 5).
//!
//! Every route the [`crate::router`] serves is wrapped in [`require`], and its 404 and 405
//! fallbacks call [`check_caller`], so no handler runs (and no file, SQL or key is touched)
//! before the caller is authenticated and the route's [`Action`] is authorised. Hosts guard
//! the routes they serve themselves with the same [`require`].
//!
//! - **Deny by default.** A host that installed no [`Authoriser`] refuses every call. An
//!   authoriser grants only what its [`Policy`] explicitly permits.
//! - **Credentials.** `Authorization: Bearer <token>`, or on a WebSocket upgrade the
//!   `filesquid.bearer.<token>` subprotocol ([`presented_token`]).
//! - **Desktop.** One local owner holding the per-launch [`LaunchToken`]
//!   ([`Authoriser::desktop_owner`]).
//! - **Server.** The administrator token ([`Authoriser::server_administrator`]) until accounts,
//!   groups and collection permissions arrive in API-F1.9 as another [`Authenticator`] and
//!   [`Policy`].
//! - **Denials** are typed [`ApiError`]s with a [`DenialReason`]: `missing_credentials` (401
//!   `unauthorized`), `invalid_credentials` or `not_permitted` (403 `forbidden`). None is
//!   retryable, and none repeats the presented credential.
//! - **Journal.** Every refusal is journaled as [`DENIED_OPERATION`]; the desktop journals
//!   [`TOKEN_ISSUED_OPERATION`] at launch. Neither record can carry a secret.

mod credentials;
mod layer;
mod policy;
mod token;

pub use crate::error::{ApiError, DenialReason};
pub use credentials::{presented_token, BEARER_SUBPROTOCOL_PREFIX, EVENTS_SUBPROTOCOL};
pub use layer::{
    authorise_call, check_caller, denial_entry, require, token_issued_entry, Guard,
    ANONYMOUS_ACTOR, DENIED_OPERATION, TOKEN_ISSUED_OPERATION,
};
pub use policy::{
    Action, Authenticator, DenyAll, HostPolicy, Policy, Principal, Resource, TokenAuthenticator,
};
pub use token::{generate_token, LaunchToken, TOKEN_BYTES};

use std::sync::Arc;

/// The decision point of one host: who a token identifies, and what they may do.
/// Cheap to clone. `Debug` output never shows a secret.
#[derive(Clone)]
pub struct Authoriser {
    authenticator: Arc<dyn Authenticator>,
    policy: Arc<dyn Policy>,
}

impl std::fmt::Debug for Authoriser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Authoriser { .. }")
    }
}

impl Authoriser {
    /// Combine an authenticator and a policy. This is the seam for server accounts, groups
    /// and collection-level permissions (API-F1.9).
    pub fn new(authenticator: Arc<dyn Authenticator>, policy: Arc<dyn Policy>) -> Self {
        Self {
            authenticator,
            policy,
        }
    }

    /// Accept nobody and permit nothing: what a host without an installed authoriser uses.
    pub fn deny_all() -> Self {
        Self::new(Arc::new(DenyAll), Arc::new(DenyAll))
    }

    /// The desktop: the holder of `token` is the [`Principal::Owner`] and may do everything
    /// on the host ([`HostPolicy`]).
    pub fn desktop_owner(token: &LaunchToken) -> Self {
        let authenticator = TokenAuthenticator::new(token.secret(), Principal::Owner);
        Self::new(Arc::new(authenticator), Arc::new(HostPolicy))
    }

    /// `filesquid-server` before accounts exist: the holder of the administrator `token` is
    /// the [`Principal::Administrator`] and may do everything on the host ([`HostPolicy`]).
    pub fn server_administrator(token: impl Into<String>) -> Self {
        let authenticator = TokenAuthenticator::new(token, Principal::Administrator);
        Self::new(Arc::new(authenticator), Arc::new(HostPolicy))
    }

    /// Who presented `token`: a missing token is `missing_credentials`, one no authenticator
    /// accepts is `invalid_credentials`. Pure; no I/O.
    pub fn authenticate(&self, token: Option<&str>) -> Result<Principal, DenialReason> {
        let token = token.ok_or(DenialReason::MissingCredentials)?;
        self.authenticator
            .authenticate(token)
            .ok_or(DenialReason::InvalidCredentials)
    }

    /// Whether `principal` may perform `action` on `resource`; `not_permitted` otherwise.
    /// Pure; no I/O.
    pub fn authorise(
        &self,
        principal: &Principal,
        action: Action,
        resource: &Resource,
    ) -> Result<(), DenialReason> {
        if self.policy.permits(principal, action, resource) {
            Ok(())
        } else {
            Err(DenialReason::NotPermitted)
        }
    }
}
