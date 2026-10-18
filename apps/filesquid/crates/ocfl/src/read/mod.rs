//! Read and browse OCFL 1.1 storage roots and objects through any
//! [`StorageBackend`](crate::storage::StorageBackend) (OCFL-S-004;
//! [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md),
//! [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)
//! Decisions 2 to 4).
//!
//! - [`StorageRoot::open`]: check the storage-root declaration, run the
//!   storage-root guard and discover the object roots in one bounded walk.
//! - [`Object::open`]: guard the object root, check its declaration, read
//!   the root inventory and its sidecar, and check the head version
//!   inventory. Only what the root inventory commits is visible: a `vN`
//!   above its `head` is unpublished (ADR 016 "Published head (readers)").
//! - [`Object::state`] / [`resolve_state`]: the logical state of a
//!   **caller-pinned** version, resolved through `state` and `manifest`
//!   only, so unchanged bytes kept in an older version directory are found
//!   and files the inventory does not list are not (T-039). The state and
//!   its [`LogicalFile`]s carry the object id, version, logical path, digest
//!   and content path that T-039 names.
//! - [`Object::stream_file`] and [`Object::copy_file`]: read one logical file
//!   in bounded chunks and check it against its inventory digest (E092).
//! - [`Object::history`] / [`version_history`] and [`Object::diff`] /
//!   [`diff_versions`]: version history and the diff of two versions.
//!
//! Reading is not validation: structural validation of every OCFL rule is
//! [`crate::validate`]. A reader checks what it needs to return correct
//! bytes and refuses with the OCFL 1.1 code of the rule it meets broken.

pub(crate) mod committed;
pub(crate) mod declaration;
mod error;
mod history;
mod object;
mod root;
pub(crate) mod small;
mod state;
mod stream;

pub use declaration::MAX_DECLARATION_BYTES;
pub use error::ReadError;
pub use history::{diff_versions, version_history, Change, VersionInfo};
pub use object::Object;
pub use root::StorageRoot;
pub use state::{resolve_state, LogicalFile, LogicalState};
pub use stream::LogicalFileReader;
