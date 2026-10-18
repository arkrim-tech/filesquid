//! Writing OCFL 1.1 versions in crash-safe order through any
//! [`StorageBackend`](crate::storage::StorageBackend) (OCFL-S-018;
//! [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md)
//! "Write (forward delta)", [ADR 007](../../../../../../resources/architecture/007-durable-jobs-and-interactive-performance.md)
//! exclusive leases, [ADR 012](../../../../../../resources/architecture/012-s3-compatible-ciphertext-storage.md)
//! conflict guard).
//!
//! - [`ObjectWriter::commit`]: publish one [`VersionDraft`] as the next
//!   version. `vN` is reserved before any of its bytes; content, then the
//!   version inventory and sidecar, then the root inventory and its sidecar
//!   last; the version is assembled (digests, inventory) before anything is
//!   written. Only bytes the manifest lacks are written.
//! - [`VersionReservations`]: the fenced reservation every write passes
//!   through. A writer whose reservation expired or passed to another
//!   writer is refused at its next write and never publishes the head.
//!   [`LeaseStore`] keeps leases in any compare-and-swap backend outside the
//!   storage root (a local directory, or another S3 prefix); a host with
//!   PostgreSQL MAY implement the trait over a generation column instead.
//! - [`Clock`], [`SystemClock`] (native) and [`rfc3339_utc`]: time for
//!   leases and `created` values; `wasm32` callers supply their own clock.
//!
//! Unpublished versions: until the root sidecar commits the new root
//! inventory, readers see the previous head ([`crate::read`]), and the next
//! commit of the same version reconciles what a crashed writer left. This
//! module is the engine's only write path; the change model (stage,
//! status, diff, commit, ...) is built on it.

mod base;
mod clock;
mod commit;
mod draft;
mod error;
mod fence;
mod lease;
mod plan;
mod publish;
mod reconcile;
mod reservation;

#[cfg(not(target_arch = "wasm32"))]
pub use clock::SystemClock;
pub use clock::{rfc3339_utc, Clock};
pub use commit::{CommitReceipt, ObjectWriter};
pub use draft::{DraftFile, VersionDraft};
pub use error::{CommitError, ReservationError};
pub use lease::{LeaseConfig, LeaseStore};
pub use reservation::{Reservation, VersionReservations};
