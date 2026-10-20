//! Deny-by-default authentication and authorisation for every API call except the public
//! liveness probe (API-S-004, API-S-015,
//! [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md)
//! Decision 5).
//!
//! Every route the [`crate::router`] serves is wrapped in [`require`], except the public
//! liveness probe `GET /api/v1/health/live` ([`Action::Public`], API-S-015), and its 404 and
//! 405 fallbacks call [`check_caller`], so no other handler runs (and no file, SQL or key is
//! touched) before the caller is authenticated and the route's [`Action`] is authorised.
//! Hosts guard the routes they serve themselves with the same [`require`], which never
//! bypasses authentication.
//!
//! - **Deny by default.** A host that installed no [`Authoriser`] refuses every guarded call. An
//!   authoriser grants only what its [`Policy`] explicitly permits, with one exception:
//!   `host.read` ([`Action::ReadHost`], the host descriptor) is granted to every authenticated
//!   caller whatever the policy says ([`Action::granted_to_every_credential`], API-S-020).
//!   Authentication is still required for it.
//! - **Credentials.** `Authorization: Bearer <token>` on any request, WebSocket upgrades
//!   included; browsers, which cannot set headers on a WebSocket, offer the
//!   `filesquid.bearer.<token>` subprotocol instead ([`presented_credential`]). A present but
//!   unusable `Authorization` header is `invalid_credentials`.
//! - **Desktop.** One local owner holding the per-launch [`LaunchToken`]
//!   ([`Authoriser::desktop_owner`]).
//! - **Server.** The administrator token ([`Authoriser::server_administrator`]) until accounts,
//!   groups and collection permissions arrive in API-F1.9 as another [`Authenticator`] and
//!   [`Policy`].
//! - **Denials** are typed [`ApiError`]s with a [`DenialReason`]: `missing_credentials` (401
//!   `unauthorized`), `invalid_credentials` or `not_permitted` (403 `forbidden`). None is
//!   retryable, and none repeats the presented credential.
//! - **Journal.** Every refusal is journaled as [`DENIED_OPERATION`]; each host journals
//!   [`TOKEN_ISSUED_OPERATION`] at startup ([`record_token_issue`]). Neither record can carry
//!   a secret.

mod credentials;
mod layer;
mod policy;
mod token;

pub use crate::error::{ApiError, DenialReason};
pub use credentials::{
    presented_credential, Credential, BEARER_SUBPROTOCOL_PREFIX, EVENTS_SUBPROTOCOL,
};
pub use layer::{
    authorise_call, check_caller, denial_entry, record_token_issue, require, token_issued_entry,
    Guard, ANONYMOUS_ACTOR, DENIED_OPERATION, NO_ROUTE_TARGET, TOKEN_ISSUED_OPERATION,
};
pub(crate) use layer::{deny, journal_entry};
pub use policy::{
    Action, Authenticator, DenyAll, HostPolicy, Policy, Principal, Resource, TokenAuthenticator,
};
pub(crate) use token::digest as token_digest;
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
    /// the [`Principal::Administrator`] and may do everything on the host except the
    /// version-inventory repair (`library.repair`), which only a library's owner may run
    /// ([`HostPolicy`]).
    pub fn server_administrator(token: impl Into<String>) -> Self {
        let authenticator = TokenAuthenticator::new(token, Principal::Administrator);
        Self::new(Arc::new(authenticator), Arc::new(HostPolicy))
    }

    /// Who presented `credential`: none is `missing_credentials`; an unusable header, or a
    /// token no authenticator accepts, is `invalid_credentials`. Pure; no I/O.
    pub fn authenticate(&self, credential: Credential<'_>) -> Result<Principal, DenialReason> {
        let token = match credential {
            Credential::Absent => return Err(DenialReason::MissingCredentials),
            Credential::Unusable => return Err(DenialReason::InvalidCredentials),
            Credential::Bearer(token) => token,
        };
        self.authenticator
            .authenticate(token)
            .ok_or(DenialReason::InvalidCredentials)
    }

    /// Who presented the token whose SHA-256 digest is `digest`, if this host still accepts
    /// it; `invalid_credentials` otherwise. Content handles call it at every request start
    /// (API-S-018). Pure; no I/O.
    pub fn authenticate_digest(&self, digest: &[u8; 32]) -> Result<Principal, DenialReason> {
        self.authenticator
            .authenticate_digest(digest)
            .ok_or(DenialReason::InvalidCredentials)
    }

    /// Whether `principal` may perform `action` on `resource`; `not_permitted` otherwise.
    /// An action [granted to every credential](Action::granted_to_every_credential) is
    /// permitted to every authenticated principal before the policy is consulted. Pure; no I/O.
    pub fn authorise(
        &self,
        principal: &Principal,
        action: Action,
        resource: &Resource,
    ) -> Result<(), DenialReason> {
        if action.granted_to_every_credential() || self.policy.permits(principal, action, resource)
        {
            Ok(())
        } else {
            Err(DenialReason::NotPermitted)
        }
    }
}
