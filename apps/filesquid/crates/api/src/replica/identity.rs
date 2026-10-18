//! Permission-aware cache identity (API-S-024). A cached answer is reachable only through a
//! key derived from who asked, on which host type, for which library and under which grants,
//! so a changed grant or another viewer can never read an earlier viewer's entries.

use super::{ReplicaError, LOG_TARGET};
use crate::host::HostType;
use crate::library::QueryRequest;
use filesquid_core::LibraryId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Schema version of the identity and its key encoding.
pub const CACHE_SCHEMA_VERSION: u32 = 1;

/// Longest viewer ID, in bytes.
pub const MAX_VIEWER_BYTES: usize = 256;

/// Most scopes a viewer's grants may name.
pub const MAX_GRANTS: usize = 64;

/// Longest scope name, in bytes.
pub const MAX_GRANT_BYTES: usize = 128;

/// Domain separator of identity keys.
const KEY_DOMAIN: &str = "filesquid.replica.cache.v1";

/// Domain separator of query keys.
const QUERY_DOMAIN: &str = "filesquid.replica.query.v1";

/// Opaque identifier of the person or credential that reads the replica (not a name).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct ViewerId(String);

impl ViewerId {
    /// A viewer ID of 1 to [`MAX_VIEWER_BYTES`] bytes without control characters.
    ///
    /// # Errors
    ///
    /// Returns `invalid_viewer` otherwise.
    pub fn new(value: impl Into<String>) -> Result<Self, ReplicaError> {
        let value = value.into();
        let ok = !value.is_empty()
            && value.len() <= MAX_VIEWER_BYTES
            && !value.chars().any(char::is_control);
        if !ok {
            return Err(ReplicaError::invalid(
                "invalid_viewer",
                "The viewer identifier is empty, too long or has control characters.",
            ));
        }
        Ok(Self(value))
    }

    /// The identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ViewerId {
    type Error = ReplicaError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// SHA-256 (lowercase hex) of a viewer's granted scope names, sorted and de-duplicated, so
/// the order and repeats of the grants do not matter but any added or removed scope does.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct GrantDigest(String);

impl GrantDigest {
    /// The digest of `scopes`: 1 to [`MAX_GRANTS`] scope names (before de-duplication) of 1
    /// to [`MAX_GRANT_BYTES`] bytes each.
    ///
    /// # Errors
    ///
    /// Returns `invalid_grants` when the list or a name is outside those bounds.
    pub fn of(scopes: &[&str]) -> Result<Self, ReplicaError> {
        let in_bounds = !scopes.is_empty()
            && scopes.len() <= MAX_GRANTS
            && scopes
                .iter()
                .all(|s| !s.is_empty() && s.len() <= MAX_GRANT_BYTES);
        if !in_bounds {
            return Err(grants_error());
        }
        let mut sorted: Vec<&str> = scopes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        let mut hasher = Sha256::new();
        for scope in sorted {
            absorb(&mut hasher, scope.as_bytes());
        }
        Ok(Self(hex(&hasher.finalize())))
    }

    /// The digest as 64 lowercase hex characters.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GrantDigest {
    type Error = ReplicaError;

    /// Accept only a canonical digest: 64 lowercase hex characters.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !is_lower_hex_digest(&value) {
            return Err(grants_error());
        }
        Ok(Self(value))
    }
}

/// The error for grants outside the bounds or a malformed digest.
fn grants_error() -> ReplicaError {
    ReplicaError::invalid(
        "invalid_grants",
        "The access grants are missing, too many or malformed.",
    )
}

/// A cache key: 64 lowercase hex characters of a SHA-256. It carries no name or path.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct CacheKey(String);

impl CacheKey {
    /// The key text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CacheKey {
    type Error = ReplicaError;

    /// Accept only a canonical key: 64 lowercase hex characters.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !is_lower_hex_digest(&value) {
            return Err(ReplicaError::invalid(
                "invalid_cache_key",
                "The stored view key is malformed.",
            ));
        }
        Ok(Self(value))
    }
}

/// Who a cached answer was made for.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawIdentity")]
pub struct CacheIdentity {
    schema_version: u32,
    host: HostType,
    viewer: ViewerId,
    library_id: LibraryId,
    grants: GrantDigest,
}

impl CacheIdentity {
    /// The identity of `viewer` on `host` for `library_id` under `grants`, at
    /// [`CACHE_SCHEMA_VERSION`].
    pub fn new(
        host: HostType,
        viewer: ViewerId,
        library_id: LibraryId,
        grants: GrantDigest,
    ) -> Self {
        Self {
            schema_version: CACHE_SCHEMA_VERSION,
            host,
            viewer,
            library_id,
            grants,
        }
    }

    /// The key of this identity: SHA-256 over the domain separator and each component,
    /// every one length-prefixed so no two identities share an encoding.
    pub fn key(&self) -> CacheKey {
        let mut hasher = Sha256::new();
        absorb(&mut hasher, KEY_DOMAIN.as_bytes());
        hasher.update(self.schema_version.to_be_bytes());
        absorb(&mut hasher, host_name(self.host).as_bytes());
        absorb(&mut hasher, self.viewer.as_str().as_bytes());
        absorb(&mut hasher, self.library_id.as_str().as_bytes());
        absorb(&mut hasher, self.grants.as_str().as_bytes());
        CacheKey(hex(&hasher.finalize()))
    }

    /// The key of `request` asked under this identity: it covers the identity key and the
    /// whole canonical request (envelope, cursor and query).
    ///
    /// # Errors
    ///
    /// Returns [`ReplicaError::Internal`] if the request cannot be encoded (the detail goes to
    /// stdout only).
    pub fn query_key(&self, request: &QueryRequest) -> Result<CacheKey, ReplicaError> {
        let body = serde_json::to_vec(request).map_err(|err| {
            tracing::info!(target: LOG_TARGET, error_code = "internal", detail = %err, "query could not be encoded");
            ReplicaError::Internal
        })?;
        let mut hasher = Sha256::new();
        absorb(&mut hasher, QUERY_DOMAIN.as_bytes());
        absorb(&mut hasher, self.key().as_str().as_bytes());
        absorb(&mut hasher, &body);
        Ok(CacheKey(hex(&hasher.finalize())))
    }

    /// True when `newer` is the same viewer, host and library under different grants: this
    /// identity's entries are dropped by `ReplicaCache::drop_superseded` and never served.
    pub fn is_superseded_by(&self, newer: &CacheIdentity) -> bool {
        self.host == newer.host
            && self.viewer == newer.viewer
            && self.library_id == newer.library_id
            && self.grants != newer.grants
    }
}

/// The wire form of [`CacheIdentity`], whose schema version is checked before it is trusted.
#[derive(Deserialize)]
struct RawIdentity {
    schema_version: u32,
    host: HostType,
    viewer: ViewerId,
    library_id: LibraryId,
    grants: GrantDigest,
}

impl TryFrom<RawIdentity> for CacheIdentity {
    type Error = ReplicaError;

    /// Accept only [`CACHE_SCHEMA_VERSION`], so restored state cannot name another encoding.
    fn try_from(raw: RawIdentity) -> Result<Self, Self::Error> {
        if raw.schema_version != CACHE_SCHEMA_VERSION {
            return Err(ReplicaError::invalid(
                "invalid_cache_identity",
                "The stored view identity uses an unsupported version.",
            ));
        }
        Ok(Self::new(raw.host, raw.viewer, raw.library_id, raw.grants))
    }
}

/// The wire name of a host type, fixed here so a rename elsewhere cannot change keys.
fn host_name(host: HostType) -> &'static str {
    match host {
        HostType::Desktop => "desktop",
        HostType::Server => "server",
        HostType::Browser => "browser",
    }
}

/// Feed `bytes` to `hasher` with a 4-byte big-endian length prefix.
fn absorb(hasher: &mut Sha256, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    hasher.update(len.to_be_bytes());
    hasher.update(bytes);
}

/// Lowercase hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// True for exactly 64 lowercase hex characters.
fn is_lower_hex_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
