//! Where each imported file goes in its destination collection (Task DE-1.7; DE-S-019,
//! API-S-011 **Same logical path**, API-S-036,
//! [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md)
//! Decision 12).
//!
//! Each picked source keeps its name, after the API-S-005 safe transformation and in NFC,
//! beneath the destination folder; a picked folder keeps its structure below it. When the
//! source's path is already taken in the collection (a deleted file's path included), the
//! source gets the first free number beside it (`Holidays-1`, `photo-1.jpg`), decided once
//! when the job makes its manifest, so all its files land together. A source whose 9,999
//! numbers are all taken (`path_suffix_exhausted`), or a name still over 255 bytes after the
//! safe transformation or once numbered (`name_too_long`), quarantines its files with that
//! code before any byte is read. Nothing here names a path in an answer or an error.

use super::index::ImportIndex;
use super::plans::ImportPlan;
use filesquid_api::canonical::collection_path::{
    check_collection_path, free_path, join, safe_segment, ItemKind, PathError,
};
use filesquid_core::CollectionId;
use std::path::{Component, Path};

/// The collection logical path of each source of `plan` in `collection_id` beneath the
/// destination folder, numbered when taken in the index or by a source before it in the same
/// plan (two picked folders of one name land beside each other), or the code that
/// quarantines its files. Reads filesystem metadata and the index.
///
/// # Errors
/// `unavailable` when the index cannot answer.
pub fn source_roots(
    plan: &ImportPlan,
    collection_id: &CollectionId,
    index: &dyn ImportIndex,
) -> Result<Vec<Result<String, &'static str>>, &'static str> {
    let mut chosen: Vec<String> = Vec::with_capacity(plan.sources.len());
    let mut roots = Vec::with_capacity(plan.sources.len());
    for source in &plan.sources {
        let taken = |path: &str| {
            let by_plan = chosen.iter().any(|root| overlaps(root, path));
            Ok(by_plan || index.path_taken(collection_id, path)?)
        };
        let root = source_root(source, plan.folder.as_deref(), taken)?;
        if let Ok(path) = &root {
            chosen.push(path.clone());
        }
        roots.push(root);
    }
    Ok(roots)
}

/// Whether the paths `a` and `b` are the same or one lies below the other. Pure.
fn overlaps(a: &str, b: &str) -> bool {
    let below = |inner: &str, outer: &str| {
        inner
            .strip_prefix(outer)
            .is_some_and(|rest| rest.starts_with('/'))
    };
    a == b || below(a, b) || below(b, a)
}

/// The path of the source `source` beneath `folder`, numbered while `taken` says a path is
/// used. Reads filesystem metadata and, through `taken`, the index.
fn source_root(
    source: &Path,
    folder: Option<&str>,
    mut taken: impl FnMut(&str) -> Result<bool, String>,
) -> Result<Result<String, &'static str>, &'static str> {
    let Some(name) = source.file_name() else {
        return Ok(Err(PathError::Invalid.error_code()));
    };
    let segment = match safe_segment(&name.to_string_lossy()) {
        Ok(segment) => segment,
        Err(err) => return Ok(Err(err.error_code())),
    };
    let kind = match std::fs::metadata(source).is_ok_and(|meta| meta.is_dir()) {
        true => ItemKind::Folder,
        false => ItemKind::File,
    };
    let wanted = join(folder, &segment);
    let free = free_path(&wanted, kind, &mut taken).map_err(|_| "unavailable")?;
    Ok(free.map_err(|err| err.error_code()))
}

/// The collection logical path of `file`, listed from the source `source` whose path is
/// `root`: `root` for the source itself, else `root` and the file's path below the source,
/// each segment made safe and NFC. Pure.
///
/// # Errors
/// `name_too_long` for a segment over 255 bytes after the safe transformation;
/// `invalid_path` for a path outside the collection-path rule.
pub fn logical_path(root: &str, source: &Path, file: &Path) -> Result<String, &'static str> {
    let below = file.strip_prefix(source).map_err(|_| "invalid_path")?;
    let mut path = root.to_string();
    for component in below.components() {
        let Component::Normal(name) = component else {
            return Err(PathError::Invalid.error_code());
        };
        let segment = safe_segment(&name.to_string_lossy()).map_err(|err| err.error_code())?;
        path = format!("{path}/{segment}");
    }
    check_collection_path(&path).map_err(|err| err.error_code())?;
    Ok(path)
}
