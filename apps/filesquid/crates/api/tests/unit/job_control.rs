//! Task API-F1.6: controlling durable jobs (API-S-037; NFR-003, ADR 007, CORE-S-006). A job
//! can be paused, resumed and cancelled whether it waits or runs, a worker sees the request at
//! its next checkpoint, a classified kind is held to its cap, every stored change publishes one
//! `job.update` event, and the job view never carries the source or the checkpoint.

use filesquid_api::events::topic;
use filesquid_api::jobs::{
    Halt, Job, JobClass, JobControl, JobError, JobFilter, JobLimits, JobSpec, JobState, JobView,
    Jobs, MemoryJobStore, JOB_CANCELLED, JOB_CANCEL_REQUESTED, JOB_PAUSED, JOB_REFUSED,
    JOB_RESUMED,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::metrics::Metrics;
use filesquid_api::registry::ProcessRegistry;
use filesquid_api::EventBus;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

const START_MS: i64 = 1_760_000_000_000;
const TTL_MS: i64 = 15_000;

/// One host's job service over an in-memory store, journal, event bus and hand-moved clock.
struct Rig {
    store: Arc<MemoryJobStore>,
    journal: Arc<MemoryJournal>,
    writer: JournalWriter,
    registry: ProcessRegistry,
    events: EventBus,
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
            events.clone(),
            metrics,
            Arc::new(move || tick.load(Ordering::SeqCst)),
        );
        Self {
            store: Arc::new(MemoryJobStore::new()),
            journal,
            writer,
            registry,
            events,
            clock,
        }
    }

    fn jobs(&self) -> Jobs {
        let clock = self.clock.clone();
        Jobs::new(
            self.store.clone(),
            self.writer.sender(),
            self.registry.clone(),
            "launch-a",
        )
        .with_clock(Arc::new(move || clock.load(Ordering::SeqCst)))
        .with_lease_ttl(TTL_MS)
        .with_events(self.events.clone())
    }

    fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    fn journaled(&self, operation: &str) -> Vec<JournalEntry> {
        self.writer.sender().flush();
        let query = JournalQuery {
            limit: 1000,
            ..JournalQuery::default()
        };
        let page = self.journal.query(&query).expect("query");
        page.entries
            .into_iter()
            .filter(|e| e.operation == operation)
            .collect()
    }
}

fn spec(kind: &str, op: &str) -> JobSpec {
    JobSpec {
        kind: kind.into(),
        operation_id: op.into(),
        source: format!("source:{op}"),
    }
}

fn submit(jobs: &Jobs, op: &str) -> Job {
    jobs.submit(spec("import", op)).expect("submit")
}

#[test]
fn a_paused_pending_job_does_not_start_until_it_is_resumed() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let paused = jobs.pause(&job.id).expect("pause");
    assert_eq!(paused.control, Some(JobControl::Pause));
    assert_eq!(paused.state, JobState::Pending);
    let refused = jobs.start(&job.id, &job.source).expect_err("held");
    assert_eq!(refused, JobError::Conflict("job_paused"));
    let resumed = jobs.resume(&job.id).expect("resume");
    assert_eq!(resumed.control, None);
    jobs.start(&job.id, &job.source)
        .expect("starts after resume");
    assert_eq!(rig.journaled(JOB_PAUSED).len(), 1);
    assert_eq!(rig.journaled(JOB_RESUMED).len(), 1);
}

#[test]
fn a_running_job_sees_a_pause_at_its_next_checkpoint_and_stays_paused() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let mut running = jobs.start(&job.id, &job.source).expect("start");
    running
        .checkpoint("next=1", 1, Some(4))
        .expect("checkpoint");
    assert_eq!(running.requested(), None);
    jobs.pause(&job.id).expect("pause");
    running
        .checkpoint("next=2", 2, Some(4))
        .expect("checkpoint");
    assert_eq!(running.requested(), Some(JobControl::Pause));
    running.suspend().expect("suspend");
    let stored = jobs.job(&job.id).expect("job");
    assert_eq!(stored.state, JobState::Pending);
    assert_eq!(stored.control, Some(JobControl::Pause));
    assert_eq!(stored.checkpoint.as_deref(), Some("next=2"));
    assert_eq!(
        jobs.start(&job.id, &job.source).expect_err("paused"),
        JobError::Conflict("job_paused")
    );
    jobs.resume(&job.id).expect("resume");
    let again = jobs.start(&job.id, &job.source).expect("start again");
    assert_eq!(
        again.checkpoint_text(),
        Some("next=2"),
        "resumes from its checkpoint"
    );
}

#[test]
fn a_renewal_also_brings_the_request_to_the_worker() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let running = jobs.start(&job.id, &job.source).expect("start");
    jobs.cancel(&job.id).expect("cancel");
    running.renew().expect("renew");
    assert_eq!(running.requested(), Some(JobControl::Cancel));
}

#[test]
fn cancelling_a_pending_job_finishes_it() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let cancelled = jobs.cancel(&job.id).expect("cancel");
    assert_eq!(cancelled.state, JobState::Cancelled);
    assert_eq!(rig.journaled(JOB_CANCELLED).len(), 1);
}

#[test]
fn a_running_job_under_a_cancel_request_is_cancelled_when_it_suspends() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let mut running = jobs.start(&job.id, &job.source).expect("start");
    let requested = jobs.cancel(&job.id).expect("cancel");
    assert_eq!(requested.state, JobState::Running);
    assert_eq!(requested.control, Some(JobControl::Cancel));
    running.checkpoint("next=1", 1, None).expect("checkpoint");
    running.suspend().expect("suspend");
    assert_eq!(jobs.job(&job.id).expect("job").state, JobState::Cancelled);
    assert_eq!(rig.journaled(JOB_CANCEL_REQUESTED).len(), 1);
    assert_eq!(rig.journaled(JOB_CANCELLED).len(), 1);
}

#[test]
fn a_crashed_run_under_a_cancel_request_is_cancelled_at_recovery() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let running = jobs.start(&job.id, &job.source).expect("start");
    jobs.cancel(&job.id).expect("cancel");
    drop(running);
    rig.advance(TTL_MS + 1);
    jobs.recover().expect("recover");
    assert_eq!(jobs.job(&job.id).expect("job").state, JobState::Cancelled);
}

#[test]
fn controls_that_do_not_apply_are_refused_and_journaled() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let running = jobs.start(&job.id, &job.source).expect("start");
    assert_eq!(
        jobs.resume(&job.id).expect_err("running"),
        JobError::Conflict("job_running")
    );
    jobs.cancel(&job.id).expect("cancel");
    assert_eq!(
        jobs.pause(&job.id).expect_err("cancelling"),
        JobError::Conflict("cancel_requested")
    );
    running.cancel().expect("worker stops");
    for refused in [
        jobs.pause(&job.id),
        jobs.resume(&job.id),
        jobs.cancel(&job.id),
    ] {
        assert_eq!(
            refused.expect_err("finished"),
            JobError::Conflict("job_finished")
        );
    }
    assert_eq!(
        jobs.pause("no-such-job").expect_err("unknown"),
        JobError::NotFound
    );
    assert_eq!(rig.journaled(JOB_REFUSED).len(), 6);
}

#[test]
fn the_same_control_twice_changes_nothing_the_second_time() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let first = jobs.pause(&job.id).expect("pause");
    let second = jobs.pause(&job.id).expect("pause again");
    assert_eq!(first, second);
    assert_eq!(rig.journaled(JOB_PAUSED).len(), 1, "journaled once");
}

#[test]
fn a_classified_kind_is_held_to_its_cap() {
    let rig = Rig::new();
    let jobs = rig
        .jobs()
        .with_class("import", JobClass::Transfer)
        .with_limits(JobLimits {
            transfers: 1,
            cpu_heavy: 2,
        });
    let first = submit(&jobs, "op-1");
    let second = submit(&jobs, "op-2");
    let running = jobs.start(&first.id, &first.source).expect("first");
    let refused = jobs.start(&second.id, &second.source).expect_err("full");
    assert_eq!(refused, JobError::Conflict("slots_full"));
    assert_eq!(jobs.job(&second.id).expect("job").state, JobState::Pending);
    running.complete().expect("done");
    jobs.start(&second.id, &second.source)
        .expect("a slot is free");
}

#[test]
fn an_expired_lease_holds_no_slot() {
    let rig = Rig::new();
    let jobs = rig
        .jobs()
        .with_class("import", JobClass::Transfer)
        .with_limits(JobLimits {
            transfers: 1,
            cpu_heavy: 2,
        });
    let first = submit(&jobs, "op-1");
    let second = submit(&jobs, "op-2");
    let crashed = jobs.start(&first.id, &first.source).expect("first");
    drop(crashed);
    rig.advance(TTL_MS + 1);
    jobs.start(&second.id, &second.source)
        .expect("the crashed run holds no slot");
}

#[test]
fn the_default_caps_are_the_core_ones() {
    let limits = JobLimits::default();
    assert_eq!(
        (limits.transfers, limits.cpu_heavy),
        (
            filesquid_core::DEFAULT_ACTIVE_TRANSFERS,
            filesquid_core::DEFAULT_CPU_HEAVY_WORKERS
        )
    );
}

#[test]
fn every_stored_change_publishes_one_job_update_and_a_renewal_none() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let mut feed = rig.events.subscribe();
    let job = submit(&jobs, "op-1");
    let mut running = jobs.start(&job.id, &job.source).expect("start");
    running
        .checkpoint("next=1", 1, Some(2))
        .expect("checkpoint");
    running.renew().expect("renew");
    jobs.pause(&job.id).expect("pause");
    running.suspend().expect("suspend");
    let mut updates = Vec::new();
    while let Ok(event) = feed.try_recv() {
        if event.topic == topic::JOB_UPDATE {
            updates.push(event.data);
        }
    }
    let states: Vec<&str> = updates
        .iter()
        .map(|v| v["state"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        states,
        ["pending", "running", "running", "running", "pending"],
        "submit, start, checkpoint, pause, suspend"
    );
    assert_eq!(updates[4]["paused"], true);
    assert!(updates.iter().all(|v| v.get("source").is_none()));
}

#[test]
fn listing_is_newest_first_filtered_and_bounded() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let ids: Vec<String> = (0..3)
        .map(|n| {
            rig.advance(1);
            submit(&jobs, &format!("op-{n}")).id
        })
        .collect();
    jobs.submit(spec("backfill", "op-x")).expect("other kind");
    jobs.cancel(&ids[0]).expect("cancel");
    let imports = jobs
        .list(&JobFilter {
            kind: Some("import".into()),
            state: None,
            limit: 2,
        })
        .expect("list");
    let listed: Vec<&str> = imports.jobs.iter().map(|j| j.id.as_str()).collect();
    assert_eq!(listed, [ids[2].as_str(), ids[1].as_str()]);
    assert!(imports.truncated);
    let cancelled = jobs
        .list(&JobFilter {
            kind: None,
            state: Some(JobState::Cancelled),
            limit: 100,
        })
        .expect("list");
    assert_eq!(cancelled.jobs.len(), 1);
    assert!(!cancelled.truncated);
}

#[test]
fn the_job_view_carries_no_source_checkpoint_or_lease() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let mut running = jobs.start(&job.id, &job.source).expect("start");
    running
        .checkpoint("next=secret", 1, Some(3))
        .expect("checkpoint");
    let view = JobView::of(&jobs.job(&job.id).expect("job"));
    let text = serde_json::to_string(&view).expect("json");
    for leak in ["source:op-1", "next=secret", "launch-a", "fence"] {
        assert!(!text.contains(leak), "{leak} in {text}");
    }
    assert_eq!(view.done, 1);
    assert_eq!(view.total, Some(3));
    assert!(view.created.ends_with('Z'));
}

/// The `job.update` states received so far on `feed`.
fn update_states(
    feed: &mut tokio::sync::broadcast::Receiver<filesquid_api::ApiEvent>,
) -> Vec<String> {
    let mut states = Vec::new();
    while let Ok(event) = feed.try_recv() {
        if event.topic == topic::JOB_UPDATE {
            states.push(event.data["state"].as_str().unwrap_or_default().to_string());
        }
    }
    states
}

/// Cubic round 1: a cancel, a completion and a recovery publish too.
#[test]
fn ends_and_recoveries_publish_a_job_update() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let mut feed = rig.events.subscribe();
    let waiting = submit(&jobs, "op-1");
    jobs.cancel(&waiting.id).expect("cancel");
    let done = submit(&jobs, "op-2");
    jobs.start(&done.id, &done.source)
        .expect("start")
        .complete()
        .expect("complete");
    let crashed = submit(&jobs, "op-3");
    drop(jobs.start(&crashed.id, &crashed.source).expect("start"));
    rig.advance(TTL_MS + 1);
    jobs.recover().expect("recover");
    assert_eq!(
        update_states(&mut feed),
        [
            "pending",
            "cancelled",
            "pending",
            "running",
            "completed",
            "pending",
            "running",
            "pending"
        ],
        "submit and cancel; submit, start and complete; submit, start and release"
    );
}

/// Cubic round 1: the cap counts every live lease of the class, however many other jobs wait.
#[test]
fn the_cap_counts_a_live_lease_behind_many_waiting_jobs() {
    let rig = Rig::new();
    let jobs = rig
        .jobs()
        .with_class("transfer", JobClass::Transfer)
        .with_limits(JobLimits {
            transfers: 1,
            cpu_heavy: 2,
        });
    for n in 0..1_001 {
        submit(&jobs, &format!("op-wait-{n}"));
    }
    rig.advance(1);
    let first = jobs.submit(spec("transfer", "op-t1")).expect("submit");
    let second = jobs.submit(spec("transfer", "op-t2")).expect("submit");
    let _running = jobs.start(&first.id, &first.source).expect("first");
    let refused = jobs.start(&second.id, &second.source).expect_err("full");
    assert_eq!(refused, JobError::Conflict("slots_full"));
}

/// Cubic round 1: a job that could not start anyway is refused for its own reason, not the cap.
#[test]
fn a_full_class_refuses_a_paused_job_as_paused() {
    let rig = Rig::new();
    let jobs = rig
        .jobs()
        .with_class("import", JobClass::Transfer)
        .with_limits(JobLimits {
            transfers: 1,
            cpu_heavy: 2,
        });
    let first = submit(&jobs, "op-1");
    let second = submit(&jobs, "op-2");
    let _running = jobs.start(&first.id, &first.source).expect("first");
    jobs.pause(&second.id).expect("pause");
    let refused = jobs.start(&second.id, &second.source).expect_err("paused");
    assert_eq!(refused, JobError::Conflict("job_paused"));
}

/// Cubic round 1: a crashed run asked to cancel is never started again, and cancelling it
/// again ends it at once.
#[test]
fn a_crashed_run_asked_to_cancel_is_not_restarted() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    drop(jobs.start(&job.id, &job.source).expect("start"));
    jobs.cancel(&job.id).expect("cancel requested");
    rig.advance(TTL_MS + 1);
    let refused = jobs.start(&job.id, &job.source).expect_err("cancelling");
    assert_eq!(refused, JobError::Conflict("cancel_requested"));
    let crashed = submit(&jobs, "op-2");
    drop(jobs.start(&crashed.id, &crashed.source).expect("start"));
    rig.advance(TTL_MS + 1);
    let cancelled = jobs.cancel(&crashed.id).expect("cancel");
    assert_eq!(cancelled.state, JobState::Cancelled);
}

/// Cubic round 1: a resume that clears the pause before the worker halts keeps the run going;
/// a pause still asked suspends it.
#[test]
fn halting_checks_the_request_in_the_same_step() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let mut running = jobs.start(&job.id, &job.source).expect("start");
    jobs.pause(&job.id).expect("pause");
    running
        .checkpoint("next=2", 1, Some(2))
        .expect("checkpoint");
    assert_eq!(running.requested(), Some(JobControl::Pause));
    jobs.resume(&job.id).expect("resume");
    let Halt::Continued(running) = running.halt_if_requested().expect("halt") else {
        panic!("a resumed run keeps going");
    };
    assert_eq!(running.requested(), None);
    jobs.pause(&job.id).expect("pause again");
    let halted = running.halt_if_requested().expect("halt");
    assert!(matches!(halted, Halt::Halted(JobState::Pending)));
    assert!(jobs.job(&job.id).expect("job").is_paused());
}

#[test]
fn the_view_carries_the_result_once_the_job_completes_and_null_otherwise() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let plain = submit(&jobs, "op-plain");
    let run = jobs.start(&plain.id, &plain.source).expect("start");
    run.complete().expect("complete");
    let view = serde_json::to_value(JobView::of(&jobs.job(&plain.id).expect("job"))).expect("json");
    assert_eq!(
        view["result"],
        serde_json::Value::Null,
        "a kind without one: {view}"
    );
    let job = submit(&jobs, "op-result");
    let mut run = jobs.start(&job.id, &job.source).expect("start");
    run.checkpoint_with_result("next=1", r#"{"objects":[1]}"#, 1, Some(2))
        .expect("checkpoint");
    let running = jobs.job(&job.id).expect("job");
    assert_eq!(running.result.as_deref(), Some(r#"{"objects":[1]}"#));
    assert_eq!(running.checkpoint.as_deref(), Some("next=1"));
    assert_eq!(JobView::of(&running).result, None, "not before it finishes");
    run.complete_with_result(r#"{"objects":[1,2]}"#)
        .expect("complete");
    let done = jobs.job(&job.id).expect("job");
    assert_eq!(done.state, JobState::Completed);
    assert_eq!(
        JobView::of(&done).result,
        Some(serde_json::json!({ "objects": [1, 2] }))
    );
}

#[test]
fn a_checkpoint_with_a_result_under_an_ended_lease_is_fenced_and_stores_nothing() {
    let rig = Rig::new();
    let jobs = rig.jobs();
    let job = submit(&jobs, "op-1");
    let mut run = jobs.start(&job.id, &job.source).expect("start");
    rig.advance(TTL_MS + 1);
    let refused = run
        .checkpoint_with_result("next=1", "{}", 1, Some(2))
        .expect_err("fenced");
    assert_eq!(refused, JobError::Fenced);
    let stored = jobs.job(&job.id).expect("job");
    assert_eq!((stored.checkpoint, stored.result), (None, None));
}
