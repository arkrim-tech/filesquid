//! [`FileOperations`]: the object-safe handle to a host's [`FileService`], installed in
//! [`crate::ApiState`] with [`crate::ApiState::install_files`] for the file routes and the
//! content route.

use super::copy::{CopyRequest, CopyView};
use super::download::Download;
use super::drive_facts::{CollectionHistorySizeView, FactsAnswer};
use super::error::FileError;
use super::nodes::ScopeObject;
use super::provenance::{Caller, Committed};
use super::scope_listing::{ScopeAfter, ScopeListing, ScopePage};
use super::service::{ContentAnswer, ContentRequest, FileService};
use super::statements::{
    FactsView, FileRecordQuery, MetadataCommitView, NamespacesView, RegisterNamespaceRequest,
    RemoveStatementRequest, RenameRequest, SetStatementRequest, StatementsView, TermQuery,
    TermsView,
};
use super::view::{
    FileDetailsView, FileHistorySizeView, FileQuery, FolderPageView, FolderRequest, FolderView,
    HistoryQuery, HistoryView, LabelRequest, LibraryHistorySizeView, LibraryQuery, ListingQuery,
    MoveRequest, ObjectQuery, RemoveRequest, RestoreRequest, UploadQuery, UploadView, VersionsView,
};
use crate::content::ContentTarget;
use crate::navigation::FolderScope;
use axum::body::Bytes;
use filesquid_core::FileId;
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

    /// The whole-file download of one file version, streamed.
    fn download(&self, query: FileQuery) -> FileFuture<'_, Download>;

    /// The object that holds the files of `scope`, or `None`. May block.
    fn scope_object(&self, scope: &FolderScope) -> Option<ScopeObject>;

    /// One page of a scope's folder from the host's index of registered files, or `None`
    /// when the host keeps none for `scope`. May block.
    ///
    /// # Errors
    /// As the host's index answers.
    fn scope_page(
        &self,
        scope: &FolderScope,
        folder: &str,
        after: Option<&ScopeAfter>,
        limit: usize,
    ) -> Result<Option<ScopePage>, FileError>;

    /// The listing page of a scope's folder that the index answered, each file read from its
    /// own object.
    fn scope_listing(&self, listing: ScopeListing) -> FileFuture<'_, FolderPageView>;

    /// `not_found` unless `target` names a file of an existing version.
    fn check_target(&self, target: ContentTarget) -> FileFuture<'_, ()>;

    /// Upload one file by `actor`: a new file object, or new content for an existing file.
    /// `expected` is the file object's head the caller read, or
    /// [`super::NEW_FILE_REVISION`] for a new file; any other is a `conflict`.
    fn upload(
        &self,
        query: UploadQuery,
        bytes: Bytes,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed<UploadView>>;

    /// Whether a folder exists or stays pending at the object's head `expected`; writes
    /// nothing.
    fn create_folder(&self, request: FolderRequest, expected: String)
        -> FileFuture<'_, FolderView>;

    /// Move or rename one file or folder by `actor`, if the object's head is `expected`.
    fn move_path(
        &self,
        request: MoveRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed>;

    /// Remove one file or folder by `actor`, if the object's head is `expected`.
    fn remove(
        &self,
        request: RemoveRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed>;

    /// Restore files or folders from an earlier version by `actor`, in one version, if the
    /// object's head is `expected`.
    fn restore(
        &self,
        request: RestoreRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed>;

    /// Set or clear the label of one version after commit, by `actor`, if the object's head
    /// is `expected`.
    fn set_label(
        &self,
        request: LabelRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed>;

    /// The statements of one file.
    fn statements(&self, query: FileRecordQuery) -> FileFuture<'_, StatementsView>;

    /// The read-only projection of one file's import record.
    fn facts(&self, query: FileRecordQuery) -> FileFuture<'_, FactsView>;

    /// Where a bare term name can go, in the add order.
    fn terms(&self, query: TermQuery) -> FileFuture<'_, TermsView>;

    /// A library's namespace registry.
    fn namespaces(&self, library_id: String) -> FileFuture<'_, NamespacesView>;

    /// Set one statement by `actor`.
    fn set_statement(
        &self,
        request: SetStatementRequest,
        caller: Caller,
    ) -> FileFuture<'_, Committed<MetadataCommitView>>;

    /// Rename a file (its display name only) by `actor`, if the file object's head is
    /// `expected`.
    fn rename(
        &self,
        request: RenameRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed<MetadataCommitView>>;

    /// Copy a file at its head into a new file by `actor`, if the source's head is
    /// `expected`.
    fn copy(
        &self,
        request: CopyRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed<CopyView>>;

    /// Remove one statement by `actor`.
    fn remove_statement(
        &self,
        request: RemoveStatementRequest,
        caller: Caller,
    ) -> FileFuture<'_, Committed<MetadataCommitView>>;

    /// Register a namespace by `actor`.
    fn register_namespace(
        &self,
        request: RegisterNamespaceRequest,
        caller: Caller,
    ) -> FileFuture<'_, Committed<MetadataCommitView>>;

    /// The bytes one file's history keeps.
    fn history_size(&self, query: HistoryQuery) -> FileFuture<'_, FileHistorySizeView>;

    /// The bytes a library's history keeps.
    fn library_history_size(&self, query: LibraryQuery) -> FileFuture<'_, LibraryHistorySizeView>;

    /// What each of `files` of `library_id` is at its head, for the view operation.
    fn file_facts(&self, library_id: String, files: Vec<FileId>) -> FileFuture<'_, FactsAnswer>;

    /// The bytes the history of `files`, the files of `collection_id`, keeps.
    fn files_history_size(
        &self,
        library_id: String,
        collection_id: String,
        files: Vec<FileId>,
    ) -> FileFuture<'_, CollectionHistorySizeView>;
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

    fn download(&self, query: FileQuery) -> FileFuture<'_, Download> {
        Box::pin(self.download(query))
    }

    fn scope_object(&self, scope: &FolderScope) -> Option<ScopeObject> {
        FileService::scope_object(self, scope)
    }

    fn scope_page(
        &self,
        scope: &FolderScope,
        folder: &str,
        after: Option<&ScopeAfter>,
        limit: usize,
    ) -> Result<Option<ScopePage>, FileError> {
        FileService::scope_page(self, scope, folder, after, limit)
    }

    fn scope_listing(&self, listing: ScopeListing) -> FileFuture<'_, FolderPageView> {
        Box::pin(self.scope_listing(listing))
    }

    fn check_target(&self, target: ContentTarget) -> FileFuture<'_, ()> {
        Box::pin(async move { FileService::check_target(self, &target).await })
    }

    fn upload(
        &self,
        query: UploadQuery,
        bytes: Bytes,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed<UploadView>> {
        Box::pin(async move { FileService::upload(self, query, &bytes, caller, &expected).await })
    }

    fn create_folder(
        &self,
        request: FolderRequest,
        expected: String,
    ) -> FileFuture<'_, FolderView> {
        Box::pin(async move { FileService::create_folder(self, request, &expected).await })
    }

    fn move_path(
        &self,
        request: MoveRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed> {
        Box::pin(async move { FileService::move_path(self, request, caller, &expected).await })
    }

    fn remove(
        &self,
        request: RemoveRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed> {
        Box::pin(async move { FileService::remove(self, request, caller, &expected).await })
    }

    fn restore(
        &self,
        request: RestoreRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed> {
        Box::pin(async move { FileService::restore(self, request, caller, &expected).await })
    }

    fn set_label(
        &self,
        request: LabelRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed> {
        Box::pin(async move { FileService::set_label(self, request, caller, &expected).await })
    }

    fn history_size(&self, query: HistoryQuery) -> FileFuture<'_, FileHistorySizeView> {
        Box::pin(self.history_size(query))
    }

    fn statements(&self, query: FileRecordQuery) -> FileFuture<'_, StatementsView> {
        Box::pin(self.statements(query))
    }

    fn facts(&self, query: FileRecordQuery) -> FileFuture<'_, FactsView> {
        Box::pin(self.facts(query))
    }

    fn terms(&self, query: TermQuery) -> FileFuture<'_, TermsView> {
        Box::pin(self.terms(query))
    }

    fn namespaces(&self, library_id: String) -> FileFuture<'_, NamespacesView> {
        Box::pin(async move { FileService::namespaces(self, &library_id).await })
    }

    fn set_statement(
        &self,
        request: SetStatementRequest,
        caller: Caller,
    ) -> FileFuture<'_, Committed<MetadataCommitView>> {
        Box::pin(async move { FileService::set_statement(self, request, caller).await })
    }

    fn rename(
        &self,
        request: RenameRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed<MetadataCommitView>> {
        Box::pin(async move { FileService::rename(self, request, caller, &expected).await })
    }

    fn copy(
        &self,
        request: CopyRequest,
        caller: Caller,
        expected: String,
    ) -> FileFuture<'_, Committed<CopyView>> {
        Box::pin(async move { FileService::copy(self, request, caller, &expected).await })
    }

    fn remove_statement(
        &self,
        request: RemoveStatementRequest,
        caller: Caller,
    ) -> FileFuture<'_, Committed<MetadataCommitView>> {
        Box::pin(async move { FileService::remove_statement(self, request, caller).await })
    }

    fn register_namespace(
        &self,
        request: RegisterNamespaceRequest,
        caller: Caller,
    ) -> FileFuture<'_, Committed<MetadataCommitView>> {
        Box::pin(async move { FileService::register_namespace(self, request, caller).await })
    }

    fn library_history_size(&self, query: LibraryQuery) -> FileFuture<'_, LibraryHistorySizeView> {
        Box::pin(self.library_history_size(query))
    }

    fn file_facts(&self, library_id: String, files: Vec<FileId>) -> FileFuture<'_, FactsAnswer> {
        Box::pin(async move { FileService::file_facts(self, &library_id, files).await })
    }

    fn files_history_size(
        &self,
        library_id: String,
        collection_id: String,
        files: Vec<FileId>,
    ) -> FileFuture<'_, CollectionHistorySizeView> {
        Box::pin(async move {
            FileService::files_history_size(self, &library_id, &collection_id, files).await
        })
    }
}
