//! [`LibraryOperations`]: the object-safe handle to a host's [`LibraryCommands`], installed
//! in [`crate::ApiState`] with [`crate::ApiState::install_libraries`] for the library and
//! repair routes.

use super::commands::LibraryCommands;
use super::error::LocalLibraryError;
use super::facts::{LibraryFacts, RecentLibraries, RenameLibraryRequest};
use super::locations::{
    LibraryLocations, LibraryRequest, LibraryView, LocationRequest, Opened, SnapshotCopyRequest,
};
use super::rebuild::{RebuildEnd, RebuildProgress};
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
/// implementation, never by the caller. The commands take the revision the caller expects
/// (`expected`, from its envelope; API-S-019) and refuse a stale one as `conflict`.
pub trait LibraryOperations: Send + Sync {
    /// Create a library in an empty location, for `actor`, expecting
    /// [`crate::envelope::NEW_REVISION`].
    fn create(
        &self,
        actor: String,
        request: LocationRequest,
        expected: String,
    ) -> LibraryFuture<'_, LibraryView>;

    /// Open the library in a location, for `actor`, expecting
    /// [`crate::envelope::NEW_REVISION`] or the current revision of a library already open
    /// there.
    fn open(
        &self,
        actor: String,
        request: LocationRequest,
        expected: String,
    ) -> LibraryFuture<'_, Opened>;

    /// Copy an open library's completed snapshot into an empty location, for `actor`,
    /// expecting the library's current revision.
    fn copy_snapshot(
        &self,
        actor: String,
        request: SnapshotCopyRequest,
        expected: String,
    ) -> LibraryFuture<'_, CopyReport>;

    /// Refuse a rebuild of a library that is not open, or one read at another revision, for
    /// `actor`, before its job is submitted (API-S-025 **Rebuild as a job**).
    fn rebuild_ready(
        &self,
        actor: String,
        request: LibraryRequest,
        expected: String,
    ) -> LibraryFuture<'_, ()>;

    /// Rebuild an open library from its files after the cursor `after`, for `actor`, telling
    /// `progress` about each object, which may stop it: one run of a `library.rebuild` job.
    fn rebuild_from<'a>(
        &'a self,
        actor: String,
        request: LibraryRequest,
        after: Option<String>,
        progress: &'a mut (dyn RebuildProgress + Send),
    ) -> LibraryFuture<'a, RebuildEnd>;

    /// The repair candidates of an open library, for `actor`.
    fn repair_candidates(
        &self,
        actor: String,
        request: LibraryRequest,
    ) -> LibraryFuture<'_, RepairCandidates>;

    /// Run the consented version-inventory repair, for `caller`.
    fn repair(&self, caller: Principal, request: RepairRequest) -> LibraryFuture<'_, RepairAnswer>;

    /// Rename a library open on this host, for `actor`, expecting its current revision
    /// (API-S-038, Task API-F2.19); answers its facts and whether the name changed.
    fn rename(
        &self,
        actor: String,
        request: RenameLibraryRequest,
        expected: String,
    ) -> LibraryFuture<'_, (LibraryFacts, bool)>;

    /// The facts of a library this host opened, for `actor` (API-S-038).
    fn facts(&self, actor: String, library_id: LibraryId) -> LibraryFuture<'_, LibraryFacts>;

    /// The libraries this host opened that `readable` keeps, at most 20, newest first, for
    /// `actor` (API-S-038).
    fn recent(&self, actor: String, readable: Readable) -> LibraryFuture<'_, RecentLibraries>;
}

impl<L: LibraryLocations> LibraryOperations for LibraryCommands<L> {
    fn create(
        &self,
        actor: String,
        request: LocationRequest,
        expected: String,
    ) -> LibraryFuture<'_, LibraryView> {
        Box::pin(async move { LibraryCommands::create(self, &actor, request, &expected).await })
    }

    fn open(
        &self,
        actor: String,
        request: LocationRequest,
        expected: String,
    ) -> LibraryFuture<'_, Opened> {
        Box::pin(async move { LibraryCommands::open(self, &actor, request, &expected).await })
    }

    fn copy_snapshot(
        &self,
        actor: String,
        request: SnapshotCopyRequest,
        expected: String,
    ) -> LibraryFuture<'_, CopyReport> {
        Box::pin(
            async move { LibraryCommands::copy_snapshot(self, &actor, request, &expected).await },
        )
    }

    fn rebuild_ready(
        &self,
        actor: String,
        request: LibraryRequest,
        expected: String,
    ) -> LibraryFuture<'_, ()> {
        Box::pin(async move { LibraryCommands::rebuild_ready(self, &actor, &request, &expected) })
    }

    fn rebuild_from<'a>(
        &'a self,
        actor: String,
        request: LibraryRequest,
        after: Option<String>,
        progress: &'a mut (dyn RebuildProgress + Send),
    ) -> LibraryFuture<'a, RebuildEnd> {
        Box::pin(async move {
            LibraryCommands::rebuild_from(self, &actor, request, after, progress).await
        })
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
        expected: String,
    ) -> LibraryFuture<'_, (LibraryFacts, bool)> {
        Box::pin(async move { LibraryCommands::rename(self, &actor, request, &expected).await })
    }

    fn facts(&self, actor: String, library_id: LibraryId) -> LibraryFuture<'_, LibraryFacts> {
        Box::pin(async move { LibraryCommands::facts(self, &actor, &library_id).await })
    }

    fn recent(&self, actor: String, readable: Readable) -> LibraryFuture<'_, RecentLibraries> {
        Box::pin(async move { LibraryCommands::recent(self, &actor, readable.as_ref()).await })
    }
}
