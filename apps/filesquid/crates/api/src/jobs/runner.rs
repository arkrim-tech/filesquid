//! [`JobRunner`]: how a host hands a resumed job back to the worker of its kind (API-S-037).

use super::model::{Job, JobError};

/// The host's worker for one job kind. `POST /api/v1/jobs/{job}/resume` calls it after the
/// job was resumed; it starts the job (through [`super::Jobs::start`]) on its own thread or
/// queue and returns at once.
pub trait JobRunner: Send + Sync {
    /// Queue `job` to run from its checkpoint.
    ///
    /// # Errors
    /// [`JobError::Unavailable`] when the worker cannot take it now; the job stays pending.
    fn resume(&self, job: &Job) -> Result<(), JobError>;
}
