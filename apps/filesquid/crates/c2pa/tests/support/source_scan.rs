//! Source scan for `filesquid-c2pa` (C2PA-S-006, C2PA-S-008).
//!
//! The core MUST NOT touch the filesystem, the network or processes directly,
//! and only the crypto backend module may name a cryptography crate. Line and
//! block comments are removed before matching so documentation can describe
//! the rules; whitespace is removed too, and grouped imports such as
//! `use std::{fs, io};` are expanded.

use std::fs;
use std::path::{Path, PathBuf};

/// Patterns that mean direct I/O, a network client or an async runtime.
pub const IO_PATTERNS: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "tokio",
    "async_std",
    "reqwest",
    "hyper",
    "ureq",
    "isahc",
    "File::open",
    "File::create",
];

/// Patterns that name a cryptography crate.
pub const CRYPTO_PATTERNS: &[&str] = &[
    "sha2::",
    "p256::",
    "p384::",
    "p521::",
    "ecdsa::",
    "ed25519_dalek",
    "rsa::",
    "x509_cert",
    "cms::",
    "ring::",
    "openssl",
    "aws_lc",
];

/// The one module allowed to name cryptography crates, relative to `src/`.
pub const CRYPTO_BACKEND_MODULE: &str = "crypto/rust_crypto.rs";

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

/// One code character kept by [`code_chars`].
#[derive(Debug, Clone, Copy)]
pub struct CodeChar {
    /// The character.
    pub c: char,
    /// 1-based line number.
    pub line: usize,
    /// Whether whitespace or a comment was removed right before it.
    pub gap: bool,
}

/// Return the code characters of `text` with comments and whitespace
/// removed, each with its 1-based line number and whether a gap preceded it.
///
/// Line comments, nested block comments and string literals are tracked, so
/// a `//` inside a string does not hide the code after it. Raw strings are
/// treated as ordinary strings; `src/` does not use `r#"..."#`.
pub fn code_chars(text: &str) -> Vec<CodeChar> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::with_capacity(chars.len());
    let (mut state, mut line, mut i, mut gap) = (Lex::Code, 1usize, 0usize, true);
    while i < chars.len() {
        let (c, next) = (chars[i], chars.get(i + 1).copied());
        let (new_state, consumed, keep) = step(state, c, next);
        if keep && !c.is_whitespace() {
            out.push(CodeChar { c, line, gap });
            gap = false;
        } else {
            gap = true;
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

/// Return the char indices in `code` where `pattern` starts on a path-segment
/// boundary: at the start, after removed whitespace or a comment, or after a
/// character that cannot continue an identifier. So `ring::` matches
/// `ring::digest` but not `std::string::String`.
pub fn boundary_matches(code: &[CodeChar], pattern: &str) -> Vec<usize> {
    let compact: String = code.iter().map(|ch| ch.c).collect();
    compact
        .match_indices(pattern)
        .map(|(byte, _)| compact[..byte].chars().count())
        .filter(|&k| k == 0 || code[k].gap || !is_ident_char(code[k - 1].c))
        .collect()
}

/// Whether `c` can be part of a Rust identifier.
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Return (line, pattern) for each `std` module named inside a grouped
/// `std::{...}` import in `code` (whitespace already removed).
fn grouped_std_imports(code: &[CodeChar]) -> Vec<(usize, &'static str)> {
    let chars: Vec<char> = code.iter().map(|ch| ch.c).collect();
    let mut found = Vec::new();
    for start in boundary_matches(code, "std::{") {
        let open = start + "std::{".len();
        let items = group_items(&chars[open..chars.len().min(open + MAX_GROUP_CHARS)]);
        for item in items {
            for (module, pattern) in STD_GROUP_MODULES {
                let rest = item.strip_prefix(module).unwrap_or("-");
                if rest.is_empty() || rest.starts_with("::") || rest.starts_with("as") {
                    found.push((code[start].line, *pattern));
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

/// Return the patterns from `patterns` found in the code of `text` at a
/// path-segment boundary, ignoring comments and whitespace, including `std`
/// modules named in grouped imports.
pub fn scan_text(file: &str, text: &str, patterns: &[&'static str]) -> Vec<Finding> {
    let code = code_chars(text);
    let mut hits: Vec<(usize, &'static str)> = Vec::new();
    for pattern in patterns {
        for start in boundary_matches(&code, pattern) {
            hits.push((code[start].line, *pattern));
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

/// Every direct I/O or network finding in `src/`.
pub fn io_findings() -> Result<Vec<Finding>, String> {
    Ok(read_sources()?
        .iter()
        .flat_map(|(file, text)| scan_text(file, text, IO_PATTERNS))
        .collect())
}

/// Every cryptography-crate finding in `src/` outside the backend module.
pub fn crypto_findings_outside_backend() -> Result<Vec<Finding>, String> {
    Ok(read_sources()?
        .iter()
        .filter(|(file, _)| file != CRYPTO_BACKEND_MODULE)
        .flat_map(|(file, text)| scan_text(file, text, CRYPTO_PATTERNS))
        .collect())
}
