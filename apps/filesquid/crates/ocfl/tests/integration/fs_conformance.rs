//! Filesystem backend conformance (OCFL-S-007): the shared suite run against
//! `FsBackend` on a real directory, plus filesystem-only cases.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::storage::{
    ByteRange, FsBackend, MultipartWriter, ObjectMeta, Precondition, StorageBackend, StorageError,
    StorageKey,
};

/// A scratch directory under Cargo's per-target temporary directory, removed
/// on drop so every test closes what it opened.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("ocfl-backend-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fs_backend(label: &str) -> (FsBackend, Scratch) {
    let scratch = Scratch::new(label);
    let backend =
        FsBackend::new(scratch.0.join("root"), scratch.0.join("staging")).expect("fs backend");
    (backend, scratch)
}

#[test]
fn fs_staging_must_be_outside_the_storage_root() {
    let scratch = Scratch::new("staging-inside");
    let root = scratch.0.join("root");
    let err = FsBackend::new(&root, root.join("staging")).expect_err("inside");
    assert!(matches!(err, StorageError::InvalidConfig { .. }), "{err:?}");
}

#[tokio::test]
async fn fs_backend_writes_plain_files_under_the_root() {
    let (backend, scratch) = fs_backend("plain");
    backend
        .put(&key("objects/a/0=ocfl_object_1.1"), b"ocfl_object_1.1\n")
        .await
        .expect("put");
    let on_disk = scratch.0.join("root/objects/a/0=ocfl_object_1.1");
    assert_eq!(
        std::fs::read(on_disk).expect("plain file"),
        b"ocfl_object_1.1\n"
    );
}

#[tokio::test]
async fn fs_backend_leaves_no_temporary_files_or_staging() {
    let (backend, scratch) = fs_backend("tidy");
    let k = key("objects/a/v1/content/f.bin");
    backend.put(&k, b"abc").await.expect("put");
    let mut writer = MultipartWriter::start(&backend, k.clone())
        .await
        .expect("start");
    writer.write_part(b"def").await.expect("part");
    writer.finish(None).await.expect("finish");
    let names: Vec<String> = std::fs::read_dir(scratch.0.join("root/objects/a/v1/content"))
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["f.bin"]);
    let staged = std::fs::read_dir(scratch.0.join("staging").join("uploads"))
        .expect("uploads")
        .count();
    assert_eq!(staged, 0, "multipart staging must be cleaned up");
}

#[tokio::test]
async fn fs_delete_prunes_empty_directories() {
    let (backend, scratch) = fs_backend("prune");
    backend.put(&key("a/b/c/d.txt"), b"d").await.expect("put");
    backend.put(&key("a/keep.txt"), b"k").await.expect("put");
    backend.delete(&key("a/b/c/d.txt")).await.expect("delete");
    assert!(!scratch.0.join("root/a/b").exists(), "empty dirs remain");
    assert!(scratch.0.join("root/a/keep.txt").exists());
}

#[test]
fn fs_staging_aliased_into_the_root_is_rejected() {
    let scratch = Scratch::new("staging-alias");
    std::fs::create_dir_all(scratch.0.join("alias")).expect("alias dir");
    let aliased_root = scratch.0.join("alias").join("..").join("store");
    let err = FsBackend::new(&aliased_root, scratch.0.join("store").join("staging"))
        .expect_err("lexical alias");
    assert!(matches!(err, StorageError::InvalidConfig { .. }), "{err:?}");
}

#[cfg(unix)]
#[test]
fn fs_staging_reached_through_a_symlink_into_the_root_is_rejected() {
    let scratch = Scratch::new("staging-symlink");
    let root = scratch.0.join("store");
    std::fs::create_dir_all(&root).expect("root");
    std::os::unix::fs::symlink(&root, scratch.0.join("link")).expect("symlink");
    let err =
        FsBackend::new(&root, scratch.0.join("link").join("staging")).expect_err("symlink alias");
    assert!(matches!(err, StorageError::InvalidConfig { .. }), "{err:?}");
}

#[test]
fn fs_root_is_reported_canonical() {
    let scratch = Scratch::new("canonical");
    std::fs::create_dir_all(scratch.0.join("a")).expect("dir");
    let root = scratch.0.join("a").join("..").join("root");
    let backend = FsBackend::new(root, scratch.0.join("st")).expect("backend");
    let expected = scratch.0.canonicalize().expect("canon").join("root");
    assert_eq!(backend.root(), expected.as_path());
}

#[cfg(unix)]
#[tokio::test]
async fn fs_keys_cannot_escape_through_a_symlinked_parent() {
    let (backend, scratch) = fs_backend("symlink-parent");
    let outside = scratch.0.join("outside");
    std::fs::create_dir_all(&outside).expect("outside");
    std::fs::write(outside.join("secret.txt"), b"secret").expect("secret");
    std::fs::create_dir_all(scratch.0.join("root")).expect("root");
    std::os::unix::fs::symlink(&outside, scratch.0.join("root").join("link")).expect("link");
    let write = backend.put(&key("link/new.txt"), b"x").await;
    assert!(
        matches!(write, Err(StorageError::SymlinkForbidden { .. })),
        "{write:?}"
    );
    assert!(!outside.join("new.txt").exists(), "write escaped the root");
    let range = ByteRange::new(0, 16).expect("range");
    let read = backend.get_range(&key("link/secret.txt"), range).await;
    assert!(
        matches!(read, Err(StorageError::SymlinkForbidden { .. })),
        "{read:?}"
    );
    let head = backend.head(&key("link/secret.txt")).await;
    assert!(
        matches!(head, Err(StorageError::SymlinkForbidden { .. })),
        "{head:?}"
    );
    let delete = backend.delete(&key("link/secret.txt")).await;
    assert!(
        matches!(delete, Err(StorageError::SymlinkForbidden { .. })),
        "{delete:?}"
    );
    assert!(outside.join("secret.txt").exists());
    let list = backend.list("link/", None, 10).await;
    assert!(
        matches!(list, Err(StorageError::SymlinkForbidden { .. })),
        "{list:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn fs_keys_cannot_read_through_a_symlinked_file() {
    let (backend, scratch) = fs_backend("symlink-file");
    std::fs::write(scratch.0.join("secret.txt"), b"secret").expect("secret");
    std::fs::create_dir_all(scratch.0.join("root")).expect("root");
    std::os::unix::fs::symlink(scratch.0.join("secret.txt"), scratch.0.join("root/evil"))
        .expect("link");
    let range = ByteRange::new(0, 16).expect("range");
    let read = backend.get_range(&key("evil"), range).await;
    assert!(
        matches!(read, Err(StorageError::SymlinkForbidden { .. })),
        "{read:?}"
    );
}

#[tokio::test]
async fn fs_reserved_internal_names_are_rejected() {
    let (backend, _scratch) = fs_backend("reserved");
    for raw in [".fsquid-tmp-1", "objects/.fsquid-lock"] {
        let err = backend.put(&key(raw), b"x").await.expect_err(raw);
        assert!(
            matches!(err, StorageError::InvalidKey { .. }),
            "{raw}: {err:?}"
        );
    }
}

/// Drive a filesystem-backend future on this thread.
fn block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(future)
}

/// Count successful conditional writes, panicking on any error other than a
/// failed precondition.
fn count_winners(results: Vec<Result<ObjectMeta, StorageError>>) -> usize {
    let mut wins = 0;
    for result in results {
        match result {
            Ok(_) => wins += 1,
            Err(StorageError::PreconditionFailed { .. }) => {}
            Err(other) => panic!("unexpected error {other:?}"),
        }
    }
    wins
}

/// Run `writers` threads, each with its own `FsBackend` on the same root, that
/// all try one conditional write at once; return how many succeeded.
fn race(label: &str, writers: usize, shared_staging: bool, setup: Option<&[u8]>) -> usize {
    let scratch = Scratch::new(label);
    let root = scratch.0.join("root");
    let staging = |i: usize| match shared_staging {
        true => scratch.0.join("staging"),
        false => scratch.0.join(format!("staging-{i}")),
    };
    let k = key("objects/a/inventory.json");
    let first = FsBackend::new(&root, staging(0)).expect("backend");
    let base = setup.map(|bytes| block(first.put(&k, bytes)).expect("setup").etag);
    let barrier = std::sync::Barrier::new(writers);
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..writers)
            .map(|i| {
                let (barrier, k, base) = (&barrier, &k, &base);
                let backend = FsBackend::new(&root, staging(i)).expect("backend");
                scope.spawn(move || {
                    let condition = match base {
                        Some(tag) => Precondition::IfMatch(tag.clone()),
                        None => Precondition::IfAbsent,
                    };
                    let body = format!("writer-{i}");
                    barrier.wait();
                    block(backend.put_if(k, body.as_bytes(), &condition))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("thread"))
            .collect::<Vec<_>>()
    });
    count_winners(results)
}

#[test]
fn fs_create_only_has_one_winner_across_instances_sharing_staging() {
    for round in 0..5 {
        assert_eq!(race(&format!("absent-shared-{round}"), 8, true, None), 1);
    }
}

#[test]
fn fs_create_only_has_one_winner_across_instances_with_separate_staging() {
    for round in 0..5 {
        assert_eq!(race(&format!("absent-separate-{round}"), 8, false, None), 1);
    }
}

#[test]
fn fs_head_swap_has_one_winner_across_instances() {
    for round in 0..5 {
        assert_eq!(race(&format!("match-{round}"), 8, true, Some(b"v1")), 1);
    }
}

/// A filesystem backend with its scratch directory; the directory is
/// removed when the fixture is dropped.
struct FsFixture(FsBackend, #[allow(dead_code)] Scratch);

/// A fresh filesystem fixture.
fn fs_fixture() -> FsFixture {
    let (backend, scratch) = fs_backend("suite");
    FsFixture(backend, scratch)
}

#[path = "../support/conformance.rs"]
mod suite;
use suite::key;

mod filesystem {
    use super::*;
    conformance_tests!(fs_fixture());
}

/// Entries left in the filesystem backend's multipart staging.
fn staged_uploads(scratch: &Scratch) -> Vec<String> {
    std::fs::read_dir(scratch.0.join("staging").join("uploads"))
        .expect("uploads dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect()
}

#[tokio::test]
async fn fs_abort_removes_the_upload_staging_directory() {
    let (backend, scratch) = fs_backend("abort-cleans");
    let mut writer = MultipartWriter::start(&backend, key("objects/a/big.bin"))
        .await
        .expect("start");
    writer.write_part(b"partial").await.expect("part");
    assert_eq!(staged_uploads(&scratch).len(), 1);
    writer.abort().await.expect("abort");
    assert!(staged_uploads(&scratch).is_empty(), "abort left staging");
}

#[tokio::test]
async fn fs_complete_closes_the_upload_durably() {
    let (backend, scratch) = fs_backend("complete-closes");
    let k = key("objects/a/f.bin");
    let upload = backend.create_multipart(&k).await.expect("create");
    let p1 = backend.upload_part(&upload, 1, b"ab").await.expect("part");
    backend
        .complete_multipart(&upload, &[p1], None)
        .await
        .expect("complete");
    assert!(staged_uploads(&scratch).is_empty());
    let late = backend.upload_part(&upload, 2, b"cd").await;
    assert!(
        matches!(late, Err(StorageError::UnknownUpload { .. })),
        "{late:?}"
    );
    assert!(
        staged_uploads(&scratch).is_empty(),
        "a late part reopened the upload"
    );
}

#[test]
fn fs_abort_and_upload_part_never_resurrect_an_upload() {
    let (backend, scratch) = fs_backend("abort-race");
    let k = key("objects/a/race.bin");
    for _ in 0..50 {
        let upload = block(backend.create_multipart(&k)).expect("create");
        std::thread::scope(|scope| {
            let parts = scope.spawn(|| {
                for n in 1..=20 {
                    let _ = block(backend.upload_part(&upload, n, b"x"));
                }
            });
            let _ = block(backend.abort_multipart(&upload));
            parts.join().expect("parts thread");
        });
        let left = staged_uploads(&scratch);
        assert!(left.is_empty(), "an aborted upload came back: {left:?}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn fs_upload_ids_cannot_follow_a_symlink_out_of_staging() {
    let (backend, scratch) = fs_backend("upload-symlink");
    let outside = scratch.0.join("outside");
    std::fs::create_dir_all(&outside).expect("outside");
    let upload = backend
        .create_multipart(&key("objects/a/f.bin"))
        .await
        .expect("create");
    let dir = scratch
        .0
        .join("staging")
        .join("uploads")
        .join(upload.as_str());
    std::fs::remove_dir_all(&dir).expect("remove upload dir");
    std::os::unix::fs::symlink(&outside, &dir).expect("symlink");
    let part = backend.upload_part(&upload, 1, b"escaped").await;
    assert!(part.is_err(), "{part:?}");
    assert_eq!(std::fs::read_dir(&outside).expect("outside").count(), 0);
    let abort = backend.abort_multipart(&upload).await;
    assert!(abort.is_err(), "{abort:?}");
    assert!(outside.exists(), "abort followed the link");
}

#[cfg(unix)]
#[tokio::test]
async fn fs_part_open_errors_are_not_reported_as_part_mismatch() {
    let (backend, scratch) = fs_backend("part-loop");
    let upload = backend
        .create_multipart(&key("objects/a/f.bin"))
        .await
        .expect("create");
    let p1 = backend.upload_part(&upload, 1, b"ab").await.expect("part");
    let part = scratch
        .0
        .join("staging/uploads")
        .join(upload.as_str())
        .join("part-00001");
    std::fs::remove_file(&part).expect("remove part");
    std::os::unix::fs::symlink(&part, &part).expect("self-loop");
    let err = backend
        .complete_multipart(&upload, &[p1], None)
        .await
        .expect_err("loop");
    assert!(
        !matches!(err, StorageError::PartMismatch { .. }),
        "an I/O failure was hidden as a part mismatch: {err:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn fs_listing_returns_only_regular_files() {
    let (backend, scratch) = fs_backend("list-regular");
    backend
        .put(&key("objects/a/real.txt"), b"r")
        .await
        .expect("put");
    std::fs::write(scratch.0.join("outside.txt"), b"o").expect("outside");
    std::os::unix::fs::symlink(
        scratch.0.join("outside.txt"),
        scratch.0.join("root/objects/a/link.txt"),
    )
    .expect("link");
    let page = backend.list("", None, 10).await.expect("list");
    let keys: Vec<&str> = page.keys.iter().map(StorageKey::as_str).collect();
    assert_eq!(keys, ["objects/a/real.txt"]);
}

#[tokio::test]
async fn fs_walk_depth_bound_is_exact() {
    let (backend, scratch) = fs_backend("walk-depth");
    let mut dir = scratch.0.join("root");
    for i in 0..=filesquid_ocfl::storage::MAX_WALK_DEPTH {
        dir.push(format!("d{i}"));
    }
    std::fs::create_dir_all(&dir).expect("deep dirs");
    let err = backend.list("", None, 10).await.expect_err("too deep");
    assert!(matches!(err, StorageError::LimitExceeded { .. }), "{err:?}");
}

#[test]
fn fs_default_staging_is_outside_a_root_that_holds_the_temp_dir() {
    let temp = std::env::temp_dir();
    let staging = filesquid_ocfl::storage::default_staging_dir(&temp).expect("staging");
    let temp = temp.canonicalize().expect("canonical temp");
    assert!(
        !staging.starts_with(&temp),
        "{} inside {}",
        staging.display(),
        temp.display()
    );
}
