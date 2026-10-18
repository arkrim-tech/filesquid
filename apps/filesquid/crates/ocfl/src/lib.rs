//! Stand-alone FOSS OCFL 1.1 engine and its own tool (`filesquid-ocfl`,
//! import `filesquid_ocfl`).
//!
//! Default public API is generic OCFL. FileSquid is one caller. Digital
//! preservation programmes can install this crate without FileSquid. The crate
//! remains extractable to its own Git repository
//! ([ADR 016](../../../resources/architecture/016-rust-ocfl-library.md),
//! [ADR 023](../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)).
//!
//! The engine reaches bytes only through the async, runtime-agnostic
//! [`storage::StorageBackend`] trait; the host chooses the backend
//! ([ADR 025](../../../resources/architecture/025-client-side-filesquid-engines.md)).
//! With default features off the crate builds for `wasm32-unknown-unknown`.
//!
//! Features:
//!
//! - `fs` (default): the local filesystem backend [`storage::FsBackend`] and
//!   the path-based helpers [`write_logical_file`], [`read_logical_file`] and
//!   [`reject_git_in_ocfl_root`]. Native targets only.
//!
//! Inventory reading, validation and `vN` publication are later work; this
//! crate currently provides the storage layer and encodes the Git prohibition.

mod error;
#[cfg(all(feature = "fs", not(target_arch = "wasm32")))]
mod logical_file;
pub mod storage;

pub use error::{OcflError, WriteError};
#[cfg(all(feature = "fs", not(target_arch = "wasm32")))]
pub use logical_file::{read_logical_file, reject_git_in_ocfl_root, write_logical_file};
