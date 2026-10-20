//! The host port of the library routes (API-S-025, API-S-006): where a host's storage roots
//! come from, and the request and answer bodies the routes add around the CL-1.1 operations.
//!
//! A folder the owner chose never crosses the API as a path. The host registers it under an
//! opaque location ID (on the desktop, after the owner picks a folder, DE-1.1) and resolves
//! that ID to a [`LibraryRoot`] through [`LibraryLocations`]; the API keeps only the IDs.

use filesquid_core::LibraryId;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::write::VersionReservations;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Longest location ID a request may name, in bytes.
pub const MAX_LOCATION_ID_BYTES: usize = 128;

/// What a host injects for one location: the OCFL storage root and the fenced version
/// reservations every writer of that root shares (the repair reserves through them).
pub struct LibraryRoot<B, R> {
    /// The storage root.
    pub backend: Arc<B>,
    /// The version reservations of the root's writers.
    pub reservations: Arc<R>,
}

impl<B, R> Clone for LibraryRoot<B, R> {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            reservations: self.reservations.clone(),
        }
    }
}

/// The locations a host can offer the library routes. Implementations keep the folder
/// paths; the API sees opaque IDs only. Both calls are pure lookups the host answers from
/// memory, without blocking I/O.
pub trait LibraryLocations: Send + Sync + 'static {
    /// The storage backend of a location.
    type Backend: StorageBackend + 'static;
    /// The version reservations of a location's writers.
    type Reservations: VersionReservations + 'static;

    /// The storage root registered as `location_id`, or `None` when the host does not know
    /// that ID.
    fn location(&self, location_id: &str)
        -> Option<LibraryRoot<Self::Backend, Self::Reservations>>;

    /// Whether the two locations are the same folder or one lies inside the other, so a
    /// snapshot copy never lands inside the library it copies (OCFL-S-019). A host that
    /// cannot tell MUST answer `true`.
    fn overlaps(&self, a: &str, b: &str) -> bool;
}

/// Body of `POST /api/v1/libraries` (create) and `POST /api/v1/libraries/openings` (open).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationRequest {
    /// The opaque ID under which the host registered the folder the owner chose.
    pub location_id: String,
    /// The library ID to use; a new UUID v4 is minted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_id: Option<LibraryId>,
}

/// The answer to create and open: the library and what its storage root holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryView {
    /// The library, bound to the location until the host stops.
    pub library_id: LibraryId,
    /// The OCFL version the storage root declares (`1.1`).
    pub spec_version: String,
    /// The objects discovered in it.
    pub objects: usize,
}

/// Body of `POST /api/v1/libraries/snapshot-copies`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotCopyRequest {
    /// The open library to copy.
    pub library_id: LibraryId,
    /// The opaque ID of the empty folder to copy into.
    pub destination_location_id: String,
}

/// Body of `POST /api/v1/libraries/rebuilds`, and the query of
/// `GET /api/v1/libraries/repair-candidates`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryRequest {
    /// The open library.
    pub library_id: LibraryId,
}
