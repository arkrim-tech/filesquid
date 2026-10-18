//! The replica: permission-aware cache identity, visible coverage and recovery states, and
//! the pending-change ledger of a client that works from a local copy (API-S-024, Task
//! CL-0.3).
//!
//! A browser or other client reads a decrypted replica of what its viewer may see and keeps
//! changes it has not yet published as pending ([ADR 008](../../../../../../resources/architecture/008-browser-wasm-scope.md),
//! [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md)).
//! This module is pure computation: it does no storage I/O and holds no filesystem path or
//! SQL ([API-S-006](../../resources/srs.md)), uses only portable dependencies (so it does not block the
//! `wasm32-unknown-unknown` build of API-S-010) and adds no dependency. The host injects the cache backend ([`ReplicaCache`]) and keeps the ledger
//! snapshot wherever it stores local state.
//!
//! Rules every host gets from here:
//!
//! - **Cache identity.** A [`CacheIdentity`] names the host type, the viewer, the library and
//!   the digest of the viewer's granted scopes. Its [`CacheKey`] and every query key change
//!   when any part changes, so entries cached under other grants are never reachable, and a
//!   page stored for another identity is refused ([`ReplicaError::IdentityMismatch`]).
//! - **Visible coverage.** Every answer says whether the replica is [`Coverage::Complete`] or
//!   still incomplete. A missing entry is `not_cached` and an evicted one `evicted`, both
//!   retryable recovery states; neither is ever an empty page (T-022).
//! - **Pending changes.** A [`PendingChange`] keeps its command unchanged and becomes
//!   canonical (`committed`) only when its fenced head publication succeeds
//!   ([`PendingLedger::commit_published`]). Changes publish in queue order; a conflict or a
//!   stalled change blocks the ones behind it until the owner rebases or discards it, and no
//!   change is ever overwritten.

/// `tracing` target of this module's stdout-only diagnostics, outside `filesquid::activity`.
const LOG_TARGET: &str = "filesquid::replica";

mod cache;
mod coverage;
mod error;
mod identity;
mod pending;

pub use cache::{
    answer_from_cache, CacheLookup, CachedPage, MemoryReplicaCache, ReplicaAnswer, ReplicaCache,
    ReplicaRequest, DEFAULT_CACHE_CAPACITY,
};
pub use coverage::{Coverage, MAX_SHARDS};
pub use error::{Recovery, ReplicaError};
pub use identity::{
    CacheIdentity, CacheKey, GrantDigest, ViewerId, CACHE_SCHEMA_VERSION, MAX_GRANTS,
    MAX_GRANT_BYTES, MAX_VIEWER_BYTES,
};
pub use pending::{
    HeadPublication, PendingChange, PendingLedger, PendingState, MAX_ATTEMPTS, MAX_LIBRARIES,
    MAX_PENDING_PER_LIBRARY, MAX_SNAPSHOT_BYTES,
};
