//! Step definitions for `read_and_browse_OCFL-S-004.feature` and the read
//! scenario of `OCFL_1.1_fixture_conformance.feature` (IO-0.4).

use std::path::PathBuf;

use cucumber::{given, then, when};
use filesquid_ocfl::read::{Change, LogicalState, Object, ReadError, StorageRoot, VersionInfo};
use filesquid_ocfl::storage::{complete_immediately, FsBackend, MemoryBackend, StorageBackend};
use filesquid_ocfl::VersionNum;

use crate::builder::{build, delete, put, write_inventory, ObjectSpec};
use crate::OcflWorld;

/// What the read scenarios carry between steps.
#[derive(Debug, Default)]
pub struct ReadState {
    backend: Option<MemoryBackend>,
    state: Option<LogicalState>,
    history: Vec<VersionInfo>,
    changes: Vec<Change>,
    head: Option<VersionNum>,
    objects: Vec<String>,
    refusal: Option<ReadError>,
    streamed: usize,
    fixtures: Option<PathBuf>,
}

/// The object the scenarios read: v2 renames b.txt to c.txt and adds d.txt;
/// v3 changes a.txt and removes d.txt.
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

/// Drive a backend future that never waits.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("backend futures complete at once")
}

/// A version number from its name.
fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

/// The scenario's backend.
fn backend(world: &OcflWorld) -> &MemoryBackend {
    world.read.backend.as_ref().expect("Given an OCFL object")
}

/// Open the scenario's object, `obj`.
fn open(world: &OcflWorld) -> Result<Object<'_, MemoryBackend>, ReadError> {
    ready(Object::open(backend(world), "obj"))
}

/// A comma-separated list as strings.
fn list(text: &str) -> Vec<String> {
    text.split(", ").map(str::to_string).collect()
}

#[given(expr = "an OCFL object {string} with three versions in an in-memory storage backend")]
fn object_with_three_versions(world: &mut OcflWorld, root: String) {
    let backend = MemoryBackend::new();
    build(&backend, &root, &three_versions());
    world.read.backend = Some(backend);
}

#[when(expr = "I resolve the logical state of version {string}")]
fn resolve_version(world: &mut OcflWorld, version: String) {
    let state = open(world)
        .expect("open")
        .state(&v(&version))
        .expect("state");
    world.read.state = Some(state);
}

#[then(expr = "the logical paths should be {string}")]
fn logical_paths(world: &mut OcflWorld, expected: String) {
    let state = world.read.state.as_ref().expect("a resolved state");
    assert_eq!(
        state.paths().map(str::to_string).collect::<Vec<_>>(),
        list(&expected)
    );
}

#[then(expr = "{string} should be read from the content path {string}")]
fn read_from(world: &mut OcflWorld, path: String, content_path: String) {
    let state = world.read.state.as_ref().expect("a resolved state");
    assert_eq!(state.get(&path).expect("path").content_path, content_path);
}

/// Stream one logical file to its end in `chunk`-byte chunks.
fn stream_file<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    path: &str,
    chunk: u64,
) -> Result<Vec<u8>, ReadError> {
    let mut reader = object.stream_file(version, path, chunk)?;
    let mut out = Vec::new();
    for _ in 0..100_000 {
        match ready(reader.next_chunk())? {
            Some(bytes) => out.extend_from_slice(&bytes),
            None => return Ok(out),
        }
    }
    panic!("{path} did not end");
}

/// Stream every file of every version; return how many were read.
fn stream_everything<B: StorageBackend>(
    object: &Object<'_, B>,
    chunk: u64,
) -> Result<usize, ReadError> {
    let algorithm = object.inventory().digest_algorithm;
    let mut count = 0;
    for info in object.history() {
        for file in object.state(&info.version)?.files() {
            let bytes = stream_file(object, &info.version, &file.path, chunk)?;
            assert!(algorithm.matches(&algorithm.digest_hex(&bytes), &file.digest));
            count += 1;
        }
    }
    Ok(count)
}

#[when(expr = "I stream every logical file of every version in chunks of {int} bytes")]
fn stream_all(world: &mut OcflWorld, chunk: u64) {
    let object = open(world).expect("open");
    let count = stream_everything(&object, chunk).expect("stream");
    world.read.streamed = count;
}

#[then("every streamed logical file should match its inventory digest")]
fn all_streams_matched(world: &mut OcflWorld) {
    assert!(world.read.streamed > 0, "nothing was streamed");
}

#[when("I read the version history")]
fn read_history(world: &mut OcflWorld) {
    world.read.history = open(world).expect("open").history();
}

/// One history entry as a table row: version, created, message, user name
/// and address (empty when absent).
fn history_row(info: &VersionInfo) -> Vec<String> {
    let user = info.user.as_ref();
    vec![
        info.version.to_string(),
        info.created.clone(),
        info.message.clone().unwrap_or_default(),
        user.map(|u| u.name.clone()).unwrap_or_default(),
        user.and_then(|u| u.address.clone()).unwrap_or_default(),
    ]
}

#[then("the history should be:")]
fn history_is(world: &mut OcflWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("a history table");
    let expected: Vec<Vec<String>> = table.rows.iter().skip(1).cloned().collect();
    let actual: Vec<Vec<String>> = world.read.history.iter().map(history_row).collect();
    assert_eq!(actual, expected);
}

#[when(expr = "I compare version {string} with version {string}")]
fn compare(world: &mut OcflWorld, from: String, to: String) {
    world.read.changes = open(world)
        .expect("open")
        .diff(&v(&from), &v(&to))
        .expect("diff");
}

/// One change in words, as the feature writes it.
fn describe(change: &Change) -> String {
    match change {
        Change::Added { path, .. } => format!("added {path}"),
        Change::Deleted { path, .. } => format!("deleted {path}"),
        Change::Modified { path, .. } => format!("modified {path}"),
        Change::Renamed { from, to, .. } => format!("renamed {from} to {to}"),
    }
}

#[then(expr = "the changes should be {string}")]
fn changes_are(world: &mut OcflWorld, expected: String) {
    let described: Vec<String> = world.read.changes.iter().map(describe).collect();
    assert_eq!(described, list(&expected));
}

#[given(expr = "a stray file {string} in the object")]
fn stray_file(world: &mut OcflWorld, rel: String) {
    put(backend(world), "obj", &rel, b"not in the inventory");
}

#[given(
    expr = "version {string} content and a version inventory exist while the root head is {string}"
)]
fn unpublished_version(world: &mut OcflWorld, version: String, _head: String) {
    let mut spec = three_versions();
    spec.versions.push(vec![("e.txt", b"echo" as &[u8])]);
    let next = build(&MemoryBackend::new(), "x", &spec);
    put(
        backend(world),
        "obj",
        &format!("{version}/content/e.txt"),
        b"echo",
    );
    write_inventory(backend(world), "obj", &version, &next);
}

#[when("I open the object")]
fn open_object(world: &mut OcflWorld) {
    match open(world) {
        Ok(object) => world.read.head = Some(object.head().clone()),
        Err(err) => world.read.refusal = Some(err),
    }
}

#[then(expr = "its head should be {string}")]
fn head_is(world: &mut OcflWorld, expected: String) {
    assert_eq!(world.read.head.as_ref().expect("opened"), &v(&expected));
}

#[then(expr = "pinning version {string} should be refused as an unknown version")]
fn pin_refused(world: &mut OcflWorld, version: String) {
    let err = open(world)
        .expect("open")
        .state(&v(&version))
        .expect_err("unpublished");
    assert!(matches!(err, ReadError::VersionNotFound { .. }), "{err:?}");
}

#[given(expr = "the storage root that holds it also holds the objects {string} and {string}")]
fn storage_root(world: &mut OcflWorld, first: String, second: String) {
    let backend = backend(world);
    put(backend, "", "0=ocfl_1.1", b"ocfl_1.1\n");
    build(backend, &first, &three_versions());
    build(backend, &second, &three_versions());
}

#[when("I open the storage root")]
fn open_storage_root(world: &mut OcflWorld) {
    let root = ready(StorageRoot::open(backend(world))).expect("storage root");
    world.read.objects = root.objects().to_vec();
}

#[then(expr = "the discovered objects should be {string}")]
fn discovered(world: &mut OcflWorld, expected: String) {
    assert_eq!(world.read.objects, list(&expected));
}

#[given(expr = "the content file {string} is altered")]
fn altered(world: &mut OcflWorld, rel: String) {
    put(backend(world), "obj", &rel, b"altered bytes");
}

#[when(expr = "I stream {string} of version {string}")]
fn stream_one(world: &mut OcflWorld, path: String, version: String) {
    let result = stream_file(&open(world).expect("open"), &v(&version), &path, 2);
    world.read.refusal = result.err();
}

#[given(expr = "the object file {string} is removed")]
fn removed_root_file(world: &mut OcflWorld, rel: String) {
    delete(backend(world), "obj", &rel);
}

#[given(expr = "the object file {string} is replaced with {string}")]
fn replaced_root_file(world: &mut OcflWorld, rel: String, text: String) {
    put(backend(world), "obj", &rel, text.as_bytes());
}

#[then(expr = "the read should be refused with OCFL code {string}")]
fn refused_with(world: &mut OcflWorld, code: String) {
    let err = world.read.refusal.take().expect("a refusal");
    assert_eq!(err.code(), Some(code.as_str()), "{err:?}");
}

#[given("the pinned OCFL 1.1 community fixtures")]
fn pinned_fixtures(world: &mut OcflWorld) {
    let dir = std::env::var_os("FILESQUID_OCFL_FIXTURES").expect("filtered without fixtures");
    world.read.fixtures = Some(PathBuf::from(dir));
}

#[when("I resolve every version of every object in good-objects")]
fn read_good_fixtures(world: &mut OcflWorld) {
    let group = world
        .read
        .fixtures
        .as_ref()
        .expect("fixtures")
        .join("good-objects");
    let staging = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("features-fixture-reads");
    let backend = FsBackend::new(&group, staging).expect("fs backend");
    let mut names: Vec<String> = std::fs::read_dir(&group)
        .expect("list")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in names {
        let object = ready(Object::open(&backend, &name)).expect("open fixture");
        let count = stream_everything(&object, 4096).expect("read fixture");
        world.read.streamed += count;
    }
}
