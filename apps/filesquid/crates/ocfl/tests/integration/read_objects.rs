//! Opening and reading OCFL objects and storage roots through a storage
//! backend (IO-0.4; OCFL-S-004, OCFL-S-001, OCFL-S-002, OCFL-S-017;
//! ADR 016 "Published head (readers)"). Every scenario runs on the
//! in-memory backend and on a real directory through `FsBackend`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::inventory::{FormatError, Inventory, MAX_SIDECAR_BYTES};
use filesquid_ocfl::read::{Change, Object, ReadError, StorageRoot};
use filesquid_ocfl::storage::{
    complete_immediately, ByteRange, CompletedPart, FsBackend, ListPage, MemoryBackend, ObjectMeta,
    Precondition, StorageBackend, StorageError, StorageKey, UploadId,
};
use filesquid_ocfl::{OcflError, VersionNum};

#[path = "../support/object_builder.rs"]
mod builder;
use builder::{build, delete, key_in, put, write_inventory, ObjectSpec};

/// Scratch directory for one filesystem backend, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh `FsBackend` on its own scratch directory.
fn fs_backend() -> (FsBackend, Scratch) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ocfl-read-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    (backend, Scratch(dir))
}

/// Run the named scenario on a memory backend and on a filesystem backend.
fn on_both(name: &str) {
    run_scenario(&MemoryBackend::new(), name);
    let (backend, _scratch) = fs_backend();
    run_scenario(&backend, name);
}

/// Drive a backend future that never waits.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("backend futures complete at once")
}

/// A version number from its name.
fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

/// The three-version object most scenarios use: v2 renames b.txt to c.txt
/// and adds d.txt; v3 changes a.txt and removes d.txt.
fn three_versions() -> ObjectSpec<'static> {
    ObjectSpec::new(
        "urn:example:three",
        vec![
            vec![("a.txt", b"alpha" as &[u8]), ("b.txt", b"bravo")],
            vec![
                ("a.txt", b"alpha"),
                ("c.txt", b"bravo"),
                ("d.txt", b"delta"),
            ],
            vec![("a.txt", b"alpha 2"), ("c.txt", b"bravo")],
        ],
    )
}

/// Open the object at `root`.
fn open<'a, B: StorageBackend>(backend: &'a B, root: &str) -> Result<Object<'a, B>, ReadError> {
    ready(Object::open(backend, root))
}

/// Stream one logical file in `chunk`-byte chunks.
fn stream<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &str,
    path: &str,
    chunk: u64,
) -> Result<Vec<u8>, ReadError> {
    let mut reader = object.stream_file(&v(version), path, chunk)?;
    let mut out = Vec::new();
    for _ in 0..1_000 {
        match ready(reader.next_chunk())? {
            Some(bytes) => {
                assert!(bytes.len() as u64 <= chunk, "chunk above {chunk}");
                out.extend_from_slice(&bytes);
            }
            None => return Ok(out),
        }
    }
    panic!("stream of {path} did not end");
}

/// The OCFL code of an error, or a panic when it has none.
fn code(err: &ReadError) -> &'static str {
    err.code()
        .unwrap_or_else(|| panic!("no OCFL code in {err:?}"))
}

/// Run one named scenario against `backend`.
fn run_scenario<B: StorageBackend>(backend: &B, name: &str) {
    match name {
        "resolve" => resolve_every_version(backend),
        "stream" => stream_every_file(backend),
        "browse" => history_and_diff(backend),
        "pinned" => pinned_versions_only(backend),
        "stray" => stray_files_are_not_state(backend),
        "unpublished" => unpublished_version_is_invisible(backend),
        "corrupt" => corrupt_content_is_e092(backend),
        "missing-content" => missing_content_is_e092(backend),
        "copy" => copy_streams_into_another_key(backend),
        "copy-corrupt" => corrupt_copy_is_never_published(backend),
        "sha512-upper" => sha512_uppercase_object_reads(backend),
        "git" => git_in_object_root_is_refused(backend),
        "storage-root" => storage_root_discovers_objects(backend),
        other => panic!("unknown scenario {other}"),
    }
}

fn resolve_every_version<B: StorageBackend>(backend: &B) {
    build(backend, "obj", &three_versions());
    let object = open(backend, "obj").expect("open");
    assert_eq!(object.id(), "urn:example:three");
    assert_eq!(object.head(), &v("v3"));
    assert_eq!(object.root(), "obj");
    let v2 = object.state(&v("v2")).expect("v2");
    assert_eq!(v2.paths().collect::<Vec<_>>(), ["a.txt", "c.txt", "d.txt"]);
    assert_eq!(v2.get("a.txt").expect("a").content_path, "v1/content/a.txt");
    assert_eq!(v2.get("c.txt").expect("c").content_path, "v1/content/b.txt");
    let v3 = object.state(object.head()).expect("head");
    assert_eq!(v3.get("a.txt").expect("a").content_path, "v3/content/a.txt");
    let key = object
        .content_key(v3.get("c.txt").expect("c"))
        .expect("key");
    assert_eq!(key.as_str(), "obj/v1/content/b.txt");
}

fn stream_every_file<B: StorageBackend>(backend: &B) {
    let spec = three_versions();
    build(backend, "obj", &spec);
    let object = open(backend, "obj").expect("open");
    for (index, files) in spec.versions.iter().enumerate() {
        let version = format!("v{}", index + 1);
        for (path, bytes) in files {
            assert_eq!(stream(&object, &version, path, 2).expect("stream"), *bytes);
        }
    }
}

fn history_and_diff<B: StorageBackend>(backend: &B) {
    build(backend, "obj", &three_versions());
    let object = open(backend, "obj").expect("open");
    let history = object.history();
    assert_eq!(history.len(), 3);
    assert_eq!(history[2].message.as_deref(), Some("Version v3"));
    assert_eq!(history[0].created, "2026-10-11T00:00:00Z");
    let changes = object.diff(&v("v1"), &v("v2")).expect("diff");
    let names: Vec<(&str, &str)> = changes
        .iter()
        .map(|c| match c {
            Change::Added { path, .. } => ("added", path.as_str()),
            Change::Deleted { path, .. } => ("deleted", path.as_str()),
            Change::Modified { path, .. } => ("modified", path.as_str()),
            Change::Renamed { to, .. } => ("renamed", to.as_str()),
        })
        .collect();
    assert_eq!(names, [("renamed", "c.txt"), ("added", "d.txt")]);
    let later = object.diff(&v("v2"), &v("v3")).expect("diff");
    assert!(matches!(&later[0], Change::Modified { path, .. } if path == "a.txt"));
    assert!(matches!(&later[1], Change::Deleted { path, .. } if path == "d.txt"));
}

fn pinned_versions_only<B: StorageBackend>(backend: &B) {
    build(backend, "obj", &three_versions());
    let object = open(backend, "obj").expect("open");
    let err = object.state(&v("v4")).expect_err("v4 is not published");
    assert!(matches!(err, ReadError::VersionNotFound { .. }), "{err:?}");
    let err = object
        .stream_file(&v("v3"), "d.txt", 4)
        .expect_err("d.txt was removed in v3");
    assert!(
        matches!(&err, ReadError::PathNotFound { version, path } if version == "v3" && path == "d.txt"),
        "{err:?}"
    );
}

fn stray_files_are_not_state<B: StorageBackend>(backend: &B) {
    build(backend, "obj", &three_versions());
    put(
        backend,
        "obj",
        "v3/content/stray.txt",
        b"not in the inventory",
    );
    put(
        backend,
        "obj",
        "v2/content/c.txt",
        b"a decoy where v2's c.txt is not",
    );
    let object = open(backend, "obj").expect("open");
    let v3 = object.state(&v("v3")).expect("v3");
    assert_eq!(v3.paths().collect::<Vec<_>>(), ["a.txt", "c.txt"]);
    assert_eq!(stream(&object, "v2", "c.txt", 64).expect("c"), b"bravo");
}

fn unpublished_version_is_invisible<B: StorageBackend>(backend: &B) {
    let mut spec = three_versions();
    spec.versions.push(vec![("e.txt", b"echo" as &[u8])]);
    let v4 = build(backend, "scratch", &spec);
    build(backend, "obj", &three_versions());
    put(backend, "obj", "v4/content/e.txt", b"echo");
    write_inventory(backend, "obj", "v4", &v4);
    let object = open(backend, "obj").expect("open");
    assert_eq!(object.head(), &v("v3"));
    assert_eq!(object.history().len(), 3);
    let err = object.state(&v("v4")).expect_err("unpublished");
    assert!(matches!(err, ReadError::VersionNotFound { .. }), "{err:?}");
}

fn corrupt_content_is_e092<B: StorageBackend>(backend: &B) {
    build(backend, "obj", &three_versions());
    put(backend, "obj", "v1/content/b.txt", b"brav0");
    let object = open(backend, "obj").expect("open");
    let err = stream(&object, "v3", "c.txt", 2).expect_err("digest mismatch");
    assert_eq!(code(&err), "E092");
    assert!(err.to_string().contains("obj/v1/content/b.txt"), "{err}");
    let mut reader = object.stream_file(&v("v3"), "c.txt", 64).expect("reader");
    assert_eq!(
        ready(reader.next_chunk()).expect("bytes"),
        Some(b"brav0".to_vec())
    );
    for _ in 0..2 {
        let again = ready(reader.next_chunk()).expect_err("the failure is kept");
        assert_eq!(code(&again), "E092");
    }
    assert_eq!(stream(&object, "v3", "a.txt", 2).expect("a"), b"alpha 2");
}

fn missing_content_is_e092<B: StorageBackend>(backend: &B) {
    build(backend, "obj", &three_versions());
    delete(backend, "obj", "v3/content/a.txt");
    let object = open(backend, "obj").expect("open");
    let err = stream(&object, "v3", "a.txt", 2).expect_err("missing");
    assert_eq!(code(&err), "E092");
}

fn copy_streams_into_another_key<B: StorageBackend>(backend: &B) {
    build(backend, "obj", &three_versions());
    let object = open(backend, "obj").expect("open");
    let dest = StorageKey::new("export/c.txt").expect("key");
    let meta = ready(object.copy_file(&v("v2"), "c.txt", backend, &dest, None)).expect("copy");
    assert_eq!(meta.size, 5);
    let copied = ready(backend.get_range(&dest, ByteRange::new(0, 64).expect("range")));
    assert_eq!(copied.expect("copied"), b"bravo");
    let again = ready(object.copy_file(
        &v("v2"),
        "c.txt",
        backend,
        &dest,
        Some(&Precondition::IfAbsent),
    ));
    assert!(
        matches!(
            again,
            Err(ReadError::Storage(StorageError::PreconditionFailed { .. }))
        ),
        "{again:?}"
    );
}

fn corrupt_copy_is_never_published<B: StorageBackend>(backend: &B) {
    build(backend, "obj", &three_versions());
    put(backend, "obj", "v1/content/a.txt", b"alphA");
    let object = open(backend, "obj").expect("open");
    let dest = StorageKey::new("export/a.txt").expect("key");
    let err = ready(object.copy_file(&v("v1"), "a.txt", backend, &dest, None)).expect_err("bad");
    assert_eq!(code(&err), "E092");
    assert!(!ready(backend.exists(&dest)).expect("exists"));
}

fn sha512_uppercase_object_reads<B: StorageBackend>(backend: &B) {
    let mut spec = three_versions();
    spec.algorithm = DigestAlgorithm::Sha512;
    spec.uppercase = true;
    build(backend, "obj", &spec);
    let object = open(backend, "obj").expect("open");
    let v3 = object.state(&v("v3")).expect("v3");
    assert!(v3.files().all(|f| f.digest.len() == 128));
    assert_eq!(stream(&object, "v3", "c.txt", 3).expect("c"), b"bravo");
}

fn git_in_object_root_is_refused<B: StorageBackend>(backend: &B) {
    build(backend, "obj", &three_versions());
    put(backend, "obj", ".git/HEAD", b"ref: refs/heads/main\n");
    let err = open(backend, "obj").expect_err("refused");
    assert!(
        matches!(&err, ReadError::Guard(OcflError::GitDirectoryForbidden { root }) if root == "obj"),
        "{err:?}"
    );
}

fn storage_root_discovers_objects<B: StorageBackend>(backend: &B) {
    put(backend, "", "0=ocfl_1.1", b"ocfl_1.1\n");
    build(backend, "ab/obj1", &three_versions());
    build(backend, "cd/ef/obj2", &three_versions());
    put(
        backend,
        "ab/obj1",
        "v1/content/inner/0=ocfl_object_1.1",
        b"x",
    );
    let root = ready(StorageRoot::open(backend)).expect("storage root");
    assert_eq!(root.objects(), ["ab/obj1", "cd/ef/obj2"]);
    let object = ready(root.open_object("cd/ef/obj2")).expect("object");
    assert_eq!(object.head(), &v("v3"));
}

#[test]
fn every_version_resolves_through_state_and_manifest() {
    on_both("resolve");
}

#[test]
fn every_logical_file_streams_in_bounded_chunks() {
    on_both("stream");
}

#[test]
fn history_and_diff_come_from_the_inventory() {
    on_both("browse");
}

#[test]
fn only_versions_and_paths_in_the_inventory_can_be_pinned() {
    on_both("pinned");
}

#[test]
fn files_outside_the_inventory_are_not_logical_state() {
    on_both("stray");
}

#[test]
fn a_version_above_the_root_head_is_unpublished() {
    on_both("unpublished");
}

#[test]
fn content_that_does_not_match_its_digest_is_e092() {
    on_both("corrupt");
}

#[test]
fn content_missing_from_storage_is_e092() {
    on_both("missing-content");
}

#[test]
fn a_logical_file_copies_into_another_key() {
    on_both("copy");
}

#[test]
fn a_copy_that_fails_its_digest_is_never_published() {
    on_both("copy-corrupt");
}

#[test]
fn sha512_objects_with_uppercase_digests_read() {
    on_both("sha512-upper");
}

#[test]
fn opening_an_object_runs_the_git_guard() {
    on_both("git");
}

#[test]
fn a_storage_root_discovers_its_outermost_objects() {
    on_both("storage-root");
}

/// Build the three-version object, apply `change`, and return the refusal.
fn refusal(change: impl Fn(&MemoryBackend)) -> ReadError {
    let backend = MemoryBackend::new();
    build(&backend, "obj", &three_versions());
    change(&backend);
    open(&backend, "obj").expect_err("the object must be refused")
}

#[test]
fn declaration_faults_have_their_codes() {
    let none = refusal(|b| delete(b, "obj", "0=ocfl_object_1.1"));
    assert_eq!(code(&none), "E003");
    let two = refusal(|b| put(b, "obj", "0=ocfl_object_1.0", b"ocfl_object_1.0\n"));
    assert_eq!(code(&two), "E003");
    let text = refusal(|b| put(b, "obj", "0=ocfl_object_1.1", b"ocfl_object_1.1"));
    assert_eq!(code(&text), "E007");
}

#[test]
fn an_oversized_declaration_is_refused_before_it_is_read() {
    let err = refusal(|b| put(b, "obj", "0=ocfl_object_1.1", &[b'x'; 65]));
    assert!(
        matches!(err, ReadError::Format(FormatError::TooLarge { .. })),
        "{err:?}"
    );
}

#[test]
fn inventory_and_sidecar_faults_have_their_codes() {
    let no_inventory = refusal(|b| delete(b, "obj", "inventory.json"));
    assert_eq!(code(&no_inventory), "E063");
    let no_sidecar = refusal(|b| delete(b, "obj", "inventory.json.sha256"));
    assert_eq!(code(&no_sidecar), "E058");
    let wrong = format!("{} inventory.json\n", "0".repeat(64));
    let mismatch = refusal(|b| put(b, "obj", "inventory.json.sha256", wrong.as_bytes()));
    assert_eq!(code(&mismatch), "E060");
    let malformed = refusal(|b| put(b, "obj", "inventory.json.sha256", b"nonsense"));
    assert_eq!(code(&malformed), "E061");
    let broken = refusal(|b| put(b, "obj", "inventory.json", b"{}"));
    assert_eq!(code(&broken), "E036");
}

#[test]
fn an_oversized_sidecar_is_refused_before_it_is_read() {
    let big = vec![b'0'; MAX_SIDECAR_BYTES + 1];
    let err = refusal(|b| put(b, "obj", "inventory.json.sha256", &big));
    assert!(
        matches!(err, ReadError::Format(FormatError::TooLarge { .. })),
        "{err:?}"
    );
}

#[test]
fn a_head_version_inventory_that_differs_from_the_root_is_e064() {
    let err = refusal(|b| {
        let mut other = three_versions();
        other.id = "urn:example:other";
        let inventory: Inventory = build(&MemoryBackend::new(), "x", &other);
        write_inventory(b, "obj", "v3", &inventory);
    });
    assert_eq!(code(&err), "E064");
}

#[test]
fn a_missing_head_version_inventory_is_allowed() {
    let backend = MemoryBackend::new();
    build(&backend, "obj", &three_versions());
    delete(&backend, "obj", "v3/inventory.json");
    delete(&backend, "obj", "v3/inventory.json.sha256");
    assert_eq!(open(&backend, "obj").expect("open").head(), &v("v3"));
}

#[test]
fn an_object_at_the_backend_root_opens() {
    let backend = MemoryBackend::new();
    build(&backend, "", &three_versions());
    let object = open(&backend, "").expect("open");
    let key = object
        .content_key(object.state(&v("v1")).expect("v1").get("a.txt").expect("a"))
        .expect("key");
    assert_eq!(key.as_str(), "v1/content/a.txt");
}

#[test]
fn storage_root_declaration_faults_have_their_codes() {
    let backend = MemoryBackend::new();
    build(&backend, "obj", &three_versions());
    let missing = ready(StorageRoot::open(&backend)).expect_err("no declaration");
    assert_eq!(code(&missing), "E069");
    put(&backend, "", "0=ocfl_1.1", b"ocfl_1.1");
    let text = ready(StorageRoot::open(&backend)).expect_err("bad contents");
    assert_eq!(code(&text), "E080");
}

#[test]
fn opening_a_storage_root_runs_the_guard() {
    let backend = MemoryBackend::new();
    put(&backend, "", "0=ocfl_1.1", b"ocfl_1.1\n");
    build(&backend, "obj", &three_versions());
    put(&backend, "", ".git/config", b"[core]\n");
    let err = ready(StorageRoot::open(&backend)).expect_err("refused");
    assert!(
        matches!(&err, ReadError::Guard(OcflError::GitDirectoryForbidden { root }) if root.is_empty()),
        "{err:?}"
    );
}

/// A memory backend where another writer publishes a new head between the
/// reader's root inventory read and its sidecar read, once.
struct RacingWriter {
    inner: MemoryBackend,
    next: Inventory,
    fired: AtomicBool,
}

impl RacingWriter {
    /// Publish `next` (version content already stored) the first time the
    /// root sidecar is looked at.
    fn race(&self, key: &StorageKey) {
        if key.as_str() != "obj/inventory.json.sha256" || self.fired.swap(true, Ordering::SeqCst) {
            return;
        }
        write_inventory(&self.inner, "obj", "", &self.next);
    }
}

impl StorageBackend for RacingWriter {
    async fn put(&self, k: &StorageKey, b: &[u8]) -> Result<ObjectMeta, StorageError> {
        self.inner.put(k, b).await
    }
    async fn put_if(
        &self,
        k: &StorageKey,
        b: &[u8],
        p: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        self.inner.put_if(k, b, p).await
    }
    async fn get_range(&self, k: &StorageKey, r: ByteRange) -> Result<Vec<u8>, StorageError> {
        self.inner.get_range(k, r).await
    }
    async fn head(&self, k: &StorageKey) -> Result<ObjectMeta, StorageError> {
        self.race(k);
        self.inner.head(k).await
    }
    async fn exists(&self, k: &StorageKey) -> Result<bool, StorageError> {
        self.inner.exists(k).await
    }
    async fn list(
        &self,
        p: &str,
        a: Option<&StorageKey>,
        l: usize,
    ) -> Result<ListPage, StorageError> {
        self.inner.list(p, a, l).await
    }
    async fn delete(&self, k: &StorageKey) -> Result<(), StorageError> {
        self.inner.delete(k).await
    }
    async fn create_multipart(&self, k: &StorageKey) -> Result<UploadId, StorageError> {
        self.inner.create_multipart(k).await
    }
    async fn upload_part(
        &self,
        u: &UploadId,
        n: u32,
        b: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        self.inner.upload_part(u, n, b).await
    }
    async fn complete_multipart(
        &self,
        u: &UploadId,
        p: &[CompletedPart],
        c: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        self.inner.complete_multipart(u, p, c).await
    }
    async fn abort_multipart(&self, u: &UploadId) -> Result<(), StorageError> {
        self.inner.abort_multipart(u).await
    }
}

#[test]
fn a_head_published_during_open_is_read_again() {
    let inner = MemoryBackend::new();
    build(&inner, "obj", &three_versions());
    let mut spec = three_versions();
    spec.versions.push(vec![("e.txt", b"echo" as &[u8])]);
    let next = build(&MemoryBackend::new(), "x", &spec);
    put(&inner, "obj", "v4/content/e.txt", b"echo");
    write_inventory(&inner, "obj", "v4", &next);
    let racing = RacingWriter {
        inner,
        next,
        fired: AtomicBool::new(false),
    };
    let object = open(&racing, "obj").expect("open after the race");
    assert!(
        racing.fired.load(Ordering::SeqCst),
        "the race did not happen"
    );
    assert_eq!(object.head(), &v("v4"));
    assert_eq!(stream(&object, "v4", "e.txt", 4).expect("e"), b"echo");
    assert_eq!(key_in("obj", "v4").as_str(), "obj/v4");
}
