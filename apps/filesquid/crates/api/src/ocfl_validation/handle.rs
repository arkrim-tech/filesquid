//! [`OcflValidation`]: the object-safe handle to a host's [`ValidationService`], installed in
//! [`crate::ApiState`] for the routes and called by the host's ticker, commit paths and
//! shutdown.
//!
//! Runs it starts go to the background on the Tokio runtime the service was created on (or
//! the one current at the call); the caller gets an answer at once and follows progress in
//! the process registry and on the event channel.

use super::error::ValidationError;
use super::service::{RunLock, ValidationConfig, ValidationService};
use super::store::RunTrigger;
use super::view::{
    FindingView, FindingsView, LibraryRuns, RunAccepted, RunRefusal, RunRequest, RunView,
};
use super::{LOG_TARGET, MAX_FINDINGS_VIEW, RUN_OPERATION};
use crate::journal::format_rfc3339_ms;
use filesquid_core::LibraryId;
use filesquid_ocfl::storage::StorageBackend;
use std::future::Future;
use std::time::Duration;

/// What the routes and the host need from a host's validation service.
pub trait OcflValidation: Send + Sync {
    /// Start runs for `actor`: of the one library `request` names, or of every open library.
    ///
    /// # Errors
    ///
    /// `not_found` for a library that is not open; `already_running` when the one named
    /// library is running (several libraries list theirs in the answer instead);
    /// `unavailable` without a runtime to run on or after shutdown. Each refusal is
    /// journaled.
    fn start_runs(&self, request: RunRequest, actor: &str) -> Result<RunAccepted, ValidationError>;

    /// The open findings (locations only when `show_locations`), the settings and, per open
    /// library, the last run and the next scheduled run.
    ///
    /// # Errors
    ///
    /// `unavailable` (or `internal`) when the findings store fails.
    fn findings_view(&self, show_locations: bool) -> Result<FindingsView, ValidationError>;

    /// Change the schedule and cap for `actor`, journaled.
    ///
    /// # Errors
    ///
    /// `invalid_config` for values out of bounds; nothing changes.
    fn configure(&self, actor: &str, config: ValidationConfig) -> Result<(), ValidationError>;

    /// Start the scheduled runs that are due. The host calls it from its monitor ticker.
    fn tick(&self);

    /// After a commit on `object_id` at `object_root` of `library_id`: validate the object
    /// in the background when the commit took over an expired or lost reservation (see
    /// [`ValidationService::validate_after_commit`]). A check that cannot start or fails is
    /// journaled.
    fn on_commit(
        &self,
        library_id: &LibraryId,
        object_root: &str,
        object_id: &str,
        took_over: bool,
    );

    /// Stop starting runs and cancel the running ones, waiting up to `wait` for them to end.
    /// The host calls it before it stops its journal.
    fn shut_down(&self, wait: Duration) -> bool;
}

impl<B: StorageBackend + 'static> ValidationService<B> {
    /// The open findings, the settings and the runs of every open library (see
    /// [`OcflValidation::findings_view`]).
    ///
    /// # Errors
    ///
    /// `unavailable` (or `internal`) when the findings store fails.
    pub fn findings(&self, show_locations: bool) -> Result<FindingsView, ValidationError> {
        let mut open = self.inner.store.open_findings(MAX_FINDINGS_VIEW + 1)?;
        let truncated = open.len() > MAX_FINDINGS_VIEW;
        open.truncate(MAX_FINDINGS_VIEW);
        let findings = open
            .into_iter()
            .map(|f| FindingView::of(f, show_locations))
            .collect();
        let mut libraries = Vec::new();
        for (library_id, running, due) in self.libraries() {
            let last_run = self.inner.store.last_run(&library_id)?.map(RunView::from);
            libraries.push(LibraryRuns {
                library_id,
                running,
                last_run,
                next_run: due.map(format_rfc3339_ms),
            });
        }
        Ok(FindingsView {
            findings,
            truncated,
            settings: self.config(),
            libraries,
        })
    }

    /// Run `lock`'s library in the background.
    ///
    /// # Errors
    ///
    /// `unavailable` when there is no runtime to run on (the lock is released).
    fn spawn_run(
        &self,
        lock: RunLock<B>,
        trigger: RunTrigger,
        from_start: bool,
        actor: &str,
    ) -> Result<(), ValidationError> {
        let service = self.clone();
        let actor = actor.to_string();
        self.spawn(async move {
            service.run_locked(lock, trigger, from_start, &actor).await;
        })
    }

    /// Spawn `task` on the service's runtime, or the current one.
    ///
    /// # Errors
    ///
    /// `unavailable` when neither exists; `task` is dropped.
    fn spawn(
        &self,
        task: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), ValidationError> {
        let runtime = self
            .inner
            .runtime
            .clone()
            .or_else(|| tokio::runtime::Handle::try_current().ok());
        match runtime {
            Some(handle) => {
                handle.spawn(task);
                Ok(())
            }
            None => {
                tracing::warn!(target: LOG_TARGET, "no runtime for OCFL validation");
                Err(ValidationError::Unavailable)
            }
        }
    }

    /// Lock and start one library's run, journaling a refusal.
    fn start_one(
        &self,
        library_id: &LibraryId,
        trigger: RunTrigger,
        from_start: bool,
        actor: &str,
    ) -> Result<(), ValidationError> {
        self.lock(library_id)
            .and_then(|lock| self.spawn_run(lock, trigger, from_start, actor))
            .inspect_err(|err| self.refused(actor, library_id, err))
    }
}

impl<B: StorageBackend + 'static> OcflValidation for ValidationService<B> {
    fn start_runs(&self, request: RunRequest, actor: &str) -> Result<RunAccepted, ValidationError> {
        if let Some(library_id) = &request.library_id {
            self.start_one(library_id, RunTrigger::Requested, request.from_start, actor)?;
            return Ok(RunAccepted {
                started: vec![library_id.clone()],
                refused: Vec::new(),
            });
        }
        let mut accepted = RunAccepted::default();
        for (library_id, _, _) in self.libraries() {
            match self.start_one(
                &library_id,
                RunTrigger::Requested,
                request.from_start,
                actor,
            ) {
                Ok(()) => accepted.started.push(library_id),
                Err(err) => accepted.refused.push(RunRefusal {
                    library_id,
                    error_code: err.error_code().to_string(),
                }),
            }
        }
        Ok(accepted)
    }

    fn findings_view(&self, show_locations: bool) -> Result<FindingsView, ValidationError> {
        self.findings(show_locations)
    }

    fn configure(&self, actor: &str, config: ValidationConfig) -> Result<(), ValidationError> {
        ValidationService::configure(self, actor, config)
    }

    fn tick(&self) {
        for library_id in self.due_libraries() {
            let started = self.start_one(&library_id, RunTrigger::Scheduled, false, "system");
            if started.is_err() {
                tracing::info!(target: LOG_TARGET, operation = RUN_OPERATION, "scheduled run not started");
            }
        }
    }

    fn on_commit(
        &self,
        library_id: &LibraryId,
        object_root: &str,
        object_id: &str,
        took_over: bool,
    ) {
        if !took_over {
            return;
        }
        let service = self.clone();
        let (library, root, object) = (
            library_id.clone(),
            object_root.to_string(),
            object_id.to_string(),
        );
        let spawned = self.spawn(async move {
            let checked = service
                .validate_after_commit(&library, &root, &object, true)
                .await;
            if let Err(err) = checked {
                service.refused("system", &library, &err);
            }
        });
        if let Err(err) = spawned {
            self.refused("system", library_id, &err);
        }
    }

    fn shut_down(&self, wait: Duration) -> bool {
        ValidationService::shut_down(self, wait)
    }
}
