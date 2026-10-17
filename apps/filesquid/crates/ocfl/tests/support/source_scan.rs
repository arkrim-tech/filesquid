//! Source scan for `filesquid-ocfl` (OCFL-S-007): the engine core MUST NOT
//! call `std::fs`, `tokio::fs` or a network client directly. Only the
//! filesystem backend (`src/storage/fs/`) and the path-based helpers
//! (`src/logical_file.rs`) may touch the filesystem.
//!
//! Adapted from `filesquid-c2pa`'s `tests/support/source_scan.rs` (CP-0.1).
//! Module crates may not depend on each other's test code, so the scanner is
//! copied rather than shared; keep the two in step when either changes.
//!
//! Line and block comments are removed before matching, so documentation can
//! describe the rules; whitespace is removed too, and grouped imports such as
//! `use std::{fs, io};` and aliases such as `use std::fs as f;` are caught.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// Patterns that mean direct filesystem, network or process I/O, or an async
/// runtime.
pub const IO_PATTERNS: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "std::os::unix::fs",
    "tokio",
    "async_std",
    "reqwest",
    "hyper",
    "ureq",
    "isahc",
    "File::open",
    "File::create",
    "OpenOptions",
];

/// Files and directories under `src/` allowed to do filesystem I/O.
pub const IO_ALLOWED: &[&str] = &["storage/fs/", "logical_file.rs"];

/// Directory depth cap for the `src/` walk.
const MAX_DEPTH: usize = 8;

/// File-count cap for the `src/` walk.
const MAX_FILES: usize = 512;

/// One forbidden pattern found in a source file.
#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    /// Path relative to `src/`, with `/` separators.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// The pattern that matched.
    pub pattern: &'static str,
}

/// Lexer state while removing comments.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lex {
    /// Ordinary code.
    Code,
    /// Inside `// ...` until the end of the line.
    LineComment,
    /// Inside `/* ... */`, with the nesting depth (Rust block comments nest).
    Block(usize),
    /// Inside a `"..."` string literal.
    Str,
}

/// Return the code characters of `text` with comments and whitespace
/// removed, each paired with its 1-based line number.
///
/// Line comments, nested block comments and string literals are tracked, so
/// a `//` inside a string does not hide the code after it. Raw strings are
/// treated as ordinary strings.
pub fn code_chars(text: &str) -> Vec<(char, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::with_capacity(chars.len());
    let (mut state, mut line, mut i) = (Lex::Code, 1usize, 0usize);
    while i < chars.len() {
        let (c, next) = (chars[i], chars.get(i + 1).copied());
        let (new_state, consumed, keep) = step(state, c, next);
        if keep && !c.is_whitespace() {
            out.push((c, line));
        }
        for consumed_char in &chars[i..i + consumed] {
            line += usize::from(*consumed_char == '\n');
        }
        state = new_state;
        i += consumed;
    }
    out
}

/// One lexer transition: (next state, characters consumed, keep `c`).
fn step(state: Lex, c: char, next: Option<char>) -> (Lex, usize, bool) {
    match (state, c, next) {
        (Lex::Code, '/', Some('/')) => (Lex::LineComment, 2, false),
        (Lex::Code, '/', Some('*')) => (Lex::Block(1), 2, false),
        (Lex::Code, '"', _) => (Lex::Str, 1, true),
        (Lex::Code, _, _) => (Lex::Code, 1, true),
        (Lex::LineComment, '\n', _) => (Lex::Code, 1, false),
        (Lex::LineComment, _, _) => (Lex::LineComment, 1, false),
        (Lex::Block(depth), '/', Some('*')) => (Lex::Block(depth + 1), 2, false),
        (Lex::Block(1), '*', Some('/')) => (Lex::Code, 2, false),
        (Lex::Block(depth), '*', Some('/')) => (Lex::Block(depth - 1), 2, false),
        (Lex::Block(depth), _, _) => (Lex::Block(depth), 1, false),
        (Lex::Str, '\\', Some(_)) => (Lex::Str, 2, true),
        (Lex::Str, '"', _) => (Lex::Code, 1, true),
        (Lex::Str, _, _) => (Lex::Str, 1, true),
    }
}

/// `std` modules a grouped import such as `use std::{fs, io};` can name,
/// with the pattern each one stands for.
const STD_GROUP_MODULES: &[(&str, &str)] = &[
    ("fs", "std::fs"),
    ("net", "std::net"),
    ("process", "std::process"),
];

/// Cap on characters scanned inside one `std::{...}` group.
const MAX_GROUP_CHARS: usize = 4096;

/// Return (line, pattern) for each `std` module named inside a grouped
/// `std::{...}` import in `code` (whitespace already removed), at any
/// nesting level such as `std::{io, fs::File}` or `std::{os::{unix}, fs}`.
fn grouped_std_imports(code: &[(char, usize)]) -> Vec<(usize, &'static str)> {
    let text: String = code.iter().map(|(c, _)| *c).collect();
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    for (byte, _) in text.match_indices("std::{") {
        let start = text[..byte].chars().count();
        let open = start + "std::{".len();
        let items = group_items(&chars[open..chars.len().min(open + MAX_GROUP_CHARS)]);
        for item in items {
            for (module, pattern) in STD_GROUP_MODULES {
                let rest = item.strip_prefix(module).unwrap_or("-");
                if rest.is_empty() || rest.starts_with("::") || rest.starts_with("as") {
                    found.push((code[start].1, *pattern));
                }
            }
        }
    }
    found
}

/// Split the body of a `{...}` group (starting after `{`) into its
/// top-level items, stopping at the matching `}`.
fn group_items(body: &[char]) -> Vec<String> {
    let (mut items, mut current, mut depth) = (Vec::new(), String::new(), 0usize);
    for c in body {
        match c {
            '{' => depth += 1,
            '}' if depth == 0 => break,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                items.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(*c);
    }
    items.push(current);
    items
}

/// Return the patterns from `patterns` found in the code of `text`, ignoring
/// comments and whitespace, including `std` modules named in grouped imports.
pub fn scan_text(file: &str, text: &str, patterns: &[&'static str]) -> Vec<Finding> {
    let code = code_chars(text);
    let compact: String = code.iter().map(|(c, _)| *c).collect();
    let mut hits: Vec<(usize, &'static str)> = Vec::new();
    for pattern in patterns {
        for (byte, _) in compact.match_indices(pattern) {
            hits.push((code[compact[..byte].chars().count()].1, *pattern));
        }
    }
    hits.extend(
        grouped_std_imports(&code)
            .into_iter()
            .filter(|(_, pattern)| patterns.contains(pattern)),
    );
    hits.sort();
    hits.dedup();
    hits.into_iter()
        .map(|(line, pattern)| Finding {
            file: file.to_string(),
            line,
            pattern,
        })
        .collect()
}

/// The crate's `src/` directory.
pub fn src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Read every `.rs` file under `src/` as (relative path, text), bounded by
/// [`MAX_DEPTH`] and [`MAX_FILES`].
pub fn read_sources() -> Result<Vec<(String, String)>, String> {
    let root = src_dir();
    let mut files = Vec::new();
    collect(&root, &root, 0, &mut files)?;
    files.sort();
    Ok(files)
}

/// Walk `dir` for `.rs` files, recursing at most [`MAX_DEPTH`] levels.
fn collect(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("{} is deeper than {MAX_DEPTH}", dir.display()));
    }
    let entries = fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|err| err.to_string())?.path();
        if path.is_dir() {
            collect(root, &path, depth + 1, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            push_source(root, &path, out)?;
        }
    }
    Ok(())
}

/// Read one source file and record it with its `src/`-relative path.
fn push_source(root: &Path, path: &Path, out: &mut Vec<(String, String)>) -> Result<(), String> {
    if out.len() >= MAX_FILES {
        return Err(format!("more than {MAX_FILES} source files"));
    }
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let relative = path
        .strip_prefix(root)
        .map_err(|err| err.to_string())?
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    out.push((relative, text));
    Ok(())
}

/// Whether `file` (relative to `src/`) may do filesystem I/O.
pub fn io_allowed(file: &str) -> bool {
    IO_ALLOWED
        .iter()
        .any(|allowed| file == *allowed || (allowed.ends_with('/') && file.starts_with(allowed)))
}

/// Every direct I/O, network or runtime finding in `src/` outside the
/// allowed files, and the number of files scanned.
pub fn io_findings_outside_allowed() -> Result<(Vec<Finding>, usize), String> {
    let sources = read_sources()?;
    let scanned: Vec<_> = sources.iter().filter(|(f, _)| !io_allowed(f)).collect();
    let findings = scanned
        .iter()
        .flat_map(|(file, text)| scan_text(file, text, IO_PATTERNS))
        .collect();
    Ok((findings, scanned.len()))
}
