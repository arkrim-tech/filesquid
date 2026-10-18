//! Storage-hierarchy guard on a real directory (OCFL-S-001, ADR 016 layout
//! constraints): soft links, hard links, empty directories and other
//! non-regular entries are refused, and so is a `.git` directory, even an
//! empty one.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use filesquid_ocfl::storage::{
    complete_immediately, FsBackend, StorageBackend, StorageError, StorageKey,
};
use filesquid_ocfl::{check_storage_root, OcflError};

/// A scratch directory under Cargo's per-target temporary directory, removed
/// on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("ocfl-hierarchy-{label}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A filesystem backend with a small valid layout written through it.
fn fs_root(label: &str) -> (FsBackend, Scratch) {
    let scratch = Scratch::new(label);
    let backend =
        FsBackend::new(scratch.0.join("root"), scratch.0.join("staging")).expect("fs backend");
    for raw in [
        "0=ocfl_1.1",
        "obj/0=ocfl_object_1.1",
        "obj/v1/content/a.txt",
    ] {
        let key = StorageKey::new(raw).expect("key");
        complete_immediately(backend.put(&key, b"x"))
            .expect("ready")
            .expect("put");
    }
    (backend, scratch)
}

/// Run the backend's hierarchy check on the whole root.
fn hierarchy(backend: &FsBackend) -> Result<(), StorageError> {
    complete_immediately(backend.check_hierarchy("")).expect("ready")
}

/// Run the full engine guard.
fn guard(backend: &FsBackend) -> Result<(), OcflError> {
    complete_immediately(check_storage_root(backend)).expect("ready")
}

fn root(backend: &FsBackend) -> &Path {
    backend.root()
}

#[test]
fn a_root_written_through_the_backend_passes() {
    let (backend, _scratch) = fs_root("clean");
    assert_eq!(hierarchy(&backend), Ok(()));
    assert_eq!(guard(&backend), Ok(()));
}

#[test]
fn a_root_that_was_never_written_passes() {
    let scratch = Scratch::new("unwritten");
    let backend =
        FsBackend::new(scratch.0.join("root"), scratch.0.join("staging")).expect("fs backend");
    assert_eq!(hierarchy(&backend), Ok(()));
    assert_eq!(guard(&backend), Ok(()));
}

#[test]
fn deletes_leave_no_empty_directories() {
    let (backend, _scratch) = fs_root("delete");
    let key = StorageKey::new("obj/v1/content/a.txt").expect("key");
    complete_immediately(backend.delete(&key))
        .expect("ready")
        .expect("delete");
    assert_eq!(hierarchy(&backend), Ok(()));
}

#[test]
fn an_empty_directory_is_rejected() {
    let (backend, _scratch) = fs_root("empty");
    fs::create_dir_all(root(&backend).join("obj/v1/content/empty")).expect("dir");
    assert_eq!(
        hierarchy(&backend),
        Err(StorageError::EmptyDirectoryForbidden {
            key: "obj/v1/content/empty".to_string()
        })
    );
    assert!(matches!(guard(&backend), Err(OcflError::Storage(_))));
}

#[test]
fn an_empty_git_directory_is_rejected() {
    let (backend, _scratch) = fs_root("empty-git");
    fs::create_dir_all(root(&backend).join(".git")).expect("dir");
    assert_eq!(
        hierarchy(&backend),
        Err(StorageError::EmptyDirectoryForbidden {
            key: ".git".to_string()
        })
    );
}

#[test]
fn a_populated_git_directory_is_rejected() {
    let (backend, _scratch) = fs_root("git");
    fs::create_dir_all(root(&backend).join("obj/.git")).expect("dir");
    fs::write(root(&backend).join("obj/.git/HEAD"), "ref: refs/heads/main").expect("head");
    assert_eq!(
        guard(&backend),
        Err(OcflError::GitDirectoryForbidden {
            root: "obj".to_string()
        })
    );
}

#[test]
fn a_root_that_is_a_regular_file_is_rejected() {
    let scratch = Scratch::new("file-root");
    let file = scratch.0.join("root");
    fs::write(&file, b"not a directory").expect("file root");
    let backend = FsBackend::new(&file, scratch.0.join("staging")).expect("fs backend");
    let checked = hierarchy(&backend);
    assert!(
        matches!(
            checked,
            Err(StorageError::Io {
                kind: std::io::ErrorKind::NotADirectory,
                ..
            })
        ),
        "{checked:?}"
    );
    assert!(matches!(guard(&backend), Err(OcflError::Storage(_))));
}

#[test]
fn a_prefix_below_a_regular_file_is_rejected() {
    let (backend, _scratch) = fs_root("below-file");
    let checked = complete_immediately(backend.check_hierarchy("obj/v1/content/a.txt/child/"))
        .expect("ready");
    assert!(
        matches!(
            checked,
            Err(StorageError::Io {
                kind: std::io::ErrorKind::NotADirectory,
                ..
            })
        ),
        "{checked:?}"
    );
}

#[test]
fn a_missing_prefix_passes() {
    let (backend, _scratch) = fs_root("missing-prefix");
    let checked = complete_immediately(backend.check_hierarchy("nowhere/deeper/")).expect("ready");
    assert_eq!(checked, Ok(()));
}

#[test]
fn a_hierarchy_check_below_a_prefix_skips_the_rest() {
    let (backend, _scratch) = fs_root("prefix");
    fs::create_dir_all(root(&backend).join("other/empty")).expect("dir");
    let checked = complete_immediately(backend.check_hierarchy("obj/")).expect("ready");
    assert_eq!(checked, Ok(()));
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn a_symlinked_file_is_rejected() {
        let (backend, scratch) = fs_root("link-file");
        fs::write(scratch.0.join("outside"), b"x").expect("outside");
        symlink(
            scratch.0.join("outside"),
            root(&backend).join("obj/v1/content/b.txt"),
        )
        .expect("link");
        assert_eq!(
            hierarchy(&backend),
            Err(StorageError::SymlinkForbidden {
                key: "obj/v1/content/b.txt".to_string()
            })
        );
    }

    #[test]
    fn a_symlinked_directory_is_rejected() {
        let (backend, scratch) = fs_root("link-dir");
        fs::create_dir_all(scratch.0.join("elsewhere")).expect("dir");
        symlink(scratch.0.join("elsewhere"), root(&backend).join("obj/v2")).expect("link");
        assert_eq!(
            hierarchy(&backend),
            Err(StorageError::SymlinkForbidden {
                key: "obj/v2".to_string()
            })
        );
    }

    #[test]
    fn a_link_with_a_reserved_name_is_rejected() {
        let (backend, scratch) = fs_root("link-reserved");
        fs::write(scratch.0.join("outside"), b"x").expect("outside");
        symlink(scratch.0.join("outside"), root(&backend).join(".fsquid-x")).expect("link");
        assert_eq!(
            hierarchy(&backend),
            Err(StorageError::SymlinkForbidden {
                key: ".fsquid-x".to_string()
            })
        );
    }

    #[test]
    fn links_and_empty_directories_inside_a_preserved_git_are_rejected() {
        let (backend, scratch) = fs_root("preserved-git");
        let git = root(&backend).join("obj/v1/content/repo/.git");
        fs::create_dir_all(&git).expect("git dir");
        fs::write(git.join("HEAD"), "ref: refs/heads/main").expect("head");
        assert_eq!(guard(&backend), Ok(()));
        fs::create_dir_all(git.join("refs")).expect("empty refs");
        assert_eq!(
            hierarchy(&backend),
            Err(StorageError::EmptyDirectoryForbidden {
                key: "obj/v1/content/repo/.git/refs".to_string()
            })
        );
        fs::remove_dir(git.join("refs")).expect("remove refs");
        fs::write(scratch.0.join("outside"), b"x").expect("outside");
        symlink(scratch.0.join("outside"), git.join("config")).expect("link");
        assert_eq!(
            hierarchy(&backend),
            Err(StorageError::SymlinkForbidden {
                key: "obj/v1/content/repo/.git/config".to_string()
            })
        );
    }

    #[test]
    fn a_dangling_symlink_is_rejected() {
        let (backend, scratch) = fs_root("link-dangling");
        symlink(scratch.0.join("missing"), root(&backend).join("dangling")).expect("link");
        assert_eq!(
            hierarchy(&backend),
            Err(StorageError::SymlinkForbidden {
                key: "dangling".to_string()
            })
        );
    }

    #[test]
    fn a_hard_link_is_rejected() {
        let (backend, scratch) = fs_root("hard");
        fs::hard_link(
            root(&backend).join("obj/v1/content/a.txt"),
            scratch.0.join("outside-copy"),
        )
        .expect("hard link");
        assert_eq!(
            hierarchy(&backend),
            Err(StorageError::HardLinkForbidden {
                key: "obj/v1/content/a.txt".to_string()
            })
        );
        assert!(matches!(guard(&backend), Err(OcflError::Storage(_))));
    }

    #[test]
    fn a_fifo_is_rejected() {
        let (backend, _scratch) = fs_root("fifo");
        let fifo = root(&backend).join("obj/pipe");
        let made = std::process::Command::new("mkfifo").arg(&fifo).status();
        if !made.is_ok_and(|status| status.success()) {
            eprintln!("mkfifo unavailable; skipping");
            return;
        }
        let checked = hierarchy(&backend);
        assert!(
            matches!(checked, Err(StorageError::Io { ref key, .. }) if key == "obj/pipe"),
            "{checked:?}"
        );
    }

    #[test]
    fn a_symlinked_prefix_is_rejected() {
        let (backend, scratch) = fs_root("link-prefix");
        fs::create_dir_all(scratch.0.join("elsewhere")).expect("dir");
        symlink(scratch.0.join("elsewhere"), root(&backend).join("linked")).expect("link");
        let checked = complete_immediately(backend.check_hierarchy("linked/")).expect("ready");
        assert!(
            matches!(checked, Err(StorageError::SymlinkForbidden { .. })),
            "{checked:?}"
        );
    }
}
