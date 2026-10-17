//! C2PA-S-006 / C2PA-S-008: `src/` makes no direct filesystem, network or
//! process call, and only the crypto backend module names a cryptography
//! crate. The scanner is also tested on synthetic text.

#[path = "../support/source_scan.rs"]
mod source_scan;

use source_scan::{
    crypto_findings_outside_backend, io_findings, read_sources, scan_text, Finding,
    CRYPTO_BACKEND_MODULE, CRYPTO_PATTERNS, IO_PATTERNS,
};

#[test]
fn scanner_flags_direct_filesystem_and_network_use() {
    let text = "use std::fs::File;\nlet c = reqwest::Client::new();\n";
    let found = scan_text("lib.rs", text, IO_PATTERNS);
    assert_eq!(
        found,
        [
            Finding {
                file: "lib.rs".into(),
                line: 1,
                pattern: "std::fs"
            },
            Finding {
                file: "lib.rs".into(),
                line: 2,
                pattern: "reqwest"
            },
        ]
    );
}

#[test]
fn scanner_ignores_comment_lines() {
    let text = "//! The core MUST NOT call std::fs.\n    /// no hyper here\n";
    assert!(scan_text("lib.rs", text, IO_PATTERNS).is_empty());
}

#[test]
fn scanner_ignores_block_comments_including_nested_ones() {
    let text = "/* reads std::fs\n   /* nested std::net */ still std::process */\nfn f() {}\n";
    assert!(scan_text("lib.rs", text, IO_PATTERNS).is_empty());
}

#[test]
fn scanner_sees_code_after_a_block_comment_on_the_same_line() {
    let text = "/* note */ let x = std::fs::read(p);\n";
    let found = scan_text("lib.rs", text, IO_PATTERNS);
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].line, found[0].pattern), (1, "std::fs"));
}

#[test]
fn scanner_sees_code_after_a_url_in_a_string() {
    let text = "let u = \"https://x\"; let n = std::net::TcpStream::connect(u);\n";
    let found = scan_text("lib.rs", text, IO_PATTERNS);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].pattern, "std::net");
}

#[test]
fn scanner_flags_grouped_std_imports() {
    for (text, pattern) in [
        ("use std::{fs};\nfn f() { fs::read(\"a\"); }\n", "std::fs"),
        ("use std::{io, net::TcpStream};\n", "std::net"),
        (
            "use std::{\n    collections::HashMap,\n    process as p,\n};\n",
            "std::process",
        ),
        ("use std :: { fs :: File };\n", "std::fs"),
    ] {
        let found = scan_text("lib.rs", text, IO_PATTERNS);
        assert!(
            found.iter().any(|f| f.pattern == pattern),
            "{text:?} must report {pattern}: {found:?}"
        );
    }
}

#[test]
fn scanner_allows_harmless_grouped_imports() {
    let text = "use std::{future::Future, fmt, ops::Range};\n";
    assert!(scan_text("lib.rs", text, IO_PATTERNS).is_empty());
}

#[test]
fn grouped_import_findings_carry_the_line_of_the_group() {
    let text = "fn a() {}\nuse std::{\n    fs,\n};\n";
    let found = scan_text("lib.rs", text, IO_PATTERNS);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 2);
}

#[test]
fn scanner_flags_crypto_crates() {
    let found = scan_text("claim.rs", "use sha2::Sha256;\n", CRYPTO_PATTERNS);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].pattern, "sha2::");
}

#[test]
fn sources_include_the_crypto_backend_module() {
    let files = read_sources().expect("read src");
    assert!(files.iter().any(|(file, _)| file == "lib.rs"));
    assert!(files.iter().any(|(file, _)| file == CRYPTO_BACKEND_MODULE));
}

#[test]
fn core_has_no_direct_io() {
    let found = io_findings().expect("scan");
    assert!(found.is_empty(), "direct I/O in src/: {found:?}");
}

#[test]
fn crypto_crates_stay_behind_the_backend() {
    let found = crypto_findings_outside_backend().expect("scan");
    assert!(
        found.is_empty(),
        "crypto crate outside the backend: {found:?}"
    );
}
