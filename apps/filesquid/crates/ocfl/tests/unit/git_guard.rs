//! OCFL Git-guard unit tests (OCFL-S-001, ADR 016 layout constraints).
//!
//! The engine guard runs over the in-memory backend, so these tests need no
//! Cargo features. The path-based helper is checked when `fs` is on.

use filesquid_ocfl::storage::{complete_immediately, MemoryBackend, StorageBackend, StorageKey};
use filesquid_ocfl::{check_storage_root, reject_git_in_root, OcflError};

/// The OCFL 1.1 object declaration file name.
const OBJECT_DECLARATION: &str = "0=ocfl_object_1.1";

/// An in-memory storage root holding an empty object at each of `keys`.
fn root_with(keys: &[&str]) -> MemoryBackend {
    let backend = MemoryBackend::new();
    for raw in keys {
        let key = StorageKey::new(raw).expect("valid key");
        complete_immediately(backend.put(&key, b"x"))
            .expect("ready")
            .expect("put");
    }
    backend
}

/// Run the full storage-root guard on `backend`.
fn check(backend: &MemoryBackend) -> Result<(), OcflError> {
    complete_immediately(check_storage_root(backend)).expect("ready")
}

/// The error for a `.git` held by the directory `root`.
fn git_at(root: &str) -> Result<(), OcflError> {
    Err(OcflError::GitDirectoryForbidden {
        root: root.to_string(),
    })
}

/// A small valid layout: storage declaration and two objects.
const LAYOUT: [&str; 5] = [
    "0=ocfl_1.1",
    "ab/obj-1/0=ocfl_object_1.1",
    "ab/obj-1/inventory.json",
    "cd/obj-2/0=ocfl_object_1.1",
    "cd/obj-2/v1/content/a.txt",
];

#[test]
fn a_clean_storage_root_passes() {
    assert_eq!(check(&root_with(&LAYOUT)), Ok(()));
}

#[test]
fn an_empty_storage_root_passes() {
    assert_eq!(check(&MemoryBackend::new()), Ok(()));
}

#[test]
fn git_in_the_storage_root_is_rejected() {
    let mut keys = LAYOUT.to_vec();
    keys.push(".git/HEAD");
    assert_eq!(check(&root_with(&keys)), git_at(""));
}

#[test]
fn a_git_file_in_the_storage_root_is_rejected() {
    let mut keys = LAYOUT.to_vec();
    keys.push(".git");
    assert_eq!(check(&root_with(&keys)), git_at(""));
}

#[test]
fn git_in_an_object_root_is_rejected() {
    let mut keys = LAYOUT.to_vec();
    keys.push("cd/obj-2/.git/config");
    assert_eq!(check(&root_with(&keys)), git_at("cd/obj-2"));
}

#[test]
fn git_in_a_storage_hierarchy_directory_is_rejected() {
    let mut keys = LAYOUT.to_vec();
    keys.push("ab/.git/HEAD");
    assert_eq!(check(&root_with(&keys)), git_at("ab"));
}

#[test]
fn git_inside_object_content_is_preserved_content() {
    let mut keys = LAYOUT.to_vec();
    keys.push("cd/obj-2/v1/content/repo/.git/HEAD");
    assert_eq!(check(&root_with(&keys)), Ok(()));
}

#[test]
fn a_declaration_inside_content_is_not_an_object_root() {
    let nested = format!("cd/obj-2/v1/content/inner/{OBJECT_DECLARATION}");
    let mut keys = LAYOUT.to_vec();
    keys.push(&nested);
    keys.push("cd/obj-2/v1/content/inner/.git/HEAD");
    assert_eq!(check(&root_with(&keys)), Ok(()));
}

#[test]
fn reject_git_in_root_checks_one_root() {
    let backend = root_with(&["obj/.git/HEAD", "other/inventory.json"]);
    let at = |root: &str| complete_immediately(reject_git_in_root(&backend, root)).expect("ready");
    assert_eq!(at("obj"), git_at("obj"));
    assert_eq!(at("obj/"), git_at("obj"));
    assert_eq!(at("other"), Ok(()));
    assert_eq!(at(""), Ok(()));
}

#[test]
fn reject_git_in_root_refuses_an_escaping_root() {
    let backend = MemoryBackend::new();
    let result = complete_immediately(reject_git_in_root(&backend, "../up")).expect("ready");
    assert!(matches!(result, Err(OcflError::Storage(_))), "{result:?}");
}

#[test]
fn object_stores_have_no_links_or_empty_directories() {
    let backend = root_with(&LAYOUT);
    let checked = complete_immediately(backend.check_hierarchy("")).expect("ready");
    assert_eq!(checked, Ok(()));
}

#[cfg(feature = "fs")]
mod path_helper {
    use filesquid_ocfl::{reject_git_in_ocfl_root, OcflError};
    use std::fs;

    /// A scratch directory under Cargo's per-target temporary directory,
    /// removed on drop.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                .join(format!("ocfl-git-guard-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("scratch");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn forbidden(dir: &std::path::Path) -> Result<(), OcflError> {
        Err(OcflError::GitDirectoryForbidden {
            root: dir.display().to_string(),
        })
    }

    #[test]
    fn git_directory_is_rejected() {
        let scratch = Scratch::new("dir");
        fs::create_dir_all(scratch.0.join(".git")).expect("git dir");
        assert_eq!(reject_git_in_ocfl_root(&scratch.0), forbidden(&scratch.0));
    }

    #[test]
    fn git_file_is_rejected() {
        let scratch = Scratch::new("file");
        fs::write(scratch.0.join(".git"), "gitdir: ../elsewhere").expect("git file");
        assert_eq!(reject_git_in_ocfl_root(&scratch.0), forbidden(&scratch.0));
    }

    #[cfg(unix)]
    #[test]
    fn dangling_git_link_is_rejected() {
        let scratch = Scratch::new("link");
        std::os::unix::fs::symlink(scratch.0.join("missing"), scratch.0.join(".git"))
            .expect("dangling link");
        assert_eq!(reject_git_in_ocfl_root(&scratch.0), forbidden(&scratch.0));
    }

    #[test]
    fn a_root_without_git_passes() {
        let scratch = Scratch::new("clean");
        assert_eq!(reject_git_in_ocfl_root(&scratch.0), Ok(()));
    }
}
