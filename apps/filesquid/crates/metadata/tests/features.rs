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
use filesquid_metadata::csv::apply_csv_with;
use filesquid_metadata::csv::{apply_csv, to_csv};
use filesquid_metadata::extract::{extract_from_bytes, extract_from_reader};
use filesquid_metadata::fact::{Fact, Reason};
use filesquid_metadata::file_record::{extension_of, FileRecord, RecordError};
use filesquid_metadata::file_ref::FileRef;
use filesquid_metadata::import::attributes::{AttrValue, AttributeCollector, Attributes};
use filesquid_metadata::import::scrub::IdentityClass;
use filesquid_metadata::import::{ImportEntry, ImportError, NameForm};
use filesquid_metadata::json::apply_json_with;
use filesquid_metadata::json::{apply_json, parse_json, sha256_hex, to_json, JsonTarget};
use filesquid_metadata::markdown::{from_markdown, to_markdown};
use filesquid_metadata::namespace::{
    NamespaceRegistry, Registrant, Registration, RegistryError, NAMESPACES_MD,
};
use filesquid_metadata::raw::Raw;
use filesquid_metadata::record::parse_curie_with;
use filesquid_metadata::record::Record;
use filesquid_metadata::store::ocfl::LocalObject;
use filesquid_metadata::store::sidecar::SidecarStore;
use filesquid_metadata::store::{MetadataStore, StoreError, METADATA_MD};
use filesquid_metadata::xmp::{from_xmp, to_xmp};
use filesquid_metadata::xmp::{from_xmp_with, to_xmp_with};
use filesquid_metadata::{MetadataError, MetadataNamespace};
use filesquid_ocfl::change::{ChangeError, ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{complete_immediately, FsBackend, MemoryBackend};
use filesquid_ocfl::validate::validate_object;
use filesquid_ocfl::write::{rfc3339_utc, CommitError, LeaseConfig, LeaseStore, SystemClock};
use filesquid_ocfl::VersionNum;

#[path = "support/import_rig.rs"]
mod import_rig;
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
    /// Versions, message, user and paths of the last commit made with options.
    options_outcome: Option<CommittedWithOptions>,
    /// The OCFL object of the import record scenarios.
    scene: Option<ImportScene>,
    /// The object's `v1` files right after the first import.
    v1_files: BTreeMap<String, Vec<u8>>,
    /// The object's versions before the last save.
    versions_before_save: Vec<String>,
    /// Why the last save of a file record was refused.
    refusal: Option<RecordError>,
    /// Why the last import entry was rejected.
    entry_error: Option<ImportError>,
    /// The import entry a scenario built.
    entry: Option<ImportEntry>,
    /// The attributes a scenario collected.
    attributes: Option<Attributes>,
    /// JSON a scenario produced (a projection or a scrubbed entry).
    produced_json: String,
    /// The namespace registry of the MD-S-012 scenarios.
    registry: NamespaceRegistry,
    /// Why the last registration was refused.
    registry_refusal: Option<RegistryError>,
    /// The record extension statements are authored on.
    ns_record: Record,
    /// The OCFL object that holds a committed registry.
    registry_scene: Option<ImportScene>,
    /// Exit code of the last CLI `validate`.
    cli_code: Option<i32>,
}

/// The in-memory OCFL object of an import record scenario.
struct ImportScene(rig::Rig<MemoryBackend>);

impl std::fmt::Debug for ImportScene {
    /// The rig holds backends without a useful `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImportScene")
    }
}

/// What a commit with options published: the object's versions, and the new
/// version's message, user and logical paths.
type CommittedWithOptions = (
    Vec<String>,
    Option<String>,
    Option<filesquid_ocfl::inventory::User>,
    Vec<String>,
);

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

#[when(
    "I commit a metadata edit with a version user, an operation message and a version-bound record"
)]
fn commit_with_options(world: &mut MetadataWorld) {
    let rig = rig::Rig::memory();
    rig.seed(rig::OBJECT_ID, &[("photo.jpg", b"pixels")]);
    let record = |context: &filesquid_ocfl::write::BoundContext<'_>| {
        let name = format!("provenance/{}.jsonld", context.version);
        Ok(std::collections::BTreeMap::from([(name, b"{}".to_vec())]))
    };
    let options = filesquid_metadata::store::ocfl::CommitOptions {
        user: Some(installation_user()),
        message: Some(OPERATION_MESSAGE.to_string()),
        bound: Some(&record),
    };
    let file = FileRef::new(rig::OBJECT_ID, "photo.jpg").expect("file ref");
    let mut edit = Record::default();
    edit.set(MetadataNamespace::Dcterms, "title", "Dawn")
        .expect("title");
    let receipt = ready(rig.store().commit_record_with(&file, &edit, &options))
        .expect("commit")
        .expect("a version");
    let object = ready(Object::open(
        &rig.storage,
        &rig::object_root(rig::OBJECT_ID),
    ))
    .expect("open");
    let block = &object.inventory().versions[&receipt.version];
    world.options_outcome = Some((
        rig.versions(rig::OBJECT_ID),
        block.message.clone(),
        block.user.clone(),
        block.state.values().flatten().cloned().collect(),
    ));
}

#[then("the edit should publish one version carrying that user, that message and the record")]
fn options_were_committed(world: &mut MetadataWorld) {
    let (versions, message, user, paths) = world.options_outcome.take().expect("committed");
    assert_eq!(versions, ["v1", "v2"]);
    assert_eq!(message.as_deref(), Some(OPERATION_MESSAGE));
    assert_eq!(user, Some(installation_user()));
    assert!(
        paths.contains(&"provenance/v2.jsonld".to_string()),
        "{paths:?}"
    );
    assert!(paths.contains(&METADATA_MD.to_string()));
}

/// The version message FileSquid passes for a metadata edit.
const OPERATION_MESSAGE: &str =
    r#"filesquid.operation.v1 {"operation_id":"op-1","kind":"metadata.edit"}"#;

/// The OCFL version user FileSquid sets: the installation agent.
fn installation_user() -> filesquid_ocfl::inventory::User {
    filesquid_ocfl::inventory::User {
        name: "FileSquid installation".into(),
        address: Some("urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b".into()),
    }
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

// MD-S-008, MD-S-013: the per-file record and its import section.

/// The stored original and its per-file record.
fn record_file() -> FileRef {
    FileRef::new(import_rig::OBJECT, import_rig::STORED)
        .and_then(|file| file.with_record_path(import_rig::RECORD))
        .expect("file ref")
}

/// A file record of an upload named `name`.
fn record_named(name: &[u8]) -> FileRecord {
    let text = String::from_utf8_lossy(name).into_owned();
    FileRecord::new(
        Raw::from_bytes(name),
        NameForm::Nfc,
        extension_of(&text).map(str::to_string),
        Some("JPEG".to_string()),
        Some("image/jpeg".to_string()),
        import_rig::entry_named(name),
    )
    .expect("file record")
}

/// Import `record`: stage the content and commit the record over it.
fn import_into_scene(world: &mut MetadataWorld, record: &FileRecord) {
    let scene = ImportScene(rig::Rig::memory());
    let store = scene.0.store();
    let model = store.change_model(import_rig::OBJECT).expect("model");
    ready(model.stage_bytes(import_rig::STORED, b"pixels")).expect("stage content");
    ready(store.commit_import(&record_file(), record)).expect("import");
    let root = rig::object_root(import_rig::OBJECT);
    world.v1_files = scene.0.snapshot(&format!("{root}/v1/"));
    drop(store);
    world.scene = Some(scene);
}

/// The scene of the running scenario.
fn scene(world: &MetadataWorld) -> &rig::Rig<MemoryBackend> {
    &world.scene.as_ref().expect("an imported object").0
}

/// The head record of the scenario's object.
fn head_record(world: &MetadataWorld) -> FileRecord {
    ready(scene(world).store().read_file_record(&record_file()))
        .expect("read")
        .expect("a record")
}

/// The stored text of the head record, as the object holds it.
fn head_text(world: &MetadataWorld) -> String {
    let rig = scene(world);
    let version = rig.head(import_rig::OBJECT);
    let root = rig::object_root(import_rig::OBJECT);
    let key = format!("{root}/{version}/content/{}", import_rig::RECORD);
    let bytes = rig.snapshot(&format!("{root}/")).remove(&key);
    // The head version may not hold the record itself; the newest one does.
    let bytes = bytes.unwrap_or_else(|| newest_record_bytes(rig));
    String::from_utf8(bytes).expect("record text")
}

/// The record's bytes in the newest version that holds them.
fn newest_record_bytes(rig: &rig::Rig<MemoryBackend>) -> Vec<u8> {
    let root = rig::object_root(import_rig::OBJECT);
    let files = rig.snapshot(&format!("{root}/"));
    let suffix = format!("/content/{}", import_rig::RECORD);
    let mut found: Vec<(&String, &Vec<u8>)> =
        files.iter().filter(|(k, _)| k.ends_with(&suffix)).collect();
    found.sort_by_key(|(k, _)| k.len());
    found
        .last()
        .map(|(_, v)| (*v).clone())
        .expect("a stored record")
}

/// Keep the refusal of a save for the Then steps.
fn keep_refusal(world: &mut MetadataWorld, result: Result<(), StoreError>) {
    world.refusal = match result {
        Err(StoreError::Record(err)) => Some(err),
        Err(other) => panic!("unexpected {other:?}"),
        Ok(()) => None,
    };
}

#[when(expr = "I import a file named {string} into an OCFL object")]
fn import_named(world: &mut MetadataWorld, name: String) {
    import_into_scene(world, &record_named(name.as_bytes()));
}

#[then("the version should hold the stored original and its per-file metadata record")]
fn version_holds_both(world: &mut MetadataWorld) {
    let root = rig::object_root(import_rig::OBJECT);
    assert!(world
        .v1_files
        .contains_key(&format!("{root}/v1/content/{}", import_rig::STORED)));
    assert!(world
        .v1_files
        .contains_key(&format!("{root}/v1/content/{}", import_rig::RECORD)));
}

#[then("the object should hold no metadata.md")]
fn no_plain_metadata_md(world: &mut MetadataWorld) {
    let root = rig::object_root(import_rig::OBJECT);
    let files = scene(world).snapshot(&format!("{root}/"));
    let plain = format!("/content/{METADATA_MD}");
    assert!(!files.keys().any(|key| key.ends_with(&plain)));
}

#[then("the imported object should pass the OCFL validator")]
fn imported_object_is_valid(world: &mut MetadataWorld) {
    let rig = scene(world);
    let root = rig::object_root(import_rig::OBJECT);
    let report = ready(validate_object(&rig.storage, &root)).expect("validate");
    assert!(report.is_valid(), "{:?}", report.errors());
}

#[then("the record should open with the schema identifier and the original name Holiday 2023.JPG")]
fn record_opens_with_schema(world: &mut MetadataWorld) {
    let text = head_text(world);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[1], "schema: filesquid.original.v1");
    assert_eq!(lines[2], r#"original_name: "Holiday 2023.JPG""#);
}

#[then("the display name should be the original name")]
fn display_is_original(world: &mut MetadataWorld) {
    let record = head_record(world);
    assert_eq!(record.display_name(), "Holiday 2023.JPG");
}

#[then("the extension, format and media type should be recorded")]
fn header_facts_recorded(world: &mut MetadataWorld) {
    let record = head_record(world);
    assert_eq!(record.extension(), Some("JPG"));
    assert_eq!(record.format(), Some("JPEG"));
    assert_eq!(record.media_type(), Some("image/jpeg"));
}

/// A second acquisition of the same file.
fn second_entry() -> ImportEntry {
    let mut again = import_rig::entry();
    again.acquisition.job_id = Fact::value("job-2".to_string());
    again
}

#[when("I import the same file again")]
fn import_again(world: &mut MetadataWorld) {
    let options = filesquid_metadata::store::ocfl::CommitOptions::default();
    let store = scene(world).store();
    ready(store.append_import_with(&record_file(), second_entry(), &options))
        .expect("append")
        .expect("a version");
}

#[then("the object should have a new version with two import entries")]
fn two_entries_in_a_new_version(world: &mut MetadataWorld) {
    assert_eq!(scene(world).versions(import_rig::OBJECT), ["v1", "v2"]);
    assert_eq!(head_record(world).imports().len(), 2);
}

#[then("the first version should be unchanged")]
fn v1_unchanged(world: &mut MetadataWorld) {
    let root = rig::object_root(import_rig::OBJECT);
    let now = scene(world).snapshot(&format!("{root}/v1/"));
    assert_eq!(now, world.v1_files);
}

/// Save `record` over the head and keep the refusal, if any.
fn save_whole_record(world: &mut MetadataWorld, record: &FileRecord) {
    world.versions_before_save = scene(world).versions(import_rig::OBJECT);
    let options = filesquid_metadata::store::ocfl::CommitOptions::default();
    let result = ready(scene(world).store().commit_file_record_with(
        &record_file(),
        record,
        &options,
    ));
    keep_refusal(world, result.map(|_| ()));
}

#[when("I save the record with a changed import entry")]
fn save_changed_entry(world: &mut MetadataWorld) {
    let text = head_text(world).replace("studio.local", "other.local");
    save_whole_record(world, &FileRecord::from_markdown(&text).expect("parse"));
}

#[when("I save the record with only the first import entry")]
fn save_first_entry_only(world: &mut MetadataWorld) {
    save_whole_record(world, &record_named(b"Holiday 2023.JPG"));
}

#[when("I save the record with another original name")]
fn save_other_name(world: &mut MetadataWorld) {
    let text = head_text(world).replacen(
        r#"original_name: "Holiday 2023.JPG""#,
        r#"original_name: "Other.JPG""#,
        1,
    );
    save_whole_record(world, &FileRecord::from_markdown(&text).expect("parse"));
}

#[then("the save should be refused for changing an import entry")]
fn refused_changed_entry(world: &mut MetadataWorld) {
    assert_eq!(
        world.refusal.take(),
        Some(RecordError::ImportEntryChanged { index: 0 })
    );
}

#[then("the save should be refused for removing an import entry")]
fn refused_removed_entry(world: &mut MetadataWorld) {
    assert_eq!(world.refusal.take(), Some(RecordError::ImportEntryRemoved));
}

#[then("the save should be refused for changing the original name")]
fn refused_changed_name(world: &mut MetadataWorld) {
    assert_eq!(world.refusal.take(), Some(RecordError::OriginalNameChanged));
}

#[then("the object's versions should be unchanged")]
fn versions_unchanged(world: &mut MetadataWorld) {
    let now = scene(world).versions(import_rig::OBJECT);
    assert_eq!(now, world.versions_before_save);
}

#[when(expr = "I rename the file to {string}")]
fn rename_file(world: &mut MetadataWorld, name: String) {
    let mut record = head_record(world);
    record.set_display_name(&name).expect("display name");
    save_whole_record(world, &record);
    assert_eq!(world.refusal, None);
}

#[then("the display name should be Harbour.jpg and the original name Holiday 2023.JPG")]
fn renamed_display_only(world: &mut MetadataWorld) {
    let record = head_record(world);
    assert_eq!(record.display_name(), "Harbour.jpg");
    assert_eq!(record.original_name().as_bytes(), b"Holiday 2023.JPG");
}

#[then("the import entries should be unchanged")]
fn imports_unchanged(world: &mut MetadataWorld) {
    let record = head_record(world);
    assert_eq!(
        record.imports(),
        record_named(b"Holiday 2023.JPG").imports()
    );
}

#[when("I set dcterms:title Harbour on the file's record")]
fn set_title_on_file_record(world: &mut MetadataWorld) {
    let mut statements = Record::default();
    statements
        .set(MetadataNamespace::Dcterms, "title", "Harbour")
        .expect("title");
    ready(scene(world).store().save(&record_file(), &statements)).expect("save");
}

#[then(
    "the record should hold dcterms title Harbour and still hold its original name and import entry"
)]
fn statements_kept_header(world: &mut MetadataWorld) {
    let record = head_record(world);
    assert_eq!(
        record.statements().get("dcterms:title").unwrap().value,
        "Harbour"
    );
    assert_eq!(record.original_name().as_bytes(), b"Holiday 2023.JPG");
    assert_eq!(record.imports().len(), 1);
}

#[when("I import a file whose reported size differs from the verified size")]
fn import_mismatched_size(world: &mut MetadataWorld) {
    let scene = ImportScene(rig::Rig::memory());
    let store = scene.0.store();
    let model = store.change_model(import_rig::OBJECT).expect("model");
    ready(model.stage_bytes(import_rig::STORED, b"pixels")).expect("stage content");
    let mut entry = import_rig::entry();
    entry.source.reported_size = Fact::value(7);
    let record = FileRecord::new(
        Raw::from_bytes(b"Holiday 2023.JPG"),
        NameForm::Nfc,
        Some("JPG".to_string()),
        None,
        None,
        entry,
    );
    world.entry_error = match record {
        Err(RecordError::Import(error)) => Some(error),
        Err(other) => panic!("unexpected {other:?}"),
        // No record can be built, so no commit is attempted.
        Ok(record) => {
            let _ = ready(store.commit_import(&record_file(), &record));
            None
        }
    };
    drop(store);
    world.scene = Some(scene);
}

#[then("the import should be rejected for the size mismatch")]
fn import_rejected_for_size(world: &mut MetadataWorld) {
    let error = world.entry_error.take();
    assert_eq!(
        error,
        Some(ImportError::SizeMismatch {
            reported: 7,
            verified: 6
        })
    );
}

#[then("nothing should be published")]
fn nothing_published(world: &mut MetadataWorld) {
    assert!(!scene(world).storage_has_object(import_rig::OBJECT));
}

/// A file name that is not UTF-8.
const BINARY_NAME: [u8; 6] = [b'a', 0xFF, b'.', b'j', b'p', b'g'];

#[when("I import a file whose name and extended attribute are not UTF-8")]
fn import_binary(world: &mut MetadataWorld) {
    let mut entry = import_rig::entry_named(&BINARY_NAME);
    let mut collector = AttributeCollector::new();
    collector
        .add(&[0xC3, 0x28], 2, &[0xFF, 0x00])
        .expect("attribute");
    entry.attributes = Fact::value(collector.finish());
    let record = FileRecord::new(
        Raw::from_bytes(&BINARY_NAME),
        NameForm::Unnormalised,
        Some("jpg".to_string()),
        None,
        None,
        entry,
    )
    .expect("record");
    import_into_scene(world, &record);
}

#[then("the record should store them as base64 marked with encoding base64")]
fn stored_as_base64(world: &mut MetadataWorld) {
    let text = head_text(world);
    assert!(
        text.contains(r#"original_name: {"encoding":"base64""#),
        "{text}"
    );
    assert!(
        text.matches(r#""encoding":"base64""#).count() >= 3,
        "{text}"
    );
}

#[then("they should read back byte for byte")]
fn base64_reads_back(world: &mut MetadataWorld) {
    let record = head_record(world);
    assert_eq!(record.original_name().as_bytes(), &BINARY_NAME);
    let Fact::Value(attributes) = &record.imports()[0].attributes else {
        panic!("attributes");
    };
    assert_eq!(attributes.entries[0].name.as_bytes(), &[0xC3, 0x28]);
    let AttrValue::Full { value } = &attributes.entries[0].value else {
        panic!("value");
    };
    assert_eq!(value.as_bytes(), &[0xFF, 0x00]);
}

#[when("I import a file through an upload that provides only a name, size, time and media type")]
fn upload_entry(world: &mut MetadataWorld) {
    world.entry = Some(import_rig::entry());
}

/// Whether `fact` is unavailable because an upload does not carry it.
fn not_uploaded<T>(fact: &Fact<T>) -> bool {
    matches!(fact, Fact::Unavailable(u) if u.unavailable == Reason::NotProvidedByUpload)
}

#[then(
    "every other source, volume and attribute fact should be unavailable with the reason not provided by upload"
)]
fn the_rest_is_unavailable(world: &mut MetadataWorld) {
    let entry = world.entry.take().expect("entry");
    let s = &entry.source;
    let v = &entry.volume;
    let checks = [
        not_uploaded(&s.absolute_path),
        not_uploaded(&s.import_root),
        not_uploaded(&s.relative_path),
        not_uploaded(&s.birth_time),
        not_uploaded(&s.status_changed_time),
        not_uploaded(&s.accessed_time),
        not_uploaded(&s.date_added_time),
        not_uploaded(&s.owner_name),
        not_uploaded(&s.owner_id),
        not_uploaded(&s.group_name),
        not_uploaded(&s.group_id),
        not_uploaded(&s.posix_mode),
        not_uploaded(&s.acl),
        not_uploaded(&s.flags),
        not_uploaded(&s.link),
        not_uploaded(&v.name),
        not_uploaded(&v.uuid),
        not_uploaded(&v.filesystem_type),
        not_uploaded(&v.mount_point),
        not_uploaded(&v.kind),
        not_uploaded(&entry.attributes),
    ];
    assert!(checks.iter().all(|ok| *ok), "{checks:?}");
}

#[when("I inspect 300 extended attributes of 8 KiB each")]
fn inspect_many_attributes(world: &mut MetadataWorld) {
    const VALUE: usize = 8 * 1024;
    let value = vec![9u8; VALUE];
    let mut collector = AttributeCollector::new();
    for index in 0..300 {
        if collector.names_remaining() == 0 {
            break;
        }
        let take = (collector.read_budget() as usize).min(VALUE);
        let name = format!("user.attribute{index}");
        collector
            .add(name.as_bytes(), VALUE as u64, &value[..take])
            .expect("add");
    }
    collector.note_names_enumerated(300);
    world.attributes = Some(collector.finish());
}

#[then("at most 256 names should be kept and the entry marked truncated with 300 names enumerated")]
fn names_capped(world: &mut MetadataWorld) {
    let attrs = world.attributes.as_ref().expect("attributes");
    assert_eq!(attrs.entries.len(), 256);
    assert!(attrs.truncated);
    assert_eq!(attrs.names_enumerated, 300);
    assert_eq!(attrs.validate(), Ok(()));
}

#[then("at most 1 MiB of value bytes should be recorded as read")]
fn bytes_capped(world: &mut MetadataWorld) {
    let attrs = world.attributes.as_ref().expect("attributes");
    assert_eq!(attrs.bytes_inspected, 1024 * 1024);
}

#[then("every value cut by the budget should have an incomplete digest")]
fn cut_values_incomplete(world: &mut MetadataWorld) {
    let attrs = world.attributes.as_ref().expect("attributes");
    let whole = attrs
        .entries
        .iter()
        .filter(|a| matches!(a.value, AttrValue::Full { .. }))
        .count();
    let cut = attrs
        .entries
        .iter()
        .filter(|a| matches!(a.value, AttrValue::Incomplete))
        .count();
    assert_eq!((whole, cut), (128, 128));
}

#[then("a re-import should get its own budget of 256 names and 1 MiB")]
fn re_import_gets_its_own_budget(_world: &mut MetadataWorld) {
    let fresh = AttributeCollector::new();
    assert_eq!(fresh.names_remaining(), 256);
    assert_eq!(fresh.read_budget(), 1024 * 1024);
}

#[when("I inspect an extended attribute of 70000 bytes")]
fn inspect_big_attribute(world: &mut MetadataWorld) {
    let value = vec![1u8; 70_000];
    let mut collector = AttributeCollector::new();
    collector.add(b"user.big", 70_000, &value).expect("add");
    world.attributes = Some(collector.finish());
}

#[then("the record should hold its size and SHA-256 and not the value")]
fn big_value_is_digest_only(world: &mut MetadataWorld) {
    let attrs = world.attributes.as_ref().expect("attributes");
    let entry = &attrs.entries[0];
    assert_eq!(entry.size, 70_000);
    let AttrValue::Digest { sha256 } = &entry.value else {
        panic!("{:?}", entry.value);
    };
    assert_eq!(
        sha256,
        "267ccbdc4e1d24891e18130d2690fc4f796b460f92a7fff15773b694143a965b"
    );
}

#[when("I import a file with a full set of source facts and read its projection")]
fn read_projection(world: &mut MetadataWorld) {
    let mut record = record_named(b"Holiday 2023.JPG");
    record
        .append_import(import_rig::rich_entry())
        .expect("append");
    world.produced_json = serde_json::to_string(&record.facts()).expect("json");
}

#[then("the projection should show the original name, relative path, volume name and OCFL object")]
fn projection_shows_names(world: &mut MetadataWorld) {
    for wanted in ["Holiday 2023.JPG", "Macintosh HD", import_rig::OBJECT] {
        assert!(world.produced_json.contains(wanted), "{wanted}");
    }
}

#[then("it should not show the absolute path, the import root, the host name or any account")]
fn projection_hides_identity(world: &mut MetadataWorld) {
    for secret in [
        "/Users/pieter",
        "studio.local",
        "pieter",
        "staff",
        "/Volumes",
    ] {
        assert!(!world.produced_json.contains(secret), "{secret}");
    }
}

#[when("I scrub every identity class of a full import entry")]
fn scrub_everything(world: &mut MetadataWorld) {
    let scrubbed = import_rig::rich_entry().scrubbed(&IdentityClass::ALL);
    assert_eq!(scrubbed.validate(), Ok(()));
    world.produced_json = serde_json::to_string(&scrubbed).expect("json");
    world.entry = Some(scrubbed);
}

#[then(
    "no path, account, host name, numeric ID, origin URL, quarantine agent or Finder comment should remain"
)]
fn nothing_identifying_remains(world: &mut MetadataWorld) {
    for secret in [
        "/Users",
        "pieter",
        "studio.local",
        "staff",
        "0081;",
        "http",
        "memo",
    ] {
        assert!(!world.produced_json.contains(secret), "{secret}");
    }
    let entry = world.entry.take().expect("scrubbed entry");
    assert!(matches!(entry.source.owner_id, Fact::Unavailable(_)));
    assert!(matches!(entry.source.group_id, Fact::Unavailable(_)));
}

#[then("each removed fact should be listed as withheld by policy")]
fn removed_facts_are_listed(world: &mut MetadataWorld) {
    assert!(world.produced_json.contains("withheld_by_policy"));
    assert!(world.produced_json.contains(r#""state":"withheld""#));
}

#[when("I inspect the import record modules")]
fn inspect_import_modules(world: &mut MetadataWorld) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = [
        "raw.rs",
        "fact.rs",
        "file_record.rs",
        "import/mod.rs",
        "import/attributes.rs",
        "import/scrub.rs",
        "import/time.rs",
    ];
    let mut hits = Vec::new();
    for file in files {
        let text = fs::read_to_string(src.join(file)).expect("module source");
        for needle in [
            "std::fs",
            "std::path",
            "std::env",
            "std::process",
            "std::net",
        ] {
            if text.contains(needle) {
                hits.push(format!("{file}: {needle}"));
            }
        }
    }
    world.inspected = hits.join("\n");
}

#[then("they should use no filesystem, path or environment access")]
fn no_filesystem_access(world: &mut MetadataWorld) {
    assert_eq!(world.inspected, "", "the import record reads no filesystem");
}

/// The RiC-O IRI the registry scenarios use.
const RICO_IRI: &str = "https://www.ica.org/standards/RiC/ontology#";

/// The object that keeps the registry in the persistence scenario.
const REGISTRY_OBJECT: &str = "urn:example:repository";

/// The `rico` extension with a term list, registered by a distribution.
fn rico_registration() -> Registration {
    let terms = ["title", "hasCreator", "date"].map(str::to_string).to_vec();
    Registration::extension(
        "rico",
        RICO_IRI,
        Some(terms),
        Registrant::Distribution("arkrim".to_string()),
    )
}

/// The registry record of [`REGISTRY_OBJECT`].
fn registry_file() -> FileRef {
    FileRef::new(REGISTRY_OBJECT, NAMESPACES_MD)
        .unwrap()
        .with_record_path(NAMESPACES_MD)
        .unwrap()
}

#[given("the namespace rico is registered with a term list")]
fn rico_registered(world: &mut MetadataWorld) {
    world.registry.register(rico_registration()).expect("rico");
}

#[given("the owner declares the custom namespace mine")]
fn mine_declared(world: &mut MetadataWorld) {
    let mine = Registration::custom("mine", "urn:uuid:0b5c4d1e-8a6f-4b21-9a0e-1f2d3c4b5a69");
    world.registry.register(mine).expect("mine");
}

#[when(expr = "I author {word} {string} on a file")]
fn author_extension(world: &mut MetadataWorld, curie: String, value: String) {
    let registry = world.registry.clone();
    world.write_result = Some(
        parse_curie_with(&curie, &registry).and_then(|(namespace, term)| {
            world
                .ns_record
                .set_registered(&registry, namespace, term, value.as_str())
        }),
    );
}

#[then(expr = "metadata.md should hold {word} {word} {string} in its own map")]
fn holds_extension_map(world: &mut MetadataWorld, prefix: String, term: String, value: String) {
    assert_eq!(world.write_result, Some(Ok(())));
    let text = to_markdown(&world.ns_record);
    assert!(
        text.contains(&format!("{prefix}:\n  {term}: {value}\n")),
        "{text}"
    );
}

#[then("no dcterms statement should be written")]
fn no_dcterms_statement(world: &mut MetadataWorld) {
    let text = to_markdown(&world.ns_record);
    assert!(!text.contains("dcterms:"), "{text}");
}

#[when(expr = "I register the namespace {word}")]
fn register_prefix(world: &mut MetadataWorld, prefix: String) {
    let registration = Registration::extension(
        &prefix,
        "https://example.org/other#",
        None,
        Registrant::Owner,
    );
    world.registry_refusal = world.registry.register(registration).err();
}

#[when("I register rico again with another IRI")]
fn register_rico_again(world: &mut MetadataWorld) {
    let other =
        Registration::extension("rico", "https://example.org/rico#", None, Registrant::Owner);
    world.registry_refusal = world.registry.register(other).err();
}

#[then(expr = "the registration should be refused naming {word}")]
fn registration_refused(world: &mut MetadataWorld, prefix: String) {
    let refusal = world.registry_refusal.as_ref().expect("refused");
    assert!(refusal.to_string().contains(&prefix), "{refusal}");
    let registered = world.registry.get(&prefix);
    assert!(registered.is_none_or(|r| r.iri() == RICO_IRI));
}

#[then("rico should keep its first IRI")]
fn rico_keeps_iri(world: &mut MetadataWorld) {
    assert_eq!(world.registry.get("rico").unwrap().iri(), RICO_IRI);
}

#[then(expr = "the write should be rejected naming {word}")]
fn write_rejected_naming(world: &mut MetadataWorld, name: String) {
    let outcome = world.write_result.as_ref().expect("a write");
    let err = outcome.as_ref().expect_err("rejected");
    assert!(err.to_string().contains(&name), "{err}");
    assert!(world.ns_record.statements.is_empty());
}

#[then("the statement should round-trip unchanged through metadata.md, CSV, JSON and XMP")]
fn extension_round_trips(world: &mut MetadataWorld) {
    let record = &world.ns_record;
    let registry = &world.registry;
    assert_eq!(&from_markdown(&to_markdown(record)), record);
    let mut csv = Record::default();
    apply_csv_with(&to_csv("urn:x:1", record), registry, &mut csv).unwrap();
    assert_eq!(&csv, record);
    let digest = sha256_hex(b"bytes");
    let target = JsonTarget {
        object_id: "urn:x:1",
        digest: &digest,
    };
    let mut json = Record::default();
    apply_json_with(
        &to_json("urn:x:1", &digest, record),
        &target,
        registry,
        &mut json,
    )
    .unwrap();
    assert_eq!(&json, record);
    let xmp = from_xmp_with(&to_xmp_with(record, registry).unwrap(), registry);
    assert_eq!(&xmp, record);
}

#[when("I commit the registry to an OCFL object")]
fn commit_registry(world: &mut MetadataWorld) {
    let scene = rig::Rig::memory();
    let options = filesquid_metadata::store::ocfl::CommitOptions::default();
    let store = scene.store();
    let committed = complete_immediately(store.commit_registry_with(
        &registry_file(),
        &world.registry,
        &options,
    ))
    .expect("ready");
    assert!(matches!(committed, Ok(Some(_))), "{committed:?}");
    world.registry_scene = Some(ImportScene(scene));
}

#[then("a store opened afresh over that object should recover the registration")]
fn registry_recovered(world: &mut MetadataWorld) {
    let scene = &world.registry_scene.as_ref().expect("committed").0;
    let fresh = scene.store();
    let read = complete_immediately(fresh.read_registry(&registry_file())).expect("ready");
    assert_eq!(read.expect("read"), world.registry);
}

#[then("the registry object should pass the OCFL validator")]
fn registry_object_valid(world: &mut MetadataWorld) {
    let scene = &world.registry_scene.as_ref().expect("committed").0;
    let root = rig::object_root(REGISTRY_OBJECT);
    let report = complete_immediately(validate_object(&scene.storage, &root))
        .expect("ready")
        .expect("validate");
    assert!(report.is_valid(), "{:?}", report.errors());
}

#[given("an OCFL object whose registry holds rico")]
fn object_with_rico_registry(world: &mut MetadataWorld) {
    let dir = fresh_object_dir();
    let object = LocalObject::open(&dir, Some("urn:x:registry")).expect("open");
    let mut registry = NamespaceRegistry::default();
    registry.register(rico_registration()).expect("rico");
    object.save_registry(&registry).expect("save registry");
    world.object_dir = Some(dir);
}

#[when(expr = "I validate {word} with the CLI")]
fn validate_with_cli(world: &mut MetadataWorld, curie: String) {
    let dir = world.object_dir.as_ref().expect("object dir");
    let args = [
        "validate".to_string(),
        curie,
        "--object".to_string(),
        dir.to_string_lossy().into_owned(),
    ];
    world.cli_code = Some(filesquid_metadata::cli::run(&args).expect("validate runs"));
}

#[then("validate should fail")]
fn validate_failed(world: &mut MetadataWorld) {
    assert_eq!(world.cli_code, Some(1));
}

#[then("validate should succeed")]
fn validate_succeeded(world: &mut MetadataWorld) {
    assert_eq!(world.cli_code, Some(0));
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
