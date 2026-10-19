//! Forward-delta change model (IO-0.10; OCFL-S-014, OCFL-S-015,
//! OCFL-S-016, OCFL-S-018, OCFL-S-020, OCFL-S-010 (b); PRD F-051, NFR-001;
//! ADR 016 "Change model (public API)").
//!
//! Every scenario that does not need a real directory runs on the in-memory
//! backend and on `FsBackend`. Staging lives on the rig's staging backend,
//! outside the storage root.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::change::{compute_delta, ChangeError, PathChange, StagingArea};
use filesquid_ocfl::read::ReadError;
use filesquid_ocfl::storage::{FsBackend, MemoryBackend, StorageBackend};
use filesquid_ocfl::write::CommitError;
use filesquid_ocfl::ChangeModel;

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

use change::{commit_files, labels, meta, model, staged_keys, state_of, status};
use rig::{key, ready, sha256, v, Rig, OBJECT_ID, ROOT};

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
        .join(format!("ocfl-change-{}-{n}", std::process::id()));
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

/// Commit v1 with a.txt, b.txt and c.txt through the change model.
fn commit_v1<B: StorageBackend>(rig: &Rig<B>) {
    let m = model(rig, "writer-a");
    let files: [(&str, &[u8]); 3] = [
        ("a.txt", b"alpha"),
        ("b.txt", b"bravo"),
        ("c.txt", b"charlie"),
    ];
    let receipt = commit_files(&m, rig, &files, "v1").expect("commit v1");
    assert_eq!(receipt.version, v("v1"));
}

fn additions_changes_renames_and_deletes_write_only_new_bytes<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let m = model(rig, "writer-a");
    ready(m.stage_bytes("a.txt", b"alpha two")).expect("change a");
    ready(m.stage_rename("b.txt", "docs/b.txt")).expect("rename b");
    ready(m.stage_remove("c.txt")).expect("delete c");
    ready(m.stage_bytes("d.txt", b"delta")).expect("add d");
    ready(m.stage_bytes("e.txt", b"alpha")).expect("add a copy of a");
    let changes = status(&m).changes;
    assert_eq!(
        labels(&changes),
        [
            "changed a.txt",
            "deleted c.txt",
            "added d.txt",
            "renamed b.txt -> docs/b.txt",
            "added e.txt",
        ]
    );
    let receipt = ready(m.commit(meta(rig, "v2"))).expect("commit v2");
    assert_eq!(receipt.version, v("v2"));
    assert_eq!(receipt.written, ["v2/content/a.txt", "v2/content/d.txt"]);
    let stored: Vec<_> = rig.snapshot("obj/v2/content").into_keys().collect();
    assert_eq!(stored, ["obj/v2/content/a.txt", "obj/v2/content/d.txt"]);
    let state = state_of(rig, "v2");
    assert_eq!(state["docs/b.txt"], sha256(b"bravo"));
    assert_eq!(state["e.txt"], sha256(b"alpha"));
    assert!(!state.contains_key("c.txt"));
    rig.assert_valid();
}

#[test]
fn a_commit_writes_only_bytes_the_manifest_lacks() {
    on_both!(additions_changes_renames_and_deletes_write_only_new_bytes);
}

fn reinstated_paths_point_at_existing_manifest_entries<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let m = model(rig, "writer-a");
    ready(m.stage_remove("c.txt")).expect("delete c");
    ready(m.commit(meta(rig, "v2"))).expect("commit v2");
    ready(m.stage_bytes("c.txt", b"charlie")).expect("restore c");
    let changes = status(&m).changes;
    assert_eq!(
        changes,
        [PathChange::Reinstated {
            path: "c.txt".into(),
            digest: sha256(b"charlie"),
        }]
    );
    let receipt = ready(m.commit(meta(rig, "v3"))).expect("commit v3");
    assert!(receipt.written.is_empty(), "{:?}", receipt.written);
    let object = rig.open().expect("open");
    let file = object.state(&v("v3")).expect("v3").get("c.txt").cloned();
    assert_eq!(file.expect("c.txt").content_path, "v1/content/c.txt");
    rig.assert_valid();
}

#[test]
fn a_reinstated_path_copies_no_bytes() {
    on_both!(reinstated_paths_point_at_existing_manifest_entries);
}

fn staging_a_tree_classifies_every_path<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let work = MemoryBackend::new();
    for (path, bytes) in [
        ("tree/a.txt", &b"alpha"[..]),
        ("tree/sub/b.txt", b"bravo"),
        ("tree/new.txt", b"new"),
    ] {
        ready(work.put(&key(path), bytes)).expect("working file");
    }
    let m = model(rig, "writer-a");
    ready(m.stage_tree(&work, "tree")).expect("stage tree");
    assert_eq!(
        labels(&status(&m).changes),
        [
            "deleted c.txt",
            "added new.txt",
            "renamed b.txt -> sub/b.txt"
        ]
    );
    let receipt = ready(m.commit(meta(rig, "v2"))).expect("commit");
    assert_eq!(receipt.written, ["v2/content/new.txt"]);
    rig.assert_valid();
}

#[test]
fn a_working_directory_is_staged_as_one_change_set() {
    on_both!(staging_a_tree_classifies_every_path);
}

fn revert_adds_a_version_and_keeps_history<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let m = model(rig, "writer-a");
    let files: [(&str, &[u8]); 1] = [("a.txt", b"alpha two")];
    ready(m.stage_remove("b.txt")).expect("delete b");
    commit_files(&m, rig, &files, "v2").expect("commit v2");
    let before = rig.snapshot("obj/v");
    let receipt = ready(m.revert(&v("v1"), meta(rig, "revert to v1"))).expect("revert");
    assert_eq!(receipt.version, v("v3"));
    assert!(receipt.written.is_empty(), "{:?}", receipt.written);
    assert_eq!(state_of(rig, "v3"), state_of(rig, "v1"));
    let after = rig.snapshot("obj/v");
    for (k, tag) in &before {
        assert_eq!(after.get(k), Some(tag), "{k} changed");
    }
    rig.assert_valid();
}

#[test]
fn revert_commits_a_new_version_without_touching_earlier_ones() {
    on_both!(revert_adds_a_version_and_keeps_history);
}

fn reinstate_restores_chosen_paths<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let m = model(rig, "writer-a");
    ready(m.stage_remove("b.txt")).expect("delete b");
    ready(m.stage_remove("c.txt")).expect("delete c");
    ready(m.commit(meta(rig, "v2"))).expect("commit v2");
    let before = rig.snapshot("obj/v");
    let receipt =
        ready(m.reinstate(&v("v1"), &["b.txt"], meta(rig, "bring b back"))).expect("reinstate");
    assert_eq!(receipt.version, v("v3"));
    assert!(receipt.written.is_empty());
    let state = state_of(rig, "v3");
    assert_eq!(state.keys().collect::<Vec<_>>(), ["a.txt", "b.txt"]);
    let after = rig.snapshot("obj/v");
    for (k, tag) in &before {
        assert_eq!(after.get(k), Some(tag), "{k} changed");
    }
    rig.assert_valid();
}

#[test]
fn reinstate_commits_a_new_version_with_the_chosen_paths() {
    on_both!(reinstate_restores_chosen_paths);
}

#[test]
fn reinstate_refuses_a_path_the_version_does_not_hold() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    let err = ready(m.reinstate(&v("v1"), &["zzz.txt"], meta(&rig, "x"))).unwrap_err();
    assert!(
        matches!(&err, ChangeError::PathNotFound { path, .. } if path == "zzz.txt"),
        "{err:?}"
    );
    assert_eq!(rig.open().expect("open").head(), &v("v1"));
}

#[test]
fn an_empty_change_set_publishes_nothing() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    let err = ready(m.commit(meta(&rig, "nothing"))).unwrap_err();
    assert!(matches!(err, ChangeError::NothingToCommit), "{err:?}");
    ready(m.stage_bytes("a.txt", b"alpha")).expect("stage unchanged bytes");
    assert!(status(&m).changes.is_empty());
    let err = ready(m.revert(&v("v1"), meta(&rig, "same"))).unwrap_err();
    assert!(matches!(err, ChangeError::NothingToCommit), "{err:?}");
    assert_eq!(rig.open().expect("open").head(), &v("v1"));
}

fn a_moved_head_rebases_when_no_staged_path_changed<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let a = model(rig, "writer-a");
    let b = model(rig, "writer-b");
    ready(a.stage_bytes("a.txt", b"from a")).expect("a stages");
    let files: [(&str, &[u8]); 1] = [("b.txt", b"from b")];
    commit_files(&b, rig, &files, "b's v2").expect("b commits v2");
    let pending = status(&a);
    assert_eq!(pending.base, Some(v("v1")));
    assert_eq!(pending.head, Some(v("v2")));
    let receipt = ready(a.commit(meta(rig, "a's change"))).expect("rebased");
    assert_eq!(receipt.version, v("v3"));
    let state = state_of(rig, "v3");
    assert_eq!(state["a.txt"], sha256(b"from a"));
    assert_eq!(state["b.txt"], sha256(b"from b"));
    rig.assert_valid();
}

#[test]
fn a_moved_head_is_rebased_onto() {
    on_both!(a_moved_head_rebases_when_no_staged_path_changed);
}

fn a_moved_head_that_changed_a_staged_path_is_a_conflict<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let a = model(rig, "writer-a");
    let b = model(rig, "writer-b");
    ready(a.stage_bytes("a.txt", b"from a")).expect("a stages");
    ready(a.stage_remove("c.txt")).expect("a stages a delete");
    ready(b.stage_remove("c.txt")).expect("b deletes c");
    ready(b.commit(meta(rig, "b's v2"))).expect("b commits v2");
    let before = rig.snapshot("obj");
    let err = ready(a.commit(meta(rig, "a's change"))).unwrap_err();
    let ChangeError::Conflict { base, head, paths } = &err else {
        panic!("expected a conflict, got {err:?}");
    };
    assert_eq!(base.as_deref(), Some("v1"));
    assert_eq!(head.as_deref(), Some("v2"));
    assert_eq!(paths, &["c.txt"]);
    assert_eq!(rig.snapshot("obj"), before, "nothing was published");
    assert_eq!(
        labels(&status(&a).changes),
        ["changed a.txt", "deleted c.txt"]
    );
}

#[test]
fn a_moved_head_that_touched_a_staged_path_reports_a_conflict() {
    on_both!(a_moved_head_that_changed_a_staged_path_is_a_conflict);
}

#[test]
fn a_head_without_its_version_inventory_stays_refused_and_unrepaired() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    for raw in ["obj/v1/inventory.json", "obj/v1/inventory.json.sha256"] {
        ready(rig.storage.delete(&key(raw))).expect("remove the head version inventory");
    }
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("d.txt", b"delta")).expect("stage");
    let before = rig.snapshot("obj");
    let err = ready(m.commit(meta(&rig, "v2"))).unwrap_err();
    assert!(
        matches!(
            err,
            ChangeError::Commit(CommitError::MissingVersionInventory { .. })
        ),
        "{err:?}"
    );
    assert_eq!(rig.snapshot("obj"), before, "no repair and no version");
    assert!(!rig.stored("obj/v1/inventory.json"));
}

#[test]
fn staging_and_checkout_inside_an_ocfl_root_are_refused() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let inside = StagingArea::new(&rig.storage, "obj/staging").expect("prefix");
    let m = ChangeModel::new(&rig.storage, &*rig.leases, "w", ROOT, OBJECT_ID, inside);
    let err = ready(m.stage_bytes("d.txt", b"delta")).unwrap_err();
    assert!(
        matches!(err, ChangeError::Read(ReadError::CopyIntoOcflRoot { .. })),
        "{err:?}"
    );
    let outside = model(&rig, "writer-a");
    let err = ready(outside.checkout(&v("v1"), &rig.storage, "obj/out")).unwrap_err();
    assert!(
        matches!(err, ChangeError::Read(ReadError::CopyIntoOcflRoot { .. })),
        "{err:?}"
    );
    assert!(rig.snapshot("obj/staging").is_empty());
    assert!(rig.snapshot("obj/out").is_empty());
}

#[test]
fn staging_is_never_inside_the_object_and_reads_need_none_of_it() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    assert!(staged_keys(&rig, "changes/writer-a").is_empty(), "cleared");
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("d.txt", b"delta")).expect("stage");
    assert!(staged_keys(&rig, "changes/writer-a")
        .iter()
        .any(|k| k == "changes/writer-a/index.json"));
    let object_keys: Vec<_> = rig.snapshot("obj").into_keys().collect();
    assert!(object_keys.iter().all(|k| !k.contains("extensions")));
    ready(rig.staging.delete(&key("changes/writer-a/index.json"))).expect("drop staging");
    let object = rig.open().expect("read without staging");
    assert_eq!(object.head(), &v("v1"));
    assert_eq!(state_of(&rig, "v1").len(), 3);
}

#[test]
fn checkout_writes_a_version_to_a_caller_location() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let out = MemoryBackend::new();
    let written = ready(model(&rig, "w").checkout(&v("v1"), &out, "out")).expect("checkout");
    let keys: Vec<_> = written.iter().map(|k| k.as_str().to_string()).collect();
    assert_eq!(keys, ["out/a.txt", "out/b.txt", "out/c.txt"]);
    let bytes = ready(filesquid_ocfl::storage::read_verified(
        &out,
        &key("out/c.txt"),
        1024,
    ));
    assert_eq!(bytes.expect("read"), b"charlie");
}

#[test]
fn log_and_diff_read_the_inventory() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_rename("a.txt", "z.txt")).expect("rename");
    ready(m.commit(meta(&rig, "rename"))).expect("v2");
    let log = ready(m.log()).expect("log");
    let versions: Vec<_> = log.iter().map(|i| i.version.to_string()).collect();
    assert_eq!(versions, ["v2", "v1"]);
    assert_eq!(log[0].message.as_deref(), Some("rename"));
    let diff = ready(m.diff(&v("v1"), &v("v2"))).expect("diff");
    assert_eq!(labels(&diff), ["renamed a.txt -> z.txt"]);
}

#[test]
fn compute_delta_classifies_a_working_state_against_a_base() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_remove("c.txt")).expect("delete");
    ready(m.commit(meta(&rig, "v2"))).expect("v2");
    let object = rig.open().expect("open");
    let working = [
        ("a.txt", sha256(b"alpha")),
        ("b2.txt", sha256(b"bravo")),
        ("c.txt", sha256(b"charlie")),
        ("n.txt", sha256(b"new")),
    ]
    .into_iter()
    .map(|(p, d)| (p.to_string(), d))
    .collect();
    let delta = compute_delta(Some(object.inventory()), Some(&v("v2")), &working).expect("delta");
    assert_eq!(
        labels(&delta),
        ["renamed b.txt -> b2.txt", "reinstated c.txt", "added n.txt"]
    );
    assert!(delta.iter().any(|c| matches!(
        c,
        PathChange::Added { path, new_bytes: true, .. } if path == "n.txt"
    )));
}

#[test]
fn committed_objects_pass_the_engine_and_community_validators() {
    let (backend, _scratch) = fs_backend();
    let rig = Rig::new(backend);
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("a.txt", b"alpha two")).expect("change");
    ready(m.stage_rename("b.txt", "sub/b.txt")).expect("rename");
    ready(m.stage_remove("c.txt")).expect("delete");
    ready(m.commit(meta(&rig, "v2"))).expect("v2");
    ready(m.reinstate(&v("v1"), &["c.txt"], meta(&rig, "v3"))).expect("v3");
    ready(m.revert(&v("v1"), meta(&rig, "v4"))).expect("v4");
    rig.assert_valid();
    println!("filesquid-ocfl change-model object: VALID (4 versions)");
    let object_root = rig.storage.inner().root().join(ROOT);
    for validator in community::validators_or_skip() {
        let verdict = validator.run_validation(&object_root);
        assert!(verdict.valid, "{}: {}", validator.name(), verdict.output);
        println!(
            "{} change-model object: VALID\n{}",
            validator.name(),
            verdict.output.trim_end()
        );
    }
}

#[test]
fn a_damaged_or_foreign_staging_index_is_refused() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    let index = key("changes/writer-a/index.json");
    for bad in [
        &b"not json"[..],
        br#"{"type":"filesquid-ocfl-staging","version":2,"objectId":"urn:example:obj","base":"v1","paths":{}}"#,
        br#"{"type":"filesquid-ocfl-staging","version":1,"objectId":"urn:example:obj","base":"v1","paths":{"/a":{"removed":true}}}"#,
        br#"{"type":"filesquid-ocfl-staging","version":1,"objectId":"urn:example:obj","base":"v1","paths":{"a.txt":{"sha256":"xyz"}}}"#,
    ] {
        ready(rig.staging.put(&index, bad)).expect("damage the index");
        let err = ready(m.status()).unwrap_err();
        assert!(matches!(err, ChangeError::StagingCorrupt { .. }), "{err:?}");
    }
    let foreign = br#"{"type":"filesquid-ocfl-staging","version":1,"objectId":"urn:other","base":"v1","paths":{}}"#;
    ready(rig.staging.put(&index, foreign)).expect("foreign index");
    let err = ready(m.stage_bytes("d.txt", b"d")).unwrap_err();
    assert!(
        matches!(err, ChangeError::StagingMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn a_staged_tree_conflicts_with_a_path_a_newer_version_added() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let work = MemoryBackend::new();
    ready(work.put(&key("tree/a.txt"), b"alpha")).expect("working file");
    let a = model(&rig, "writer-a");
    ready(a.stage_tree(&work, "tree")).expect("stage tree");
    let files: [(&str, &[u8]); 1] = [("new.txt", b"from b")];
    commit_files(&model(&rig, "writer-b"), &rig, &files, "b adds").expect("b commits v2");
    let err = ready(a.commit(meta(&rig, "tree"))).unwrap_err();
    let ChangeError::Conflict { paths, .. } = &err else {
        panic!("expected a conflict, got {err:?}");
    };
    assert_eq!(paths, &["new.txt"]);
    assert_eq!(rig.open().expect("open").head(), &v("v2"));
}

#[test]
fn a_staging_area_inside_an_object_cannot_be_reset() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let inside = StagingArea::new(&rig.storage, "obj").expect("prefix");
    let m = ChangeModel::new(&rig.storage, &*rig.leases, "w", ROOT, OBJECT_ID, inside);
    let before = rig.snapshot("obj");
    let err = ready(m.reset()).unwrap_err();
    assert!(
        matches!(err, ChangeError::Read(ReadError::CopyIntoOcflRoot { .. })),
        "{err:?}"
    );
    assert_eq!(rig.snapshot("obj"), before);
}

#[test]
fn changes_staged_during_a_commit_are_kept() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_bytes("d.txt", b"delta")).expect("stage");
    let staging = rig.staging.clone();
    rig.storage.after_write(
        "obj/inventory.json.sha256",
        Box::new(move || {
            let index = key("changes/writer-a/index.json");
            let bytes = ready(filesquid_ocfl::storage::read_verified(
                &*staging, &index, 1024,
            ))
            .expect("read index");
            let text = String::from_utf8(bytes).expect("utf-8");
            let changed = text.replace("\"snapshot\": false", "\"snapshot\": true");
            assert_ne!(changed, text, "the index changed");
            ready(staging.put(&index, changed.as_bytes())).expect("restage");
        }),
    );
    let err = ready(m.commit(meta(&rig, "v2"))).unwrap_err();
    let ChangeError::StagingNotCleared { receipt, .. } = &err else {
        panic!("expected StagingNotCleared, got {err:?}");
    };
    assert_eq!(receipt.version, v("v2"));
    assert_eq!(rig.open().expect("open").head(), &v("v2"));
    let kept = staged_keys(&rig, "changes/writer-a");
    assert!(kept.iter().any(|k| k.ends_with("index.json")), "{kept:?}");
    assert!(kept.iter().any(|k| k.contains("/blobs/")), "{kept:?}");
}

#[test]
fn a_path_that_conflicts_with_a_directory_is_refused_when_staged() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    let err = ready(m.stage_bytes("a.txt/inner", b"x")).unwrap_err();
    assert!(
        matches!(&err, ChangeError::InvalidPath { path, .. } if path == "a.txt/inner"),
        "{err:?}"
    );
    assert_eq!(staged_keys(&rig, "changes/writer-a"), Vec::<String>::new());
}

#[test]
fn a_reverse_diff_reports_restored_content_as_reinstated() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    ready(m.stage_remove("c.txt")).expect("delete");
    ready(m.commit(meta(&rig, "v2"))).expect("v2");
    let diff = ready(m.diff(&v("v2"), &v("v1"))).expect("diff");
    assert_eq!(labels(&diff), ["reinstated c.txt"]);
}

#[test]
fn a_tree_at_the_root_of_a_backend_is_staged() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let work = MemoryBackend::new();
    ready(work.put(&key("a.txt"), b"alpha")).expect("working file");
    let m = model(&rig, "writer-a");
    ready(m.stage_tree(&work, "")).expect("stage the root");
    assert_eq!(
        labels(&status(&m).changes),
        ["deleted b.txt", "deleted c.txt"]
    );
}

#[test]
fn compute_delta_without_an_inventory_has_an_empty_base() {
    let working = [("a.txt".to_string(), sha256(b"alpha"))]
        .into_iter()
        .collect();
    let delta = compute_delta(None, Some(&v("v1")), &working).expect("delta");
    assert_eq!(labels(&delta), ["added a.txt"]);
}

#[test]
fn an_uncommitted_first_version_of_another_object_is_refused() {
    let rig = Rig::new(MemoryBackend::new());
    let mut draft = filesquid_ocfl::write::VersionDraft::new("urn:other", rig.now());
    ready(rig.staging.put(&key("stage/x"), b"x")).expect("stage");
    draft.stage("x.txt", key("stage/x"));
    rig.storage.crash_at("obj/inventory.json.sha256");
    let writer = rig.writer("other");
    let crashed = filesquid_ocfl::storage::complete_immediately(writer.commit(
        ROOT,
        None,
        &*rig.staging,
        &draft,
    ));
    assert!(crashed.is_err(), "the first commit crashed");
    let err = ready(model(&rig, "writer-a").status()).unwrap_err();
    assert!(matches!(err, ChangeError::WrongObject { .. }), "{err:?}");
}

#[test]
fn an_object_root_holding_git_metadata_is_refused() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    ready(rig.storage.put(&key("obj/.git/HEAD"), b"ref")).expect("plant .git");
    let err = ready(model(&rig, "writer-a").status()).unwrap_err();
    assert!(
        matches!(err, ChangeError::Commit(CommitError::Guard(_))),
        "{err:?}"
    );
}

#[test]
fn checkout_writes_nothing_when_any_destination_is_inside_an_ocfl_root() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    let files: [(&str, &[u8]); 1] = [("z/z.txt", b"zulu")];
    commit_files(&m, &rig, &files, "v2").expect("v2");
    let out = MemoryBackend::new();
    ready(out.put(&key("out/z/0=ocfl_object_1.1"), b"ocfl_object_1.1\n")).expect("declaration");
    let err = ready(m.checkout(&v("v2"), &out, "out")).unwrap_err();
    assert!(
        matches!(err, ChangeError::Read(ReadError::CopyIntoOcflRoot { .. })),
        "{err:?}"
    );
    assert!(!ready(out.exists(&key("out/a.txt"))).expect("exists"));
}

#[test]
fn checkout_to_the_root_of_a_backend_uses_the_logical_paths() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let out = MemoryBackend::new();
    let written = ready(model(&rig, "w").checkout(&v("v1"), &out, "")).expect("checkout");
    let keys: Vec<_> = written.iter().map(|k| k.as_str().to_string()).collect();
    assert_eq!(keys, ["a.txt", "b.txt", "c.txt"]);
}

#[test]
fn a_damaged_staged_blob_is_staged_again() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    let blob = key(&format!("changes/writer-a/blobs/{}", sha256(b"delta")));
    ready(rig.staging.put(&blob, b"not delta")).expect("damage the blob");
    ready(m.stage_bytes("d.txt", b"delta")).expect("stage");
    let stored = ready(filesquid_ocfl::storage::read_verified(
        &*rig.staging,
        &blob,
        1024,
    ));
    assert_eq!(stored.expect("blob"), b"delta");
    ready(m.commit(meta(&rig, "v2"))).expect("commit");
    assert_eq!(state_of(&rig, "v2")["d.txt"], sha256(b"delta"));
    rig.assert_valid();
}

#[test]
fn removing_or_renaming_an_invalid_path_is_refused_as_invalid() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let m = model(&rig, "writer-a");
    let err = ready(m.stage_remove("/a.txt")).unwrap_err();
    assert!(matches!(err, ChangeError::InvalidPath { .. }), "{err:?}");
    let err = ready(m.stage_rename("../a.txt", "z.txt")).unwrap_err();
    assert!(matches!(err, ChangeError::InvalidPath { .. }), "{err:?}");
}
