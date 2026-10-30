//! Durable jobs (API-S-033; PRD NFR-003; ADR 007): work with identity that survives a quit
//! or a crash, not a UI process.
//!
//! A job is a row in the host's operational store (SQLite on the desktop, PostgreSQL later
//! on the server) with an idempotent operation ID, an opaque source identity, a bounded
//! checkpoint and an exclusive lease. The rules live here, once, for every host:
//!
//! - [`rules`]: the pure transitions of a job: a start that checks the source and takes an
//!   exclusive lease with a fencing number in one step (a worker can neither checkpoint,
//!   renew nor finish once its lease is gone or taken over, and a lease names its job), a
//!   size-capped checkpoint, suspension at quit (the checkpoint is kept and the lease given
//!   back), finishing in a final state, and the release of a crashed run's expired lease.
//! - [`JobStore`]: what a host stores: insert, read, one atomic read-modify-write, and a
//!   bounded list of unfinished jobs. [`MemoryJobStore`] is the reference store.
//! - [`Jobs`] and [`RunningJob`]: the service a host runs jobs through. Starting a job
//!   revalidates its source identity, registers it in the process registry and journals it;
//!   every state change and every refusal is journaled, with the job and operation IDs and
//!   an error code only, never the source identity or the checkpoint. [`Jobs::recover`] is
//!   the bounded launch-time reconciliation: it reads at most [`MAX_RECOVERED_JOBS`] jobs,
//!   never the media they cover.
//! - Control and caps (API-S-037): [`Jobs::pause`], [`Jobs::resume`] and [`Jobs::cancel`]
//!   change a job in one atomic step, and a running worker sees the request through
//!   [`RunningJob::requested`] at its next checkpoint or renewal; a kind classified as a
//!   [`JobClass`] is held to its cap ([`JobLimits`]); every stored change publishes one
//!   `job.update` event carrying the [`JobView`]. The job routes (`routes`) report and
//!   control jobs, and hand a resumed job to the host's [`JobRunner`] for its kind.
//! - Per-file progress (API-S-037, Task API-F2.16): the [`JobItems`] port answers an upload
//!   job's destination and its files, and [`Jobs::retry`] puts failed files back to queued,
//!   journaling each as `job.item_retried`; [`MemoryJobItems`] is the reference port.

mod items;
mod model;
pub(crate) mod routes;
pub mod rules;
mod runner;
mod service;
mod slots;
mod store;
mod view;

pub use items::{
    display_name, named_page, page_of as page_of_items, requeue, JobDestination, JobItem,
    JobItemPage, JobItemState, JobItems, MemoryJobItems, MAX_LISTED_ITEMS,
};
pub use model::{Job, JobControl, JobError, JobSpec, JobState, Lease};
pub use runner::JobRunner;
pub use service::{mint_job_id, Halt, Jobs, RunningJob};
pub use slots::{JobClass, JobLimits};
pub use store::{
    same_id, JobFilter, JobList, JobStore, MemoryJobStore, SlotKinds, MAX_LISTED_JOBS,
};
pub use view::{JobControlAnswer, JobPage, JobView};

/// Default lease length: a worker that neither checkpoints nor finishes within it loses the
/// job to the next one that starts it.
pub const JOB_LEASE_TTL_MS: i64 = 15_000;

/// Largest checkpoint a job may store, in bytes.
pub const MAX_CHECKPOINT_BYTES: usize = 64 * 1024;

/// Largest result a job may store, in bytes (a rebuild report of about 300,000 objects).
pub const MAX_RESULT_BYTES: usize = 64 * 1024 * 1024;

/// Most unfinished jobs one recovery reads.
pub const MAX_RECOVERED_JOBS: usize = 1_000;

/// Journal operation of a submitted job.
pub const JOB_CREATED: &str = "job.created";
/// Journal operation of a started (`started`) or resumed (`resumed`) job.
pub const JOB_STARTED: &str = "job.started";
/// Journal operation of a job suspended at quit with its checkpoint kept.
pub const JOB_SUSPENDED: &str = "job.suspended";
/// Journal operation of a completed job.
pub const JOB_COMPLETED: &str = "job.completed";
/// Journal operation of a failed job.
pub const JOB_FAILED: &str = "job.failed";
/// Journal operation of a cancelled job.
pub const JOB_CANCELLED: &str = "job.cancelled";
/// Journal operation of a crashed run's expired lease released at recovery.
pub const JOB_RECOVERED: &str = "job.recovered";
/// Journal operation of a refused job call (a held lease, a fenced write, a finished job).
pub const JOB_REFUSED: &str = "job.refused";
/// Journal operation of a pause asked of a job (API-S-037).
pub const JOB_PAUSED: &str = "job.paused";
/// Journal operation of a resumed job (API-S-037).
pub const JOB_RESUMED: &str = "job.resumed";
/// Journal operation of a cancel asked of a running job (API-S-037).
pub const JOB_CANCEL_REQUESTED: &str = "job.cancel_requested";
/// Journal operation of one failed file of an upload job put back to queued by a retry
/// (API-S-037 **Per-file progress**); its targets are the job and the item.
pub const JOB_ITEM_RETRIED: &str = "job.item_retried";
