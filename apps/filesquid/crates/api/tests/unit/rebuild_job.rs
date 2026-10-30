//! Task API-F2.17: the rebuild from files as a durable job (API-S-025 **Rebuild as a job**,
//! API-S-033, API-S-037). The runner reads a library's objects through the host's
//! [`LibraryOperations`], checkpoints a cursor and the partial report every batch, stops at a
//! pause or cancel, resumes from its checkpoint without reading a completed object again,
//! survives a crash, and leaves its report as the job's result.

use filesquid_api::auth::Principal;
use filesquid_api::jobs::{
    Job, JobState, JobView, Jobs, MemoryJobStore, JOB_CANCELLED, JOB_CANCEL_REQUESTED, JOB_PAUSED,
    JOB_RESUMED,
};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::local_library::{
    rebuild_source, submit_rebuild, LibraryFuture, LibraryOperations, LibraryRequest, LibraryView,
    LocalLibraryError, LocationRequest, RebuildEnd, RebuildEntry, RebuildProgress, RebuildReport,
    RebuildRunner, RebuildStep, RebuiltObject, RepairAnswer, RepairCandidates, RepairRequest,
    SnapshotCopyRequest, UnreadableObject, REBUILD_JOB_KIND,
};
use filesquid_api::metrics::Metrics;
use filesquid_api::registry::ProcessRegistry;
use filesquid_api::EventBus;
use filesquid_core::LibraryId;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const START_MS: i64 = 1_760_000_000_000;
const TTL_MS: i64 = 15_000;
const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// What the scripted library does before it reads one object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// Read it.
    Go,
    /// Never finish reading it, as a crashed process never would.
    Hang,
}

/// The hook the scripted library calls with an object's index before it reads it.
type Hook = Arc<dyn Fn(usize) -> Step + Send + Sync>;

/// A library of `count` objects (`obj-00`, `obj-01`...), the third of which has a validation
/// error and the fifth of which is unreadable; it records every object it reads.
struct ScriptedLibrary {
    count: usize,
    read: Mutex<Vec<String>>,
    hook: Mutex<Option<Hook>>,
}

impl ScriptedLibrary {
    fn new(count: usize) -> Arc<Self> {
        Arc::new(Self {
            count,
            read: Mutex::new(Vec::new()),
            hook: Mutex::new(None),
        })
    }

    fn on_object(&self, hook: impl Fn(usize) -> Step + Send + Sync + 'static) {
        *self.hook.lock().expect("hook") = Some(Arc::new(hook));
    }

    fn read(&self) -> Vec<String> {
        self.read.lock().expect("read").clone()
    }

    fn root(index: usize) -> String {
        format!("obj-{index:02}")
    }

    /// The entry of object `index`.
    fn entry(index: usize) -> RebuildEntry {
        if index == 4 {
            return RebuildEntry::Unreadable(UnreadableObject {
                error_code: "E034".into(),
            });
        }
        let errors = match index {
            2 => vec!["E093".to_string()],
            _ => Vec::new(),
        };
        RebuildEntry::Read(RebuiltObject {
            object_id: format!("urn:uuid:{index}"),
            head: "v1".into(),
            files: 1,
            digest_algorithm: "sha512".into(),
            error_codes: errors,
            missing_version_inventory: false,
        })
    }

    /// The report a one-shot rebuild of the whole library answers.
    fn full_report(&self) -> RebuildReport {
        let mut report = RebuildReport {
            objects: Vec::new(),
            unreadable: Vec::new(),
            journal: None,
        };
        for index in 0..self.count {
            match Self::entry(index) {
                RebuildEntry::Read(object) => report.objects.push(object),
                RebuildEntry::Unreadable(item) => report.unreadable.push(item),
            }
        }
        report
    }

    /// The scripted rebuild: every object after `after`, in root order, unless the progress
    /// stops it.
    async fn rebuild(
        &self,
        after: Option<String>,
        progress: &mut (dyn RebuildProgress + Send),
    ) -> Result<RebuildEnd, LocalLibraryError> {
        let roots: Vec<String> = (0..self.count).map(Self::root).collect();
        let start = after.map_or(0, |after| roots.partition_point(|r| *r <= after));
        let total = self.count as u64;
        for (index, root) in roots.iter().enumerate().skip(start) {
            let hook = self.hook.lock().expect("hook").clone();
            if hook.is_some_and(|hook| hook(index) == Step::Hang) {
                std::future::pending::<()>().await;
            }
            self.read.lock().expect("read").push(root.clone());
            let step = RebuildStep {
                root,
                entry: Self::entry(index),
                done: index as u64 + 1,
                total,
            };
            if progress.object(step) == ControlFlow::Break(()) {
                return Ok(RebuildEnd::Stopped);
            }
            tokio::task::yield_now().await;
        }
        Ok(RebuildEnd::Finished {
            total,
            journal: None,
        })
    }
}

/// Every route but the rebuild is unused here.
fn unused<T>() -> LibraryFuture<'static, T> {
    Box::pin(async { Err(LocalLibraryError::Unavailable) })
}

impl LibraryOperations for ScriptedLibrary {
    fn create(&self, _: String, _: LocationRequest) -> LibraryFuture<'_, LibraryView> {
        unused()
    }

    fn open(&self, _: String, _: LocationRequest) -> LibraryFuture<'_, LibraryView> {
        unused()
    }

    fn copy_snapshot(
        &self,
        _: String,
        _: SnapshotCopyRequest,
    ) -> LibraryFuture<'_, filesquid_api::local_library::CopyReport> {
        unused()
    }

    fn rebuild_ready(&self, _: String, request: LibraryRequest) -> LibraryFuture<'_, ()> {
        let known = request.library_id.as_str() == LIBRARY;
        Box::pin(async move {
            match known {
                true => Ok(()),
                false => Err(LocalLibraryError::not_found("library_not_open", "not open")),
            }
        })
    }

    fn rebuild_from<'a>(
        &'a self,
        _: String,
        _: LibraryRequest,
        after: Option<String>,
        progress: &'a mut (dyn RebuildProgress + Send),
    ) -> LibraryFuture<'a, RebuildEnd> {
        Box::pin(self.rebuild(after, progress))
    }

    fn repair_candidates(
        &self,
        _: String,
        _: LibraryRequest,
    ) -> LibraryFuture<'_, RepairCandidates> {
        unused()
    }

    fn repair(&self, _: Principal, _: RepairRequest) -> LibraryFuture<'_, RepairAnswer> {
        unused()
    }

    fn rename(
        &self,
        _: String,
        _: filesquid_api::local_library::RenameLibraryRequest,
    ) -> LibraryFuture<'_, filesquid_api::local_library::LibraryFacts> {
        unused()
    }

    fn facts(
        &self,
        _: String,
        _: LibraryId,
    ) -> LibraryFuture<'_, filesquid_api::local_library::LibraryFacts> {
        unused()
    }

    fn recent(
        &self,
        _: String,
        _: filesquid_api::local_library::Readable,
    ) -> LibraryFuture<'_, filesquid_api::local_library::RecentLibraries> {
        unused()
    }
}

/// A job service over an in-memory store and journal with a hand-moved clock, and a
/// scripted library.
struct Rig {
    store: Arc<MemoryJobStore>,
    journal: Arc<MemoryJournal>,
    writer: JournalWriter,
    registry: ProcessRegistry,
    clock: Arc<AtomicI64>,
    library: Arc<ScriptedLibrary>,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.writer.shutdown();
    }
}

impl Rig {
    fn new(objects: usize) -> Self {
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
            library: ScriptedLibrary::new(objects),
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
    }

    /// A runner over the scripted library that checkpoints every `batch` objects.
    fn runner(&self, batch: usize) -> RebuildRunner {
        RebuildRunner::new(self.jobs(), self.library.clone(), "owner").with_batch(batch)
    }

    fn submit(&self) -> Job {
        let library = LibraryId::new(LIBRARY).expect("library");
        submit_rebuild(&self.jobs(), &library).expect("submit")
    }

    fn job(&self, id: &str) -> Job {
        self.jobs().job(id).expect("job")
    }

    fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    fn journal(&self) -> Vec<JournalEntry> {
        self.writer.sender().flush();
        let query = JournalQuery {
            limit: 1000,
            ..JournalQuery::default()
        };
        self.journal.query(&query).expect("query").entries
    }

    fn journaled(&self, operation: &str) -> Vec<JournalEntry> {
        let entries = self.journal().into_iter();
        entries.filter(|e| e.operation == operation).collect()
    }
}

/// The report a finished job carries in its view.
fn result_of(job: &Job) -> RebuildReport {
    let view = JobView::of(job);
    let result = view.result.expect("a finished rebuild carries its report");
    serde_json::from_value(result).expect("the result is a rebuild report")
}

#[tokio::test]
async fn a_rebuild_is_a_pending_library_rebuild_job_whose_source_names_the_library() {
    let rig = Rig::new(3);
    let job = rig.submit();
    assert_eq!(job.kind, REBUILD_JOB_KIND);
    assert_eq!(job.kind, "library.rebuild");
    assert_eq!(job.state, JobState::Pending);
    let library = LibraryId::new(LIBRARY).expect("library");
    assert_eq!(job.source, rebuild_source(&library));
    assert_eq!(
        JobView::of(&job).result,
        None,
        "no result before it finishes"
    );
}

#[tokio::test]
async fn a_repeated_rebuild_answers_the_unfinished_job() {
    let rig = Rig::new(3);
    let first = rig.submit();
    let second = rig.submit();
    assert_eq!(first.id, second.id, "one rebuild per library at a time");
    rig.runner(2).run(&first.id).await.expect("run");
    let third = rig.submit();
    assert_ne!(
        third.id, first.id,
        "a finished rebuild does not hold a new one"
    );
}

#[tokio::test]
async fn progress_counts_objects_and_the_report_is_the_jobs_result() {
    let rig = Rig::new(7);
    let job = rig.submit();
    let state = rig.runner(3).run(&job.id).await.expect("run");
    assert_eq!(state, JobState::Completed);
    let done = rig.job(&job.id);
    assert_eq!(done.state, JobState::Completed);
    assert_eq!((done.done, done.total), (7, Some(7)), "progress in objects");
    assert_eq!(result_of(&done), rig.library.full_report());
    let report = result_of(&done);
    assert_eq!(report.objects.len(), 6, "objects read");
    assert_eq!(report.unreadable.len(), 1, "objects not readable");
    let invalid: Vec<_> = report
        .objects
        .iter()
        .filter(|o| !o.error_codes.is_empty())
        .collect();
    assert_eq!(invalid.len(), 1, "each object with validation errors");
}

#[tokio::test]
async fn an_empty_library_completes_with_zero_of_zero_objects() {
    let rig = Rig::new(0);
    let job = rig.submit();
    assert_eq!(
        rig.runner(3).run(&job.id).await.expect("run"),
        JobState::Completed
    );
    let done = rig.job(&job.id);
    assert_eq!((done.done, done.total), (0, Some(0)));
    assert_eq!(result_of(&done), rig.library.full_report());
}

#[tokio::test]
async fn the_checkpoint_holds_a_cursor_and_the_partial_report_is_stored_with_it() {
    let rig = Rig::new(6);
    let job = rig.submit();
    let jobs = rig.jobs();
    let id = job.id.clone();
    rig.library.on_object(move |index| {
        if index == 3 {
            jobs.pause(&id).expect("pause");
        }
        Step::Go
    });
    assert_eq!(
        rig.runner(2).run(&job.id).await.expect("run"),
        JobState::Pending
    );
    let held = rig.job(&job.id);
    let checkpoint = held.checkpoint.clone().expect("checkpoint");
    assert!(checkpoint.len() <= filesquid_api::jobs::MAX_CHECKPOINT_BYTES);
    assert!(
        !checkpoint.contains("urn:uuid"),
        "the checkpoint is a cursor, not the report: {checkpoint}"
    );
    let partial: RebuildReport =
        serde_json::from_str(held.result.as_deref().expect("partial result")).expect("partial");
    assert_eq!(
        partial.objects.len() + partial.unreadable.len(),
        held.done as usize,
        "the partial report matches the checkpoint's progress"
    );
    assert_eq!(
        JobView::of(&held).result,
        None,
        "not shown before it finishes"
    );
}

#[tokio::test]
async fn pause_holds_the_rebuild_at_a_checkpoint_and_resume_reads_no_completed_object_again() {
    let rig = Rig::new(8);
    let job = rig.submit();
    let jobs = rig.jobs();
    let id = job.id.clone();
    rig.library.on_object(move |index| {
        if index == 2 {
            jobs.pause(&id).expect("pause");
        }
        Step::Go
    });
    let runner = rig.runner(4);
    assert_eq!(runner.run(&job.id).await.expect("run"), JobState::Pending);
    let held = rig.job(&job.id);
    assert!(held.is_paused(), "held by the pause");
    assert_eq!(
        held.done, 4,
        "it suspended at its next checkpoint, after the batch"
    );
    let before = rig.library.read();
    assert_eq!(before, ["obj-00", "obj-01", "obj-02", "obj-03"]);
    rig.library.on_object(|_| Step::Go);
    rig.jobs().resume(&job.id).expect("resume");
    assert_eq!(runner.run(&job.id).await.expect("run"), JobState::Completed);
    let after = rig.library.read();
    assert_eq!(
        after[before.len()..],
        ["obj-04", "obj-05", "obj-06", "obj-07"],
        "only the objects after the checkpoint are read"
    );
    let done = rig.job(&job.id);
    assert_eq!(
        result_of(&done),
        rig.library.full_report(),
        "no object twice"
    );
    assert_eq!(rig.journaled(JOB_PAUSED).len(), 1);
    assert_eq!(rig.journaled(JOB_RESUMED).len(), 1);
}

#[tokio::test]
async fn a_cancel_asked_during_the_last_object_cancels_instead_of_completing() {
    let rig = Rig::new(3);
    let job = rig.submit();
    let jobs = rig.jobs();
    let id = job.id.clone();
    rig.library.on_object(move |index| {
        if index == 2 {
            jobs.cancel(&id).expect("cancel");
        }
        Step::Go
    });
    let state = rig.runner(100).run(&job.id).await.expect("run");
    assert_eq!(state, JobState::Cancelled);
    assert_eq!(JobView::of(&rig.job(&job.id)).result, None);
}

#[tokio::test]
async fn a_submission_past_the_unfinished_bound_is_refused_instead_of_duplicated() {
    let rig = Rig::new(1);
    let jobs = rig.jobs();
    for index in 0..filesquid_api::jobs::MAX_RECOVERED_JOBS {
        let other = format!("00000000-0000-4000-8000-{index:012}");
        let library = LibraryId::new(&other).expect("library");
        submit_rebuild(&jobs, &library).expect("submit");
    }
    let library = LibraryId::new(LIBRARY).expect("library");
    let refused = submit_rebuild(&jobs, &library).expect_err("bound reached");
    assert_eq!(refused.error.error_code(), "unavailable");
    assert!(!refused.journaled, "the caller journals it");
}

#[tokio::test]
async fn a_paused_rebuild_is_not_started_until_it_is_resumed() {
    let rig = Rig::new(3);
    let job = rig.submit();
    rig.jobs().pause(&job.id).expect("pause");
    let refused = rig.runner(2).run(&job.id).await.expect_err("held");
    assert_eq!(refused.error_code(), "job_paused");
    assert!(rig.library.read().is_empty(), "nothing read");
}

#[tokio::test]
async fn cancel_ends_the_rebuild_as_cancelled() {
    let rig = Rig::new(6);
    let job = rig.submit();
    let jobs = rig.jobs();
    let id = job.id.clone();
    rig.library.on_object(move |index| {
        if index == 1 {
            jobs.cancel(&id).expect("cancel");
        }
        Step::Go
    });
    let state = rig.runner(3).run(&job.id).await.expect("run");
    assert_eq!(state, JobState::Cancelled);
    let cancelled = rig.job(&job.id);
    assert_eq!(cancelled.state, JobState::Cancelled);
    assert_eq!(
        JobView::of(&cancelled).result,
        None,
        "a cancelled job has no result"
    );
    assert!(rig.library.read().len() < 6, "it stopped early");
    assert_eq!(rig.journaled(JOB_CANCEL_REQUESTED).len(), 1);
    assert_eq!(rig.journaled(JOB_CANCELLED).len(), 1);
}

#[tokio::test]
async fn cancelling_a_pending_rebuild_reads_nothing() {
    let rig = Rig::new(3);
    let job = rig.submit();
    rig.jobs().cancel(&job.id).expect("cancel");
    let refused = rig.runner(2).run(&job.id).await.expect_err("finished");
    assert_eq!(refused.error_code(), "job_finished");
    assert!(rig.library.read().is_empty());
    assert_eq!(rig.journaled(JOB_CANCELLED).len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_crash_resumes_from_the_checkpoint() {
    let rig = Rig::new(8);
    let job = rig.submit();
    rig.library
        .on_object(|index| if index == 5 { Step::Hang } else { Step::Go });
    let runner = rig.runner(2);
    let crashed = tokio::time::timeout(Duration::from_millis(300), runner.run(&job.id)).await;
    assert!(crashed.is_err(), "the run never ended: it crashed");
    let left = rig.job(&job.id);
    assert_eq!(
        left.state,
        JobState::Running,
        "the crashed run still holds its lease"
    );
    assert_eq!(
        left.done, 4,
        "the last checkpoint was after the fourth object"
    );
    rig.advance(TTL_MS + 1);
    let recovered = rig.jobs().recover().expect("recover");
    assert_eq!(
        recovered[0].state,
        JobState::Pending,
        "recovery released the lease"
    );
    rig.library.on_object(|_| Step::Go);
    assert_eq!(runner.run(&job.id).await.expect("run"), JobState::Completed);
    let read = rig.library.read();
    assert_eq!(
        read[..5],
        ["obj-00", "obj-01", "obj-02", "obj-03", "obj-04"]
    );
    assert_eq!(
        read[5..],
        ["obj-04", "obj-05", "obj-06", "obj-07"],
        "objects 0 to 3 were checkpointed and not read again; object 4 was not"
    );
    let done = rig.job(&job.id);
    assert_eq!(
        result_of(&done),
        rig.library.full_report(),
        "no object twice"
    );
    assert_eq!(done.attempts, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_long_object_keeps_the_lease() {
    let rig = Rig::new(3);
    let job = rig.submit();
    let clock = rig.clock.clone();
    rig.library.on_object(move |index| {
        if index == 1 {
            for _ in 0..4 {
                clock.fetch_add(TTL_MS / 2, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(60));
            }
        }
        Step::Go
    });
    let runner = rig.runner(1).with_renewal(Duration::from_millis(10));
    assert_eq!(runner.run(&job.id).await.expect("run"), JobState::Completed);
    assert_eq!(result_of(&rig.job(&job.id)), rig.library.full_report());
}

#[tokio::test]
async fn every_control_is_journaled_once_and_no_entry_names_the_source_or_checkpoint() {
    let rig = Rig::new(6);
    let job = rig.submit();
    let jobs = rig.jobs();
    let id = job.id.clone();
    rig.library.on_object(move |index| {
        if index == 1 {
            jobs.pause(&id).expect("pause");
            jobs.pause(&id).expect("a repeated pause changes nothing");
        }
        Step::Go
    });
    let runner = rig.runner(2);
    runner.run(&job.id).await.expect("run");
    rig.library.on_object(|_| Step::Go);
    rig.jobs().resume(&job.id).expect("resume");
    runner.run(&job.id).await.expect("run");
    assert_eq!(rig.journaled(JOB_PAUSED).len(), 1);
    assert_eq!(rig.journaled(JOB_RESUMED).len(), 1);
    let held = rig.job(&job.id);
    let journal = serde_json::to_string(&rig.journal()).expect("json");
    assert!(
        !journal.contains(&held.source),
        "the source is never journaled"
    );
    assert!(!journal.contains("obj-0"), "no object root is journaled");
    assert!(!journal.contains("urn:uuid"), "no report is journaled");
    for entry in rig
        .journal()
        .iter()
        .filter(|e| e.operation.starts_with("job."))
    {
        assert_eq!(entry.target_ids, std::slice::from_ref(&job.id));
        assert_eq!(
            entry.operation_id.as_deref(),
            Some(job.operation_id.as_str())
        );
    }
}

#[tokio::test]
async fn the_view_carries_no_source_checkpoint_lease_or_fence() {
    let rig = Rig::new(3);
    let job = rig.submit();
    rig.runner(1).run(&job.id).await.expect("run");
    let done = rig.job(&job.id);
    let view = serde_json::to_value(JobView::of(&done)).expect("view");
    let text = view.to_string();
    assert!(!text.contains(&done.source));
    for field in [
        "source",
        "checkpoint",
        "holder",
        "fence",
        "lease_expires_ms",
    ] {
        assert!(view.get(field).is_none(), "{field} is not shown");
    }
    assert!(view["result"]["objects"].is_array(), "{view}");
}

#[tokio::test]
async fn a_library_that_is_no_longer_open_fails_the_job_with_its_code() {
    let rig = Rig::new(3);
    let other = LibraryId::new("0d6e1f2a-3b4c-4d5e-8f60-718293a4b5c6").expect("library");
    let job = submit_rebuild(&rig.jobs(), &other).expect("submit");
    let refusing = RebuildRunner::new(rig.jobs(), Arc::new(Refusing), "owner");
    assert_eq!(refusing.run(&job.id).await.expect("run"), JobState::Failed);
    let failed = rig.job(&job.id);
    assert_eq!(failed.error_code.as_deref(), Some("library_not_open"));
    assert_eq!(JobView::of(&failed).result, None);
}

/// A library host that refuses every rebuild as `library_not_open`.
struct Refusing;

impl LibraryOperations for Refusing {
    fn create(&self, _: String, _: LocationRequest) -> LibraryFuture<'_, LibraryView> {
        unused()
    }

    fn open(&self, _: String, _: LocationRequest) -> LibraryFuture<'_, LibraryView> {
        unused()
    }

    fn copy_snapshot(
        &self,
        _: String,
        _: SnapshotCopyRequest,
    ) -> LibraryFuture<'_, filesquid_api::local_library::CopyReport> {
        unused()
    }

    fn rebuild_ready(&self, _: String, _: LibraryRequest) -> LibraryFuture<'_, ()> {
        unused()
    }

    fn rebuild_from<'a>(
        &'a self,
        _: String,
        _: LibraryRequest,
        _: Option<String>,
        _: &'a mut (dyn RebuildProgress + Send),
    ) -> LibraryFuture<'a, RebuildEnd> {
        Box::pin(async { Err(LocalLibraryError::not_found("library_not_open", "not open")) })
    }

    fn repair_candidates(
        &self,
        _: String,
        _: LibraryRequest,
    ) -> LibraryFuture<'_, RepairCandidates> {
        unused()
    }

    fn repair(&self, _: Principal, _: RepairRequest) -> LibraryFuture<'_, RepairAnswer> {
        unused()
    }

    fn rename(
        &self,
        _: String,
        _: filesquid_api::local_library::RenameLibraryRequest,
    ) -> LibraryFuture<'_, filesquid_api::local_library::LibraryFacts> {
        unused()
    }

    fn facts(
        &self,
        _: String,
        _: LibraryId,
    ) -> LibraryFuture<'_, filesquid_api::local_library::LibraryFacts> {
        unused()
    }

    fn recent(
        &self,
        _: String,
        _: filesquid_api::local_library::Readable,
    ) -> LibraryFuture<'_, filesquid_api::local_library::RecentLibraries> {
        unused()
    }
}
