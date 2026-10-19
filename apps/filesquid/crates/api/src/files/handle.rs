//! [`FileOperations`]: the object-safe handle to a host's [`FileService`], installed in
//! [`crate::ApiState`] with [`crate::ApiState::install_files`] for the file routes and the
//! content route.

use super::error::FileError;
use super::service::{ContentAnswer, ContentRequest, FileService};
use super::view::{
    CommitView, FileDetailsView, FileQuery, FolderPageView, FolderRequest, FolderView,
    HistoryQuery, HistoryView, ListingQuery, MoveRequest, ObjectQuery, RemoveRequest,
    RestoreRequest, UploadQuery, VersionsView,
};
use crate::content::ContentTarget;
use axum::body::Bytes;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::write::VersionReservations;
use std::future::Future;
use std::pin::Pin;

/// The future of one file operation.
pub type FileFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, FileError>> + Send + 'a>>;

/// What the routes need from a host's file service. Every write publishes exactly one OCFL
/// version through the change model or is refused without publishing.
pub trait FileOperations: Send + Sync {
    /// One page of a folder listing.
    fn listing(&self, query: ListingQuery) -> FileFuture<'_, FolderPageView>;

    /// Storage facts about one file of a version.
    fn details(&self, query: FileQuery) -> FileFuture<'_, FileDetailsView>;

    /// The versions that changed one file.
    fn history(&self, query: HistoryQuery) -> FileFuture<'_, HistoryView>;

    /// Every version of an object.
    fn versions(&self, query: ObjectQuery) -> FileFuture<'_, VersionsView>;

    /// The bytes and facts of one content answer.
    fn content(&self, request: ContentRequest) -> FileFuture<'_, ContentAnswer>;

    /// `not_found` unless `target` names a file of an existing version.
    fn check_target(&self, target: ContentTarget) -> FileFuture<'_, ()>;

    /// Add or replace one file by `actor`.
    fn upload(&self, query: UploadQuery, bytes: Bytes, actor: String)
        -> FileFuture<'_, CommitView>;

    /// Whether a folder exists or stays pending; writes nothing.
    fn create_folder(&self, request: FolderRequest) -> FileFuture<'_, FolderView>;

    /// Move or rename one file or folder by `actor`.
    fn move_path(&self, request: MoveRequest, actor: String) -> FileFuture<'_, CommitView>;

    /// Remove one file or folder by `actor`.
    fn remove(&self, request: RemoveRequest, actor: String) -> FileFuture<'_, CommitView>;

    /// Restore files or folders from an earlier version by `actor`.
    fn restore(&self, request: RestoreRequest, actor: String) -> FileFuture<'_, CommitView>;
}

impl<B, R, S> FileOperations for FileService<B, R, S>
where
    B: StorageBackend + 'static,
    R: VersionReservations + 'static,
    S: StorageBackend + 'static,
{
    fn listing(&self, query: ListingQuery) -> FileFuture<'_, FolderPageView> {
        Box::pin(self.listing(query))
    }

    fn details(&self, query: FileQuery) -> FileFuture<'_, FileDetailsView> {
        Box::pin(self.details(query))
    }

    fn history(&self, query: HistoryQuery) -> FileFuture<'_, HistoryView> {
        Box::pin(self.history(query))
    }

    fn versions(&self, query: ObjectQuery) -> FileFuture<'_, VersionsView> {
        Box::pin(self.versions(query))
    }

    fn content(&self, request: ContentRequest) -> FileFuture<'_, ContentAnswer> {
        Box::pin(self.content(request))
    }

    fn check_target(&self, target: ContentTarget) -> FileFuture<'_, ()> {
        Box::pin(async move { FileService::check_target(self, &target).await })
    }

    fn upload(
        &self,
        query: UploadQuery,
        bytes: Bytes,
        actor: String,
    ) -> FileFuture<'_, CommitView> {
        Box::pin(async move { FileService::upload(self, query, &bytes, &actor).await })
    }

    fn create_folder(&self, request: FolderRequest) -> FileFuture<'_, FolderView> {
        Box::pin(self.create_folder(request))
    }

    fn move_path(&self, request: MoveRequest, actor: String) -> FileFuture<'_, CommitView> {
        Box::pin(async move { FileService::move_path(self, request, &actor).await })
    }

    fn remove(&self, request: RemoveRequest, actor: String) -> FileFuture<'_, CommitView> {
        Box::pin(async move { FileService::remove(self, request, &actor).await })
    }

    fn restore(&self, request: RestoreRequest, actor: String) -> FileFuture<'_, CommitView> {
        Box::pin(async move { FileService::restore(self, request, &actor).await })
    }
}
