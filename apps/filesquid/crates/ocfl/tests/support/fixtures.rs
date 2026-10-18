//! Where the pinned OCFL 1.1 community fixtures are (OCFL-S-011).
//!
//! The fixtures are committed under `tests/fixtures/ocfl/1.1`
//! (`OCFL/fixtures` commit `d323fc207126becec066998b43408d998bf0fb1b`), so
//! every test reads them offline. `FILESQUID_OCFL_FIXTURES` may name another
//! copy of a `1.1` tree, for example to check a new pin before it is
//! committed. A missing tree fails the test: conformance is never skipped.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use filesquid_ocfl::storage::FsBackend;

/// The upstream commit the committed fixtures are copied from.
pub const PINNED_COMMIT: &str = "d323fc207126becec066998b43408d998bf0fb1b";

/// The object groups of the `1.1` tree with their object counts at the
/// pinned commit.
pub const PINNED_GROUPS: [(&str, usize); 3] = [
    ("good-objects", 12),
    ("warn-objects", 13),
    ("bad-objects", 55),
];

/// The committed fixtures directory (holding `NOTICE.md` and `1.1/`).
pub fn committed_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ocfl")
}

/// The `1.1` tree the tests read: `FILESQUID_OCFL_FIXTURES` when set,
/// otherwise the committed copy. Panics when it is not a `1.1` tree.
pub fn tree() -> PathBuf {
    let dir = std::env::var_os("FILESQUID_OCFL_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| committed_dir().join("1.1"));
    assert!(
        dir.join("good-objects").is_dir(),
        "{} is not the OCFL 1.1 fixture tree; the pinned fixtures are \
         committed under tests/fixtures/ocfl/1.1",
        dir.display()
    );
    dir
}

/// A read-only backend over one group directory (such as `good-objects`),
/// staging in the test target directory under `tag`.
pub fn group_backend(group: &Path, tag: &str) -> FsBackend {
    let name = group.file_name().expect("group name").to_string_lossy();
    let staging = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{name}"));
    FsBackend::new(group, staging).expect("fs backend")
}

/// The object directory names in `group`, sorted.
pub fn objects(group: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(group)
        .expect("list fixtures")
        .map(|entry| entry.expect("entry"))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The validation codes a fixture name starts with, such as `E092` and
/// `E093` for `E092_E093_content_path_does_not_exist`.
pub fn prefix_codes(name: &str) -> Vec<&str> {
    let is_code = |part: &&str| {
        part.len() == 4
            && matches!(part.as_bytes()[0], b'E' | b'W')
            && part[1..].bytes().all(|b| b.is_ascii_digit())
    };
    name.split('_').take_while(is_code).collect()
}
