//! UI view API (IO-1.2; OCFL-S-012, OCFL-S-016, OCFL-S-018, OCFL-S-020;
//! PRD F-055; C-055; ADR 028, ADR 030).
//!
//! Reads go through `Object` at a pinned version; every version-producing
//! write goes through the IO-0.10 change model. Scenarios that need no real
//! directory run on the in-memory backend and on `FsBackend`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::change::ChangeError;
use filesquid_ocfl::storage::{FsBackend, MemoryBackend, StorageBackend};
use filesquid_ocfl::view::{self, Entry, FileChange, ViewError, MAX_PAGE_ENTRIES, MAX_RANGE_BYTES};
use filesquid_ocfl::write::CommitError;

#[path = "../support/object_builder.rs"]
mod builder;
#[path = "../support/change_rig.rs"]
mod change;
#[path = "../support/community_validator.rs"]
mod community;
#[path = "../support/fault_backend.rs"]
mod fault;
#[path = "../support/write_rig.rs"]
mod rig;

use builder::{build, write_inventory, ObjectSpec};
use filesquid_ocfl::read::Object;

use change::{commit_files, meta, model, staged_keys, state_of};
use rig::{key, ready, sha256, v, Rig, ROOT};

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
        .join(format!("ocfl-view-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    (backend, Scratch(dir))
}

/// Run `$scenario` on a memory backend and on a filesystem backend.
macro_rules! on_both {
    ($scenario:ident) => {{
        $scenario(&Rig::new(MemoryBackend::new()));
        let (backend, _scratch) = fs_backend();
        $scenario(&Rig::new(backend));
    }};
}

/// v1: `readme.md`, `docs/a.txt`, `docs/b.txt`, `docs/deep/c.txt`.
/// v2: `docs/a.txt` changed and `photo.JPG` added.
fn two_versions<B: StorageBackend>(rig: &Rig<B>) {
    let m = model(rig, "writer-a");
    let v1: [(&str, &[u8]); 4] = [
        ("readme.md", b"read me"),
        ("docs/a.txt", b"alpha"),
        ("docs/b.txt", b"bravo"),
        ("docs/deep/c.txt", b"charlie"),
    ];
    commit_files(&m, rig, &v1, "first").expect("v1");
    rig.clock.advance(1_000);
    let v2: [(&str, &[u8]); 2] = [("docs/a.txt", b"alpha two"), ("photo.JPG", b"\xff\xd8jpeg")];
    commit_files(&m, rig, &v2, "second").expect("v2");
}

/// The names of `entries`, folders marked with a trailing `/`.
fn names(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| match entry {
            Entry::Folder(folder) => format!("{}/", folder.name),
            Entry::File(file) => file.name.clone(),
        })
        .collect()
}

fn listing_shows_implied_folders_then_files<B: StorageBackend>(rig: &Rig<B>) {
    two_versions(rig);
    let object = rig.open().expect("open");
    let root = ready(view::list_folder(&object, &v("v2"), "", None, 100)).expect("root");
    assert_eq!(names(&root.entries), ["docs/", "photo.JPG", "readme.md"]);
    assert_eq!(root.next, None);
    let docs = ready(view::list_folder(&object, &v("v2"), "docs", None, 100)).expect("docs");
    assert_eq!(names(&docs.entries), ["deep/", "a.txt", "b.txt"]);
    let Entry::File(a) = &docs.entries[1] else {
        panic!("a.txt is a file");
    };
    assert_eq!(a.path, "docs/a.txt");
    assert_eq!(a.size, 9);
    assert_eq!(a.digest, sha256(b"alpha two"));
    assert_eq!(a.modified_version, v("v2"));
    assert_eq!(a.type_hint.as_deref(), Some("txt"));
    let Entry::File(b) = &docs.entries[2] else {
        panic!("b.txt is a file");
    };
    assert_eq!(b.modified_version, v("v1"), "unchanged since v1");
    let pinned = ready(view::list_folder(&object, &v("v1"), "", None, 100)).expect("v1");
    assert_eq!(names(&pinned.entries), ["docs/", "readme.md"]);
}

#[test]
fn a_folder_lists_implied_folders_and_files_at_a_pinned_version() {
    on_both!(listing_shows_implied_folders_then_files);
}

#[test]
fn listing_pages_with_an_opaque_cursor() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let object = rig.open().expect("open");
    let first = ready(view::list_folder(&object, &v("v2"), "", None, 2)).expect("page 1");
    assert_eq!(names(&first.entries), ["docs/", "photo.JPG"]);
    let cursor = first.next.clone().expect("a next cursor");
    let second = ready(view::list_folder(&object, &v("v2"), "", Some(&cursor), 2)).expect("page 2");
    assert_eq!(names(&second.entries), ["readme.md"]);
    assert_eq!(second.next, None);
    let err = ready(view::list_folder(&object, &v("v2"), "", Some("zz"), 2)).unwrap_err();
    assert!(matches!(err, ViewError::InvalidCursor { .. }), "{err:?}");
    let err = ready(view::list_folder(
        &object,
        &v("v2"),
        "",
        None,
        MAX_PAGE_ENTRIES + 1,
    ));
    assert!(
        matches!(err, Err(ViewError::InvalidRequest { .. })),
        "{err:?}"
    );
}

#[test]
fn listing_a_missing_folder_or_a_file_is_refused() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let object = rig.open().expect("open");
    let err = ready(view::list_folder(&object, &v("v2"), "nope", None, 10)).unwrap_err();
    assert!(matches!(err, ViewError::FolderNotFound { .. }), "{err:?}");
    let err = ready(view::list_folder(&object, &v("v2"), "readme.md", None, 10)).unwrap_err();
    assert!(matches!(err, ViewError::FolderNotFound { .. }), "{err:?}");
}

#[test]
fn file_details_carry_storage_facts_only() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let object = rig.open().expect("open");
    let details = ready(view::file_details(&object, &v("v2"), "photo.JPG")).expect("details");
    assert_eq!(details.object_id, "urn:example:obj");
    assert_eq!(details.size, 6);
    assert_eq!(details.digest_algorithm, "sha256");
    assert_eq!(details.content_path, "v2/content/photo.JPG");
    assert_eq!(details.type_hint.as_deref(), Some("jpg"));
    assert_eq!(details.last_change.version, v("v2"));
    assert_eq!(details.last_change.message.as_deref(), Some("second"));
    let json = details.to_json();
    let keys: Vec<&str> = json
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "contentPath",
            "digest",
            "digestAlgorithm",
            "fixity",
            "lastChange",
            "name",
            "objectId",
            "path",
            "size",
            "typeHint",
            "version"
        ]
    );
    let err = ready(view::file_details(&object, &v("v1"), "photo.JPG")).unwrap_err();
    assert!(matches!(err, ViewError::Read(_)), "{err:?}");
}

#[test]
fn a_file_history_lists_the_versions_that_changed_it() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let m = model(&rig, "writer-a");
    ready(view::remove_path(&m, "docs/a.txt", meta(&rig, "drop a"))).expect("v3");
    ready(view::restore(
        &m,
        &v("v1"),
        &["docs/a.txt"],
        meta(&rig, "back"),
    ))
    .expect("v4");
    let object = rig.open().expect("open");
    let history = view::file_history(&object, "docs/a.txt").expect("history");
    let kinds: Vec<(String, FileChange)> = history
        .iter()
        .map(|entry| (entry.version.version.to_string(), entry.change))
        .collect();
    assert_eq!(
        kinds,
        [
            ("v1".to_string(), FileChange::Added),
            ("v2".to_string(), FileChange::Changed),
            ("v3".to_string(), FileChange::Deleted),
            ("v4".to_string(), FileChange::Added),
        ]
    );
    assert_eq!(
        history[3].digest.as_deref(),
        Some(sha256(b"alpha").as_str())
    );
    let versions = view::object_versions(&object);
    assert_eq!(versions.len(), 4);
    assert_eq!(
        view::versions_json(&versions).as_array().map(Vec::len),
        Some(4)
    );
}

#[test]
fn ranged_reads_return_the_requested_bytes_within_the_bound() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let object = rig.open().expect("open");
    let bytes = ready(view::read_range(&object, &v("v2"), "docs/a.txt", 2, 5)).expect("range");
    assert_eq!(bytes, b"pha t");
    let tail = ready(view::read_range(&object, &v("v2"), "docs/a.txt", 6, 100)).expect("tail");
    assert_eq!(tail, b"two");
    let err = ready(view::read_range(
        &object,
        &v("v2"),
        "docs/a.txt",
        0,
        MAX_RANGE_BYTES + 1,
    ));
    assert!(
        matches!(err, Err(ViewError::InvalidRequest { .. })),
        "{err:?}"
    );
    let err = ready(view::read_range(&object, &v("v2"), "docs/a.txt", 0, 0));
    assert!(
        matches!(err, Err(ViewError::InvalidRequest { .. })),
        "{err:?}"
    );
}

#[test]
fn a_fixity_check_reports_every_digest_it_verifies() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let object = rig.open().expect("open");
    let report = ready(view::check_fixity(&object, &v("v2"), "readme.md")).expect("fixity");
    assert!(report.ok, "{report:?}");
    assert_eq!(report.checks[0].algorithm, "sha256");
    ready(
        rig.storage
            .put(&key("obj/v1/content/readme.md"), b"tampered"),
    )
    .expect("tamper");
    let object = rig.open().expect("open");
    let report = ready(view::check_fixity(&object, &v("v2"), "readme.md")).expect("fixity");
    assert!(!report.ok);
    assert!(!report.checks[0].ok);
}

fn writes_commit_one_version_each<B: StorageBackend>(rig: &Rig<B>) {
    two_versions(rig);
    let m = model(rig, "writer-a");
    let added = ready(view::add_file(
        &m,
        "new/inner.txt",
        b"new",
        meta(rig, "add"),
    ));
    assert_eq!(added.expect("add").version, v("v3"));
    let moved = ready(view::move_path(
        &m,
        "docs",
        "archive/docs",
        meta(rig, "move folder"),
    ));
    assert_eq!(moved.expect("move").written, Vec::<String>::new());
    let renamed = ready(view::move_path(
        &m,
        "readme.md",
        "README.md",
        meta(rig, "rename"),
    ));
    assert_eq!(renamed.expect("rename").version, v("v5"));
    let removed = ready(view::remove_path(
        &m,
        "archive/docs/deep",
        meta(rig, "rm folder"),
    ));
    assert_eq!(removed.expect("remove").version, v("v6"));
    let state = state_of(rig, "v6");
    let paths: Vec<&str> = state.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        [
            "README.md",
            "archive/docs/a.txt",
            "archive/docs/b.txt",
            "new/inner.txt",
            "photo.JPG"
        ]
    );
    assert!(
        staged_keys(rig, "changes/writer-a").is_empty(),
        "staging left clean"
    );
    rig.assert_valid();
}

#[test]
fn add_move_and_remove_commit_one_version_each_through_the_change_model() {
    on_both!(writes_commit_one_version_each);
}

#[test]
fn an_empty_folder_is_persisted_only_by_a_file_beneath_it() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let object = rig.open().expect("open");
    let err = ready(view::list_folder(&object, &v("v2"), "pending", None, 10)).unwrap_err();
    assert!(
        matches!(err, ViewError::FolderNotFound { .. }),
        "not persisted"
    );
    let m = model(&rig, "writer-a");
    ready(view::add_file(
        &m,
        "pending/first.txt",
        b"1",
        meta(&rig, "add"),
    ))
    .expect("v3");
    let object = rig.open().expect("open");
    let page = ready(view::list_folder(&object, &v("v3"), "", None, 10)).expect("root");
    assert!(names(&page.entries).contains(&"pending/".to_string()));
}

#[test]
fn a_write_refused_by_the_change_model_leaves_staging_clean() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    for raw in ["obj/v2/inventory.json", "obj/v2/inventory.json.sha256"] {
        ready(rig.storage.delete(&key(raw))).expect("remove the head version inventory");
    }
    let m = model(&rig, "writer-a");
    let err = ready(view::add_file(&m, "x.txt", b"x", meta(&rig, "x"))).unwrap_err();
    assert!(
        matches!(
            err,
            ViewError::Change(ChangeError::Commit(
                CommitError::MissingVersionInventory { .. }
            ))
        ),
        "{err:?}"
    );
    assert!(!rig.stored("obj/v2/inventory.json"), "not repaired");
    assert!(staged_keys(&rig, "changes/writer-a").is_empty());
}

#[test]
fn a_write_is_refused_while_other_changes_are_staged() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("other.txt", b"o")).expect("stage");
    let err = ready(view::remove_path(&m, "readme.md", meta(&rig, "rm"))).unwrap_err();
    assert!(matches!(err, ViewError::StagingNotEmpty), "{err:?}");
    assert_eq!(rig.open().expect("open").head(), &v("v2"));
    let err = ready(view::move_path(
        &model(&rig, "b"),
        "nope",
        "x",
        meta(&rig, "mv"),
    ))
    .unwrap_err();
    assert!(matches!(err, ViewError::PathNotFound { .. }), "{err:?}");
}

#[test]
fn view_results_serialize_to_json() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let object = rig.open().expect("open");
    let page = ready(view::list_folder(&object, &v("v2"), "docs", None, 2)).expect("page");
    let json = page.to_json();
    assert_eq!(json["objectId"], "urn:example:obj");
    assert_eq!(json["version"], "v2");
    assert_eq!(json["entries"][0]["kind"], "folder");
    assert_eq!(json["entries"][1]["kind"], "file");
    assert_eq!(json["entries"][1]["size"], 9);
    assert!(json["next"].is_string());
    let history = view::file_history(&object, "docs/a.txt").expect("history");
    assert_eq!(view::history_json(&history)[1]["change"], "changed");
    let report = ready(view::check_fixity(&object, &v("v2"), "readme.md")).expect("fixity");
    assert_eq!(report.to_json()["ok"], true);
}

#[test]
fn objects_written_through_the_view_api_pass_the_community_validator() {
    let (backend, _scratch) = fs_backend();
    let rig = Rig::new(backend);
    writes_commit_one_version_each(&rig);
    let object_root = rig.storage.inner().root().join(ROOT);
    for validator in community::validators_or_skip() {
        let verdict = validator.run_validation(&object_root);
        assert!(verdict.valid, "{}: {}", validator.name(), verdict.output);
        println!("{} view-api object: VALID", validator.name());
    }
}

#[test]
fn a_cursor_from_another_listing_is_refused() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let object = rig.open().expect("open");
    let root = ready(view::list_folder(&object, &v("v2"), "", None, 1)).expect("root");
    let cursor = root.next.expect("cursor");
    let err = ready(view::list_folder(
        &object,
        &v("v2"),
        "docs",
        Some(&cursor),
        1,
    ))
    .unwrap_err();
    assert!(matches!(err, ViewError::InvalidCursor { .. }), "{err:?}");
    let err = ready(view::list_folder(&object, &v("v1"), "", Some(&cursor), 1)).unwrap_err();
    assert!(matches!(err, ViewError::InvalidCursor { .. }), "{err:?}");
}

#[test]
fn the_root_of_an_empty_version_is_an_empty_page() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let m = model(&rig, "writer-a");
    for path in [
        "readme.md",
        "docs/a.txt",
        "docs/b.txt",
        "docs/deep/c.txt",
        "photo.JPG",
    ] {
        ready(m.stage_remove(path)).expect("remove");
    }
    ready(m.commit(meta(&rig, "empty"))).expect("v3");
    let object = rig.open().expect("open");
    let page = ready(view::list_folder(&object, &v("v3"), "", None, 10)).expect("root");
    assert!(page.entries.is_empty());
    assert_eq!(page.next, None);
}

#[test]
fn reads_and_writes_refuse_invalid_paths_and_ranges() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let object = rig.open().expect("open");
    let err = ready(view::file_details(&object, &v("v2"), "/readme.md")).unwrap_err();
    assert!(matches!(err, ViewError::InvalidRequest { .. }), "{err:?}");
    assert!(matches!(
        view::file_history(&object, "docs/../a.txt"),
        Err(ViewError::InvalidRequest { .. })
    ));
    let err = ready(view::read_range(
        &object,
        &v("v2"),
        "readme.md",
        u64::MAX,
        2,
    ))
    .unwrap_err();
    assert!(matches!(err, ViewError::InvalidRequest { .. }), "{err:?}");
    let m = model(&rig, "writer-a");
    let err = ready(view::move_path(&m, "docs/", "archive", meta(&rig, "mv"))).unwrap_err();
    assert!(matches!(err, ViewError::InvalidRequest { .. }), "{err:?}");
    let err = ready(view::restore(&m, &v("v1"), &["gone.txt"], meta(&rig, "r"))).unwrap_err();
    assert!(
        matches!(&err, ViewError::PathNotFound { version, .. } if version == "v1"),
        "{err:?}"
    );
}

#[test]
fn a_leftover_empty_index_does_not_cause_a_false_conflict() {
    let rig = Rig::new(MemoryBackend::new());
    two_versions(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("x.txt", b"x")).expect("stage");
    ready(m.stage_remove("x.txt")).expect("unstage");
    assert!(staged_keys(&rig, "changes/writer-a")
        .iter()
        .any(|k| k.ends_with("index.json")));
    let files: [(&str, &[u8]); 1] = [("docs/a.txt", b"from b")];
    commit_files(&model(&rig, "writer-b"), &rig, &files, "b").expect("b commits v3");
    let receipt = ready(view::add_file(&m, "docs/a.txt", b"from a", meta(&rig, "a")));
    assert_eq!(receipt.expect("no false conflict").version, v("v4"));
}

#[test]
fn fixity_values_and_digest_spelling_come_from_the_inventory() {
    let storage = MemoryBackend::new();
    let mut spec = ObjectSpec::new("urn:built", vec![vec![("a.txt", b"alpha")]]);
    spec.uppercase = true;
    let mut inventory = build(&storage, "o", &spec);
    let md5 = filesquid_ocfl::fixity::FixityAlgorithm::Md5.digest_hex(b"alpha");
    let block = [(md5.clone(), vec!["v1/content/a.txt".to_string()])]
        .into_iter()
        .collect();
    inventory.fixity = Some([("md5".to_string(), block)].into_iter().collect());
    write_inventory(&storage, "o", "v1", &inventory);
    write_inventory(&storage, "o", "", &inventory);
    let object = ready(Object::open(&storage, "o")).expect("open");
    let details = ready(view::file_details(&object, &v("v1"), "a.txt")).expect("details");
    assert_eq!(details.to_json()["fixity"]["md5"], serde_json::json!([md5]));
    let report = ready(view::check_fixity(&object, &v("v1"), "a.txt")).expect("fixity");
    let algorithms: Vec<&str> = report.checks.iter().map(|c| c.algorithm.as_str()).collect();
    assert_eq!(algorithms, ["sha256", "md5"]);
    assert!(report.ok, "{report:?}");
    let history = view::file_history(&object, "a.txt").expect("history");
    assert_eq!(
        history[0].digest.as_deref(),
        Some(sha256(b"alpha").to_uppercase().as_str())
    );
    assert_eq!(details.digest, sha256(b"alpha").to_uppercase());
}
