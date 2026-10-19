//! Cucumber harness for the F-052 Gherkin under `tests/features/`.
//!
//! Each run clears `tests/allure-results/` and writes
//! `tests/allure-results/cucumber-junit.xml` with one `<testcase>` per scenario.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use cucumber::{given, then, when, writer, StatsWriter as _, World, WriterExt as _};
use filesquid_metadata::cli::USAGE;
use filesquid_metadata::csv::{apply_csv, to_csv};
use filesquid_metadata::extract::{extract_from_bytes, extract_from_reader};
use filesquid_metadata::file_ref::FileRef;
use filesquid_metadata::json::{apply_json, parse_json, sha256_hex, to_json, JsonTarget};
use filesquid_metadata::markdown::to_markdown;
use filesquid_metadata::record::Record;
use filesquid_metadata::store::ocfl::LocalObject;
use filesquid_metadata::store::sidecar::SidecarStore;
use filesquid_metadata::store::{MetadataStore, StoreError, METADATA_MD};
use filesquid_metadata::xmp::{from_xmp, to_xmp};
use filesquid_metadata::{MetadataError, MetadataNamespace};
use filesquid_ocfl::change::{ChangeError, ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{complete_immediately, FsBackend, MemoryBackend};
use filesquid_ocfl::validate::validate_object;
use filesquid_ocfl::write::{rfc3339_utc, CommitError, LeaseConfig, LeaseStore, SystemClock};
use filesquid_ocfl::VersionNum;

#[path = "support/ocfl_rig.rs"]
mod rig;
#[path = "support/wasm.rs"]
mod wasm;

/// Big-endian TIFF with one ASCII `ImageDescription` ("Cat").
const TIFF_DESCRIPTION: &[u8] = &[
    b'M', b'M', 0, 42, 0, 0, 0, 8, 0, 1, 0x01, 0x0E, 0, 2, 0, 0, 0, 4, b'C', b'a', b't', 0, 0, 0,
    0, 0,
];

/// Crates the library MUST NOT depend on (MD-S-003).
const FORBIDDEN_DEPS: &[&str] = &[
    "axum",
    "tauri",
    "libvips",
    "filesquid-core",
    "filesquid_core",
];

/// FileSquid application identifiers that MUST NOT appear in the public API (MD-S-001).
const APPLICATION_TYPES: &[&str] = &["filesquid_core", "Recipe", "C2pa", "CollectionSnapshot"];

/// Counter that gives each object scenario its own scratch directory.
static OBJECT_DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// World state for F-052 scenarios.
#[derive(Debug, Default, World)]
struct MetadataWorld {
    /// Outcome of the last authoring write.
    write_result: Option<Result<(), MetadataError>>,
    /// Object directory used for `metadata.md` scenarios.
    object_dir: Option<PathBuf>,
    /// Text captured from the last inspection step.
    inspected: String,
    /// Whether the last CSV import failed.
    csv_rejected: bool,
    /// JSON sidecar written by the last export step (outside the object).
    sidecar_path: Option<PathBuf>,
    /// Second object directory that a JSON sidecar is imported into.
    target_dir: Option<PathBuf>,
    /// `metadata.md` text of the target object before a JSON import.
    target_before: String,
    /// Error text of the last JSON import, if it failed.
    json_error: Option<String>,
    /// Outcome of the last `wasm32` build check.
    wasm_build: Option<Result<(), String>>,
    /// Whether the last extraction found an EXIF block and a description.
    extracted_description: bool,
    /// `dcterms:title` found by the last XMP extraction.
    extracted_title: Option<String>,
    /// The object's versions before the last edit through the `ocfl` store.
    versions_before: Vec<String>,
    /// Every file of the object's version directories before the last edit.
    earlier_versions: BTreeMap<PathBuf, Vec<u8>>,
    /// Outcome of the last edit through the `ocfl` store.
    commit_outcome: Option<Result<Option<VersionNum>, StoreError>>,
}

impl Drop for MetadataWorld {
    /// Remove every scratch directory and sidecar a scenario created.
    fn drop(&mut self) {
        for dir in [self.object_dir.take(), self.target_dir.take()]
            .into_iter()
            .flatten()
        {
            remove_dir_if_present(&dir);
        }
        if let Some(path) = self.sidecar_path.take() {
            remove_file_if_present(&path);
        }
    }
}

/// Object identifier bound into every JSON sidecar scenario.
const JSON_OBJECT_ID: &str = "urn:example:harbour";

/// Logical file the JSON sidecar describes.
const JSON_LOGICAL_FILE: &str = "photo.jpg";

/// Bytes of the described logical file.
const JSON_FILE_BYTES: &[u8] = b"harbour-pixels";

/// Remove a file, treating "already absent" as success.
fn remove_file_if_present(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => panic!("remove {}: {err}", path.display()),
    }
}

/// Most directory entries [`version_files`] walks (AGENTS.md §22).
const MAX_WALK_ENTRIES: usize = 10_000;

/// Drive a future that completes on its first poll.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("future completes at once")
}

/// Commit `files` as v1 of the OCFL object `JSON_OBJECT_ID` in `dir`
/// through the `filesquid-ocfl` change model.
fn seed_object(dir: &Path, files: &[(&str, &[u8])]) {
    let storage = FsBackend::with_default_staging(dir).expect("object backend");
    let leases = LeaseStore::new(
        MemoryBackend::new(),
        "leases",
        SystemClock,
        LeaseConfig::default(),
    )
    .expect("leases");
    let staging_backend = MemoryBackend::new();
    let staging = StagingArea::new(&staging_backend, "seed").expect("staging");
    let model = ChangeModel::new(&storage, &leases, "seeder", "", JSON_OBJECT_ID, staging);
    for (path, bytes) in files {
        ready(model.stage_bytes(path, bytes)).expect("stage");
    }
    ready(model.commit(VersionMeta::new(rfc3339_utc(1_791_590_400_000)))).expect("seed");
}

/// Create a fresh OCFL object directory holding the described logical file.
fn object_with_file(bytes: &[u8]) -> PathBuf {
    let dir = fresh_object_dir();
    seed_object(&dir, &[(JSON_LOGICAL_FILE, bytes)]);
    dir
}

/// Open the object in `dir`.
fn local(dir: &Path) -> LocalObject {
    LocalObject::open(dir, None).expect("open object")
}

/// The head `metadata.md` text of the object in `dir`.
fn metadata_text(dir: &Path) -> String {
    to_markdown(&local(dir).load().expect("load metadata.md"))
}

/// The versions of the object in `dir`, oldest first.
fn versions_of(dir: &Path) -> Vec<String> {
    let storage = FsBackend::with_default_staging(dir).expect("backend");
    let object = ready(Object::open(&storage, "")).expect("open object");
    object
        .history()
        .into_iter()
        .map(|info| info.version.to_string())
        .collect()
}

/// Every file under the `v*` version directories of `dir`, with its bytes.
fn version_files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut pending: Vec<PathBuf> = fs::read_dir(dir)
        .expect("object dir")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.is_dir())
        .collect();
    for _ in 0..MAX_WALK_ENTRIES {
        let Some(path) = pending.pop() else {
            return out;
        };
        if path.is_dir() {
            pending.extend(
                fs::read_dir(&path)
                    .expect("dir")
                    .map(|e| e.expect("entry").path()),
            );
        } else {
            out.insert(path.clone(), fs::read(&path).expect("file"));
        }
    }
    panic!(
        "more than {MAX_WALK_ENTRIES} entries under {}",
        dir.display()
    );
}

/// Run `filesquid-metadata <cmd> json <sidecar> --id --file --object <dir>`.
fn run_json_cli(cmd: &str, sidecar: &Path, object: &Path) -> Result<i32, String> {
    let args: Vec<String> = [
        cmd,
        "json",
        &sidecar.to_string_lossy(),
        "--id",
        JSON_OBJECT_ID,
        "--file",
        JSON_LOGICAL_FILE,
        "--object",
        &object.to_string_lossy(),
    ]
    .iter()
    .map(|arg| (*arg).to_string())
    .collect();
    filesquid_metadata::cli::run(&args)
}

/// Read the sidecar written by the last export step.
fn sidecar_text(world: &MetadataWorld) -> String {
    let path = world.sidecar_path.as_ref().expect("sidecar exported");
    let bytes = fs::read(path).expect("sidecar file");
    String::from_utf8(bytes).expect("sidecar is UTF-8")
}

/// Import `text` into `target` and record the outcome on the world.
fn import_into_target(world: &mut MetadataWorld, target: PathBuf) {
    world.target_before = metadata_text(&target);
    world.versions_before = versions_of(&target);
    let sidecar = world.sidecar_path.clone().expect("sidecar exported");
    world.json_error = run_json_cli("import", &sidecar, &target).err();
    world.target_dir = Some(target);
}

#[given(expr = "an OCFL object with a logical file and dcterms:title {word}")]
fn object_with_title(world: &mut MetadataWorld, title: String) {
    let dir = object_with_file(JSON_FILE_BYTES);
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "title", title)
        .expect("set title");
    local(&dir).save(&record).expect("commit metadata.md");
    world.object_dir = Some(dir);
}

#[when("I export the object's JSON sidecar")]
fn export_json_sidecar(world: &mut MetadataWorld) {
    let dir = world.object_dir.clone().expect("object dir");
    let sidecar = dir.with_extension("json");
    assert_eq!(run_json_cli("export", &sidecar, &dir), Ok(0));
    world.sidecar_path = Some(sidecar);
}

#[then("the sidecar should be one UTF-8 JSON document for that object")]
fn sidecar_is_one_document(world: &mut MetadataWorld) {
    let doc = parse_json(&sidecar_text(world)).expect("one JSON object document");
    assert_eq!(doc.object_id, JSON_OBJECT_ID);
}

#[then("it should carry the object identifier and the SHA-256 digest of the file")]
fn sidecar_binds_id_and_digest(world: &mut MetadataWorld) {
    let doc = parse_json(&sidecar_text(world)).expect("sidecar");
    assert_eq!(doc.object_id, JSON_OBJECT_ID);
    assert_eq!(doc.digest_algorithm, "sha256");
    assert_eq!(doc.digest, sha256_hex(JSON_FILE_BYTES));
}

#[then("each statement should have a namespace, term, and value")]
fn sidecar_statements_are_triples(world: &mut MetadataWorld) {
    let doc = parse_json(&sidecar_text(world)).expect("sidecar");
    let title = doc
        .statements
        .iter()
        .find(|s| s.namespace == "dcterms" && s.term == "title")
        .expect("dcterms title statement");
    assert_eq!(title.value, "Harbour");
}

#[when("I import that sidecar into an empty object holding the same file")]
fn import_into_same_file(world: &mut MetadataWorld) {
    import_into_target(world, object_with_file(JSON_FILE_BYTES));
}

#[when("I import that sidecar into an object whose file has different bytes")]
fn import_into_changed_file(world: &mut MetadataWorld) {
    let target = object_with_file(b"edited-pixels");
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "title", "Before")
        .expect("set title");
    local(&target).save(&record).expect("commit metadata.md");
    import_into_target(world, target);
}

#[then("the imported metadata.md should hold the same statements")]
fn imported_statements_match(world: &mut MetadataWorld) {
    assert_eq!(world.json_error, None, "import should succeed");
    let source = local(world.object_dir.as_ref().expect("source"))
        .load()
        .expect("source");
    let target = local(world.target_dir.as_ref().expect("target"))
        .load()
        .expect("target");
    assert_eq!(target.statements, source.statements);
}

#[then("the import should be rejected for a digest mismatch")]
fn import_rejected_for_digest(world: &mut MetadataWorld) {
    let err = world.json_error.as_ref().expect("import rejected");
    assert!(err.contains("digest"), "{err}");
}

#[then("that object's metadata.md should be unchanged")]
fn target_unchanged(world: &mut MetadataWorld) {
    let target = world.target_dir.as_ref().expect("target");
    assert_eq!(metadata_text(target), world.target_before);
    assert_eq!(
        versions_of(target),
        world.versions_before,
        "nothing published"
    );
}

#[when(expr = "I import a JSON sidecar holding {word}")]
fn import_json_holding(world: &mut MetadataWorld, curie: String) {
    let (namespace, term) = curie.split_once(':').expect("prefix:term");
    let digest = sha256_hex(JSON_FILE_BYTES);
    let text = format!(
        "{{\"objectId\":\"{JSON_OBJECT_ID}\",\"digestAlgorithm\":\"sha256\",\
         \"digest\":\"{digest}\",\"statements\":[{{\"namespace\":\"{namespace}\",\
         \"term\":\"{term}\",\"value\":\"x\"}}]}}"
    );
    let target = JsonTarget {
        object_id: JSON_OBJECT_ID,
        digest: &digest,
    };
    let mut record = Record::default();
    world.json_error = apply_json(&text, &target, &mut record)
        .err()
        .map(|err| err.to_string());
}

#[then("the JSON import should be rejected")]
fn json_import_rejected(world: &mut MetadataWorld) {
    assert!(world.json_error.is_some(), "JSON import should be rejected");
}

#[then(expr = "the JSON import should be rejected naming the prefix {word}")]
fn json_import_names_prefix(world: &mut MetadataWorld, prefix: String) {
    let err = world.json_error.as_ref().expect("JSON import rejected");
    assert!(err.contains(&prefix), "{err} should name {prefix}");
}

#[then("metadata.md should remain the canonical store in the object")]
fn metadata_md_stays_canonical(world: &mut MetadataWorld) {
    let dir = world.object_dir.as_ref().expect("object dir");
    let names: Vec<String> = fs::read_dir(dir)
        .expect("object dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        !names.iter().any(|name| name == METADATA_MD),
        "only inside a version"
    );
    let sidecars = names
        .iter()
        .filter(|name| name.ends_with(".json") && *name != "inventory.json");
    assert_eq!(sidecars.count(), 0, "no JSON sidecar in the object");
    assert!(dir.join("v2/content").join(METADATA_MD).is_file());
    let record = local(dir).load().expect("load metadata.md");
    assert_eq!(record.get("dcterms:title").expect("title").value, "Harbour");
}

/// Read a file from the crate root.
fn read_crate_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(path).expect("crate file")
}

/// Concatenate every `src/` module so API scenarios inspect the whole crate.
fn read_crate_sources() -> String {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut text = String::new();
    for entry in fs::read_dir(src).expect("src dir") {
        let entry = entry.expect("src dir entry");
        if entry.path().extension().is_some_and(|ext| ext == "rs") {
            text.push_str(&fs::read_to_string(entry.path()).expect("source"));
        }
    }
    text
}

/// Remove a scratch directory, treating "already absent" as success.
///
/// Panics on any other I/O error so a stale directory cannot leak into a run.
fn remove_dir_if_present(dir: &Path) {
    match fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => panic!("remove {}: {err}", dir.display()),
    }
}

/// A scratch directory path unique to this process and scenario, not yet
/// created (an OCFL object is created there on its first commit).
///
/// Cucumber runs scenarios concurrently, so a fixed path could race.
fn fresh_object_dir() -> PathBuf {
    let seq = OBJECT_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = format!("filesquid-metadata-features-{}-{seq}", std::process::id());
    let dir = std::env::temp_dir().join(name);
    remove_dir_if_present(&dir);
    dir
}

/// Return the `key: value` lines nested under a top-level YAML map `key`.
fn yaml_map_entries<'a>(front_matter: &'a str, key: &str) -> Vec<&'a str> {
    let header = format!("{key}:");
    front_matter
        .lines()
        .skip_while(|line| *line != header)
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .map(str::trim)
        .collect()
}

/// Return the `[dependencies]` table of `Cargo.toml` only.
fn library_dependencies() -> String {
    let manifest = read_crate_file("Cargo.toml");
    let start = manifest.find("[dependencies]").expect("[dependencies]");
    let rest = &manifest[start + "[dependencies]".len()..];
    let end = rest.find("\n[").unwrap_or(rest.len());
    rest[..end].to_string()
}

#[when("I inspect the filesquid-metadata public API")]
fn inspect_api(world: &mut MetadataWorld) {
    world.inspected = read_crate_sources();
}

#[then("dcterms register and statement operations should be available")]
fn api_has_register_and_statements(_world: &mut MetadataWorld) {
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "title", "Harbour")
        .expect("dcterms:title");
    assert_eq!(record.get("dcterms:title").expect("get").value, "Harbour");
    assert!(filesquid_metadata::is_dcterms_property("title"));
    assert_eq!(
        filesquid_metadata::map_dc_to_dcterms("creator"),
        Some("creator")
    );
}

#[then("FileSquid catalog, recipe, C2PA, and collection-snapshot types should be absent")]
fn api_excludes_application_types(world: &mut MetadataWorld) {
    for name in APPLICATION_TYPES {
        assert!(!world.inspected.contains(name), "{name} must be absent");
    }
}

#[when(expr = "I author dcterms:{word} on a file")]
fn author_dcterms(world: &mut MetadataWorld, term: String) {
    let mut record = Record::default();
    world.write_result = Some(record.set(MetadataNamespace::Dcterms, &term, "value"));
}

#[then("the write should be rejected")]
fn write_rejected(world: &mut MetadataWorld) {
    let result = world.write_result.as_ref().expect("write attempted");
    assert!(result.is_err(), "write should be rejected");
}

#[when(expr = "I set dcterms:title {word} on an object")]
fn set_title_on_object(world: &mut MetadataWorld, title: String) {
    let dir = fresh_object_dir();
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "title", title)
        .expect("set title");
    let object = LocalObject::open(&dir, Some(JSON_OBJECT_ID)).expect("new object");
    object.save(&record).expect("commit metadata.md");
    world.object_dir = Some(dir);
}

#[then(expr = "metadata.md YAML front matter should contain dcterms title {word}")]
fn front_matter_has_title(world: &mut MetadataWorld, title: String) {
    let dir = world.object_dir.as_ref().expect("object dir");
    let text = fs::read_to_string(dir.join("v1/content").join(METADATA_MD)).expect("metadata.md");
    let front_matter = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split("\n---").next())
        .expect("YAML front matter");
    let entries = yaml_map_entries(front_matter, "dcterms");
    let expected = format!("title: {title}");
    assert!(
        entries.contains(&expected.as_str()),
        "dcterms title {title} missing"
    );
}

#[then("catalogs should be rebuildable from that file")]
fn rebuild_from_metadata_md(world: &mut MetadataWorld) {
    let dir = world.object_dir.take().expect("object dir");
    let rebuilt = local(&dir).load().expect("load metadata.md");
    let original =
        fs::read_to_string(dir.join("v1/content").join(METADATA_MD)).expect("metadata.md");
    assert_eq!(to_markdown(&rebuilt), original);
    remove_dir_if_present(&dir);
}

#[when("I inspect filesquid-metadata library dependencies")]
fn inspect_dependencies(world: &mut MetadataWorld) {
    world.inspected = library_dependencies();
}

#[then("filesquid-ocfl should be an optional dependency outside the default features")]
fn ocfl_optional(world: &mut MetadataWorld) {
    let line = world
        .inspected
        .lines()
        .find(|line| line.starts_with("filesquid-ocfl"))
        .expect("filesquid-ocfl dependency");
    assert!(line.contains("optional = true"), "{line}");
    let manifest = read_crate_file("Cargo.toml");
    let default = manifest
        .lines()
        .find(|line| line.starts_with("default"))
        .expect("default features");
    assert!(
        !default.contains("ocfl") && !default.contains("cli"),
        "{default}"
    );
}

#[when("I build filesquid-metadata for wasm32-unknown-unknown with default features off")]
fn build_wasm_core(world: &mut MetadataWorld) {
    world.wasm_build = Some(wasm::check_for_wasm(""));
}

#[when("I build filesquid-metadata for wasm32-unknown-unknown with the ocfl feature")]
fn build_wasm_ocfl(world: &mut MetadataWorld) {
    world.wasm_build = Some(wasm::check_for_wasm("ocfl"));
}

#[then("the build should succeed")]
fn build_succeeded(world: &mut MetadataWorld) {
    let outcome = world.wasm_build.take().expect("build ran");
    if let Err(output) = outcome {
        panic!("wasm32 build check failed:\n{output}");
    }
}

#[when("I extract metadata from a logical file in an OCFL object")]
fn extract_from_ocfl(world: &mut MetadataWorld) {
    let rig = rig::Rig::memory();
    rig.seed(rig::OBJECT_ID, &[("photo.tif", TIFF_DESCRIPTION)]);
    let file = FileRef::new(rig::OBJECT_ID, "photo.tif").expect("file ref");
    let bytes = ready(rig.store().read_file(&file, 1024 * 1024)).expect("read through OCFL");
    let mut record = Record::default();
    let found = extract_from_bytes(&bytes, &file, &mut record).expect("extract");
    world.extracted_description = found.exif
        && record
            .get("dcterms:description")
            .is_some_and(|s| s.value.contains("Cat"));
}

#[when(expr = "I extract metadata from bytes holding an XMP packet with dc:title {word}")]
fn extract_xmp_packet(world: &mut MetadataWorld, title: String) {
    let packet = format!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
         <rdf:Description xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\
         <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{title}</rdf:li></rdf:Alt></dc:title>\
         </rdf:Description></rdf:RDF></x:xmpmeta>"
    );
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE1];
    bytes.extend_from_slice(packet.as_bytes());
    let file = FileRef::new("", "photo.jpg").expect("file ref");
    let mut record = Record::default();
    let found = extract_from_bytes(&bytes, &file, &mut record).expect("extract");
    assert!(found.xmp, "XMP packet found");
    world.extracted_title = record.get("dcterms:title").map(|s| s.value.clone());
}

#[then(expr = "the extracted dcterms:title should be {word}")]
fn extracted_title_is(world: &mut MetadataWorld, title: String) {
    assert_eq!(world.extracted_title.as_deref(), Some(title.as_str()));
}

#[given("an OCFL object whose head has no version inventory")]
fn object_without_head_inventory(world: &mut MetadataWorld) {
    let dir = object_with_file(JSON_FILE_BYTES);
    fs::remove_file(dir.join("v1/inventory.json")).expect("remove version inventory");
    world.object_dir = Some(dir);
}

#[when(expr = "I set dcterms:title {word} through the ocfl store")]
fn set_title_through_ocfl_store(world: &mut MetadataWorld, title: String) {
    let dir = world.object_dir.clone().expect("object dir");
    world.versions_before = versions_of(&dir);
    world.earlier_versions = version_files(&dir);
    let object = local(&dir);
    let mut record = object.load().expect("load");
    record
        .set(MetadataNamespace::Dcterms, "title", title)
        .expect("set title");
    world.commit_outcome = Some(object.save(&record));
}

#[then(expr = "the object should have a new version whose metadata.md holds dcterms title {word}")]
fn new_version_holds_title(world: &mut MetadataWorld, title: String) {
    let dir = world.object_dir.clone().expect("object dir");
    let outcome = world.commit_outcome.take().expect("edit made");
    let version = outcome.expect("edit committed").expect("a version");
    let mut expected = world.versions_before.clone();
    expected.push(version.to_string());
    assert_eq!(versions_of(&dir), expected, "exactly one new version");
    let record = local(&dir).load().expect("load");
    assert_eq!(record.get("dcterms:title").expect("title").value, title);
}

#[then("the earlier versions should be unchanged")]
fn earlier_versions_unchanged(world: &mut MetadataWorld) {
    let dir = world.object_dir.clone().expect("object dir");
    let after = version_files(&dir);
    assert!(!world.earlier_versions.is_empty());
    for (path, bytes) in &world.earlier_versions {
        assert_eq!(after.get(path), Some(bytes), "{} changed", path.display());
    }
}

#[then("the object should pass the OCFL validator")]
fn object_validates(world: &mut MetadataWorld) {
    let dir = world.object_dir.clone().expect("object dir");
    let storage = FsBackend::with_default_staging(&dir).expect("backend");
    let report = ready(validate_object(&storage, "")).expect("validate");
    assert!(report.is_valid(), "{:?}", report.errors());
}

#[then("no version should be published")]
fn no_version_published(world: &mut MetadataWorld) {
    let dir = world.object_dir.clone().expect("object dir");
    assert_eq!(versions_of(&dir), world.versions_before);
    match world.commit_outcome.as_ref().expect("edit made") {
        Ok(version) => assert_eq!(version, &None, "an unchanged record publishes nothing"),
        Err(err) => assert!(is_missing_version_inventory(err), "{err:?}"),
    }
}

#[then("the edit should be refused for the missing version inventory")]
fn refused_for_missing_inventory(world: &mut MetadataWorld) {
    let outcome = world.commit_outcome.as_ref().expect("edit made");
    let Err(err) = outcome else {
        panic!("expected a refusal, got {outcome:?}");
    };
    assert!(is_missing_version_inventory(err), "{err:?}");
}

/// Whether `err` is the change model's W010 refusal (OCFL-S-020).
fn is_missing_version_inventory(err: &StoreError) -> bool {
    let StoreError::Backend(source) = err else {
        return false;
    };
    matches!(
        source.downcast_ref::<ChangeError>(),
        Some(ChangeError::Commit(
            CommitError::MissingVersionInventory { .. }
        ))
    )
}

#[then("the missing version inventory should not be repaired")]
fn inventory_not_repaired(world: &mut MetadataWorld) {
    let dir = world.object_dir.clone().expect("object dir");
    assert!(!dir.join("v1/inventory.json").exists());
}

#[then("the extractor should read bytes supplied by filesquid-ocfl, not a filesystem path")]
fn extractor_reads_supplied_bytes(world: &mut MetadataWorld) {
    assert!(
        world.extracted_description,
        "bytes read through OCFL extract"
    );
    let source = read_crate_file("src/extract.rs");
    for forbidden in ["std::fs", "std::path", "read_from_path", "File::open"] {
        assert!(
            !source.contains(forbidden),
            "extract.rs must not use {forbidden}"
        );
    }
}

#[when("I inspect the filesquid-metadata extraction API")]
fn inspect_extraction_api(world: &mut MetadataWorld) {
    world.inspected = read_crate_file("src/extract.rs");
}

#[then("extraction should take bytes or a reader plus a FileRef and no path")]
fn extraction_signature(world: &mut MetadataWorld) {
    assert!(world.inspected.contains("pub fn extract_from_bytes("));
    assert!(world.inspected.contains("pub fn extract_from_reader<"));
    assert!(world.inspected.contains("file: &FileRef"));
    assert!(!world.inspected.contains("&Path"));
    let mut record = Record::default();
    let file = FileRef::new("", "a.tif").expect("file ref");
    let mut reader = std::io::Cursor::new(TIFF_DESCRIPTION.to_vec());
    let found = extract_from_reader(&mut reader, &file, &mut record).expect("extract");
    assert!(found.exif);
}

#[given("a plain file in a directory")]
fn plain_file(world: &mut MetadataWorld) {
    let dir = fresh_object_dir();
    fs::create_dir_all(&dir).expect("plain directory");
    fs::write(dir.join("photo.jpg"), b"pixels").expect("plain file");
    world.object_dir = Some(dir);
}

#[when("I save a record through the sidecar store")]
fn save_through_sidecar(world: &mut MetadataWorld) {
    let dir = world.object_dir.clone().expect("dir");
    let store = SidecarStore::new(&dir);
    let file = FileRef::new("", "photo.jpg").expect("file ref");
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "title", "Harbour")
        .expect("title");
    complete_immediately(store.save(&file, &record))
        .expect("poll")
        .expect("save");
}

#[then("the record should be loadable from the sidecar beside the file")]
fn load_from_sidecar(world: &mut MetadataWorld) {
    let dir = world.object_dir.clone().expect("dir");
    assert!(dir.join("photo.jpg.metadata.md").is_file());
    let store = SidecarStore::new(&dir);
    let file = FileRef::new("", "photo.jpg").expect("file ref");
    let loaded = complete_immediately(store.load(&file))
        .expect("poll")
        .expect("load");
    assert_eq!(loaded.get("dcterms:title").expect("title").value, "Harbour");
}

#[when("I inspect the filesquid-metadata native-only modules")]
fn inspect_native_only(world: &mut MetadataWorld) {
    world.inspected = format!(
        "{}\n{}",
        read_crate_file("src/lib.rs"),
        read_crate_file("src/store/mod.rs")
    );
    world
        .inspected
        .push_str(&read_crate_file("src/store/ocfl.rs"));
}

#[then("the CLI, the sidecar store and the OCFL path helpers should be compiled out for wasm32")]
fn native_only_gated(world: &mut MetadataWorld) {
    let gate = "not(target_arch = \"wasm32\")";
    for module in ["pub mod cli;", "pub mod sidecar;"] {
        let at = world.inspected.find(module).expect(module);
        let before = &world.inspected[..at];
        let attribute = before
            .lines()
            .rev()
            .find(|l| l.starts_with("#[cfg"))
            .expect("cfg");
        assert!(
            attribute.contains(gate),
            "{module} lacks {gate}: {attribute}"
        );
    }
    let helpers = "pub use native::";
    let at = world.inspected.find(helpers).expect("ocfl-fs re-export");
    let attribute = world.inspected[..at]
        .lines()
        .rev()
        .find(|l| l.starts_with("#[cfg"))
        .expect("cfg");
    assert!(
        attribute.contains(gate),
        "{helpers} lacks {gate}: {attribute}"
    );
}

#[then("axum, tauri, libvips, and filesquid-core should be absent")]
fn forbidden_deps_absent(world: &mut MetadataWorld) {
    for name in FORBIDDEN_DEPS {
        assert!(!world.inspected.contains(name), "{name} must be absent");
    }
}

/// Calls this library's command runner directly; `filesquid-cli` tests that
/// `fsquid metadata help` routes here (`single_fsquid_cli_F-050.feature`).
#[when("I ask the metadata command runner for help")]
fn run_help(world: &mut MetadataWorld) {
    let code = filesquid_metadata::cli::run(&["help".into()]).expect("help");
    assert_eq!(code, 0);
    world.inspected = USAGE.to_string();
}

#[then("I should see extract, get, set, export, import, and validate")]
fn help_lists_commands(world: &mut MetadataWorld) {
    for command in ["extract", "get", "set", "export", "import", "validate"] {
        let line = format!("\n  {command} ");
        assert!(world.inspected.contains(&line), "{command} missing");
    }
}

#[when(expr = "I import a CSV with headers {word}")]
fn import_csv(world: &mut MetadataWorld, headers: String) {
    let columns = headers.split(',').count();
    let row = vec!["x"; columns].join(",");
    let mut record = Record::default();
    world.csv_rejected = apply_csv(&format!("{headers}\n{row}\n"), &mut record).is_err();
}

#[then("dcterms:name should be rejected")]
fn csv_name_rejected(world: &mut MetadataWorld) {
    assert!(world.csv_rejected, "dcterms:name column should be rejected");
}

#[when("I inspect filesquid-metadata interchange")]
fn inspect_interchange(world: &mut MetadataWorld) {
    world.inspected = format!("{}{}", read_crate_sources(), library_dependencies());
}

#[then("CSV, JSON sidecar, XMP sidecar, and embedded extract should be available")]
fn interchange_round_trips(_world: &mut MetadataWorld) {
    let mut record = Record::default();
    record
        .set(MetadataNamespace::Dcterms, "creator", "George")
        .expect("creator");
    assert!(to_csv("obj-1", &record).contains("dcterms:creator"));
    let from_sidecar = from_xmp(&to_xmp(&record));
    assert_eq!(
        from_sidecar.get("dcterms:creator").expect("xmp").value,
        "George"
    );
    assert_json_round_trip(&record);
    assert!(read_crate_file("src/extract.rs").contains("pub fn extract_from_reader"));
}

/// Export `record` as a JSON sidecar, import it, and compare the statements.
fn assert_json_round_trip(record: &Record) {
    let digest = sha256_hex(JSON_FILE_BYTES);
    let text = to_json(JSON_OBJECT_ID, &digest, record);
    let target = JsonTarget {
        object_id: JSON_OBJECT_ID,
        digest: &digest,
    };
    let mut imported = Record::default();
    apply_json(&text, &target, &mut imported).expect("json import");
    assert_eq!(imported.statements, record.statements);
}

#[then("MODS and SQL should be absent")]
fn mods_and_sql_absent(world: &mut MetadataWorld) {
    for marker in ["mods", "sqlx", "rusqlite", "SELECT ", "INSERT INTO"] {
        assert!(!world.inspected.contains(marker), "{marker} must be absent");
    }
    let unknown = filesquid_metadata::cli::run(&["mods".into()]);
    assert!(unknown.is_err(), "mods is not a command");
}

#[when("I inspect filesquid-metadata product identity")]
fn inspect_identity(world: &mut MetadataWorld) {
    world.inspected = read_crate_file("Cargo.toml");
}

#[then("it should remain installable without the FileSquid app")]
fn installable_without_app(world: &mut MetadataWorld) {
    assert!(world.inspected.contains("name = \"filesquid-metadata\""));
    let deps = library_dependencies();
    for app_crate in [
        "imagesquid",
        "filesquid-core",
        "filesquid-api",
        "filesquid-server",
        "filesquid-desktop",
    ] {
        assert!(
            !deps.contains(app_crate),
            "{app_crate} must not be a dependency"
        );
    }
}

#[then("it should build and publish without FileSquid application crates")]
fn extractable(world: &mut MetadataWorld) {
    let path_deps: Vec<&str> = world
        .inspected
        .lines()
        .filter(|line| line.contains("path = \"../"))
        .collect();
    assert_eq!(
        path_deps.len(),
        1,
        "only filesquid-ocfl may be a path dependency"
    );
    assert!(path_deps[0].starts_with("filesquid-ocfl"));
}

#[tokio::main]
async fn main() {
    let results_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    remove_dir_if_present(&results_dir);
    fs::create_dir_all(&results_dir).expect("allure-results");
    let features = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/features");
    let junit = fs::File::create(results_dir.join("cucumber-junit.xml")).expect("junit file");
    // cucumber-rs has no runtime skip, so an explicit opt-out removes the
    // @wasm32 scenarios from the run and says so; it never reports a pass.
    let wasm_skip = wasm::opted_out();
    if let Some(reason) = &wasm_skip {
        println!("SKIPPED @wasm32 scenarios: {reason}");
    }
    let writer = MetadataWorld::cucumber()
        .with_writer(
            writer::Basic::stdout()
                .summarized()
                .tee::<MetadataWorld, _>(writer::JUnit::for_tee(junit, 0))
                .normalized(),
        )
        .filter_run(features, move |_, _, scenario| {
            wasm_skip.is_none() || !scenario.tags.iter().any(|tag| tag == "wasm32")
        })
        .await;
    if writer.execution_has_failed() {
        std::process::exit(1);
    }
}
