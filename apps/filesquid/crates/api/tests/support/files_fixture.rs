//! Shared fixture for the file operation and content handle tests (Task API-F1.3): a real
//! OCFL storage root on the local filesystem ([`FsBackend`]) behind a read-counting wrapper,
//! a lease store and a staging area outside the root, a [`FileService`] opened on that root
//! and installed in an [`ApiState`] with a desktop owner token, a running monitor over an
//! in-memory journal, and one hand-moved clock for both the file service and the content
//! handles.
//!
//! Every rig lives in its own directory under the system temporary directory and removes it
//! when dropped.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use filesquid_api::auth::{
    Action, Authenticator, Authoriser, LaunchToken, Policy, Principal, Resource,
};
use filesquid_api::files::{FileLibrary, FileService};
use filesquid_api::home::MemoryHomeCollections;
use filesquid_api::journal::{JournalEntry, JournalQuery, JournalStore, MemoryJournal, MAX_PAGE};
use filesquid_api::local_library::{LocalLibraryService, MemoryRepairLedger};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use filesquid_core::LibraryId;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::storage::{
    ByteRange, CompletedPart, FsBackend, ListPage, ObjectMeta, Precondition, StorageBackend,
    StorageError, StorageKey, UploadId,
};
use filesquid_ocfl::view;
use filesquid_ocfl::write::{Clock, LeaseConfig, LeaseStore};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// The fixed start time: 2026-10-18T00:00:00Z in Unix milliseconds.
pub const START_MS: i64 = 1_792_281_600_000;

/// The library every test names.
pub const LIBRARY: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// The object every test names.
pub const OBJECT: &str = "urn:uuid:0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f11";

/// Where the object lives in the storage root.
const OBJECT_ROOT: &str = "objects/holiday";

/// Lease TTL 60 s with a 10 s margin.
const LEASES: LeaseConfig = LeaseConfig {
    ttl_ms: 60_000,
    margin_ms: 10_000,
};

/// Upper bound on journal pages read by [`FilesRig::journal`] (AGENTS.md §22).
const MAX_JOURNAL_PAGES: usize = 100;

/// Rigs created so far in this process, so each gets its own directory.
static RIGS: AtomicU64 = AtomicU64::new(0);

/// Operation IDs handed out so far by [`operation_id`].
static OPERATIONS: AtomicU64 = AtomicU64::new(0);

/// The multipart boundary of [`upload_body`].
pub const BOUNDARY: &str = "filesquid-test-boundary-7f3a";

/// The `content-type` of an upload built by [`upload_body`].
pub const MULTIPART: &str = "multipart/form-data; boundary=filesquid-test-boundary-7f3a";

/// The library ID every test uses.
pub fn library() -> LibraryId {
    LibraryId::new(LIBRARY).expect("library id")
}

/// The digest the engine records for `bytes` (sha256).
pub fn digest(bytes: &[u8]) -> String {
    DigestAlgorithm::ENGINE.digest_hex(bytes)
}

/// Deterministic content of `len` bytes: byte `i` is `i mod 251`, so a misplaced range shows.
pub fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// A clock shared by the lease store, the file service and the content handles.
#[derive(Debug, Clone)]
pub struct TestClock(pub Arc<AtomicI64>);

impl TestClock {
    /// Now, in Unix milliseconds.
    pub fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }

    /// Move the clock forward by `millis`.
    pub fn advance(&self, millis: i64) {
        self.0.fetch_add(millis, Ordering::SeqCst);
    }

    /// The clock as the API takes it.
    pub fn as_fn(&self) -> Arc<dyn Fn() -> i64 + Send + Sync> {
        let now = self.0.clone();
        Arc::new(move || now.load(Ordering::SeqCst))
    }
}

impl Clock for TestClock {
    fn now_millis(&self) -> Option<u64> {
        u64::try_from(self.now()).ok()
    }
}

/// Called after every content read, so a test can move the clock while an answer is read.
pub type ReadHook = Box<dyn Fn() + Send + Sync>;

/// A filesystem backend that counts reads (`get_range` and `head` of any key) and content
/// reads (`get_range` of a key in an OCFL content directory), so a test can prove that an
/// answer read no file, or no file content.
pub struct CountedFs {
    inner: FsBackend,
    reads: AtomicU64,
    content_reads: AtomicU64,
    content_heads: AtomicU64,
    on_read: Mutex<Option<ReadHook>>,
}

impl CountedFs {
    /// Count the reads of `inner`.
    pub fn new(inner: FsBackend) -> Self {
        Self {
            inner,
            reads: AtomicU64::new(0),
            content_reads: AtomicU64::new(0),
            content_heads: AtomicU64::new(0),
            on_read: Mutex::new(None),
        }
    }

    /// Reads so far.
    pub fn reads(&self) -> u64 {
        self.reads.load(Ordering::SeqCst)
    }

    /// Ranged reads of content files so far.
    pub fn content_reads(&self) -> u64 {
        self.content_reads.load(Ordering::SeqCst)
    }

    /// `head` calls on content files so far (each may hash the whole file).
    pub fn content_heads(&self) -> u64 {
        self.content_heads.load(Ordering::SeqCst)
    }

    /// Run `hook` after every later read.
    pub fn on_read(&self, hook: ReadHook) {
        *self.on_read.lock().expect("hook lock") = Some(hook);
    }

    /// Count one read and run the hook.
    fn read(&self) {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some(hook) = self.on_read.lock().expect("hook lock").as_ref() {
            hook();
        }
    }
}

impl StorageBackend for CountedFs {
    fn content_addressed_tags(&self) -> bool {
        self.inner.content_addressed_tags()
    }

    async fn put(&self, key: &StorageKey, bytes: &[u8]) -> Result<ObjectMeta, StorageError> {
        self.inner.put(key, bytes).await
    }

    async fn put_if(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        self.inner.put_if(key, bytes, precondition).await
    }

    async fn get_range(&self, key: &StorageKey, range: ByteRange) -> Result<Vec<u8>, StorageError> {
        let bytes = self.inner.get_range(key, range).await;
        if key.as_str().contains("/content/") {
            self.content_reads.fetch_add(1, Ordering::SeqCst);
        }
        self.read();
        bytes
    }

    async fn check_hierarchy(&self, prefix: &str) -> Result<(), StorageError> {
        self.inner.check_hierarchy(prefix).await
    }

    async fn head(&self, key: &StorageKey) -> Result<ObjectMeta, StorageError> {
        if key.as_str().contains("/content/") {
            self.content_heads.fetch_add(1, Ordering::SeqCst);
        }
        self.read();
        self.inner.head(key).await
    }

    async fn exists(&self, key: &StorageKey) -> Result<bool, StorageError> {
        self.inner.exists(key).await
    }

    async fn list(
        &self,
        prefix: &str,
        start_after: Option<&StorageKey>,
        limit: usize,
    ) -> Result<ListPage, StorageError> {
        self.inner.list(prefix, start_after, limit).await
    }

    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        self.inner.delete(key).await
    }

    async fn create_multipart(&self, key: &StorageKey) -> Result<UploadId, StorageError> {
        self.inner.create_multipart(key).await
    }

    async fn upload_part(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        self.inner.upload_part(upload, part_number, bytes).await
    }

    async fn complete_multipart(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        self.inner
            .complete_multipart(upload, parts, precondition)
            .await
    }

    async fn abort_multipart(&self, upload: &UploadId) -> Result<(), StorageError> {
        self.inner.abort_multipart(upload).await
    }
}

/// The lease store the rig uses.
pub type Leases = LeaseStore<FsBackend, TestClock>;

/// The file service the rig uses.
pub type Service = FileService<CountedFs, Leases, FsBackend>;

/// A credential whose acceptance and permissions a test can withdraw, standing in for the
/// server's accounts until API-F1.9: `accepting` false is a rotated, logged-out or ended
/// credential; `permitting` false is a withdrawn permission.
pub struct Switchable {
    token: TokenDigest,
    /// Whether the credential is still accepted.
    pub accepting: AtomicBool,
    /// Whether the policy still permits reading files.
    pub permitting: AtomicBool,
}

/// The SHA-256 digest of a token.
type TokenDigest = [u8; 32];

impl Switchable {
    /// Accept `token` as a server administrator and permit everything.
    pub fn new(token: &str) -> Arc<Self> {
        Arc::new(Self {
            token: sha256(token),
            accepting: AtomicBool::new(true),
            permitting: AtomicBool::new(true),
        })
    }
}

/// The SHA-256 digest of `token`.
pub fn sha256(token: &str) -> TokenDigest {
    use sha2::Digest as _;
    sha2::Sha256::digest(token.as_bytes()).into()
}

impl Authenticator for Switchable {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        self.authenticate_digest(&sha256(token))
    }

    fn authenticate_digest(&self, digest: &TokenDigest) -> Option<Principal> {
        let accepted = self.accepting.load(Ordering::SeqCst) && *digest == self.token;
        accepted.then_some(Principal::Administrator)
    }
}

impl Policy for Switchable {
    fn permits(&self, _principal: &Principal, action: Action, _resource: &Resource) -> bool {
        let files = matches!(action, Action::ReadFiles | Action::WriteFiles);
        !files || self.permitting.load(Ordering::SeqCst)
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
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }

    /// The `result` of a command answer: what a committed command made (null otherwise).
    pub fn result(&self) -> serde_json::Value {
        self.json()["result"].clone()
    }

    /// The text of header `name`, or `""`.
    pub fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }
}

/// A storage root with one object, a file service and an API state.
pub struct FilesRig {
    /// The rig's directory (its own root, staging and leases, or staging and leases only
    /// when it serves the shared video root).
    pub dir: PathBuf,
    /// The storage root.
    pub root: Arc<CountedFs>,
    /// The leases of the root's writers.
    pub leases: Arc<Leases>,
    /// Staging for the fixture's own commits.
    pub staging: Arc<FsBackend>,
    /// The clock of the leases, the service and the content handles.
    pub clock: TestClock,
    /// The service under test.
    pub service: Service,
    /// The API state the router serves.
    pub state: ApiState,
    /// The owner's per-launch token.
    pub token: String,
    /// The journal store behind the monitor.
    pub journal: Arc<MemoryJournal>,
    /// The running monitor.
    pub monitor: Monitor,
    /// The home collections uploads and copies register their files in; it holds
    /// [`HOME_COLLECTION`] of the rig's library.
    pub home: Arc<MemoryHomeCollections>,
}

impl Drop for FilesRig {
    /// Stop the journal writer and remove the rig's directory.
    fn drop(&mut self) {
        self.monitor.shutdown();
        if let Err(err) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("could not remove the rig directory: {err}");
        }
    }
}

impl FilesRig {
    /// An empty OCFL storage root opened as the library, served to the desktop owner.
    pub async fn new() -> Self {
        let token = LaunchToken::generate().expect("random token");
        let authoriser = Authoriser::desktop_owner(&token);
        Self::with_authoriser(authoriser, token.secret().to_string()).await
    }

    /// An empty root served through `authoriser`, whose caller presents `token`.
    pub async fn with_authoriser(authoriser: Authoriser, token: String) -> Self {
        Self::build(authoriser, token, None).await
    }

    /// The shared video root ([`video_root`]) served to a desktop owner with a fresh token.
    pub async fn with_video() -> Self {
        let token = LaunchToken::generate().expect("random token");
        let authoriser = Authoriser::desktop_owner(&token);
        Self::build(
            authoriser,
            token.secret().to_string(),
            Some(video_root().await),
        )
        .await
    }

    /// The shared video root served through `authoriser`, whose caller presents `token`.
    pub async fn video_with_authoriser(authoriser: Authoriser, token: String) -> Self {
        Self::build(authoriser, token, Some(video_root().await)).await
    }

    /// A rig over `shared` (read only) or, when `None`, over its own new empty root.
    async fn build(authoriser: Authoriser, token: String, shared: Option<PathBuf>) -> Self {
        let clock = TestClock(Arc::new(AtomicI64::new(START_MS)));
        let state = ApiState::with_clock(clock.as_fn());
        assert!(state.install_authoriser(authoriser));
        let journal = Arc::new(MemoryJournal::new("desktop"));
        let monitor = Monitor::start(
            journal.clone(),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("desktop"),
            state.events.clone(),
        );
        state.install_monitor(monitor.clone());
        let dir = rig_dir();
        let root_dir = shared.clone().unwrap_or_else(|| dir.join("root"));
        let root = FsBackend::new(root_dir, dir.join("root-parts")).expect("root backend");
        let root = Arc::new(CountedFs::new(root));
        if shared.is_none() {
            create_root(&monitor, root.as_ref()).await;
        }
        let staging = Arc::new(fs_backend(&dir, "staging", "staging-parts"));
        let lease_root = fs_backend(&dir, "leases", "lease-parts");
        let leases =
            Arc::new(LeaseStore::new(lease_root, "leases", clock.clone(), LEASES).expect("leases"));
        let service = FileService::with_clock(clock.as_fn());
        let opened = service.open_library(
            &library(),
            FileLibrary {
                backend: root.clone(),
                reservations: leases.clone(),
                staging: Arc::new(fs_backend(&dir, "api-staging", "api-staging-parts")),
                staging_prefix: "api".to_string(),
            },
        );
        assert!(opened, "the library opens once");
        assert!(state.install_files(Arc::new(service.clone())));
        let home = Arc::new(MemoryHomeCollections::new());
        let collection = filesquid_core::CollectionId::new(HOME_COLLECTION).expect("collection");
        home.add_collection(&library(), &collection);
        assert!(state.install_home_collections(home.clone()));
        Self {
            dir,
            root,
            leases,
            staging,
            clock,
            service,
            state,
            token,
            journal,
            monitor,
            home,
        }
    }

    /// A root whose object holds `files` at v1.
    pub async fn with_files(files: &[(&str, &[u8])]) -> Self {
        let rig = Self::new().await;
        rig.commit(files).await;
        rig
    }

    /// Commit `files` as one new version of the object, through the engine directly.
    pub async fn commit(&self, files: &[(&str, &[u8])]) {
        let staging =
            StagingArea::new(self.staging.as_ref(), "fixture").expect("fixture staging area");
        let model = ChangeModel::new(
            self.root.as_ref(),
            self.leases.as_ref(),
            "fixture",
            OBJECT_ROOT,
            OBJECT,
            staging,
        );
        for (path, bytes) in files {
            model.stage_bytes(path, bytes).await.expect("stage");
        }
        let meta = VersionMeta::new(filesquid_ocfl::write::rfc3339_utc(self.clock.now() as u64));
        model.commit(meta).await.expect("fixture commit");
    }

    /// Remove `path` in one new version, through the engine directly.
    pub async fn remove(&self, path: &str) {
        let staging =
            StagingArea::new(self.staging.as_ref(), "fixture").expect("fixture staging area");
        let model = ChangeModel::new(
            self.root.as_ref(),
            self.leases.as_ref(),
            "fixture",
            OBJECT_ROOT,
            OBJECT,
            staging,
        );
        let meta = VersionMeta::new(filesquid_ocfl::write::rfc3339_utc(self.clock.now() as u64));
        view::remove_path(&model, path, meta)
            .await
            .expect("fixture remove");
    }

    /// The object's head version name, read from the root.
    pub async fn head(&self) -> String {
        let object = filesquid_ocfl::read::Object::open(self.root.as_ref(), OBJECT_ROOT)
            .await
            .expect("object");
        object.inventory().head.to_string()
    }

    /// Send `method` to `uri` with the owner's token, `headers` and `body`.
    pub async fn call(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> Answer {
        let bearer = format!("Bearer {}", self.token);
        let mut all = vec![("authorization", bearer.as_str())];
        all.extend_from_slice(headers);
        send(&self.state, method, uri, &all, body).await
    }

    /// `GET uri` with the owner's token.
    pub async fn get(&self, uri: &str) -> Answer {
        self.call(Method::GET, uri, &[], Vec::new()).await
    }

    /// `POST uri` with the owner's token and a JSON body.
    pub async fn post(&self, uri: &str, body: serde_json::Value) -> Answer {
        let headers = [("content-type", "application/json")];
        self.call(Method::POST, uri, &headers, body.to_string().into_bytes())
            .await
    }

    /// The head version of `object_id` in the rig's library, read through the versions
    /// route, or `"0"` when the route answers `404` (no such object): the revision a command
    /// names. Any other answer fails the test, so a refusal is never mistaken for a revision.
    pub async fn head_of(&self, object_id: &str) -> String {
        let uri = format!(
            "/api/v1/files/versions?library_id={LIBRARY}&object_id={}",
            encode(object_id)
        );
        let answer = self.get(&uri).await;
        match answer.status {
            StatusCode::OK => answer.json()["head"].as_str().expect("head").to_string(),
            StatusCode::NOT_FOUND => "0".to_string(),
            other => panic!("the versions route answered {other}: {:?}", answer.json()),
        }
    }

    /// The revision a command on `input` names: the head of `input.object_id`, else of the
    /// file object of `input.file_id`, else `"0"` (a new file).
    pub async fn revision_of(&self, input: &serde_json::Value) -> String {
        if let Some(object_id) = input["object_id"].as_str() {
            return self.head_of(object_id).await;
        }
        match input["file_id"].as_str() {
            Some(file_id) => self.head_of(&format!("urn:uuid:{file_id}")).await,
            None => "0".to_string(),
        }
    }

    /// `POST uri` of the command `input` with a fresh operation ID and the current revision.
    pub async fn command(&self, uri: &str, input: serde_json::Value) -> Answer {
        self.command_as(Method::POST, uri, input).await
    }

    /// `method uri` of the command `input` with a fresh operation ID and the current revision.
    pub async fn command_as(&self, method: Method, uri: &str, input: serde_json::Value) -> Answer {
        let revision = self.revision_of(&input).await;
        let body = command_body(&input, &operation_id(), &revision);
        self.send_command(method, uri, &body).await
    }

    /// `method uri` with the JSON command `body` as it is.
    pub async fn send_command(
        &self,
        method: Method,
        uri: &str,
        body: &serde_json::Value,
    ) -> Answer {
        let headers = [("content-type", "application/json")];
        self.call(method, uri, &headers, body.to_string().into_bytes())
            .await
    }

    /// `PUT /api/v1/files/upload` of `bytes` with the facts `input`, a fresh operation ID and
    /// the current revision.
    pub async fn upload(&self, input: serde_json::Value, bytes: &[u8]) -> Answer {
        let revision = self.revision_of(&input).await;
        let envelope = command_body(&input, &operation_id(), &revision)["envelope"].clone();
        self.send_upload(&envelope, &input, bytes).await
    }

    /// `PUT /api/v1/files/upload` of `bytes` with `envelope` and `input` as they are.
    pub async fn send_upload(
        &self,
        envelope: &serde_json::Value,
        input: &serde_json::Value,
        bytes: &[u8],
    ) -> Answer {
        let body = upload_body(Some(envelope), Some(input), Some(bytes));
        let headers = [("content-type", MULTIPART)];
        self.call(Method::PUT, UPLOAD_URI, &headers, body).await
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
}

/// A query string for the rig's library and object plus `rest` (`key=value` pairs, already
/// encoded).
pub fn query(rest: &str) -> String {
    let base = format!("library_id={LIBRARY}&object_id={}", encode(OBJECT));
    if rest.is_empty() {
        return base;
    }
    format!("{base}&{rest}")
}

/// The upload route.
pub const UPLOAD_URI: &str = "/api/v1/files/upload";

/// The collection of the rig's library that uploads and copies go to (the drive catalog's
/// collection, so a rig that also installs navigation over it agrees).
pub const HOME_COLLECTION: &str = "2c0d1e2f-3a4b-4c5d-8e6f-7a8b9c0d1e2f";

/// The upload facts of a new file named `name` declaring `size` bytes in the rig's library,
/// plus the fields of `rest` (a JSON object).
pub fn upload_input(name: &str, size: usize, rest: serde_json::Value) -> serde_json::Value {
    let mut input = serde_json::json!({
        "library_id": LIBRARY, "name": name, "size": size, "collection_id": HOME_COLLECTION
    });
    for (key, value) in rest.as_object().expect("an object of fields") {
        input[key] = value.clone();
    }
    input
}

/// [`upload_input`] with the fields of `rest`, `key=value` pairs joined by `&` with
/// percent-encoded values (the form the upload query took before Task API-F2.13).
pub fn upload_facts(name: &str, size: usize, rest: &str) -> serde_json::Value {
    let mut fields = serde_json::Map::new();
    for pair in rest.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').expect("a key=value pair");
        fields.insert(key.to_string(), serde_json::Value::String(decode(value)));
    }
    upload_input(name, size, serde_json::Value::Object(fields))
}

/// Percent-decode `text`.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).expect("hex");
                out.push(u8::from_str_radix(hex, 16).expect("a percent escape"));
                i += 3;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).expect("UTF-8")
}

/// A fresh operation ID, unique in this process.
pub fn operation_id() -> String {
    let n = OPERATIONS.fetch_add(1, Ordering::SeqCst);
    format!("op-{}-{n}", std::process::id())
}

/// The command body `{envelope, input}` of `input` under `operation` and `revision`, scoped
/// to `input.library_id`.
pub fn command_body(
    input: &serde_json::Value,
    operation: &str,
    revision: &str,
) -> serde_json::Value {
    let library = input["library_id"].as_str().unwrap_or_default();
    serde_json::json!({
        "envelope": {
            "schema_version": 1,
            "operation_id": operation,
            "expected_revision": revision,
            "scope": format!("library:{library}"),
        },
        "input": input,
    })
}

/// A `multipart/form-data` upload body ([`BOUNDARY`]) with the parts given: `envelope` and
/// `input` as JSON, `file` as the bytes.
pub fn upload_body(
    envelope: Option<&serde_json::Value>,
    input: Option<&serde_json::Value>,
    file: Option<&[u8]>,
) -> Vec<u8> {
    let mut body = Vec::new();
    let json_parts = [("envelope", envelope), ("input", input)];
    for (name, value) in json_parts {
        let Some(value) = value else { continue };
        let head = format!(
            "--{BOUNDARY}\r\ncontent-disposition: form-data; name=\"{name}\"\r\n\
             content-type: application/json\r\n\r\n"
        );
        body.extend_from_slice(head.as_bytes());
        body.extend_from_slice(value.to_string().as_bytes());
        body.extend_from_slice(b"\r\n");
    }
    if let Some(bytes) = file {
        let head = format!(
            "--{BOUNDARY}\r\ncontent-disposition: form-data; name=\"file\"; filename=\"file\"\r\n\
             content-type: application/octet-stream\r\n\r\n"
        );
        body.extend_from_slice(head.as_bytes());
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

/// Percent-encode `text` for a query value.
pub fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Send one request to the router of `state`.
pub async fn send(
    state: &ApiState,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> Answer {
    let mut request = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let request = request.body(Body::from(body)).expect("request");
    let response = router(state.clone())
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

/// A fresh directory for one rig, under the system temporary directory.
fn rig_dir() -> PathBuf {
    let n = RIGS.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("filesquid-api-files-{}-{n}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear an old rig directory");
    }
    std::fs::create_dir_all(&dir).expect("rig directory");
    dir
}

/// A filesystem backend rooted at `dir/name`, with its parts under `dir/parts`.
fn fs_backend(dir: &std::path::Path, name: &str, parts: &str) -> FsBackend {
    FsBackend::new(dir.join(name), dir.join(parts)).expect("filesystem backend")
}

/// Write the OCFL 1.1 storage-root declaration into `root`.
async fn create_root(monitor: &Monitor, root: &CountedFs) {
    let service = LocalLibraryService::new(monitor.journal(), Arc::new(MemoryRepairLedger::new()));
    service
        .create_root("owner", &library(), root)
        .await
        .expect("create root");
}

/// The shared video root of the BDD scenarios, built once per process.
static VIDEO_ROOT: tokio::sync::OnceCell<PathBuf> = tokio::sync::OnceCell::const_new();

/// Length of the shared video: 100 MB (104,857,600 bytes).
pub const VIDEO_LEN: usize = 104_857_600;

/// The directory of the shared video root and its writers' state.
fn video_dir() -> PathBuf {
    std::env::temp_dir().join(format!("filesquid-api-video-{}", std::process::id()))
}

/// A storage root, built once per process, whose object holds `readme.txt` at v1 and the
/// 100 MB video `holiday.mp4` ([`pattern`] bytes) added at v2. Rigs read it; none writes.
pub async fn video_root() -> PathBuf {
    VIDEO_ROOT.get_or_init(build_video_root).await.clone()
}

/// Remove the shared video root, if it was built. The BDD harness calls it after its run.
pub fn remove_video_root() {
    if VIDEO_ROOT.initialized() {
        if let Err(err) = std::fs::remove_dir_all(video_dir()) {
            eprintln!("could not remove the shared video root: {err}");
        }
    }
}

/// Build the shared video root.
async fn build_video_root() -> PathBuf {
    let dir = video_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear an old video root");
    }
    let root = fs_backend(&dir, "root", "root-parts");
    let journal = Arc::new(MemoryJournal::new("desktop"));
    let monitor = Monitor::start(
        journal,
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        filesquid_api::EventBus::new(),
    );
    let root = CountedFs::new(root);
    create_root(&monitor, &root).await;
    monitor.shutdown();
    let staging = fs_backend(&dir, "staging", "staging-parts");
    let clock = TestClock(Arc::new(AtomicI64::new(START_MS)));
    let leases = LeaseStore::new(
        fs_backend(&dir, "leases", "lease-parts"),
        "leases",
        clock,
        LEASES,
    )
    .expect("leases");
    let area = StagingArea::new(&staging, "fixture").expect("staging area");
    let model = ChangeModel::new(&root, &leases, "fixture", OBJECT_ROOT, OBJECT, area);
    let created = filesquid_ocfl::write::rfc3339_utc(START_MS as u64);
    model
        .stage_bytes("readme.txt", b"holiday")
        .await
        .expect("stage");
    model
        .commit(VersionMeta::new(created.clone()))
        .await
        .expect("v1");
    model
        .stage_bytes("holiday.mp4", &pattern(VIDEO_LEN))
        .await
        .expect("stage");
    model.commit(VersionMeta::new(created)).await.expect("v2");
    dir.join("root")
}
