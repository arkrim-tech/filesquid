//! Content handles for media elements (API-S-018,
//! [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md)
//! Decision 5).
//!
//! `<img>`, `<video>`, `<audio>` and `<source>` cannot send an `Authorization` header, so a
//! client that may read a file version mints a short-lived, read-only handle for it
//! (`POST /api/v1/content-handles`, authorised exactly like the ranged read of that version)
//! and puts `/api/v1/content/<handle>` in the element's `src`. The handle in the path is that
//! route's only credential:
//!
//! - **Handle.** 256 bits from the operating system's CSPRNG, written as 43 base64url
//!   characters without padding. Only its SHA-256 digest is kept, in memory only
//!   ([`ContentHandles`]); it never reaches a store, the journal, a log line or an answer
//!   other than the mint answer.
//! - **Expiry.** [`IDLE_TIMEOUT_MS`] after the mint or the last accepted request start, and
//!   [`ABSOLUTE_LIFETIME_MS`] after the mint, whichever comes first, checked only when a
//!   request starts; an answer that has started is never cut off.
//! - **Invalidation.** A handle ends with the credential that minted it (desktop relaunch,
//!   server rotation, logout, session end or restart), and that credential and its
//!   [`Action::ReadFiles`](crate::auth::Action::ReadFiles) permission are checked again at
//!   every request start.
//! - **Bound.** At most [`MAX_HANDLES_PER_CREDENTIAL`] live handles per credential; another
//!   mint evicts that credential's least recently used handle.
//! - **Denial.** Every unusable request under `/api/v1/content/` gets one identical answer
//!   ([`denial`]): `forbidden`, reason `invalid_credentials`, never 401 or 404, with the same
//!   status, headers and `ApiError` fields; every `GET` denial has the same body and a `HEAD`
//!   denial an empty one. No file is read before a handle is accepted.
//! - **Headers.** Every answer of the content route, and the mint answer that carries a
//!   handle, carries [`PRIVACY_HEADERS`].

mod encoding;
mod route;
pub mod store;

pub(crate) use route::{content_routes, is_content_path, mint_route, refuse_content};
pub use route::{denial, PRIVACY_HEADERS};
pub use store::{ContentHandles, ContentTarget, Minted};

/// Idle timeout: a handle unused for this long is refused (300 s, API-S-018).
pub const IDLE_TIMEOUT_MS: i64 = 300_000;

/// Absolute lifetime: a handle is refused this long after its mint, even in use
/// (3,600 s, API-S-018).
pub const ABSOLUTE_LIFETIME_MS: i64 = 3_600_000;

/// Most live handles one credential holds; another mint evicts its least recently used one.
pub const MAX_HANDLES_PER_CREDENTIAL: usize = 1_024;

/// Random bytes in a handle (256 bits).
pub const HANDLE_BYTES: usize = 32;

/// Characters of a handle: 32 bytes in base64url without padding.
pub const HANDLE_CHARS: usize = 43;

/// Journal operation of a minted handle: actor, opaque file ID, version and absolute expiry,
/// never the handle (API-S-017).
pub const ISSUED_OPERATION: &str = "api.content_handle.issued";

/// The content route's path relative to [`crate::API_PREFIX`], as hosts that record request
/// paths MUST write it: the template, never a handle.
pub const CONTENT_PATH_TEMPLATE: &str = "/content/{handle}";

/// Every path under [`crate::API_PREFIX`] that starts with this belongs to the content route
/// and gets its denial when unusable.
pub const CONTENT_PREFIX: &str = "/content/";
