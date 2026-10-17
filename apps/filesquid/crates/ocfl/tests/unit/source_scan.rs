//! The OCFL-S-007 source scan catches direct, grouped and aliased `std`
//! imports and ignores comments.

#[path = "../support/source_scan.rs"]
mod source_scan;

use source_scan::{io_allowed, scan_text, IO_PATTERNS};

/// The patterns found in `text`, in order.
fn patterns(text: &str) -> Vec<&'static str> {
    scan_text("x.rs", text, IO_PATTERNS)
        .into_iter()
        .map(|f| f.pattern)
        .collect()
}

#[test]
fn direct_paths_are_found() {
    assert_eq!(patterns("fn f() { std::fs::read(\"a\"); }"), ["std::fs"]);
    assert_eq!(patterns("use std :: net :: TcpStream;"), ["std::net"]);
}

#[test]
fn grouped_imports_are_found() {
    assert_eq!(patterns("use std::{fs, io};"), ["std::fs"]);
    assert_eq!(patterns("use std::{io::Read, fs::File};"), ["std::fs"]);
    assert_eq!(
        patterns("use std::{\n    io,\n    net::{TcpStream},\n};"),
        ["std::net"]
    );
}

#[test]
fn aliased_imports_are_found() {
    assert_eq!(patterns("use std::fs as disk;"), ["std::fs"]);
    assert_eq!(patterns("use std::{fs as disk, io};"), ["std::fs"]);
}

#[test]
fn comments_are_ignored_and_line_numbers_kept() {
    assert!(patterns("// std::fs is forbidden\n/* std::net */ fn f() {}").is_empty());
    let found = scan_text("x.rs", "\n\nuse std::{io, fs};", IO_PATTERNS);
    assert_eq!(found[0].line, 3);
}

#[test]
fn harmless_names_are_not_flagged() {
    assert!(patterns("use std::{io, fmt, fsync_policy};").is_empty());
    assert!(patterns("use crate::storage::fs::FsBackend;").is_empty());
}

#[test]
fn only_the_fs_backend_and_path_helpers_may_do_io() {
    assert!(io_allowed("storage/fs/mod.rs"));
    assert!(io_allowed("storage/fs/files.rs"));
    assert!(io_allowed("logical_file.rs"));
    assert!(!io_allowed("storage/memory.rs"));
    assert!(!io_allowed("storage/fs.rs.bak"));
}
