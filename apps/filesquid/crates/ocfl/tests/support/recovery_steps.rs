//! Step definitions for `independent_recovery_OCFL-S-022.feature` (IO-1.1).

use cucumber::{given, then, when};
use filesquid_ocfl::read::{Object, StorageRoot, Versions};
use filesquid_ocfl::storage::{FsBackend, MAX_CHUNK_BYTES};
use filesquid_ocfl::validate::{validate_object, ValidationReport};
use filesquid_ocfl::write::CommitError;
use filesquid_ocfl::VersionNum;

use crate::enumerate::{check_content, enumerate_all, file_rows, ready};
use crate::recovery::{
    at, backend_over, completed_snapshot, copy_of, flip_first_byte, folders, recover_all,
    replace_text, rewrite_sidecar, scratch, sidecar_pairs, snapshot, Scratch, Tree, JPEG_1,
};
use crate::rig::{sha256, Rig};
use crate::OcflWorld;

impl std::fmt::Debug for RecoveryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryState")
    }
}

/// The objects the snapshot holds.
const OBJECTS: [&str; 2] = ["obj", "zz/obj2"];

/// What the recovery scenarios carry between steps.
#[derive(Default)]
pub struct RecoveryState {
    dir: Option<Scratch>,
    source_before: Tree,
    copy_before: Tree,
    reports: Vec<(String, ValidationReport)>,
    copy_rows: Vec<String>,
}

/// The scenario's scratch directory.
fn dir(world: &OcflWorld) -> std::path::PathBuf {
    world
        .recovery
        .dir
        .as_ref()
        .expect("Given a snapshot")
        .0
        .clone()
}

/// The copy's directory.
fn copy(world: &OcflWorld) -> std::path::PathBuf {
    dir(world).join("copy")
}

/// The backend over the copy.
fn copy_backend(world: &OcflWorld) -> FsBackend {
    backend_over(&copy(world), &dir(world))
}

#[given("a completed snapshot written by the engine")]
fn snapshot_written(world: &mut OcflWorld) {
    let scratch = scratch("feature");
    let _rig = completed_snapshot(&scratch.0);
    world.recovery.source_before = snapshot(&scratch.0.join("source"));
    world.recovery.dir = Some(scratch);
}

#[given("I copy the storage root with ordinary filesystem tools")]
fn copy_storage_root(world: &mut OcflWorld) {
    let copy = copy_of(&dir(world));
    world.recovery.copy_before = snapshot(&copy);
}

#[given(expr = "a stored byte of {string} in the copy is changed")]
fn change_byte(world: &mut OcflWorld, name: String) {
    flip_first_byte(&at(
        &copy(world),
        &format!("obj/v1/content/albums/holiday/{name}"),
    ));
}

#[given("the root inventory in the copy is edited")]
fn edit_inventory(world: &mut OcflWorld) {
    replace_text(
        &at(&copy(world), "obj/inventory.json"),
        "docs/notes.md",
        "docs/notez.md",
    );
}

#[given("the root inventory and its sidecar in the copy are forged together")]
fn forge_inventory(world: &mut OcflWorld) {
    let inventory = at(&copy(world), "obj/inventory.json");
    replace_text(
        &inventory,
        &sha256(crate::recovery::JPEG_2),
        &sha256(b"forged"),
    );
    rewrite_sidecar(&inventory, "sha256");
}

#[given("the version 1 inventory in the copy is edited")]
fn edit_v1_inventory(world: &mut OcflWorld) {
    replace_text(
        &at(&copy(world), "obj/v1/inventory.json"),
        "docs/notes.md",
        "docs/notez.md",
    );
}

#[given("the head version inventory is missing from the copy")]
fn remove_head_inventory(world: &mut OcflWorld) {
    for name in ["inventory.json", "inventory.json.sha256"] {
        std::fs::remove_file(at(&copy(world), &format!("obj/v2/{name}"))).expect("remove");
    }
    world.recovery.copy_before = snapshot(&copy(world));
}

#[when("the engine opens, validates and enumerates the copy")]
fn engine_reads_copy(world: &mut OcflWorld) {
    let backend = copy_backend(world);
    let root = ready(StorageRoot::open(&backend)).expect("open the storage root");
    assert_eq!(root.objects().len(), 2);
    for object in OBJECTS {
        let report = ready(validate_object(&backend, object)).expect("validation ran");
        world.recovery.reports.push((object.to_string(), report));
    }
    let items = enumerate_all(&backend, Versions::All);
    world.recovery.copy_rows = file_rows(&items);
}

#[when("an independent reader recovers the copy with only a JSON parser and SHA-2")]
fn independent_reader(world: &mut OcflWorld) {
    let files = recover_all(&copy(world)).expect("every digest matches");
    world.recovery.copy_rows = files
        .iter()
        .map(|f| format!("{} {} {}", f.object_root, f.version, f.path))
        .collect();
}

#[then("every object of the copy should validate with zero errors")]
fn all_valid(world: &mut OcflWorld) {
    for (object, report) in &world.recovery.reports {
        assert!(
            report.errors().is_empty(),
            "{object}: {:?}",
            report.errors()
        );
    }
}

#[then("the copy should enumerate the same files as the source")]
fn same_files(world: &mut OcflWorld) {
    let source = FsBackend::new(dir(world).join("source"), dir(world).join("source-staging"))
        .expect("source");
    let rows = file_rows(&enumerate_all(&source, Versions::All));
    assert_eq!(world.recovery.copy_rows, rows);
    assert_eq!(rows.len(), 9);
    let copied = enumerate_all(&copy_backend(world), Versions::All);
    assert_eq!(check_content(&copy_backend(world), &copied), 9);
}

#[then("the copy should hold only OCFL files")]
fn only_ocfl(world: &mut OcflWorld) {
    for name in world.recovery.copy_before.keys() {
        let ocfl = name == "0=ocfl_1.1"
            || name.contains("/0=ocfl_object_1.1")
            || name.contains("inventory.json")
            || name.contains("/content/");
        assert!(ocfl && !name.contains(".git"), "{name} is not an OCFL file");
    }
}

#[then("every logical file of every version should be recovered with its original bytes")]
fn files_recovered(world: &mut OcflWorld) {
    let files = recover_all(&copy(world)).expect("recover");
    assert_eq!(files.len(), 9);
    let photo = files
        .iter()
        .find(|f| f.version == "v2" && f.path.ends_with("IMG_0001.jpg"))
        .expect("photo");
    assert_eq!(photo.bytes, JPEG_1);
}

#[then(expr = "the folders and the sidecar pairs of {string} should be rebuilt")]
fn folders_rebuilt(world: &mut OcflWorld, version: String) {
    let files = recover_all(&copy(world)).expect("recover");
    let tree = folders(&files, "obj", &version);
    assert_eq!(
        tree[""].iter().cloned().collect::<Vec<_>>(),
        ["albums/", "docs/"]
    );
    assert_eq!(sidecar_pairs(&files, "obj", &version).len(), 2);
}

#[then("the copy should be byte-identical to before")]
fn copy_unchanged(world: &mut OcflWorld) {
    assert_eq!(snapshot(&copy(world)), world.recovery.copy_before);
}

#[then("the source should be byte-identical to before")]
fn source_unchanged(world: &mut OcflWorld) {
    assert_eq!(
        snapshot(&dir(world).join("source")),
        world.recovery.source_before
    );
}

#[then(expr = "validation should report {string}")]
fn reports_code(world: &mut OcflWorld, code: String) {
    let (_, report) = &world.recovery.reports[0];
    assert!(
        report.error_codes().contains(code.as_str()),
        "{:?}",
        report.errors()
    );
}

#[then(expr = "validation should warn {string} with no errors")]
fn warns_code(world: &mut OcflWorld, code: String) {
    let (_, report) = &world.recovery.reports[0];
    assert!(
        report.warning_codes().contains(code.as_str()),
        "{:?}",
        report.warnings()
    );
    assert!(report.errors().is_empty(), "{:?}", report.errors());
}

#[then(expr = "streaming {string} of {string} should stop with {string}")]
fn stream_stops(world: &mut OcflWorld, name: String, version: String, code: String) {
    let backend = copy_backend(world);
    let object = ready(Object::open(&backend, "obj")).expect("open");
    let version = VersionNum::parse(&version).expect("version");
    let path = format!("albums/holiday/{name}");
    let mut stream = object
        .stream_file(&version, &path, MAX_CHUNK_BYTES)
        .expect("stream");
    for _ in 0..4 {
        match ready(stream.next_chunk()) {
            Ok(Some(_)) => continue,
            Ok(None) => panic!("a changed original streamed as verified"),
            Err(err) => return assert_eq!(err.code(), Some(code.as_str()), "{err}"),
        }
    }
    panic!("the stream neither ended nor failed");
}

#[then("the independent reader should refuse the copy as a digest mismatch")]
fn reader_refuses(world: &mut OcflWorld) {
    let err = recover_all(&copy(world)).expect_err("the copy must be refused");
    assert!(err.contains("digest mismatch"), "{err}");
}

#[then(expr = "opening the object should be refused with {string}")]
fn open_refused(world: &mut OcflWorld, code: String) {
    let backend = copy_backend(world);
    let Err(err) = ready(Object::open(&backend, "obj")) else {
        panic!("open must be refused");
    };
    assert_eq!(err.code(), Some(code.as_str()), "{err}");
}

#[then("a commit on the copy should be refused for the missing version inventory")]
fn commit_refused(world: &mut OcflWorld) {
    let rig = Rig::new(copy_backend(world));
    let draft = rig.draft("v3", &[("docs/notes.md", b"third note")]);
    let outcome = rig.commit("writer-b", Some("v2"), &draft);
    assert!(
        matches!(outcome, Err(CommitError::MissingVersionInventory { .. })),
        "{outcome:?}"
    );
}
