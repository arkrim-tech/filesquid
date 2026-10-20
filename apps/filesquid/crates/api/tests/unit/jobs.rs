//! Task DE-0.4: durable jobs (API-S-033; NFR-003, ADR 007). The job rules (exclusive leases
//! with fencing, source revalidation on resume, checkpoint-on-quit, bounded recovery) run
//! over the in-memory reference store.

use filesquid_api::jobs::{
    rules, Job, JobError, JobSpec, JobState, JobStore, Jobs, Lease, MemoryJobStore, JOB_COMPLETED,
    JOB_CREATED, JOB_FAILED, JOB_RECOVERED, JOB_REFUSED, JOB_STARTED, JOB_SUSPENDED,
    MAX_CHECKPOINT_BYTES, MAX_RECOVERED_JOBS,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, Outcome};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::metrics::Metrics;
use filesquid_api::registry::{ProcessRegistry, ProcessState};
use filesquid_api::EventBus;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

const START_MS: i64 = 1_760_000_000_000;
const TTL_MS: i64 = 15_000;

/// One host's job service over a shared store, journal and hand-moved clock.
struct Rig {
    store: Arc<MemoryJobStore>,
    journal: Arc<MemoryJournal>,
    writer: JournalWriter,
    registry: ProcessRegistry,
    clock: Arc<AtomicI64>,
}

impl Rig {
    fn new() -> Self {
        let journal = Arc::new(MemoryJournal::new("desktop"));
        let events = EventBus::new();
        let metrics = Arc::new(Metrics::default());
        let writer = JournalWriter::start(
            journal.clone(),
            WriterConfig::default(),
            events.clone(),
            metrics.clone(),
        );
        let clock = Arc::new(AtomicI64::new(START_MS));
        let tick = clock.clone();
        let registry = ProcessRegistry::new(
            writer.sender(),
            events,
            metrics,
            Arc::new(move || tick.load(Ordering::SeqCst)),
        );
        Self {
            store: Arc::new(MemoryJobStore::new()),
            journal,
            writer,
            registry,
            clock,
        }
    }

    /// The job service of the launch `holder`.
    fn jobs(&self, holder: &str) -> Jobs {
        let clock = self.clock.clone();
        Jobs::new(
            self.store.clone(),
            self.writer.sender(),
            self.registry.clone(),
            holder,
        )
        .with_clock(Arc::new(move || clock.load(Ordering::SeqCst)))
        .with_lease_ttl(TTL_MS)
    }

    /// Move the clock forward by `ms`.
    fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    /// Every journal entry for `operation`.
    fn journaled(&self, operation: &str) -> Vec<JournalEntry> {
        self.writer.sender().flush();
        let query = JournalQuery {
            limit: 1000,
            ..JournalQuery::default()
        };
        self.journal
            .query(&query)
            .expect("journal")
            .entries
            .into_iter()
            .filter(|e| e.operation == operation)
            .collect()
    }
}

fn spec(source: &str) -> JobSpec {
    JobSpec {
        kind: "prov.backfill".into(),
        operation_id: "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b".into(),
        source: source.into(),
    }
}

#[test]
fn a_submitted_job_is_pending_with_its_operation_id_and_journaled() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let job = jobs.submit(spec("installation:a")).expect("submitted");
    assert_eq!(job.state, JobState::Pending);
    assert_eq!(job.operation_id, "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b");
    assert_eq!(job.id.len(), 36, "a UUID v4");
    let created = rig.journaled(JOB_CREATED);
    assert_eq!(created.len(), 1);
    assert_eq!(
        created[0].operation_id.as_deref(),
        Some(job.operation_id.as_str())
    );
    assert_eq!(created[0].target_ids, std::slice::from_ref(&job.id));
}

#[test]
fn a_lease_is_exclusive_until_it_expires() {
    let rig = Rig::new();
    let first = rig.jobs("launch-1");
    let job = first.submit(spec("s")).unwrap();
    let running = first.start(&job.id, "s").expect("leased");
    assert_eq!(
        first.start(&job.id, "s").err(),
        Some(JobError::Conflict("lease_held")),
        "a live lease is exclusive within one launch too"
    );
    let second = rig.jobs("launch-2");
    assert_eq!(
        second.start(&job.id, "s").err(),
        Some(JobError::Conflict("lease_held"))
    );
    let refused = rig.journaled(JOB_REFUSED);
    assert_eq!(refused.len(), 2, "each refusal is journaled");
    assert!(refused
        .iter()
        .all(|e| e.operation_id.as_deref() == Some(job.operation_id.as_str())));
    rig.advance(TTL_MS + 1);
    let taken = second
        .start(&job.id, "s")
        .expect("expired lease taken over");
    assert!(taken.fence() > running.fence());
}

#[test]
fn an_expired_worker_cannot_checkpoint_or_finish_after_a_takeover() {
    let rig = Rig::new();
    let first = rig.jobs("launch-1");
    let job = first.submit(spec("s")).unwrap();
    let mut stale = first.start(&job.id, "s").unwrap();
    rig.advance(TTL_MS + 1);
    let mut replacement = rig.jobs("launch-2").start(&job.id, "s").unwrap();
    assert_eq!(
        stale.checkpoint(r#"{"done":["x"]}"#, 1, Some(2)),
        Err(JobError::Fenced)
    );
    assert_eq!(stale.complete(), Err(JobError::Fenced));
    replacement
        .checkpoint(r#"{"done":["y"]}"#, 1, Some(2))
        .expect("the owner checkpoints");
    let stored = rig.store.get(&job.id).unwrap().unwrap();
    assert_eq!(stored.checkpoint.as_deref(), Some(r#"{"done":["y"]}"#));
    assert_eq!(stored.state, JobState::Running);
    replacement.complete().expect("the owner finishes");
    let stored = rig.store.get(&job.id).unwrap().unwrap();
    assert_eq!(stored.state, JobState::Completed);
    assert!(
        rig.journaled(JOB_REFUSED)
            .iter()
            .any(|e| e.error_code.as_deref() == Some("fenced")),
        "the fenced writes are journaled"
    );
    assert_eq!(rig.journaled(JOB_COMPLETED).len(), 1);
}

#[test]
fn suspending_at_quit_keeps_the_checkpoint_and_the_next_launch_resumes_it() {
    let rig = Rig::new();
    let first = rig.jobs("launch-1");
    let job = first.submit(spec("s")).unwrap();
    let mut running = first.start(&job.id, "s").unwrap();
    running
        .checkpoint(r#"{"done":["lib-a"]}"#, 1, Some(2))
        .unwrap();
    running.suspend().expect("checkpointed at quit");
    let stored = rig.store.get(&job.id).unwrap().unwrap();
    assert_eq!(stored.state, JobState::Pending);
    assert_eq!(stored.checkpoint.as_deref(), Some(r#"{"done":["lib-a"]}"#));
    assert_eq!(stored.holder, None, "the lease is given back");
    assert_eq!(rig.journaled(JOB_SUSPENDED).len(), 1);
    let next = rig.jobs("launch-2");
    let resumed = next.start(&job.id, "s").expect("resumed at once");
    assert_eq!(resumed.checkpoint_text(), Some(r#"{"done":["lib-a"]}"#));
    let started = rig.journaled(JOB_STARTED);
    assert_eq!(started.last().unwrap().outcome, Outcome::Resumed);
}

#[test]
fn a_resume_whose_source_changed_fails_the_job_and_reuses_nothing() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let job = jobs.submit(spec("installation:a")).unwrap();
    let mut running = jobs.start(&job.id, "installation:a").unwrap();
    running.checkpoint(r#"{"done":["x"]}"#, 1, None).unwrap();
    running.suspend().unwrap();
    let next = rig.jobs("launch-2");
    assert_eq!(
        next.start(&job.id, "installation:b").err(),
        Some(JobError::SourceChanged)
    );
    let stored = rig.store.get(&job.id).unwrap().unwrap();
    assert_eq!(stored.state, JobState::Failed);
    assert_eq!(stored.error_code.as_deref(), Some("source_changed"));
    assert_eq!(stored.fence, 1, "the changed job was never leased again");
    assert_eq!(
        rig.journaled(JOB_STARTED).len(),
        1,
        "it never started again"
    );
    let failed = rig.journaled(JOB_FAILED);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].error_code.as_deref(), Some("source_changed"));
    assert_eq!(
        next.start(&job.id, "installation:a").err(),
        Some(JobError::Conflict("job_finished"))
    );
}

#[test]
fn recovery_releases_expired_leases_and_lists_every_unfinished_job_oldest_first() {
    let rig = Rig::new();
    let crashed = rig.jobs("launch-1");
    let a = crashed.submit(spec("s")).unwrap();
    rig.advance(1);
    let b = crashed.submit(spec("s")).unwrap();
    rig.advance(1);
    let done = crashed.submit(spec("s")).unwrap();
    let held = crashed.start(&a.id, "s").unwrap();
    crashed.start(&done.id, "s").unwrap().complete().unwrap();
    drop(held);
    rig.advance(TTL_MS + 1);
    let next = rig.jobs("launch-2");
    let unfinished = next.recover().expect("recovered");
    let ids: Vec<&str> = unfinished.iter().map(|j| j.id.as_str()).collect();
    assert_eq!(ids, [a.id.as_str(), b.id.as_str()]);
    assert!(unfinished.iter().all(|j| j.state == JobState::Pending));
    let recovered = rig.journaled(JOB_RECOVERED);
    assert_eq!(
        recovered.len(),
        1,
        "only the crashed run's lease was released"
    );
    assert_eq!(recovered[0].target_ids, std::slice::from_ref(&a.id));
}

#[test]
fn recovery_reads_a_bounded_number_of_jobs() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    for _ in 0..MAX_RECOVERED_JOBS + 5 {
        jobs.submit(spec("s")).unwrap();
    }
    assert_eq!(jobs.recover().unwrap().len(), MAX_RECOVERED_JOBS);
}

#[test]
fn a_checkpoint_past_its_size_cap_is_refused_and_the_old_one_kept() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let job = jobs.submit(spec("s")).unwrap();
    let mut running = jobs.start(&job.id, "s").unwrap();
    running.checkpoint("{}", 0, None).unwrap();
    let huge = "x".repeat(MAX_CHECKPOINT_BYTES + 1);
    assert_eq!(running.checkpoint(&huge, 1, None), Err(JobError::TooLarge));
    let stored = rig.store.get(&job.id).unwrap().unwrap();
    assert_eq!(stored.checkpoint.as_deref(), Some("{}"));
}

#[test]
fn a_running_job_is_a_registered_process_with_progress() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let job = jobs.submit(spec("s")).unwrap();
    let mut running = jobs.start(&job.id, "s").unwrap();
    running.checkpoint("{}", 3, Some(4)).unwrap();
    let view = rig
        .registry
        .snapshot()
        .into_iter()
        .find(|p| p.id == running.process_id())
        .expect("registered");
    assert_eq!(view.kind, "prov.backfill");
    assert_eq!((view.done, view.total), (3, Some(4)));
    running.fail("storage_unavailable").unwrap();
    let view = rig
        .registry
        .snapshot()
        .into_iter()
        .find(|p| p.kind == "prov.backfill")
        .expect("still listed");
    assert_eq!(view.state, ProcessState::Failed);
    let stored = rig.store.get(&job.id).unwrap().unwrap();
    assert_eq!(stored.state, JobState::Failed);
}

#[test]
fn job_journal_entries_carry_no_checkpoint_or_source_text() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let job = jobs.submit(spec("/Users/someone/Pictures")).unwrap();
    let mut running = jobs.start(&job.id, "/Users/someone/Pictures").unwrap();
    running
        .checkpoint(r#"{"path":"/Users/someone/Pictures/secret.jpg"}"#, 1, None)
        .unwrap();
    running.suspend().unwrap();
    let _ = rig.jobs("launch-2").start(&job.id, "/elsewhere");
    rig.writer.sender().flush();
    let page = rig
        .journal
        .query(&JournalQuery {
            limit: 1000,
            ..JournalQuery::default()
        })
        .unwrap();
    let text = serde_json::to_string(&page.entries).unwrap();
    assert!(!text.contains("someone"), "{text}");
    assert!(!text.contains("secret"), "{text}");
    assert!(!text.contains("elsewhere"), "{text}");
}

/// Cubic round 1: a lease authorises writes for its own job only.
#[test]
fn a_lease_of_another_job_is_refused() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let a = jobs.submit(spec("s")).unwrap();
    let b = jobs.submit(spec("s")).unwrap();
    let running_a = jobs.start(&a.id, "s").unwrap();
    drop(jobs.start(&b.id, "s").unwrap());
    let stored_b = rig.store.get(&b.id).unwrap().unwrap();
    let borrowed = Lease {
        job_id: a.id.clone(),
        holder: "launch-1".into(),
        fence: running_a.fence(),
        expires_ms: START_MS + TTL_MS,
    };
    assert_eq!(stored_b.fence, borrowed.fence, "same holder and fence");
    assert_eq!(
        rules::checkpoint(stored_b, &borrowed, ("{}", 0, None), START_MS, TTL_MS),
        Err(JobError::Fenced)
    );
}

/// Cubic round 1: finishing needs a final state.
#[test]
fn finishing_in_a_state_that_is_not_final_is_refused() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let job = jobs.submit(spec("s")).unwrap();
    let running = jobs.start(&job.id, "s").unwrap();
    let stored = rig.store.get(&job.id).unwrap().unwrap();
    let lease = rules::lease_of(&stored).unwrap();
    assert_eq!(
        rules::finish(stored, &lease, (JobState::Pending, None), START_MS),
        Err(JobError::Conflict("state_not_final"))
    );
    running.complete().unwrap();
}

/// Cubic round 1: an update may not change the job's ID.
#[test]
fn an_update_that_changes_the_job_id_stores_nothing() {
    let rig = Rig::new();
    let job = rig.jobs("launch-1").submit(spec("s")).unwrap();
    let renamed = rig.store.update(&job.id, &mut |mut j: Job| {
        j.id = "other".into();
        Ok(j)
    });
    assert!(matches!(renamed, Err(JobError::Failed(_))));
    assert_eq!(rig.store.get(&job.id).unwrap(), Some(job));
    assert_eq!(rig.store.get("other").unwrap(), None);
}

/// Cubic round 1: a renewal keeps a long unit of work's lease alive past its length.
#[test]
fn renewing_keeps_the_lease_past_its_length() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let job = jobs.submit(spec("s")).unwrap();
    let running = jobs.start(&job.id, "s").unwrap();
    for _ in 0..3 {
        rig.advance(TTL_MS / 2);
        running.renew().expect("renewed");
    }
    assert_eq!(
        rig.jobs("launch-2").start(&job.id, "s").err(),
        Some(JobError::Conflict("lease_held"))
    );
    running.complete().expect("still the owner");
}

/// Cubic round 1: the job of a kind is found even past the recovery bound.
#[test]
fn a_job_of_one_kind_is_found_past_the_recovery_bound() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    for _ in 0..MAX_RECOVERED_JOBS {
        jobs.submit(JobSpec {
            kind: "import".into(),
            ..spec("s")
        })
        .unwrap();
    }
    rig.advance(1);
    let backfill = jobs.submit(spec("s")).unwrap();
    assert!(jobs.recover().unwrap().iter().all(|j| j.id != backfill.id));
    let found = jobs.unfinished_of("prov.backfill", 1).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, backfill.id);
}

/// A store that refuses every insert.
struct RefusingStore;

impl JobStore for RefusingStore {
    fn insert(&self, _job: &Job) -> Result<(), JobError> {
        Err(JobError::Unavailable("down".into()))
    }

    fn get(&self, _id: &str) -> Result<Option<Job>, JobError> {
        Ok(None)
    }

    fn update(
        &self,
        _id: &str,
        _change: &mut dyn FnMut(Job) -> Result<Job, JobError>,
    ) -> Result<Job, JobError> {
        Err(JobError::NotFound)
    }

    fn unfinished(&self, _limit: usize) -> Result<Vec<Job>, JobError> {
        Ok(Vec::new())
    }

    fn unfinished_of(&self, _kind: &str, _limit: usize) -> Result<Vec<Job>, JobError> {
        Ok(Vec::new())
    }
}

/// Cubic round 1: a submission the store refuses is journaled.
#[test]
fn a_refused_submission_is_journaled_with_its_operation_id() {
    let rig = Rig::new();
    let jobs = Jobs::new(
        Arc::new(RefusingStore),
        rig.writer.sender(),
        rig.registry.clone(),
        "launch-1",
    );
    assert_eq!(
        jobs.submit(spec("s")).err().map(|e| e.error_code()),
        Some("unavailable")
    );
    let refused = rig.journaled(JOB_REFUSED);
    assert_eq!(refused.len(), 1);
    assert_eq!(
        refused[0].operation_id.as_deref(),
        Some("6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b")
    );
    assert_eq!(refused[0].error_code.as_deref(), Some("unavailable"));
}

/// Cubic round 2: a delayed renewal or checkpoint never moves the lease end backwards.
#[test]
fn a_late_renewal_never_shortens_the_lease() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let job = jobs.submit(spec("s")).unwrap();
    drop(jobs.start(&job.id, "s").unwrap());
    let stored = rig.store.get(&job.id).unwrap().unwrap();
    let lease = rules::lease_of(&stored).unwrap();
    let later = rules::renew(stored, &lease, START_MS + 5_000, TTL_MS).unwrap();
    let end = later.lease_expires_ms;
    let delayed = rules::renew(later.clone(), &lease, START_MS + 1_000, TTL_MS).unwrap();
    assert_eq!(delayed.lease_expires_ms, end);
    let checkpointed =
        rules::checkpoint(later, &lease, ("{}", 0, None), START_MS + 1_000, TTL_MS).unwrap();
    assert_eq!(checkpointed.lease_expires_ms, end);
}

/// Cubic round 2: a stale worker is fenced whatever it sends.
#[test]
fn a_stale_worker_is_fenced_even_with_an_oversized_checkpoint() {
    let rig = Rig::new();
    let jobs = rig.jobs("launch-1");
    let job = jobs.submit(spec("s")).unwrap();
    let mut stale = jobs.start(&job.id, "s").unwrap();
    rig.advance(TTL_MS + 1);
    let huge = "x".repeat(MAX_CHECKPOINT_BYTES + 1);
    assert_eq!(stale.checkpoint(&huge, 1, None), Err(JobError::Fenced));
}
