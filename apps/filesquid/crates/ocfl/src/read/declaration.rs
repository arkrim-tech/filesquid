//! Finding and checking the NAMASTE conformance declaration of an object
//! root or a storage root (OCFL 1.1 sections 3.2 and 4.2).
//!
//! Only files directly in the root count: a key such as `obj/0=data/x` is a
//! file in a directory, not a declaration. This reader is for OCFL 1.1, so a
//! root declared as another version is refused (OCFL-S-011: OCFL 1.0 objects
//! are out of scope).

use super::error::ReadError;
use super::small::{at_key, child, key_in, read_small};
use crate::namaste::{Declaration, DeclarationKind, SpecVersion};
use crate::storage::{StorageBackend, StorageError, StorageKey, MAX_LIST_PAGE, MAX_WALK_ENTRIES};

/// Largest declaration file a reader reads: 64 bytes. The longest one,
/// `ocfl_object_1.1` and a newline, is 16 bytes.
pub const MAX_DECLARATION_BYTES: usize = 64;

/// Most `0=` files directly in one root before it is refused.
const MAX_DECLARATION_ENTRIES: usize = 16;

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
/// # Errors
///
/// [`ReadError::Format`] with E003 (object) or E069 (storage root) unless
/// exactly one declaration of `kind` is there, or
/// [`ReadError::UnsupportedVersion`] when it declares a version other than
/// OCFL 1.1.
pub(crate) fn choose_declaration<'n>(
    dir: &str,
    kind: DeclarationKind,
    names: impl Iterator<Item = &'n str>,
) -> Result<Declaration, ReadError> {
    let found: Vec<Declaration> = names
        .filter_map(Declaration::from_file_name)
        .filter(|declaration| declaration.kind == kind)
        .collect();
    let [declaration] = found.as_slice() else {
        return Err(not_exactly_one(dir, kind, found.len()));
    };
    if declaration.spec != SpecVersion::V1_1 {
        return Err(ReadError::UnsupportedVersion {
            declaration: child(dir, &declaration.file_name()),
        });
    }
    Ok(*declaration)
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
/// [`MAX_DECLARATION_PAGES`] pages; every continuation must move forward.
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
            return Err(not_exactly_one(dir, kind, names.len()));
        }
        match page.next {
            None => return Ok(names),
            Some(next) if after.as_ref().is_some_and(|a| next <= *a) => {
                return Err(StorageError::Backend {
                    reason: format!("listing continuation {next} made no progress"),
                }
                .into())
            }
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

/// The refusal for `count` declarations of `kind` where one is required.
fn not_exactly_one(dir: &str, kind: DeclarationKind, count: usize) -> ReadError {
    let (code, root) = match kind {
        DeclarationKind::Object => ("E003", "an object root"),
        DeclarationKind::StorageRoot => ("E069", "a storage root"),
    };
    ReadError::invalid(
        code,
        dir,
        format!("{root} needs exactly one conformance declaration; found {count}"),
    )
}
