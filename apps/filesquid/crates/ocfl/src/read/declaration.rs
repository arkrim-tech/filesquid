//! Finding and checking the NAMASTE conformance declaration of an object
//! root or a storage root (OCFL 1.1 sections 3.2 and 4.2).

use super::error::ReadError;
use super::small::{at_key, child, key_in, read_small};
use crate::namaste::{Declaration, DeclarationKind};
use crate::storage::{StorageBackend, StorageKey};

/// Largest declaration file a reader reads: 64 bytes. The longest one,
/// `ocfl_object_1.1` and a newline, is 16 bytes.
pub const MAX_DECLARATION_BYTES: usize = 64;

/// Most `0=` entries looked at in one root before it is refused.
const MAX_DECLARATION_ENTRIES: usize = 16;

/// Find the one declaration of `kind` directly in `dir` and check its
/// contents.
///
/// # Errors
///
/// [`ReadError::Format`] with E003 (object) or E069 (storage root) unless
/// exactly one declaration of `kind` is there, E007 or E080 for wrong
/// contents, [`crate::inventory::FormatError::TooLarge`] above
/// [`MAX_DECLARATION_BYTES`], or a backend error.
pub(crate) async fn read_declaration<B: StorageBackend>(
    backend: &B,
    dir: &str,
    kind: DeclarationKind,
) -> Result<Declaration, ReadError> {
    let found = find_declarations(backend, dir, kind).await?;
    let [declaration] = found.as_slice() else {
        return Err(not_exactly_one(dir, kind, found.len()));
    };
    let key = key_in(dir, &declaration.file_name())?;
    let bytes = read_small(backend, &key, MAX_DECLARATION_BYTES, "declaration").await?;
    declaration
        .check_contents(&bytes)
        .map_err(|err| at_key(err, &key))?;
    Ok(*declaration)
}

/// The declarations of `kind` among the files directly in `dir`.
async fn find_declarations<B: StorageBackend>(
    backend: &B,
    dir: &str,
    kind: DeclarationKind,
) -> Result<Vec<Declaration>, ReadError> {
    let page = backend
        .list(&child(dir, "0="), None, MAX_DECLARATION_ENTRIES)
        .await?;
    if page.next.is_some() {
        return Err(not_exactly_one(dir, kind, MAX_DECLARATION_ENTRIES + 1));
    }
    Ok(page
        .keys
        .iter()
        .filter_map(|key| file_name_in(dir, key))
        .filter_map(Declaration::from_file_name)
        .filter(|declaration| declaration.kind == kind)
        .collect())
}

/// The name of `key` when it is a file directly in `dir`.
fn file_name_in<'k>(dir: &str, key: &'k StorageKey) -> Option<&'k str> {
    let name = match dir {
        "" => key.as_str(),
        _ => key.as_str().strip_prefix(dir)?.strip_prefix('/')?,
    };
    (!name.contains('/')).then_some(name)
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
