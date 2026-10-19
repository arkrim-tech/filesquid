//! Who is calling ([`Principal`]), what they ask to do ([`Action`] on a [`Resource`]), and the
//! two seams that decide it: an [`Authenticator`] turns a presented token into a principal,
//! and a [`Policy`] says whether that principal may perform the action.
//!
//! This release knows two principals: the desktop owner (per-launch token) and the server
//! administrator (administrator token). Server accounts, sessions, groups and
//! collection-level permissions (API-F1.9) are added as new [`Principal`] and [`Resource`]
//! variants with their own [`Authenticator`] and [`Policy`]; the layer itself does not change.

use super::token::{constant_time_eq, digest};

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
    /// Read the host descriptor (`GET /api/v1/host`). Granted to every authenticated caller,
    /// whatever the policy says ([`Action::granted_to_every_credential`], API-S-020).
    ReadHost,
    /// Read files: folder listings, details, history, versions and ranged content reads
    /// (`/api/v1/files/...`), and mint a content handle for a file version, which is
    /// authorised exactly like that version's ranged read (API-S-018).
    ReadFiles,
    /// Change files: upload, create folder, move, remove and restore (`/api/v1/files/...`).
    WriteFiles,
    /// Read one file version through a content handle (`/api/v1/content/{handle}`). The
    /// handle in the path is the route's only credential: the router guards this route with
    /// the content-handle check instead of a Bearer credential, and checks the minting
    /// credential's [`Action::ReadFiles`] permission again at every request (API-S-018). It
    /// is never public.
    ReadContent,
    /// Read the PROV-O history: activities, records and their export (`/history`,
    /// `/files/{id}/history`, `/files/{id}/versions/{version}/prov`; API-S-029).
    ReadHistory,
    /// Deliberately public, unauthenticated: only the liveness probe
    /// (`GET /api/v1/health/live`), which says that the process is up and nothing else. The
    /// router leaves the route-table entry with this action unguarded; the guard itself
    /// ([`crate::auth::require`]) still authenticates any route it wraps, so a host cannot
    /// make a route public with it.
    Public,
}

impl Action {
    /// Every action, in declaration order.
    pub const ALL: [Action; 10] = [
        Action::ReadHealth,
        Action::ReadContract,
        Action::SubscribeEvents,
        Action::ReadMonitor,
        Action::ReadHost,
        Action::ReadFiles,
        Action::WriteFiles,
        Action::ReadContent,
        Action::ReadHistory,
        Action::Public,
    ];

    /// Dotted wire name (`x-filesquid-action`, journal target).
    pub fn as_str(self) -> &'static str {
        match self {
            Action::ReadHealth => "health.read",
            Action::ReadContract => "contract.read",
            Action::SubscribeEvents => "events.subscribe",
            Action::ReadMonitor => "monitor.read",
            Action::ReadHost => "host.read",
            Action::ReadFiles => "files.read",
            Action::WriteFiles => "files.write",
            Action::ReadContent => "content.read",
            Action::ReadHistory => "history.read",
            Action::Public => "public",
        }
    }

    /// True for an action every authenticated caller may perform, so no [`Policy`] can
    /// withhold it: reading the host descriptor, which every UI needs to say truthfully where
    /// the caller's files are (API-S-020). Authentication is still required.
    pub fn granted_to_every_credential(self) -> bool {
        self == Action::ReadHost
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

    /// The principal the token with SHA-256 `digest` identifies, or `None` when this host no
    /// longer accepts it. Content handles keep only their minting credential's digest and
    /// call this at every request start, so a handle ends with its credential (API-S-018).
    /// The default accepts nothing, so an authenticator that cannot answer ends every handle.
    fn authenticate_digest(&self, _digest: &[u8; 32]) -> Option<Principal> {
        None
    }
}

/// Decides whether a principal may perform an action on a resource. Anything not explicitly
/// permitted is denied.
pub trait Policy: Send + Sync + 'static {
    /// True only when `principal` may perform `action` on `resource`.
    fn permits(&self, principal: &Principal, action: Action, resource: &Resource) -> bool;
}

/// Accepts exactly one token and names one principal for it. It keeps only the token's
/// SHA-256 digest and compares digests in constant time, so timing reveals neither the
/// token's content nor its length.
#[derive(Clone)]
pub struct TokenAuthenticator {
    digest: Option<[u8; 32]>,
    principal: Principal,
}

impl TokenAuthenticator {
    /// Accept `token` as `principal`. An empty token accepts nothing.
    pub fn new(token: impl Into<String>, principal: Principal) -> Self {
        let token = token.into();
        Self {
            digest: (!token.is_empty()).then(|| digest(&token)),
            principal,
        }
    }
}

impl Authenticator for TokenAuthenticator {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        self.authenticate_digest(&digest(token))
    }

    fn authenticate_digest(&self, presented: &[u8; 32]) -> Option<Principal> {
        let expected = self.digest?;
        constant_time_eq(presented, &expected).then(|| self.principal.clone())
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
