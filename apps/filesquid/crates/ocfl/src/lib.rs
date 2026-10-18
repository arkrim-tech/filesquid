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
//! The Git guard ([`check_storage_root`], [`reject_git_in_root`]) can be run
//! against every backend and target: no `.git` in a storage root or object
//! root, and no soft links, hard links or empty directories in the storage
//! hierarchy (OCFL-S-001). It checks only when called; backend reads and
//! writes do not run it. Callers MUST run it when they open a storage root
//! or an object (object discovery and reads, IO-0.4) and before they publish
//! a version (IO-0.6 onward).
//!
//! OCFL 1.1 file formats (OCFL-S-002), pure and available on every target:
//!
//! - [`digest`]: SHA-256, the engine's content-addressing algorithm, and
//!   SHA-512, which the engine reads.
//! - [`inventory`]: [`Inventory`] types and a strict parser for untrusted
//!   `inventory.json` bytes that names the OCFL 1.1 validation code of the
//!   first rule an inventory breaks; digest sidecars; version names.
//! - [`namaste`]: object and storage-root conformance declarations.
//!
//! Object discovery and reads, structural validation and `vN` publication
//! are later work.

pub mod digest;
mod error;
mod guard;
pub mod inventory;
#[cfg(all(feature = "fs", not(target_arch = "wasm32")))]
mod logical_file;
pub mod namaste;
pub mod storage;

pub use digest::DigestAlgorithm;
pub use error::{OcflError, WriteError};
pub use guard::{check_storage_root, reject_git_in_root};
pub use inventory::{FormatError, Inventory, VersionNum};
#[cfg(all(feature = "fs", not(target_arch = "wasm32")))]
pub use logical_file::{read_logical_file, reject_git_in_ocfl_root, write_logical_file};
pub use namaste::{Declaration, SpecVersion};
