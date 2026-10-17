//! The path-based logical-file helpers used by `filesquid-metadata` now go
//! through the local filesystem storage backend (OCFL-S-007) and keep their
//! Git guard (OCFL-S-001) and error contract.

use std::path::PathBuf;

use filesquid_ocfl::{read_logical_file, write_logical_file, OcflError, WriteError};

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("ocfl-logical-{label}-{}", std::process::id()));
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

#[test]
fn write_then_read_round_trips_on_a_real_directory() {
    let scratch = Scratch::new("round-trip");
    write_logical_file(&scratch.0, "sub/metadata.md", b"---\ntitle: x\n---\n").expect("write");
    let on_disk = std::fs::read(scratch.0.join("sub/metadata.md")).expect("plain file");
    assert_eq!(on_disk, b"---\ntitle: x\n---\n");
    let back = read_logical_file(&scratch.0, "sub/metadata.md").expect("read");
    assert_eq!(back, on_disk);
}

#[test]
fn large_files_round_trip_through_bounded_chunks() {
    let scratch = Scratch::new("large");
    let data: Vec<u8> = (0..(9 * 1024 * 1024 + 3))
        .map(|i| (i % 253) as u8)
        .collect();
    write_logical_file(&scratch.0, "big.bin", &data).expect("write");
    assert_eq!(
        read_logical_file(&scratch.0, "big.bin").expect("read"),
        data
    );
}

#[test]
fn missing_file_is_io_not_found() {
    let scratch = Scratch::new("missing");
    match read_logical_file(&scratch.0, "metadata.md") {
        Err(WriteError::Io(err)) => assert_eq!(err.kind(), std::io::ErrorKind::NotFound),
        other => panic!("expected Io(NotFound), got {other:?}"),
    }
    let message = read_logical_file(&scratch.0, "metadata.md")
        .expect_err("missing")
        .to_string();
    assert!(
        message.starts_with("logical-file I/O failed"),
        "a failed read must not be labelled as a write: {message}"
    );
}

#[test]
fn escaping_paths_are_rejected() {
    let scratch = Scratch::new("escape");
    for path in ["", "../x", "/abs", "a/../../x"] {
        assert!(
            matches!(
                write_logical_file(&scratch.0, path, b"x"),
                Err(WriteError::InvalidPath)
            ),
            "{path:?}"
        );
        assert!(
            matches!(
                read_logical_file(&scratch.0, path),
                Err(WriteError::InvalidPath)
            ),
            "{path:?}"
        );
    }
}

#[test]
fn git_directory_blocks_reads_and_writes() {
    let scratch = Scratch::new("git");
    std::fs::create_dir_all(scratch.0.join(".git")).expect("git dir");
    assert!(matches!(
        write_logical_file(&scratch.0, "metadata.md", b"x"),
        Err(WriteError::Git(OcflError::GitDirectoryForbidden))
    ));
    assert!(matches!(
        read_logical_file(&scratch.0, "metadata.md"),
        Err(WriteError::Git(OcflError::GitDirectoryForbidden))
    ));
}

#[test]
fn invalid_paths_are_rejected_before_the_git_guard() {
    let scratch = Scratch::new("validate-first");
    std::fs::create_dir_all(scratch.0.join(".git")).expect("git dir");
    for path in ["../x", "", "/abs"] {
        assert!(
            matches!(
                write_logical_file(&scratch.0, path, b"x"),
                Err(WriteError::InvalidPath)
            ),
            "{path:?}"
        );
    }
}

#[test]
fn names_containing_two_dots_are_allowed() {
    let scratch = Scratch::new("two-dots");
    write_logical_file(&scratch.0, "foo..bar", b"ok").expect("write");
    assert_eq!(
        read_logical_file(&scratch.0, "foo..bar").expect("read"),
        b"ok"
    );
}
