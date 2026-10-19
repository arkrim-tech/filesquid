//! Cucumber harness for filesquid-ocfl Gherkin under `tests/features/`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use cucumber::{given, then, when, writer, StatsWriter as _, World, WriterExt as _};

#[path = "support/object_builder.rs"]
mod builder;
#[path = "support/community_validator.rs"]
mod community;
#[path = "support/content_fixtures.rs"]
mod content;
#[path = "support/enumerate.rs"]
mod enumerate;
#[path = "support/enumerate_steps.rs"]
mod enumerate_steps;
#[path = "support/inventory_example.rs"]
mod example;
#[path = "support/fault_backend.rs"]
mod fault;
#[path = "support/fixtures.rs"]
mod fixtures;
#[path = "support/inventory_steps.rs"]
mod inventory_steps;
#[path = "support/opaque.rs"]
mod opaque;
#[path = "support/opaque_steps.rs"]
mod opaque_steps;
#[path = "support/payload_steps.rs"]
mod payload_steps;
#[path = "support/read_steps.rs"]
mod read_steps;
#[path = "support/recovery.rs"]
mod recovery;
#[path = "support/recovery_steps.rs"]
mod recovery_steps;
#[path = "support/repair_steps.rs"]
mod repair_steps;
#[path = "support/write_rig.rs"]
mod rig;
#[path = "support/source_scan.rs"]
mod source_scan;
#[path = "support/validate_steps.rs"]
mod validate_steps;
#[path = "support/write_steps.rs"]
mod write_steps;
use filesquid_ocfl::storage::{
    ChunkReader, ETag, FsBackend, MemoryBackend, MultipartWriter, Precondition, StorageBackend,
    StorageError, StorageKey, MAX_CHUNK_BYTES,
};
use filesquid_ocfl::{check_storage_root, OcflError};

/// The backend a storage scenario runs against. The filesystem variant owns
/// its scratch directory so it is removed when the scenario ends.
#[derive(Debug)]
enum AnyBackend {
    Memory(MemoryBackend),
    Fs {
        backend: FsBackend,
        _scratch: Scratch,
    },
}

/// Scratch directory for one filesystem scenario, removed on drop.
#[derive(Debug)]
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, Default, World)]
struct OcflWorld {
    cargo: String,
    lib: String,
    desktop_cargo: String,
    desktop_src: String,
    backend: Option<AnyBackend>,
    last_error: Option<StorageError>,
    remembered: Option<ETag>,
    uploaded: Vec<u8>,
    chunks: Vec<Vec<u8>>,
    pages: Vec<Vec<String>>,
    wasm_build: Option<(bool, String)>,
    guard_result: Option<Result<(), OcflError>>,
    inventory: inventory_steps::InventoryState,
    read: read_steps::ReadState,
    validate: validate_steps::ValidateState,
    write: write_steps::WriteState,
    recovery: recovery_steps::RecoveryState,
    repair: repair_steps::RepairState,
    payload: payload_steps::PayloadState,
    opaque: opaque_steps::OpaqueState,
    enumerate: enumerate_steps::EnumerateState,
}

/// Run `$body` with `$b` bound to the scenario's concrete backend.
macro_rules! on_backend {
    ($world:expr, $b:ident => $body:expr) => {
        match $world.backend.as_ref().expect("Given a storage backend") {
            AnyBackend::Memory(m) => {
                let $b = m;
                $body
            }
            AnyBackend::Fs { backend: f, .. } => {
                let $b = f;
                $body
            }
        }
    };
}

fn read_rel(parts: &[&str]) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for part in parts {
        path.push(part);
    }
    fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {}", path.display()))
}

fn load_identity(world: &mut OcflWorld) {
    world.cargo = read_rel(&["Cargo.toml"]);
    world.lib = read_rel(&["src", "lib.rs"]);
    world.desktop_cargo = read_rel(&["..", "desktop", "Cargo.toml"]);
    world.desktop_src = read_rel(&["..", "desktop", "src", "lib.rs"])
        + &read_rel(&["..", "desktop", "src", "main.rs"]);
}

#[when("I inspect the filesquid-ocfl public API")]
fn inspect_api(world: &mut OcflWorld) {
    load_identity(world);
}

/// FileSquid application-layer and catalog type names the engine must not
/// name anywhere in its sources (OCFL-S-003).
const APPLICATION_TYPES: [&str; 6] = [
    "CollectionSnapshot",
    "C2paImportRecord",
    "RecipeOperation",
    "FileObject",
    "CatalogEntry",
    "filesquid_core",
];

#[then("FileSquid catalog, recipe, C2PA, and collection-snapshot types should be absent")]
fn no_application_types(world: &mut OcflWorld) {
    let sources = source_scan::read_sources().expect("scan src");
    assert!(sources.len() > 5, "engine sources not found");
    for needle in APPLICATION_TYPES {
        assert!(
            !world.lib.contains(needle),
            "{needle} must be absent from the engine crate"
        );
        let found: Vec<&str> = sources
            .iter()
            .filter(|(_, text)| text.contains(needle))
            .map(|(file, _)| file.as_str())
            .collect();
        assert!(found.is_empty(), "{needle} appears in {found:?}");
    }
}

#[when("I inspect filesquid-ocfl library dependencies")]
fn inspect_deps(world: &mut OcflWorld) {
    load_identity(world);
}

#[then("Arrow, Parquet, DuckDB, and Iceberg should be absent from default features")]
fn no_analytics(world: &mut OcflWorld) {
    let cargo = world.cargo.to_ascii_lowercase();
    for needle in ["arrow", "parquet", "duckdb", "iceberg"] {
        assert!(!cargo.contains(needle), "{needle} must be absent");
    }
}

#[when("I inspect filesquid-ocfl product identity")]
fn inspect_identity(world: &mut OcflWorld) {
    load_identity(world);
}

#[then("it should remain installable without the FileSquid app")]
fn installable(world: &mut OcflWorld) {
    assert!(world
        .cargo
        .contains("Installable without the FileSquid app"));
}

#[then("it should build and publish without other FileSquid crates")]
fn standalone_crate(world: &mut OcflWorld) {
    assert!(
        !world.cargo.contains("path = \"../"),
        "filesquid-ocfl MUST NOT path-depend on another FileSquid crate"
    );
}

#[then("community OCFL programs should not be the engine")]
fn no_community_engine(world: &mut OcflWorld) {
    let cargo = world.cargo.to_ascii_lowercase();
    for needle in ["rocfl", "ocfl-py", "ocfl-java", "ocfl-tools"] {
        assert!(
            !cargo.contains(needle),
            "{needle} must not be a crate dependency"
        );
    }
}

#[when("I inspect the desktop crate")]
fn inspect_desktop(world: &mut OcflWorld) {
    load_identity(world);
}

#[then("desktop Cargo should depend on filesquid-ocfl")]
fn desktop_depends(world: &mut OcflWorld) {
    assert!(world.desktop_cargo.contains("filesquid-ocfl"));
}

#[then("FileSquid should not shell out to fsquid for canonical writes")]
fn no_cli_shell(world: &mut OcflWorld) {
    assert!(!world.desktop_src.contains("fsquid"));
}

fn key(s: &str) -> StorageKey {
    StorageKey::new(s).expect("valid key")
}

fn new_fs_backend() -> AnyBackend {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ocfl-features-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    AnyBackend::Fs {
        backend,
        _scratch: Scratch(dir),
    }
}

#[given(regex = r"^an empty (in-memory|local filesystem) storage backend$")]
fn empty_backend(world: &mut OcflWorld, kind: String) {
    world.backend = Some(match kind.as_str() {
        "in-memory" => AnyBackend::Memory(MemoryBackend::new()),
        _ => new_fs_backend(),
    });
}

#[when(expr = "I put {string} at {string}")]
async fn put_at(world: &mut OcflWorld, text: String, raw_key: String) {
    let result = match StorageKey::new(&raw_key) {
        Ok(k) => on_backend!(world, b => b.put(&k, text.as_bytes()).await.map(|_| ())),
        Err(err) => Err(err),
    };
    world.last_error = result.err();
}

#[then(expr = "{string} should exist with size {int}")]
async fn exists_with_size(world: &mut OcflWorld, raw_key: String, size: u64) {
    let k = key(&raw_key);
    let meta = on_backend!(world, b => {
        assert!(b.exists(&k).await.expect("exists"));
        b.head(&k).await.expect("head")
    });
    assert_eq!(meta.size, size);
}

async fn stream_all<B: StorageBackend>(b: &B, k: StorageKey, chunk: u64) -> Vec<Vec<u8>> {
    let mut reader = ChunkReader::new(b, k, chunk).expect("reader");
    let mut chunks = Vec::new();
    while let Some(bytes) = reader.next_chunk().await.expect("chunk") {
        chunks.push(bytes);
    }
    chunks
}

#[then(expr = "streaming {string} should return {string}")]
async fn streaming_returns(world: &mut OcflWorld, raw_key: String, text: String) {
    let k = key(&raw_key);
    let chunks = on_backend!(world, b => stream_all(b, k, 3).await);
    assert_eq!(chunks.concat(), text.as_bytes());
}

#[given(expr = "{string} holds {string} and I remember its entity tag")]
async fn holds_and_remember(world: &mut OcflWorld, raw_key: String, text: String) {
    let k = key(&raw_key);
    let meta = on_backend!(world, b => b
        .put_if(&k, text.as_bytes(), &Precondition::IfAbsent)
        .await
        .expect("create"));
    world.remembered = Some(meta.etag);
}

#[when(expr = "another writer replaces {string} with {string}")]
async fn another_writer(world: &mut OcflWorld, raw_key: String, text: String) {
    let k = key(&raw_key);
    on_backend!(world, b => b.put(&k, text.as_bytes()).await.expect("put"));
}

#[when(
    expr = "I replace {string} with {string} only if it still matches the remembered entity tag"
)]
async fn cas_replace(world: &mut OcflWorld, raw_key: String, text: String) {
    let k = key(&raw_key);
    let tag = world.remembered.clone().expect("remembered tag");
    let result = on_backend!(world, b => b
        .put_if(&k, text.as_bytes(), &Precondition::IfMatch(tag))
        .await);
    world.last_error = result.err();
}

#[when(expr = "I create {string} with {string} only if it is absent")]
async fn create_if_absent(world: &mut OcflWorld, raw_key: String, text: String) {
    let k = key(&raw_key);
    let result = on_backend!(world, b => b
        .put_if(&k, text.as_bytes(), &Precondition::IfAbsent)
        .await);
    world.last_error = result.err();
}

#[then("the write should be rejected as a precondition failure")]
fn rejected_precondition(world: &mut OcflWorld) {
    let err = world.last_error.take().expect("an error");
    assert!(
        matches!(err, StorageError::PreconditionFailed { .. }),
        "{err:?}"
    );
}

#[then("the write should be rejected as an invalid key")]
fn rejected_invalid_key(world: &mut OcflWorld) {
    let err = world.last_error.take().expect("an error");
    assert!(matches!(err, StorageError::InvalidKey { .. }), "{err:?}");
}

async fn upload_parts<B: StorageBackend>(b: &B, k: StorageKey, data: &[u8], part: usize) {
    let mut writer = MultipartWriter::start(b, k).await.expect("start");
    for bytes in data.chunks(part) {
        writer.write_part(bytes).await.expect("part");
    }
    writer.finish(None).await.expect("finish");
}

#[given(expr = "a {int} MiB file at {string} uploaded in {int} MiB multipart parts")]
async fn multipart_file(world: &mut OcflWorld, mib: usize, raw_key: String, part_mib: usize) {
    let data: Vec<u8> = (0..mib * 1024 * 1024).map(|i| (i % 249) as u8).collect();
    let k = key(&raw_key);
    on_backend!(world, b => upload_parts(b, k, &data, part_mib * 1024 * 1024).await);
    world.uploaded = data;
}

#[when(expr = "I stream {string} in {int} KiB chunks")]
async fn stream_in_chunks(world: &mut OcflWorld, raw_key: String, kib: u64) {
    let k = key(&raw_key);
    world.chunks = on_backend!(world, b => stream_all(b, k, kib * 1024).await);
}

#[then(expr = "every chunk should be at most {int} KiB")]
fn chunks_bounded(world: &mut OcflWorld, kib: usize) {
    assert!(!world.chunks.is_empty());
    assert!(world.chunks.iter().all(|c| c.len() <= kib * 1024));
}

#[then("the streamed bytes should equal the uploaded bytes")]
fn streamed_equal(world: &mut OcflWorld) {
    assert_eq!(world.chunks.concat(), world.uploaded);
}

#[given(expr = "{int} files under {string}")]
async fn files_under(world: &mut OcflWorld, count: usize, prefix: String) {
    for i in (0..count).rev() {
        let k = key(&format!("{prefix}file-{i}"));
        on_backend!(world, b => b.put(&k, b"x").await.expect("put"));
    }
    let outside = key("elsewhere/file");
    on_backend!(world, b => b.put(&outside, b"y").await.expect("put"));
}

async fn list_all<B: StorageBackend>(b: &B, prefix: &str, size: usize) -> Vec<Vec<String>> {
    let mut pages = Vec::new();
    let mut after: Option<StorageKey> = None;
    for _ in 0..100 {
        let page = b.list(prefix, after.as_ref(), size).await.expect("list");
        pages.push(page.keys.iter().map(|k| k.as_str().to_string()).collect());
        match page.next {
            Some(next) => after = Some(next),
            None => return pages,
        }
    }
    panic!("listing did not finish within 100 pages");
}

#[when(expr = "I list {string} with a page size of {int}")]
async fn list_pages(world: &mut OcflWorld, prefix: String, size: usize) {
    world.pages = on_backend!(world, b => list_all(b, &prefix, size).await);
}

#[then(expr = "I should receive {int} pages of at most {int} keys each")]
fn pages_are_bounded(world: &mut OcflWorld, pages: usize, size: usize) {
    assert_eq!(world.pages.len(), pages);
    for page in &world.pages {
        assert!(!page.is_empty() && page.len() <= size, "page {page:?}");
    }
}

#[then(expr = "the listed keys should be {string}")]
fn listed_keys_are(world: &mut OcflWorld, expected: String) {
    let expected: Vec<String> = expected.split(", ").map(str::to_string).collect();
    assert_eq!(world.pages.concat(), expected);
}

#[when("I request a read chunk one byte larger than the maximum chunk size")]
async fn oversize_chunk(world: &mut OcflWorld) {
    let k = key("objects/a/x");
    let result = on_backend!(world, b => {
        b.put(&k, b"x").await.expect("put");
        ChunkReader::new(b, k, MAX_CHUNK_BYTES + 1).map(|_| ())
    });
    world.last_error = result.err();
}

#[then("the read should be rejected as over the limit")]
fn rejected_limit(world: &mut OcflWorld) {
    let err = world.last_error.take().expect("an error");
    assert!(matches!(err, StorageError::LimitExceeded { .. }), "{err:?}");
}

fn assert_backend<B: StorageBackend>() {}

#[then("a storage backend trait with local filesystem and in-memory backends should be available")]
fn backend_available(world: &mut OcflWorld) {
    assert_backend::<MemoryBackend>();
    assert_backend::<FsBackend>();
    assert!(world.lib.contains("pub mod storage"));
    let cargo = world.cargo.replace(' ', "");
    assert!(
        cargo.contains("default=[\"fs\"]"),
        "fs MUST be a default-on feature"
    );
}

#[then("the engine core should not call std::fs directly")]
fn core_avoids_std_fs(_world: &mut OcflWorld) {
    let (findings, scanned) = source_scan::io_findings_outside_allowed().expect("scan src");
    assert!(scanned > 5, "engine sources not found");
    assert!(
        findings.is_empty(),
        "direct I/O outside the fs backend: {findings:?}"
    );
}

#[when("I build filesquid-ocfl for wasm32-unknown-unknown with default features off")]
fn build_wasm(world: &mut OcflWorld) {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("wasm32-check");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .current_dir(&workspace)
        .env("CARGO_TARGET_DIR", &target_dir)
        .args(["build", "-p", "filesquid-ocfl", "--locked"])
        .args([
            "--no-default-features",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .output()
        .expect("run cargo");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    world.wasm_build = Some((output.status.success(), stderr));
}

#[then("the build should succeed")]
fn build_succeeds(world: &mut OcflWorld) {
    let (ok, stderr) = world.wasm_build.take().expect("a build ran");
    assert!(
        ok,
        "wasm32 build failed. If the target is missing, run \
         `rustup target add wasm32-unknown-unknown`.\n{stderr}"
    );
}

/// The scenario's filesystem root and scratch directory; panics for a memory
/// backend, because links and directories exist only on a filesystem.
fn fs_dirs(world: &OcflWorld) -> (PathBuf, PathBuf) {
    match world.backend.as_ref().expect("Given a storage backend") {
        AnyBackend::Fs { backend, _scratch } => (backend.root().to_path_buf(), _scratch.0.clone()),
        AnyBackend::Memory(_) => panic!("this step needs a local filesystem backend"),
    }
}

/// Store a one-byte file at `raw_key` through the scenario's backend.
async fn store(world: &OcflWorld, raw_key: &str) {
    let k = key(raw_key);
    on_backend!(world, b => b.put(&k, b"x").await.expect("put"));
}

#[given(expr = "an OCFL storage root with the object {string}")]
async fn storage_root_with_object(world: &mut OcflWorld, object: String) {
    store(world, "0=ocfl_1.1").await;
    for name in ["0=ocfl_object_1.1", "inventory.json", "v1/content/a.txt"] {
        store(world, &format!("{object}/{name}")).await;
    }
}

#[given(expr = "a stored file at {string}")]
async fn stored_file(world: &mut OcflWorld, raw_key: String) {
    let (root, _) = match world.backend {
        Some(AnyBackend::Fs { .. }) => fs_dirs(world),
        _ => return store(world, &raw_key).await,
    };
    // Written directly: the backend itself refuses nothing here, but writing
    // with std::fs mirrors a file placed by another tool.
    let path = root.join(&raw_key);
    fs::create_dir_all(path.parent().expect("parent")).expect("parent dir");
    fs::write(path, b"x").expect("write");
}

#[given(expr = "an empty directory at {string}")]
fn empty_directory(world: &mut OcflWorld, raw_key: String) {
    let (root, _) = fs_dirs(world);
    fs::create_dir_all(root.join(raw_key)).expect("empty dir");
}

#[cfg(unix)]
#[given(expr = "a soft link at {string} to a file outside the root")]
fn soft_link(world: &mut OcflWorld, raw_key: String) {
    let (root, scratch) = fs_dirs(world);
    let outside = scratch.join("outside.txt");
    fs::write(&outside, b"outside").expect("outside file");
    std::os::unix::fs::symlink(outside, root.join(raw_key)).expect("soft link");
}

#[cfg(unix)]
#[given(expr = "the file {string} is hard linked from outside the root")]
fn hard_link(world: &mut OcflWorld, raw_key: String) {
    let (root, scratch) = fs_dirs(world);
    fs::hard_link(root.join(raw_key), scratch.join("hard-link")).expect("hard link");
}

#[when("I run the storage root guard")]
async fn run_guard(world: &mut OcflWorld) {
    let result = on_backend!(world, b => check_storage_root(b).await);
    world.guard_result = Some(result);
}

/// The guard's outcome from the When step.
fn guard_result(world: &mut OcflWorld) -> Result<(), OcflError> {
    world.guard_result.take().expect("the guard ran")
}

#[then("the guard should pass")]
fn guard_passes(world: &mut OcflWorld) {
    assert_eq!(guard_result(world), Ok(()));
}

#[then(regex = r#"^the guard should refuse a \.git directory in "(.*)"$"#)]
fn guard_refuses_git(world: &mut OcflWorld, root: String) {
    assert_eq!(
        guard_result(world),
        Err(OcflError::GitDirectoryForbidden { root })
    );
}

#[then(expr = "the guard should refuse an empty directory at {string}")]
fn guard_refuses_empty(world: &mut OcflWorld, key: String) {
    let expected = StorageError::EmptyDirectoryForbidden { key };
    assert_eq!(guard_result(world), Err(OcflError::Storage(expected)));
}

#[then(expr = "the guard should refuse a soft link at {string}")]
fn guard_refuses_soft_link(world: &mut OcflWorld, key: String) {
    let expected = StorageError::SymlinkForbidden { key };
    assert_eq!(guard_result(world), Err(OcflError::Storage(expected)));
}

#[then(expr = "the guard should refuse a hard link at {string}")]
fn guard_refuses_hard_link(world: &mut OcflWorld, key: String) {
    let expected = StorageError::HardLinkForbidden { key };
    assert_eq!(guard_result(world), Err(OcflError::Storage(expected)));
}

fn write_junit_report(path: &PathBuf, passed: usize, failed: usize, skipped: usize) {
    let tests = passed + failed + skipped;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="filesquid_ocfl.features" tests="{tests}" failures="{failed}" skipped="{skipped}" />
"#
    );
    fs::write(path, xml).expect("write junit");
}

/// Whether `scenario` runs here. Scenarios tagged `@unix` need Unix links
/// and run on Unix only (macOS and Linux); elsewhere they are left out
/// instead of failing on steps compiled only for Unix. Features tagged
/// `@fixtures` read the pinned OCFL community fixtures, which are committed
/// under `tests/fixtures/ocfl/1.1` (OCFL-S-011), so they always run.
fn runs_here(scenario: &cucumber::gherkin::Scenario) -> bool {
    cfg!(unix) || !scenario.tags.iter().any(|tag| tag == "unix")
}

#[tokio::main]
async fn main() {
    let results_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    let _ = fs::remove_dir_all(&results_dir);
    fs::create_dir_all(&results_dir).expect("allure-results");
    let features = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/features");
    let writer = OcflWorld::cucumber()
        .with_writer(writer::Basic::stdout().summarized())
        .filter_run(features, |_, _, scenario| runs_here(scenario))
        .await;
    write_junit_report(
        &results_dir.join("cucumber-junit.xml"),
        writer.passed_steps(),
        writer.failed_steps(),
        writer.skipped_steps(),
    );
    if writer.execution_has_failed() {
        std::process::exit(1);
    }
}
