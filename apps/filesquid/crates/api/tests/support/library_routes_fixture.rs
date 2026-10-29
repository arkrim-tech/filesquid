//! Shared fixture for the library and repair route tests (Task API-F1.12; API-S-025): real
//! OCFL storage roots on the local filesystem ([`FsBackend`]) that a test host registers as
//! opaque locations ([`TestLocations`], the [`LibraryLocations`] port a host implements), a
//! [`LibraryCommands`] over them installed in an [`ApiState`], a running monitor over an
//! in-memory journal, and an in-memory repair ledger.
//!
//! A rig serves either the desktop owner (per-launch token) or the server administrator, so
//! each test runs the same router on both mounts. Every rig lives in its own directory under
//! the system temporary directory and removes it when dropped.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use filesquid_api::auth::{Authoriser, LaunchToken};
use filesquid_api::jobs::{Jobs, MemoryJobStore};
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, MAX_PAGE};
use filesquid_api::local_library::{
    LibraryCommands, LibraryLocations, LibraryRoot, LocalLibraryService, MemoryRepairLedger,
    RebuildRunner, CONSENT_STATEMENT_ID, REBUILD_JOB_KIND,
};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::storage::{FsBackend, StorageBackend, StorageKey};
use filesquid_ocfl::write::{rfc3339_utc, Clock, LeaseConfig, LeaseStore};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// The fixed start time: 2026-10-18T00:00:00Z in Unix milliseconds.
pub const START_MS: i64 = 1_792_281_600_000;

/// A library ID a test may name.
pub const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// Lease TTL 60 s with a 10 s margin.
const LEASES: LeaseConfig = LeaseConfig {
    ttl_ms: 60_000,
    margin_ms: 10_000,
};

/// Upper bound on journal pages read by [`LibraryRig::journal`] (AGENTS.md §22).
const MAX_JOURNAL_PAGES: usize = 100;

/// Upper bound on job reads by [`LibraryRig::finished_job`] (AGENTS.md §22).
const MAX_JOB_POLLS: usize = 500;

/// Upper bound on directories walked by [`files_under`] (AGENTS.md §22).
const MAX_WALK: usize = 10_000;

/// Rigs created so far in this process, so each gets its own directory.
static RIGS: AtomicU64 = AtomicU64::new(0);

/// A clock the test moves by hand, shared by the lease stores.
#[derive(Debug, Clone)]
pub struct TestClock(pub Arc<AtomicI64>);

impl TestClock {
    /// Move the clock forward by `millis`.
    pub fn advance(&self, millis: i64) {
        self.0.fetch_add(millis, Ordering::SeqCst);
    }

    /// Now, in Unix milliseconds.
    pub fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

impl Clock for TestClock {
    fn now_millis(&self) -> Option<u64> {
        u64::try_from(self.now()).ok()
    }
}

/// The lease store of one location.
pub type Leases = LeaseStore<FsBackend, TestClock>;

/// The storage root of one test location.
pub type TestRoot = LibraryRoot<FsBackend, Leases>;

/// The test host's locations: each opaque ID names a folder under the rig directory, as a
/// desktop names a folder the owner chose. Paths never leave this type.
pub struct TestLocations {
    dir: PathBuf,
    clock: TestClock,
    roots: Mutex<HashMap<String, (PathBuf, TestRoot)>>,
}

impl TestLocations {
    /// Register the folder `dir/<name>` (created empty) as the location `loc-<name>`.
    pub fn register(&self, name: &str) -> String {
        let folder = self.dir.join(name);
        std::fs::create_dir_all(&folder).expect("location folder");
        let backend = FsBackend::new(folder.clone(), self.dir.join(format!("{name}-parts")))
            .expect("filesystem backend");
        let lease_backend = FsBackend::new(
            self.dir.join(format!("{name}-leases")),
            self.dir.join(format!("{name}-lease-parts")),
        )
        .expect("lease backend");
        let leases =
            LeaseStore::new(lease_backend, "leases", self.clock.clone(), LEASES).expect("leases");
        let root = LibraryRoot {
            backend: Arc::new(backend),
            reservations: Arc::new(leases),
        };
        let id = format!("loc-{name}");
        let mut roots = self.roots.lock().expect("locations");
        roots.insert(id.clone(), (folder, root));
        id
    }

    /// Register the folder of location `id` again, under the new ID `alias`, as a host would
    /// when the owner picks the same folder twice.
    pub fn alias(&self, id: &str, alias: &str) {
        let mut roots = self.roots.lock().expect("locations");
        let entry = roots.get(id).expect("registered location").clone();
        roots.insert(alias.to_string(), entry);
    }

    /// The folder of location `id`.
    pub fn folder(&self, id: &str) -> PathBuf {
        let roots = self.roots.lock().expect("locations");
        roots.get(id).expect("registered location").0.clone()
    }

    /// The storage root of location `id`.
    pub fn root(&self, id: &str) -> LibraryRoot<FsBackend, Leases> {
        let roots = self.roots.lock().expect("locations");
        roots.get(id).expect("registered location").1.clone()
    }
}

impl LibraryLocations for TestLocations {
    type Backend = FsBackend;
    type Reservations = Leases;

    fn location(&self, location_id: &str) -> Option<LibraryRoot<FsBackend, Leases>> {
        let roots = self.roots.lock().ok()?;
        roots.get(location_id).map(|(_, root)| root.clone())
    }

    fn overlaps(&self, a: &str, b: &str) -> bool {
        let Ok(roots) = self.roots.lock() else {
            return true;
        };
        match (roots.get(a), roots.get(b)) {
            (Some((pa, _)), Some((pb, _))) => pa.starts_with(pb) || pb.starts_with(pa),
            _ => true,
        }
    }
}

/// One answer of the router, with its body collected.
#[derive(Debug, Clone)]
pub struct Answer {
    /// The status.
    pub status: StatusCode,
    /// Every header.
    pub headers: HeaderMap,
    /// The body bytes.
    pub body: Vec<u8>,
}

impl Answer {
    /// The body as JSON (null when it is not JSON).
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    /// The body as text.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Which mount a rig serves: the desktop owner or the server administrator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mount {
    /// The desktop host: the owner holding the per-launch token.
    Desktop,
    /// `filesquid-server`: the administrator token.
    Server,
}

/// The test host: locations, library commands, API state, journal and ledger.
pub struct LibraryRig {
    /// The rig's directory.
    pub dir: PathBuf,
    /// The lease clock.
    pub clock: TestClock,
    /// The host's locations.
    pub locations: Arc<TestLocations>,
    /// The commands installed in the state.
    pub commands: Arc<LibraryCommands<TestLocations>>,
    /// The API state the router serves.
    pub state: ApiState,
    /// The caller's token.
    pub token: String,
    /// The journal store behind the monitor.
    pub journal: Arc<MemoryJournal>,
    /// The repair ledger.
    pub ledger: Arc<MemoryRepairLedger>,
    /// The running monitor.
    pub monitor: Monitor,
}

impl Drop for LibraryRig {
    /// Stop the journal writer and remove the rig's directory.
    fn drop(&mut self) {
        self.monitor.shutdown();
        if let Err(err) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("could not remove the rig directory: {err}");
        }
    }
}

impl LibraryRig {
    /// A rig on `mount` with no location registered, whose job service runs rebuilds.
    pub fn new(mount: Mount) -> Self {
        let rig = Self::without_rebuild_runner(mount);
        let runner = RebuildRunner::new(rig.jobs(), rig.commands.clone(), "owner");
        assert!(rig
            .state
            .install_job_runner(REBUILD_JOB_KIND, Arc::new(runner)));
        rig
    }

    /// The job service installed in the rig's state.
    pub fn jobs(&self) -> Jobs {
        self.state.jobs().expect("jobs").as_ref().clone()
    }

    /// A rig on `mount` with a job service but no runner for rebuilds: a submitted rebuild
    /// stays pending.
    pub fn without_rebuild_runner(mount: Mount) -> Self {
        let (authoriser, token) = match mount {
            Mount::Desktop => {
                let token = LaunchToken::generate().expect("random token");
                let secret = token.secret().to_string();
                (Authoriser::desktop_owner(&token), secret)
            }
            Mount::Server => {
                let token = "server-admin-token".to_string();
                (Authoriser::server_administrator(token.clone()), token)
            }
        };
        let state = ApiState::new();
        assert!(state.install_authoriser(authoriser));
        let journal = Arc::new(MemoryJournal::new("host"));
        let monitor = Monitor::start(
            journal.clone(),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("host"),
            state.events.clone(),
        );
        state.install_monitor(monitor.clone());
        let dir = rig_dir();
        let clock = TestClock(Arc::new(AtomicI64::new(START_MS)));
        let locations = Arc::new(TestLocations {
            dir: dir.clone(),
            clock: clock.clone(),
            roots: Mutex::new(HashMap::new()),
        });
        let ledger = Arc::new(MemoryRepairLedger::new());
        let service = LocalLibraryService::new(monitor.journal(), ledger.clone());
        let commands = Arc::new(LibraryCommands::new(service, locations.clone()));
        assert!(state.install_libraries(commands.clone()));
        let jobs = Jobs::new(
            Arc::new(MemoryJobStore::new()),
            monitor.journal(),
            monitor.registry().clone(),
            "launch",
        )
        .with_events(state.events.clone());
        assert!(state.install_jobs(Arc::new(jobs)));
        Self {
            dir,
            clock,
            locations,
            commands,
            state,
            token,
            journal,
            ledger,
            monitor,
        }
    }

    /// Send `method` to `uri` with `authorization` (if any) and a JSON body (if any).
    pub async fn send(
        &self,
        method: Method,
        uri: &str,
        authorization: Option<&str>,
        body: Option<Value>,
    ) -> Answer {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(value) = authorization {
            request = request.header("authorization", value);
        }
        let bytes = match body {
            Some(json) => {
                request = request.header("content-type", "application/json");
                json.to_string().into_bytes()
            }
            None => Vec::new(),
        };
        let request = request.body(Body::from(bytes)).expect("request");
        let response = router(self.state.clone())
            .oneshot(request)
            .await
            .expect("answer");
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes()
            .to_vec();
        Answer {
            status,
            headers,
            body,
        }
    }

    /// `POST uri` with the rig's token and a JSON body.
    pub async fn post(&self, uri: &str, body: Value) -> Answer {
        let bearer = format!("Bearer {}", self.token);
        self.send(Method::POST, uri, Some(&bearer), Some(body))
            .await
    }

    /// `GET uri` with the rig's token.
    pub async fn get(&self, uri: &str) -> Answer {
        let bearer = format!("Bearer {}", self.token);
        self.send(Method::GET, uri, Some(&bearer), None).await
    }

    /// Read job `id` through the job route until it has finished (at most
    /// [`MAX_JOB_POLLS`] reads, 10 ms apart); answer its last view.
    pub async fn finished_job(&self, id: &str) -> Value {
        let mut view = Value::Null;
        for _ in 0..MAX_JOB_POLLS {
            let answer = self.get(&format!("/api/v1/jobs/{id}")).await;
            assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
            view = answer.json();
            let state = view["state"].as_str().unwrap_or_default();
            if ["completed", "failed", "cancelled"].contains(&state) {
                return view;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("the job did not finish: {view}");
    }

    /// Create a library through the route on a new location `name`; answer its library ID
    /// and location ID.
    pub async fn create(&self, name: &str) -> (String, String) {
        let location = self.locations.register(name);
        let answer = self
            .post("/api/v1/libraries", json!({ "location_id": location }))
            .await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
        let library = answer.json()["library_id"]
            .as_str()
            .expect("library id")
            .to_string();
        (library, location)
    }

    /// Commit a sha256 object at `object_root` with v1 (`a.txt`) and v2 (`b.txt`) through
    /// the engine directly, then move the lease clock past the lease TTL.
    pub async fn object(&self, location: &str, object_root: &str, object_id: &str) {
        let root = self.locations.root(location);
        let staging = self.staging(&format!("{location}-{object_root}-staging"));
        for (path, bytes) in [("a.txt", b"alpha"), ("b.txt", b"bravo")] {
            let area = StagingArea::new(&staging, "fixture").expect("staging area");
            let model = ChangeModel::new(
                root.backend.as_ref(),
                root.reservations.as_ref(),
                "fixture",
                object_root,
                object_id,
                area,
            );
            model.stage_bytes(path, bytes).await.expect("stage");
            let meta = VersionMeta::new(rfc3339_utc(self.clock.now() as u64));
            model.commit(meta).await.expect("fixture commit");
        }
        self.clock.advance(120_000);
    }

    /// Commit an object as [`LibraryRig::object`] does, then remove its head (v2) version
    /// inventory and sidecar: the W010 shape.
    pub async fn w010_object(&self, location: &str, object_root: &str, object_id: &str) {
        self.object(location, object_root, object_id).await;
        let root = self.locations.root(location);
        for name in ["inventory.json", "inventory.json.sha256"] {
            let key = StorageKey::new(&format!("{object_root}/v2/{name}")).expect("key");
            root.backend.delete(&key).await.expect("remove head file");
        }
    }

    /// Commit one more version (`c.txt`) of `object_id` at `object_root`, as the next writer
    /// would.
    pub async fn commit_v3(
        &self,
        location: &str,
        object_root: &str,
        object_id: &str,
    ) -> Result<(), String> {
        let root = self.locations.root(location);
        let staging = self.staging(&format!("{location}-{object_root}-v3"));
        let area = StagingArea::new(&staging, "fixture").expect("staging area");
        let model = ChangeModel::new(
            root.backend.as_ref(),
            root.reservations.as_ref(),
            "next-writer",
            object_root,
            object_id,
            area,
        );
        model
            .stage_bytes("c.txt", b"charlie")
            .await
            .map_err(|e| e.to_string())?;
        let meta = VersionMeta::new(rfc3339_utc(self.clock.now() as u64));
        model
            .commit(meta)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// A staging backend `name` outside every location.
    fn staging(&self, name: &str) -> FsBackend {
        FsBackend::new(self.dir.join(name), self.dir.join(format!("{name}-parts")))
            .expect("staging backend")
    }

    /// A repair body naming `library` and `objects`, with the owner's consent to exactly them.
    pub fn consented(library: &str, objects: &[&str]) -> Value {
        json!({
            "library_id": library,
            "object_ids": objects,
            "consent": {
                "library_id": library,
                "object_ids": objects,
                "statement_id": CONSENT_STATEMENT_ID,
                "confirmed": true,
            },
        })
    }

    /// Every file under the folder of `location`, relative to it, sorted.
    pub fn files(&self, location: &str) -> Vec<String> {
        files_under(&self.locations.folder(location))
    }

    /// Every journal entry, oldest first.
    pub fn journal(&self) -> Vec<JournalEntry> {
        self.monitor.journal().flush();
        let mut entries = Vec::new();
        let mut query = JournalQuery {
            limit: MAX_PAGE,
            ..JournalQuery::default()
        };
        for _ in 0..MAX_JOURNAL_PAGES {
            let page = self.journal.query(&query).expect("journal query");
            entries.extend(page.entries);
            match page.next_cursor {
                Some(cursor) => query.after_id = Some(cursor),
                None => return entries,
            }
        }
        panic!("journal longer than {MAX_JOURNAL_PAGES} pages");
    }

    /// Journal entries for `operation`.
    pub fn journaled(&self, operation: &str) -> Vec<JournalEntry> {
        self.journal()
            .into_iter()
            .filter(|e| e.operation == operation)
            .collect()
    }

    /// The rig directory as text, which no answer or journal entry may contain.
    pub fn dir_text(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }
}

/// Every file under `folder`, relative to it with `/` separators, sorted.
pub fn files_under(folder: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    for _ in 0..MAX_WALK {
        let Some(dir) = pending.pop() else {
            found.sort();
            return found;
        };
        for entry in std::fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let relative = path.strip_prefix(folder).expect("under the folder");
            found.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    panic!("more than {MAX_WALK} directories under the folder");
}

/// A fresh directory for one rig, under the system temporary directory.
fn rig_dir() -> PathBuf {
    let n = RIGS.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "filesquid-api-library-routes-{}-{n}",
        std::process::id()
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear an old rig directory");
    }
    std::fs::create_dir_all(&dir).expect("rig directory");
    dir
}
