//! [`ObjectWriter`]: publishes one new version of one object in OCFL write
//! order (OCFL-S-018; ADR 016 "Write (forward delta)" steps 2 to 6).
//!
//! 1. Assemble the version outside the object root: hash the staged bytes
//!    and build the new inventory. Nothing is written.
//! 2. Reserve `vN` (fenced). Before this, no `vN` byte and no object-root
//!    byte of the version is written. A commit that carries a
//!    [`VersionBoundFiles`] function calls it now, with the reserved
//!    version, and adds its files to the plan (OCFL-S-024).
//! 3. Under the reservation: re-read the committed head and refuse a moved
//!    one, run the Git and hierarchy guards, and reconcile what a crashed
//!    or stale writer left of `vN`.
//! 4. Write the declaration (new objects), the new content, the version
//!    inventory and its sidecar, check the `vN/` listing, then the root
//!    inventory (compare-and-swap on the base) and the root sidecar last.
//!    Every write is preceded by a fence check.
//! 5. Release the reservation and return the [`CommitReceipt`]. The caller
//!    records the receipt in its operational store (ADR 016 step 7); the
//!    engine owns no database.

use std::fmt;

use super::base::{read_base, require_head_inventory, Base};
use super::bound::{bind_files, VersionBoundFiles};
use super::draft::VersionDraft;
use super::error::{CommitError, ReservationError};
use super::fence::Fence;
use super::plan::{assemble, VersionPlan};
use super::publish::{
    publish_head, upload_content, verify_version_dir, write_declaration, write_version_inventory,
};
use super::reconcile::reconcile;
use super::reservation::{Reservation, VersionReservations};
use crate::digest::DigestAlgorithm;
use crate::guard::reject_git_in_root;
use crate::inventory::VersionNum;
use crate::read::small::root_dir;
use crate::storage::StorageBackend;

/// What a commit published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitReceipt {
    /// The object identifier.
    pub object_id: String,
    /// The published version, now the head.
    pub version: VersionNum,
    /// The version's `created` value.
    pub created: String,
    /// The sha256 digest of the published inventory (the root sidecar's).
    pub inventory_digest: String,
    /// Content paths this commit wrote, in order. Content another writer
    /// left with the same bytes is reused and not listed.
    pub written: Vec<String>,
    /// Whether the commit's `vN` reservation took over one that was still
    /// held, not released, when it expired or passed to this writer
    /// ([`Reservation::took_over`]). The previous holder crashed or stalled
    /// and may have left a stray file under a version directory
    /// (OCFL-S-018 "Known gap"), so the host SHOULD validate the object now
    /// (API-S-022).
    pub took_over: bool,
}

/// Writes versions of objects into one storage root, reserving each
/// version in `R` as the writer `holder`.
pub struct ObjectWriter<'a, B, R> {
    pub(super) storage: &'a B,
    pub(super) reservations: &'a R,
    pub(super) holder: String,
}

impl<'a, B: StorageBackend, R: VersionReservations> ObjectWriter<'a, B, R> {
    /// A writer into `storage` that reserves versions in `reservations`.
    /// `holder` names this writer in reservations; it SHOULD be unique per
    /// writer instance.
    pub fn new(storage: &'a B, reservations: &'a R, holder: impl Into<String>) -> Self {
        Self {
            storage,
            reservations,
            holder: holder.into(),
        }
    }

    /// Publish `draft` as the next version of the object at `root`.
    ///
    /// `expected_head` is the head the caller built the draft on (`None`
    /// for a new object); staged bytes are read from `staging`, which MUST
    /// be outside the storage root.
    ///
    /// # Errors
    ///
    /// [`CommitError::HeadMoved`] when the committed head is not
    /// `expected_head`; [`CommitError::InvalidDraft`] or
    /// [`CommitError::UnsupportedAlgorithm`] before anything is reserved;
    /// [`CommitError::Reservation`] when the version is held by another
    /// writer or this writer's reservation was lost; and the errors of each
    /// publication step. On every error the head is unchanged.
    pub async fn commit<S: StorageBackend>(
        &self,
        root: &str,
        expected_head: Option<&VersionNum>,
        staging: &S,
        draft: &VersionDraft,
    ) -> Result<CommitReceipt, CommitError> {
        self.commit_bound(root, expected_head, staging, draft, None)
            .await
    }

    /// Publish `draft` as [`commit`](Self::commit) does, adding the files
    /// `bound` returns for the reserved version (OCFL-S-024). `bound` is
    /// called after `vN` is reserved and before any `vN` byte is written;
    /// when it fails, or returns files the version cannot hold, nothing is
    /// published and the reservation is released.
    ///
    /// # Errors
    ///
    /// As [`commit`](Self::commit), and [`CommitError::BoundFiles`].
    pub async fn commit_bound<S: StorageBackend>(
        &self,
        root: &str,
        expected_head: Option<&VersionNum>,
        staging: &S,
        draft: &VersionDraft,
        bound: Option<&dyn VersionBoundFiles>,
    ) -> Result<CommitReceipt, CommitError> {
        let root = root_dir(root)?;
        let base = read_base(self.storage, &root).await?;
        check_head(&base, expected_head)?;
        base.check_pending_id(draft.object_id())?;
        require_head_inventory(self.storage, &root, &base).await?;
        let plan = assemble(base.inventory(), staging, draft).await?;
        let reservation = self
            .reservations
            .reserve(plan.object_id(), &plan.version, &self.holder)
            .await?;
        let bound_plan = match bound {
            Some(bound) => bind_files(&plan, bound),
            None => Ok(plan),
        };
        let outcome = match bound_plan {
            Ok(plan) => {
                let job = Job {
                    root: &root,
                    plan: &plan,
                    reservation: &reservation,
                    expected_head,
                };
                self.publish(&job, staging).await
            }
            Err(err) => Err(err),
        };
        self.finish(&reservation, outcome).await
    }

    /// Steps 3 and 4, under the reservation.
    async fn publish<S: StorageBackend>(
        &self,
        job: &Job<'_>,
        staging: &S,
    ) -> Result<CommitReceipt, CommitError> {
        let fence = Fence::new(self.reservations, job.reservation);
        let mut base = read_base(self.storage, job.root).await?;
        check_head(&base, job.expected_head)?;
        reject_git_in_root(self.storage, job.root).await?;
        self.storage.check_hierarchy(job.root).await?;
        let kept = reconcile(self.storage, job.root, job.plan, &mut base, &fence).await?;
        if base.committed.is_none() {
            write_declaration(self.storage, job.root, &fence).await?;
        }
        let (tags, written) =
            upload_content(self.storage, staging, job.root, job.plan, kept, &fence).await?;
        write_version_inventory(self.storage, job.root, job.plan, &fence).await?;
        verify_version_dir(self.storage, job.root, job.plan, &tags).await?;
        publish_head(
            self.storage,
            job.root,
            job.plan,
            base.root_tag.as_ref(),
            &fence,
        )
        .await?;
        Ok(receipt(job.plan, written, job.reservation.took_over()))
    }

    /// Release the reservation unless it was lost, and log the outcome.
    async fn finish(
        &self,
        reservation: &Reservation,
        outcome: Result<CommitReceipt, CommitError>,
    ) -> Result<CommitReceipt, CommitError> {
        let lost = matches!(
            outcome,
            Err(CommitError::Reservation(ReservationError::Lost { .. }))
        );
        if !lost {
            if let Err(err) = self.reservations.release(reservation).await {
                tracing::warn!(error = %err, "could not release the version reservation");
            }
        }
        log_outcome(reservation, &outcome);
        outcome
    }
}

impl<B, R> fmt::Debug for ObjectWriter<'_, B, R> {
    /// Show the writer; backends need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectWriter")
            .field("holder", &self.holder)
            .finish_non_exhaustive()
    }
}

/// One commit's fixed inputs under its reservation.
struct Job<'j> {
    root: &'j str,
    plan: &'j VersionPlan,
    reservation: &'j Reservation,
    expected_head: Option<&'j VersionNum>,
}

/// Refuse a committed head other than `expected`.
fn check_head(base: &Base, expected: Option<&VersionNum>) -> Result<(), CommitError> {
    let found = base.head();
    if found == expected {
        return Ok(());
    }
    Err(CommitError::HeadMoved {
        expected: expected.map(ToString::to_string),
        found: found.map(ToString::to_string),
    })
}

/// The receipt for a published plan.
fn receipt(plan: &VersionPlan, written: Vec<String>, took_over: bool) -> CommitReceipt {
    let created = plan
        .inventory
        .head_version()
        .map(|v| v.created.clone())
        .unwrap_or_default();
    CommitReceipt {
        object_id: plan.inventory.id.clone(),
        version: plan.version.clone(),
        created,
        inventory_digest: DigestAlgorithm::ENGINE.digest_hex(&plan.bytes),
        written,
        took_over,
    }
}

/// Log a published version, or why the commit stopped.
fn log_outcome(reservation: &Reservation, outcome: &Result<CommitReceipt, CommitError>) {
    match outcome {
        Ok(receipt) => tracing::info!(
            object = %receipt.object_id,
            version = %receipt.version,
            written = receipt.written.len(),
            "published an OCFL version"
        ),
        Err(err) => tracing::warn!(
            object = %reservation.object_id(),
            version = %reservation.version(),
            error = %err,
            "the commit published nothing"
        ),
    }
}
