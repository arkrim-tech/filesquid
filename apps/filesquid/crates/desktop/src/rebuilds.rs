//! Rebuilds across launches (Task DE-1.8; API-S-025 **Rebuild as a job**, DE-S-014): the
//! desktop's part of the durable `library.rebuild` job of API-F2.17.
//!
//! At the app's exit the host sets the stop flag the rebuild runner was given
//! ([`filesquid_api::local_library::RebuildRunner::with_stop`]): a running rebuild checkpoints
//! after the object in hand and gives its lease back, pending, and the exit waits for that at
//! most [`REBUILD_STOP_WAIT`] ([`wait_for_rebuilds`]). A rebuild reads its library through the
//! library's open folder, and libraries are opened by the owner, not at launch, so the host
//! hands a library's unfinished rebuild to the runner when that library is opened
//! ([`resume_rebuilds_of`]), and only when this launch reconciled its jobs; a rebuild the owner
//! paused waits for their resume.

use filesquid_api::jobs::{Job, JobState, Jobs, MAX_RECOVERED_JOBS};
use filesquid_api::local_library::{rebuild_source, REBUILD_JOB_KIND};
use filesquid_api::ApiState;
use filesquid_core::LibraryId;
use std::time::{Duration, Instant};

/// Longest the app's exit waits for running rebuilds to give their lease back.
pub const REBUILD_STOP_WAIT: Duration = Duration::from_secs(10);

/// How often the exit looks at the running rebuilds.
const STOP_POLL: Duration = Duration::from_millis(20);

/// Hand every unfinished rebuild of `library` that waits to run (pending and not paused) to
/// the rebuild runner installed in `state`; answer how many were handed over. A store failure
/// or a runner that refuses is logged with its code. Reads the job store.
pub fn resume_rebuilds_of(state: &ApiState, library: &LibraryId) -> usize {
    let (Some(jobs), Some(runner)) = (state.jobs(), state.job_runner(REBUILD_JOB_KIND)) else {
        return 0;
    };
    let source = rebuild_source(library);
    let waiting: Vec<Job> = unfinished_rebuilds(&jobs)
        .into_iter()
        .filter(|job| job.source == source && job.state == JobState::Pending && !job.is_paused())
        .collect();
    waiting
        .iter()
        .filter(|job| {
            runner
                .resume(job)
                .inspect_err(|err| logged("rebuild.resume", err.error_code()))
                .is_ok()
        })
        .count()
}

/// Wait, at most `limit`, until no rebuild runs under a lease, after the stop flag was set;
/// a store that cannot be read counts as a rebuild still running, so the exit waits for it;
/// a rebuild still running at the limit is logged and left to its lease. Reads the job store.
pub fn wait_for_rebuilds(jobs: &Jobs, limit: Duration) {
    let deadline = Instant::now() + limit;
    let steps = limit.as_millis() / STOP_POLL.as_millis().max(1) + 1;
    for _ in 0..steps {
        if !rebuild_running(jobs) {
            return;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        std::thread::sleep(STOP_POLL.min(left));
    }
    logged("rebuild.stop", "stop_timeout");
}

/// Whether a rebuild runs under a lease; `true` when the store cannot be read (logged).
/// Reads the job store.
fn rebuild_running(jobs: &Jobs) -> bool {
    match jobs.unfinished_of(REBUILD_JOB_KIND, MAX_RECOVERED_JOBS) {
        Ok(open) => open.iter().any(|job| job.state == JobState::Running),
        Err(err) => {
            logged("rebuild.list", err.error_code());
            true
        }
    }
}

/// The unfinished rebuild jobs, at most [`MAX_RECOVERED_JOBS`]; none when the store cannot be
/// read (logged). Reads the job store.
fn unfinished_rebuilds(jobs: &Jobs) -> Vec<Job> {
    jobs.unfinished_of(REBUILD_JOB_KIND, MAX_RECOVERED_JOBS)
        .inspect_err(|err| logged("rebuild.list", err.error_code()))
        .unwrap_or_default()
}

/// Log, with its code only, a rebuild step that did not happen (stdout; the journal layer
/// journals warnings).
fn logged(operation: &str, code: &str) {
    tracing::warn!(
        operation,
        error_code = code,
        "a rebuild could not be handed over or stopped"
    );
}
