//! Who is calling ([`Principal`]), what they ask to do ([`Action`] on a [`Resource`]), and the
//! two seams that decide it: an [`Authenticator`] turns a presented token into a principal,
//! and a [`Policy`] says whether that principal may perform the action.
//!
//! This release knows two principals: the desktop owner (per-launch token) and the server
//! administrator (administrator token). Server accounts, sessions, groups and
//! collection-level permissions (API-F1.9) are added as new [`Principal`] and [`Resource`]
//! variants with their own [`Authenticator`] and [`Policy`]; the layer itself does not change.

use super::token::constant_time_eq;

/// An authenticated caller.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Principal {
    /// The one local owner of a desktop library, holding the per-launch token.
    Owner,
    /// A `filesquid-server` administrator, holding the administrator token.
    Administrator,
}

impl Principal {
    /// Opaque actor name recorded in the activity journal.
    pub fn actor(&self) -> &'static str {
        match self {
            Principal::Owner => "owner",
            Principal::Administrator => "administrator",
        }
    }
}

/// What a call asks to do. Every routed operation declares exactly one action; the contract
/// names it in the `x-filesquid-action` extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Action {
    /// Read process health (`GET /api/v1/health`).
    ReadHealth,
    /// Read the API contract (`GET /api/v1/openapi.json`).
    ReadContract,
    /// Subscribe to the event channel (`GET /api/v1/events`).
    SubscribeEvents,
    /// Read the monitor: health report, processes, journal and metrics (admin routes, `/metrics`).
    ReadMonitor,
}

impl Action {
    /// Every action, in contract order.
    pub const ALL: [Action; 4] = [
        Action::ReadHealth,
        Action::ReadContract,
        Action::SubscribeEvents,
        Action::ReadMonitor,
    ];

    /// Dotted wire name (`x-filesquid-action`, journal target).
    pub fn as_str(self) -> &'static str {
        match self {
            Action::ReadHealth => "health.read",
            Action::ReadContract => "contract.read",
            Action::SubscribeEvents => "events.subscribe",
            Action::ReadMonitor => "monitor.read",
        }
    }
}

/// What an action touches. Today every route acts on the host itself; API-F1.9 adds
/// libraries and collections so a policy can grant access per collection.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Resource {
    /// The host process: health, contract, events and monitor.
    Host,
}

/// Turns a presented bearer token into a [`Principal`]. Implementations MUST compare secrets
/// in constant time and MUST NOT log or store the presented token.
pub trait Authenticator: Send + Sync + 'static {
    /// The principal `token` identifies, or `None` when this host does not accept it.
    fn authenticate(&self, token: &str) -> Option<Principal>;
}

/// Decides whether a principal may perform an action on a resource. Anything not explicitly
/// permitted is denied.
pub trait Policy: Send + Sync + 'static {
    /// True only when `principal` may perform `action` on `resource`.
    fn permits(&self, principal: &Principal, action: Action, resource: &Resource) -> bool;
}

/// Accepts exactly one token and names one principal for it.
#[derive(Clone)]
pub struct TokenAuthenticator {
    token: String,
    principal: Principal,
}

impl TokenAuthenticator {
    /// Accept `token` as `principal`. An empty token accepts nothing.
    pub fn new(token: impl Into<String>, principal: Principal) -> Self {
        Self {
            token: token.into(),
            principal,
        }
    }
}

impl Authenticator for TokenAuthenticator {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        let valid =
            !self.token.is_empty() && constant_time_eq(token.as_bytes(), self.token.as_bytes());
        valid.then(|| self.principal.clone())
    }
}

impl std::fmt::Debug for TokenAuthenticator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenAuthenticator")
            .field("token", &"[redacted]")
            .field("principal", &self.principal)
            .finish()
    }
}

/// The single-owner policy of this release: the desktop owner and the server administrator
/// may perform every action on the host. Adding a principal is a compile error here until
/// its permissions are decided, so no new caller is granted anything by accident.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostPolicy;

impl Policy for HostPolicy {
    fn permits(&self, principal: &Principal, _action: Action, resource: &Resource) -> bool {
        match principal {
            Principal::Owner | Principal::Administrator => *resource == Resource::Host,
        }
    }
}

/// Authenticates nobody and permits nothing: the authoriser of a host that installed none.
#[derive(Clone, Copy, Debug, Default)]
pub struct DenyAll;

impl Authenticator for DenyAll {
    fn authenticate(&self, _token: &str) -> Option<Principal> {
        None
    }
}

impl Policy for DenyAll {
    fn permits(&self, _principal: &Principal, _action: Action, _resource: &Resource) -> bool {
        false
    }
}
