//! [`LibraryOperations`]: the object-safe handle to a host's [`LibraryCommands`], installed
//! in [`crate::ApiState`] with [`crate::ApiState::install_libraries`] for the library and
//! repair routes.

use super::commands::LibraryCommands;
use super::error::LocalLibraryError;
use super::facts::{LibraryFacts, RecentLibraries, RenameLibraryRequest};
use super::locations::{
    LibraryLocations, LibraryRequest, LibraryView, LocationRequest, SnapshotCopyRequest,
};
use super::rebuild::RebuildReport;
use super::repair::{RepairAnswer, RepairCandidates, RepairRequest};
use super::snapshot::CopyReport;
use crate::auth::Principal;
use filesquid_core::LibraryId;
use std::future::Future;
use std::pin::Pin;

/// Whether the caller may read a library, as the recent route decides it.
pub type Readable = Box<dyn Fn(&LibraryId) -> bool + Send + Sync>;

/// The future of one library operation.
pub type LibraryFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, LocalLibraryError>> + Send + 'a>>;

/// What the library routes need from a host. Every refusal is journaled once by the
/// implementation, never by the caller.
pub trait LibraryOperations: Send + Sync {
    /// Create a library in an empty location, for `actor`.
    fn create(&self, actor: String, request: LocationRequest) -> LibraryFuture<'_, LibraryView>;

    /// Open the library in a location, for `actor`.
    fn open(&self, actor: String, request: LocationRequest) -> LibraryFuture<'_, LibraryView>;

    /// Copy an open library's completed snapshot into an empty location, for `actor`.
    fn copy_snapshot(
        &self,
        actor: String,
        request: SnapshotCopyRequest,
    ) -> LibraryFuture<'_, CopyReport>;

    /// Rebuild an open library from its files, for `actor`.
    fn rebuild(&self, actor: String, request: LibraryRequest) -> LibraryFuture<'_, RebuildReport>;

    /// The repair candidates of an open library, for `actor`.
    fn repair_candidates(
        &self,
        actor: String,
        request: LibraryRequest,
    ) -> LibraryFuture<'_, RepairCandidates>;

    /// Run the consented version-inventory repair, for `caller`.
    fn repair(&self, caller: Principal, request: RepairRequest) -> LibraryFuture<'_, RepairAnswer>;

    /// Rename a library this host opened, for `actor` (API-S-038).
    fn rename(
        &self,
        actor: String,
        request: RenameLibraryRequest,
    ) -> LibraryFuture<'_, LibraryFacts>;

    /// The facts of a library this host opened, for `actor` (API-S-038).
    fn facts(&self, actor: String, library_id: LibraryId) -> LibraryFuture<'_, LibraryFacts>;

    /// The libraries this host opened that `readable` keeps, at most 20, newest first, for
    /// `actor` (API-S-038).
    fn recent(&self, actor: String, readable: Readable) -> LibraryFuture<'_, RecentLibraries>;
}

impl<L: LibraryLocations> LibraryOperations for LibraryCommands<L> {
    fn create(&self, actor: String, request: LocationRequest) -> LibraryFuture<'_, LibraryView> {
        Box::pin(async move { LibraryCommands::create(self, &actor, request).await })
    }

    fn open(&self, actor: String, request: LocationRequest) -> LibraryFuture<'_, LibraryView> {
        Box::pin(async move { LibraryCommands::open(self, &actor, request).await })
    }

    fn copy_snapshot(
        &self,
        actor: String,
        request: SnapshotCopyRequest,
    ) -> LibraryFuture<'_, CopyReport> {
        Box::pin(async move { LibraryCommands::copy_snapshot(self, &actor, request).await })
    }

    fn rebuild(&self, actor: String, request: LibraryRequest) -> LibraryFuture<'_, RebuildReport> {
        Box::pin(async move { LibraryCommands::rebuild(self, &actor, request).await })
    }

    fn repair_candidates(
        &self,
        actor: String,
        request: LibraryRequest,
    ) -> LibraryFuture<'_, RepairCandidates> {
        Box::pin(async move { LibraryCommands::repair_candidates(self, &actor, request).await })
    }

    fn repair(&self, caller: Principal, request: RepairRequest) -> LibraryFuture<'_, RepairAnswer> {
        Box::pin(async move { LibraryCommands::repair(self, &caller, request).await })
    }

    fn rename(
        &self,
        actor: String,
        request: RenameLibraryRequest,
    ) -> LibraryFuture<'_, LibraryFacts> {
        Box::pin(async move { LibraryCommands::rename(self, &actor, request).await })
    }

    fn facts(&self, actor: String, library_id: LibraryId) -> LibraryFuture<'_, LibraryFacts> {
        Box::pin(async move { LibraryCommands::facts(self, &actor, &library_id).await })
    }

    fn recent(&self, actor: String, readable: Readable) -> LibraryFuture<'_, RecentLibraries> {
        Box::pin(async move { LibraryCommands::recent(self, &actor, readable.as_ref()).await })
    }
}
