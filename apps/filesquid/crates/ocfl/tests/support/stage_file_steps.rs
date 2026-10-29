//! Step definitions for `streamed_stage_OCFL-S-026.feature` (IO-1.7). The
//! source file sits on a counting backend (in memory or on `FsBackend`),
//! and the change model is the Background's, staging on the rig's staging
//! backend.

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use cucumber::{given, then, when};
use filesquid_ocfl::change::ChangeError;
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::storage::{
    write_whole, FsBackend, MemoryBackend, StorageBackend, MAX_CHUNK_BYTES,
};

use crate::change::{meta, model, state_of};
use crate::change_steps::rig;
use crate::counting::CountingBackend;
use crate::rig::{key, ready};
use crate::{OcflWorld, Scratch};

/// The key the source file is read from.
const SOURCE_KEY: &str = "incoming/photo.raw";

/// The source of one scenario, on the backend it names.
enum Source {
    Memory(CountingBackend<MemoryBackend>),
    Fs {
        source: CountingBackend<FsBackend>,
        _scratch: Scratch,
    },
}

/// What the streamed-stage scenarios carry between steps.
#[derive(Default)]
pub struct StageFileState {
    source: Option<Source>,
    bytes: Vec<u8>,
    changed: Vec<u8>,
    result: Option<Result<String, ChangeError>>,
}

impl fmt::Debug for StageFileState {
    /// Show the outcome; the sources hold backends that are not `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StageFileState")
            .field("result", &self.result)
            .finish_non_exhaustive()
    }
}

/// Run `$body` with `$s` bound to the scenario's counting source.
macro_rules! on_source {
    ($world:expr, $s:ident => $body:expr) => {
        match $world
            .stage_file
            .source
            .as_ref()
            .expect("Given a source file")
        {
            Source::Memory($s) => $body,
            Source::Fs { source: $s, .. } => $body,
        }
    };
}

/// A fresh `FsBackend` source on its own scratch directory.
fn fs_source() -> Source {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ocfl-stage-file-bdd-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    Source::Fs {
        source: CountingBackend::new(backend),
        _scratch: Scratch(dir),
    }
}

/// An empty source on the backend named `backend`.
fn source_on(backend: &str) -> Source {
    match backend {
        "memory" => Source::Memory(CountingBackend::new(MemoryBackend::new())),
        "filesystem" => fs_source(),
        other => panic!("unknown backend {other}"),
    }
}

/// Bytes of two whole chunks and a part of a third.
fn three_chunks() -> Vec<u8> {
    let len = usize::try_from(2 * MAX_CHUNK_BYTES + 123).expect("fits");
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// The chunk reads of one whole pass over `len` bytes: a full chunk per
/// `MAX_CHUNK_BYTES`, then the short or empty read that ends the stream.
fn pass(len: usize) -> usize {
    ((len as u64) / MAX_CHUNK_BYTES + 1) as usize
}

/// Store `bytes` as the source file of `source` and forget the reads.
fn fill<B: StorageBackend>(source: &CountingBackend<B>, bytes: &[u8]) {
    ready(write_whole(source.inner(), &key(SOURCE_KEY), bytes)).expect("source");
    source.reset_counts();
}

/// Put the scenario's source on `backend` holding `bytes`.
fn give_source(world: &mut OcflWorld, backend: &str, bytes: Vec<u8>) {
    world.stage_file.source = Some(source_on(backend));
    on_source!(world, s => fill(s, &bytes));
    world.stage_file.bytes = bytes;
}

#[given(expr = "a source file of three chunks on the {word} backend")]
fn three_chunk_source(world: &mut OcflWorld, backend: String) {
    give_source(world, &backend, three_chunks());
}

#[given(expr = "a source file holding the bytes of {string} on the {word} backend")]
fn source_like(world: &mut OcflWorld, path: String, backend: String) {
    let version = rig(world).open().expect("open").head().clone();
    let state = rig(world)
        .open()
        .expect("open")
        .state(&version)
        .expect("state");
    let content = state
        .get(&path)
        .expect("committed file")
        .content_path
        .clone();
    let bytes = rig(world).bytes(&format!("obj/{content}"));
    give_source(world, &backend, bytes);
}

#[given(
    expr = "a source file of three chunks on the {word} backend that changes after it is hashed"
)]
fn changing_source(world: &mut OcflWorld, backend: String) {
    let bytes = three_chunks();
    let mut changed = bytes.clone();
    changed[0] ^= 0xff;
    give_source(world, &backend, bytes.clone());
    let after = pass(bytes.len());
    on_source!(world, s => s.swap_after(after, &key(SOURCE_KEY), changed.clone()));
    world.stage_file.changed = changed;
}

#[when(expr = "writer {string} stages it as {string} with stage_file")]
fn stage_it(world: &mut OcflWorld, holder: String, path: String) {
    let rig = rig(world);
    let m = model(rig, &holder);
    let result = on_source!(world, s => ready(m.stage_file(&path, s, SOURCE_KEY)));
    world.stage_file.result = Some(result);
}

#[when(expr = "writer {string} commits the streamed file")]
fn commit_it(world: &mut OcflWorld, holder: String) {
    let rig = rig(world);
    let receipt = ready(model(rig, &holder).commit(meta(rig, "streamed"))).expect("commit");
    assert_eq!(receipt.version.to_string(), "v2");
}

#[then("stage_file should answer the sha256 digest of the source file")]
fn answers_digest(world: &mut OcflWorld) {
    let expected = DigestAlgorithm::Sha256.digest_hex(&world.stage_file.bytes);
    let result = world.stage_file.result.as_ref().expect("staged");
    assert_eq!(result.as_ref().expect("stage_file"), &expected);
}

#[then("the source should have been read in chunks of at most 8 MiB, once to hash it and once to copy it")]
fn read_twice_in_chunks(world: &mut OcflWorld) {
    let chunks = pass(world.stage_file.bytes.len());
    let (reads, largest) = on_source!(world, s => (s.reads(), s.largest_read()));
    assert!(largest <= MAX_CHUNK_BYTES, "{largest}");
    assert!(chunks > 1, "more than one chunk");
    assert_eq!(reads, 2 * chunks);
}

#[then("the source should have been read once, to hash it")]
fn read_once(world: &mut OcflWorld) {
    let chunks = pass(world.stage_file.bytes.len());
    let reads = on_source!(world, s => s.reads());
    assert_eq!(reads, chunks);
}

#[then(expr = "the committed {string} should hold the source file's bytes")]
fn committed_bytes(world: &mut OcflWorld, path: String) {
    let digest = DigestAlgorithm::Sha256.digest_hex(&world.stage_file.bytes);
    assert_eq!(state_of(rig(world), "v2")[&path], digest);
    assert_eq!(
        rig(world).bytes(&format!("obj/v2/content/{path}")),
        world.stage_file.bytes
    );
    rig(world).assert_valid();
}

#[then("stage_file should be refused because the source changed")]
fn refused_changed(world: &mut OcflWorld) {
    let result = world.stage_file.result.as_ref().expect("staged");
    assert!(
        matches!(result, Err(ChangeError::SourceChanged { .. })),
        "{result:?}"
    );
}

#[then("no blob should have been copied into the staging area")]
fn not_copied(world: &mut OcflWorld) {
    let digest = DigestAlgorithm::Sha256.digest_hex(&world.stage_file.bytes);
    assert_no_staged_blob(world, &digest);
}

#[then("no staged blob should be left under either digest")]
fn no_blob_left(world: &mut OcflWorld) {
    for bytes in [&world.stage_file.bytes, &world.stage_file.changed] {
        let digest = DigestAlgorithm::Sha256.digest_hex(bytes);
        assert_no_staged_blob(world, &digest);
    }
}

/// Fail when writer-a's staging area holds a blob under `digest`.
fn assert_no_staged_blob(world: &OcflWorld, digest: &str) {
    let staging = &*rig(world).staging;
    let blob = key(&format!("changes/writer-a/blobs/{digest}"));
    assert!(!ready(staging.exists(&blob)).expect("exists"), "{blob}");
}
