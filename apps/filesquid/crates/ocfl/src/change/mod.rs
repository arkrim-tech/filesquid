//! The forward-delta change model (OCFL-S-014 to OCFL-S-016;
//! [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md)
//! "Change model (public API)"): a git-like stage, status, diff, commit,
//! log, checkout, revert and reinstate over the engine's only write path,
//! [`ObjectWriter::commit`](crate::write::ObjectWriter::commit).
//!
//! - [`compute_delta`]: given a working state (logical path → digest) and a
//!   base version, classify every path as added, changed, renamed, deleted
//!   or reinstated ([`PathChange`]). Only added or changed content whose
//!   digest the `manifest` lacks is written; renames, deletes and
//!   reinstatements change `state` only.
//! - [`StagingArea`]: where changes accumulate against a recorded base
//!   head, on a caller backend under a caller prefix outside every OCFL
//!   root (a directory on native, browser staging storage on `wasm32`).
//!   It is engine-private working data, never an OCFL extension, and never
//!   needed to read or recover an object. Format: [`index`] module docs.
//! - [`ChangeModel`]: the operations. Each commit, revert or reinstate
//!   publishes exactly one version through the fenced `vN` reservation and
//!   head compare-and-swap; a moved head is rebased onto when no staged
//!   path changed in the intervening versions, and is otherwise a
//!   [`ChangeError::Conflict`] that publishes nothing. A head without its
//!   version inventory (W010) stays refused (`MissingVersionInventory`);
//!   the change model never repairs it (OCFL-S-020).
//!
//! The UI view API and `fsquid` MUST call this model; there is no second
//! write path (OCFL-S-016).

mod area;
mod commit;
mod delta;
mod error;
pub mod index;
mod model;

pub use area::StagingArea;
pub use commit::MAX_REBASES;
pub(crate) use delta::state_digests;
pub use delta::{compute_delta, PathChange, WorkingState};
pub use error::ChangeError;
pub use model::{ChangeModel, Status, VersionMeta};
