//! The async, runtime-agnostic storage backend trait (OCFL-S-007, ADR 025
//! Decision 2). The engine reaches bytes only through this trait.

use std::future::Future;

use super::error::StorageError;
use super::key::StorageKey;
use super::types::{ByteRange, CompletedPart, ListPage, ObjectMeta, Precondition, UploadId};

/// `Send` on native targets and nothing on `wasm32`, so backend futures can
/// cross threads in native hosts without forcing `Send` on browser futures.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + ?Sized> MaybeSend for T {}

/// `Send` on native targets and nothing on `wasm32`.
#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSend for T {}

/// `Sync` on native targets and nothing on `wasm32`.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSync: Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Sync + ?Sized> MaybeSync for T {}

/// `Sync` on native targets and nothing on `wasm32`.
#[cfg(target_arch = "wasm32")]
pub trait MaybeSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSync for T {}

/// One storage interface for every backend: local filesystem, S3-compatible
/// object storage and in-memory.
///
/// The trait is async and does not depend on an executor: the host drives the
/// futures with whatever runtime it has (Tokio in `filesquid-server`, the
/// browser event loop on `wasm32`). Futures are `Send` on native targets and
/// carry no `Send` bound on `wasm32` ([`MaybeSend`]).
///
/// Every byte transfer is bounded: [`get_range`](Self::get_range) reads at
/// most [`MAX_CHUNK_BYTES`](super::MAX_CHUNK_BYTES), [`put`](Self::put)
/// writes at most [`MAX_PUT_BYTES`](super::MAX_PUT_BYTES), and larger objects
/// go through multipart upload. Use [`ChunkReader`](super::ChunkReader) and
/// [`MultipartWriter`](super::MultipartWriter) to stream.
///
/// Canonical heads (OCFL root inventories, `vN` reservations) MUST be written
/// with [`put_if`](Self::put_if) so a competing writer can never silently
/// overwrite them (ADR 012).
pub trait StorageBackend: MaybeSend + MaybeSync {
    /// Store `bytes` at `key`, replacing any object there. Readers see either
    /// the old or the new bytes, never a mix.
    ///
    /// # Errors
    ///
    /// [`StorageError::LimitExceeded`] above `MAX_PUT_BYTES`, or a backend error.
    fn put(
        &self,
        key: &StorageKey,
        bytes: &[u8],
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend;

    /// Store `bytes` at `key` only if `precondition` holds (compare-and-swap).
    ///
    /// # Errors
    ///
    /// [`StorageError::PreconditionFailed`] when it does not hold; nothing is
    /// written. Otherwise as [`put`](Self::put).
    fn put_if(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend;

    /// Read the bytes of `range`. Returns fewer bytes than requested at the end
    /// of the object and none when `range` starts at or after the end.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] when no object is stored at `key`.
    fn get_range(
        &self,
        key: &StorageKey,
        range: ByteRange,
    ) -> impl Future<Output = Result<Vec<u8>, StorageError>> + MaybeSend;

    /// Return the size and entity tag of the object at `key`.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] when no object is stored at `key`.
    fn head(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend;

    /// Return whether an object is stored at `key`.
    ///
    /// # Errors
    ///
    /// A backend error; a missing object is `Ok(false)`.
    fn exists(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<bool, StorageError>> + MaybeSend;

    /// List one page of keys that start with `prefix`, in lexical byte order,
    /// strictly after `start_after`, at most `limit` (1 to
    /// [`MAX_LIST_PAGE`](super::MAX_LIST_PAGE)) keys.
    ///
    /// # Errors
    ///
    /// [`StorageError::InvalidKey`] for a prefix that could escape the root,
    /// [`StorageError::LimitExceeded`] for a page above the bound.
    fn list(
        &self,
        prefix: &str,
        start_after: Option<&StorageKey>,
        limit: usize,
    ) -> impl Future<Output = Result<ListPage, StorageError>> + MaybeSend;

    /// Delete the object at `key`. Deleting a missing object succeeds. Intended
    /// for staging and recovery, never for published OCFL versions.
    ///
    /// # Errors
    ///
    /// A backend error.
    fn delete(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<(), StorageError>> + MaybeSend;

    /// Open a multipart upload that will publish at `key` on completion.
    ///
    /// # Errors
    ///
    /// A backend error.
    fn create_multipart(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<UploadId, StorageError>> + MaybeSend;

    /// Upload part `part_number` (1 to [`MAX_PARTS`](super::MAX_PARTS)) of at
    /// most [`MAX_CHUNK_BYTES`](super::MAX_CHUNK_BYTES).
    ///
    /// # Errors
    ///
    /// [`StorageError::UnknownUpload`], [`StorageError::InvalidArgument`] or
    /// [`StorageError::LimitExceeded`].
    fn upload_part(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> impl Future<Output = Result<CompletedPart, StorageError>> + MaybeSend;

    /// Publish the parts, in ascending part order, as one object, optionally
    /// only if `precondition` holds. On success the upload is closed; on
    /// failure it stays open for a retry or [`abort_multipart`](Self::abort_multipart).
    ///
    /// # Errors
    ///
    /// [`StorageError::PartMismatch`], [`StorageError::PreconditionFailed`],
    /// [`StorageError::InvalidArgument`] or [`StorageError::UnknownUpload`].
    fn complete_multipart(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> impl Future<Output = Result<ObjectMeta, StorageError>> + MaybeSend;

    /// Discard an open upload and every part stored for it.
    ///
    /// # Errors
    ///
    /// [`StorageError::UnknownUpload`] when it is not open.
    fn abort_multipart(
        &self,
        upload: &UploadId,
    ) -> impl Future<Output = Result<(), StorageError>> + MaybeSend;
}
