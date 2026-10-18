//! Finding and checking the NAMASTE conformance declaration of an object
//! root or a storage root (OCFL 1.1 sections 3.2 and 4.2).
//!
//! Only files directly in the root count: a key such as `obj/0=data/x` is a
//! file in a directory, not a declaration. This reader is for OCFL 1.1, so a
//! root declared as another version is refused (OCFL-S-011: OCFL 1.0 objects
//! are out of scope).

use super::error::ReadError;
use super::small::{at_key, child, key_in, read_small};
use crate::guard::next_start;
use crate::namaste::{Declaration, DeclarationKind, SpecVersion, MAX_DECLARATION_ENTRIES};
use crate::storage::{StorageBackend, StorageError, StorageKey, MAX_LIST_PAGE, MAX_WALK_ENTRIES};

/// Largest declaration file a reader reads: 64 bytes. The longest one,
/// `ocfl_object_1.1` and a newline, is 16 bytes.
pub const MAX_DECLARATION_BYTES: usize = 64;

/// Most listing pages read while looking for declarations: enough for the
/// largest listing a backend walks.
const MAX_DECLARATION_PAGES: usize = MAX_WALK_ENTRIES / MAX_LIST_PAGE + 1;

/// Find the one OCFL 1.1 declaration of `kind` directly in `dir` and check
/// its contents.
///
/// # Errors
///
/// As [`choose_declaration`] and [`check_declaration`], or a backend error.
pub(crate) async fn read_declaration<B: StorageBackend>(
    backend: &B,
    dir: &str,
    kind: DeclarationKind,
) -> Result<Declaration, ReadError> {
    let names = direct_declaration_names(backend, dir, kind).await?;
    let declaration = choose_declaration(dir, kind, names.iter().map(String::as_str))?;
    check_declaration(backend, dir, &declaration).await?;
    Ok(declaration)
}

/// Pick the one declaration of `kind` among `names`, the names of the files
/// directly in `dir`. Pure.
///
/// Every `0=ocfl_object_<version>` name (object) or `0=ocfl_<version>` name
/// (storage root) is a declaration of `kind`, whatever its version, so a
/// root declared as an unknown version is refused as unsupported rather
/// than as having no declaration.
///
/// # Errors
///
/// [`ReadError::Format`] with E003 (object) or E069 (storage root) unless
/// exactly one declaration of `kind` is there or when its version is not a
/// version number; [`ReadError::UnsupportedVersion`] when it declares a
/// version other than OCFL 1.1.
pub(crate) fn choose_declaration<'n>(
    dir: &str,
    kind: DeclarationKind,
    names: impl Iterator<Item = &'n str>,
) -> Result<Declaration, ReadError> {
    let found: Vec<&str> = names
        .filter(|name| declared_version(name, kind).is_some())
        .collect();
    let [name] = found.as_slice() else {
        return Err(not_exactly_one(dir, kind, found.len()));
    };
    let supported = Declaration::from_file_name(name)
        .filter(|declaration| declaration.kind == kind && declaration.spec == SpecVersion::V1_1);
    if let Some(declaration) = supported {
        return Ok(declaration);
    }
    if declared_version(name, kind).is_some_and(is_version_number) {
        return Err(ReadError::UnsupportedVersion {
            declaration: child(dir, name),
        });
    }
    Err(ReadError::invalid(
        not_exactly_one_code(kind),
        child(dir, name),
        "the declaration does not name an OCFL version",
    ))
}

/// The version text a declaration name of `kind` carries, such as `1.1` for
/// `0=ocfl_object_1.1`, or `None` when `name` is not one.
fn declared_version(name: &str, kind: DeclarationKind) -> Option<&str> {
    match kind {
        DeclarationKind::Object => name.strip_prefix("0=ocfl_object_"),
        DeclarationKind::StorageRoot => name
            .strip_prefix("0=ocfl_")
            .filter(|rest| !rest.starts_with("object_")),
    }
}

/// Whether `version` is `MAJOR.MINOR` in ASCII digits, such as `2.0`.
fn is_version_number(version: &str) -> bool {
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    version
        .split_once('.')
        .is_some_and(|(major, minor)| digits(major) && digits(minor))
}

/// Read `declaration`'s file in `dir` and check its exact contents.
///
/// # Errors
///
/// [`ReadError::Format`] with E007 (object) or E080 (storage root) for
/// wrong contents, [`crate::inventory::FormatError::TooLarge`] above
/// [`MAX_DECLARATION_BYTES`], or a backend error.
pub(crate) async fn check_declaration<B: StorageBackend>(
    backend: &B,
    dir: &str,
    declaration: &Declaration,
) -> Result<(), ReadError> {
    let key = key_in(dir, &declaration.file_name())?;
    let bytes = read_small(backend, &key, MAX_DECLARATION_BYTES, "declaration").await?;
    declaration
        .check_contents(&bytes)
        .map_err(|err| at_key(err, &key))
}

/// The names of the `0=` files directly in `dir`, paging past keys in
/// subdirectories (listings are recursive). At most
/// [`MAX_DECLARATION_PAGES`] pages. Each continuation is checked as the
/// storage-root guard checks it ([`next_start`]): it must be the page's last
/// key and move forward, so a faulty listing cannot skip a declaration.
async fn direct_declaration_names<B: StorageBackend>(
    backend: &B,
    dir: &str,
    kind: DeclarationKind,
) -> Result<Vec<String>, ReadError> {
    let prefix = child(dir, "0=");
    let mut names = Vec::new();
    let mut after: Option<StorageKey> = None;
    for _ in 0..MAX_DECLARATION_PAGES {
        let page = backend.list(&prefix, after.as_ref(), MAX_LIST_PAGE).await?;
        names.extend(page.keys.iter().filter_map(|k| file_name_in(dir, k)));
        if names.len() > MAX_DECLARATION_ENTRIES {
            return Err(too_many_declarations(dir, kind, names.len()));
        }
        match next_start(&page, after.as_ref())? {
            None => return Ok(names),
            Some(next) => after = Some(next),
        }
    }
    Err(StorageError::LimitExceeded {
        what: "listing pages read for declarations",
        limit: MAX_DECLARATION_PAGES as u64,
    }
    .into())
}

/// The name of `key` when it is a file directly in `dir`.
fn file_name_in(dir: &str, key: &StorageKey) -> Option<String> {
    let name = match dir {
        "" => key.as_str(),
        _ => key.as_str().strip_prefix(dir)?.strip_prefix('/')?,
    };
    (!name.contains('/')).then(|| name.to_string())
}

/// The OCFL code for a root without exactly one declaration of `kind`.
fn not_exactly_one_code(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::Object => "E003",
        DeclarationKind::StorageRoot => "E069",
    }
}

/// What a root of `kind` is called in messages.
fn root_name(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::Object => "an object root",
        DeclarationKind::StorageRoot => "a storage root",
    }
}

/// The refusal for `count` declarations of `kind` where one is required.
fn not_exactly_one(dir: &str, kind: DeclarationKind, count: usize) -> ReadError {
    ReadError::invalid(
        not_exactly_one_code(kind),
        dir,
        format!(
            "{} needs exactly one conformance declaration; found {count}",
            root_name(kind)
        ),
    )
}

/// The refusal for a root with `count` `0=` files, more than
/// [`MAX_DECLARATION_ENTRIES`]: it cannot have exactly one declaration, and
/// a reader does not hold that many names.
pub(crate) fn too_many_declarations(dir: &str, kind: DeclarationKind, count: usize) -> ReadError {
    ReadError::invalid(
        not_exactly_one_code(kind),
        dir,
        format!(
            "{} needs exactly one conformance declaration; found {count} 0= files, \
             more than {MAX_DECLARATION_ENTRIES}",
            root_name(kind)
        ),
    )
}
