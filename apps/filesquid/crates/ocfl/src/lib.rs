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
//! Read and browse (OCFL-S-004), on every backend and target:
//!
//! - [`read`]: open a [`StorageRoot`] and discover its objects; open an
//!   [`Object`]; resolve the logical state of a caller-pinned version through
//!   the inventory; stream or copy a logical file checked against its digest;
//!   version history and the diff of two versions.
//!
//! Structural validation (OCFL-S-009), on every backend and target:
//!
//! - [`validate`]: [`validate_object`] reports each broken OCFL 1.1 rule it
//!   reaches as a [`Finding`] with its validation code (`E###`, `W###`);
//!   one inventory is checked up to its first broken rule;
//!   only the root inventory commits a version.
//! - [`fixity`]: the fixity-table algorithms a validator verifies (md5,
//!   sha1, sha256, sha512, blake2b-512).
//!
//! Write (OCFL-S-018), on every backend and target:
//!
//! - [`write`](mod@write): [`ObjectWriter::commit`] publishes one new version in OCFL
//!   write order under a fenced reservation of `vN`
//!   ([`VersionReservations`], [`LeaseStore`]): content, version inventory
//!   and sidecar, then the root inventory and its sidecar last. An
//!   incomplete `vN` stays unpublished for readers and is reconciled by the
//!   next commit of that version.
//!
//! Opaque bytes (OCFL-S-020;
//! [ADR 010](../../../resources/architecture/010-ocfl-1-1-dual-profile.md)):
//! the engine stores the content bytes its caller supplies and reads them
//! only as bytes. Logical paths are UTF-8 strings stored as given, and they
//! must pass the OCFL 1.1 path rules (E052, E053) like any other. It
//! computes `sha256` over exactly what was stored, so manifest digest
//! verification (E092) holds over ciphertext too. FileSquid's
//! owner-readable local profile has no transform: its plaintext is staged
//! as is. Only the encrypted hosted package (sealed content and encoded
//! names) is a transform, which the caller applies before staging and after
//! reading. Both use the same [`ObjectWriter`] and [`Object`] calls; the
//! engine has no profile, key or
//! encryption type and no encryption dependency. Over sealed bytes and
//! opaque names the inventory stays structurally readable OCFL 1.1:
//! `created` is still an RFC 3339 timestamp, and any `message` or `user`
//! text is whatever the caller supplied.

pub mod digest;
mod error;
pub mod fixity;
mod guard;
pub mod inventory;
#[cfg(all(feature = "fs", not(target_arch = "wasm32")))]
mod logical_file;
pub mod namaste;
pub mod read;
pub mod storage;
pub mod validate;
pub mod write;

pub use digest::DigestAlgorithm;
pub use error::{OcflError, WriteError};
pub use guard::{check_storage_root, reject_git_in_root};
pub use inventory::{FormatError, Inventory, VersionNum};
#[cfg(all(feature = "fs", not(target_arch = "wasm32")))]
pub use logical_file::{read_logical_file, reject_git_in_ocfl_root, write_logical_file};
pub use namaste::{Declaration, SpecVersion};
pub use read::{LogicalFile, LogicalState, Object, ReadError, StorageRoot};
pub use validate::{validate_object, Finding, ValidationReport};
pub use write::{
    CommitError, CommitReceipt, LeaseStore, ObjectWriter, VersionDraft, VersionReservations,
};
