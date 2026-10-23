//! Copy-guard warnings name the object (IO-1.6; OCFL-S-025, OCFL-S-019;
//! API-S-031; PRD F-061, F-051).
//!
//! A copy refused because its destination lies inside an OCFL root warns
//! with the destination and the declaring root, and with the identifier of
//! the object the copy concerns: the source object in `Object::copy_file`
//! and in checkout, which also name the version, and the object a staging
//! area belongs to when staging, which knows no version. Every scenario
//! runs on the in-memory backend and on `FsBackend`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::change::{ChangeError, StagingArea};
use filesquid_ocfl::read::{Object, ReadError};
use filesquid_ocfl::storage::{FsBackend, MemoryBackend, StorageBackend};
use filesquid_ocfl::ChangeModel;

#[path = "../support/object_builder.rs"]
mod builder;
#[path = "../support/warning_capture.rs"]
mod capture;
#[path = "../support/change_rig.rs"]
mod change;
#[path = "../support/fault_backend.rs"]
mod fault;
#[path = "../support/write_rig.rs"]
mod rig;

use capture::{Fields, WarningCapture};
use change::{commit_files, model};
use rig::{key, ready, v, Rig, OBJECT_ID, ROOT};

/// The message of the copy guard's warning.
const REFUSED: &str = "refused a copy into an OCFL root";

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
        .join(format!("ocfl-copy-guard-{}-{n}", std::process::id()));
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

/// Commit `v1` (`a.txt`, `b.txt`) through the change model.
fn commit_v1<B: StorageBackend>(rig: &Rig<B>) {
    let m = model(rig, "writer-a");
    let files: [(&str, &[u8]); 2] = [("a.txt", b"alpha"), ("b.txt", b"bravo")];
    commit_files(&m, rig, &files, "v1").expect("commit v1");
}

/// The one copy-guard warning `capture` recorded.
fn the_warning(capture: &WarningCapture) -> Fields {
    let found = capture.with_message(REFUSED);
    assert_eq!(found.len(), 1, "one copy-guard warning: {found:?}");
    found.into_iter().next().expect("one warning")
}

/// Assert `warning` names `destination`, `root`, the rig's object and
/// `version` (`None`: no version field).
fn assert_names(warning: &Fields, destination: &str, root: &str, version: Option<&str>) {
    let field = |name: &str| warning.get(name).map(String::as_str);
    assert_eq!(field("destination"), Some(destination), "{warning:?}");
    assert_eq!(field("root"), Some(root), "{warning:?}");
    assert_eq!(field("object"), Some(OBJECT_ID), "{warning:?}");
    assert_eq!(field("version"), version, "{warning:?}");
}

fn copy_file_warning_names_object_and_version<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let capture = WarningCapture::default();
    let result = capture.run(|| {
        let object = ready(Object::open(&rig.storage, ROOT)).expect("open");
        let dest = key("obj/v1/content/c.txt");
        ready(object.copy_file(&v("v1"), "b.txt", &rig.storage, &dest, None))
    });
    assert!(
        matches!(&result, Err(ReadError::CopyIntoOcflRoot { key, root })
            if key == "obj/v1/content/c.txt" && root == ROOT),
        "{result:?}"
    );
    assert_names(
        &the_warning(&capture),
        "obj/v1/content/c.txt",
        ROOT,
        Some("v1"),
    );
}

#[test]
fn a_refused_copy_file_names_the_source_object_and_version() {
    on_both!(copy_file_warning_names_object_and_version);
}

fn checkout_warning_names_object_and_version<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let m = model(rig, "writer-a");
    let capture = WarningCapture::default();
    let result = capture.run(|| ready(m.checkout(&v("v1"), &rig.storage, "obj/out")));
    assert!(
        matches!(
            &result,
            Err(ChangeError::Read(ReadError::CopyIntoOcflRoot { .. }))
        ),
        "{result:?}"
    );
    assert_names(&the_warning(&capture), "obj/out/a.txt", ROOT, Some("v1"));
    assert!(rig.snapshot("obj/out").is_empty(), "nothing checked out");
}

#[test]
fn a_refused_checkout_names_the_object_and_version() {
    on_both!(checkout_warning_names_object_and_version);
}

fn staging_warning_names_the_object<B: StorageBackend>(rig: &Rig<B>) {
    commit_v1(rig);
    let inside = StagingArea::new(&rig.storage, "obj/staging").expect("prefix");
    let m = ChangeModel::new(&rig.storage, &*rig.leases, "w", ROOT, OBJECT_ID, inside);
    let capture = WarningCapture::default();
    let result = capture.run(|| ready(m.stage_bytes("d.txt", b"delta")));
    assert!(
        matches!(
            &result,
            Err(ChangeError::Read(ReadError::CopyIntoOcflRoot { .. }))
        ),
        "{result:?}"
    );
    let warning = the_warning(&capture);
    let destination = warning.get("destination").expect("destination").clone();
    assert!(destination.starts_with("obj/staging/"), "{warning:?}");
    assert_names(&warning, &destination, ROOT, None);
    assert!(rig.snapshot("obj/staging").is_empty(), "nothing staged");
}

#[test]
fn a_refused_staging_write_names_the_object_without_a_version() {
    on_both!(staging_warning_names_the_object);
}

#[test]
fn the_warning_comes_from_the_engine_target() {
    let rig = Rig::new(MemoryBackend::new());
    commit_v1(&rig);
    let capture = WarningCapture::default();
    let _ = capture.run(|| ready(model(&rig, "w").checkout(&v("v1"), &rig.storage, "obj/out")));
    let targets = capture.targets();
    assert!(
        targets.iter().all(|t| t.starts_with("filesquid_ocfl")),
        "{targets:?}"
    );
}
