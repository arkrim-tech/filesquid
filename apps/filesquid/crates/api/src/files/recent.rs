//! The Recent view (Task API-F2.21; API-S-011 **Recent**, SRS-F-062): the files the caller
//! changed, opened or uploaded, newest first, each with the kind of its latest action and
//! that action's time.
//!
//! The view is computed only from the caller's own succeeded journal entries: the journal is
//! filtered by actor and outcome before anything is kept, so another caller's activity never
//! adds a file or moves one. It reads at most [`MAX_JOURNAL_PAGES`] pages per library, newest
//! first, and keeps the first entry that names each file, which is its latest action. A file
//! the catalog no longer lists as ordinary (hidden, deleted or gone), or whose home
//! collection the caller may not read, is left out.
//!
//! A repository node covers every library of the repository that the caller may read, a
//! library node that library and a collection node the files whose home is that collection.
//! Reads the journal and the catalog, which may block; the view route calls it on a blocking
//! thread. Every loop is bounded: [`MAX_SEARCH_LIBRARIES`] libraries, [`MAX_JOURNAL_PAGES`]
//! journal pages per library and [`MAX_VIEW_FILES`] files in all: the newest actions across
//! the libraries, never the first library's older ones.

use super::drive::{HomeCollection, RecentKind, MAX_VIEW_FILES};
use super::drive_scope::{
    catalog_error, home_collections, read_bounded, FILE_DOWNLOAD_OPERATION, MAX_JOURNAL_PAGES,
};
use super::error::FileError;
use super::nodes::file_error;
use super::search_scope::MAX_SEARCH_LIBRARIES;
use super::{
    COPY_OPERATION, METADATA_REMOVE_OPERATION, METADATA_SET_OPERATION, MOVE_OPERATION,
    RENAME_OPERATION, RESTORE_OPERATION, UPLOAD_OPERATION,
};
use crate::auth::Principal;
use crate::journal::{JournalQuery, JournalStore, Outcome, MAX_PAGE};
use crate::library::FileEntry;
use crate::navigation::{NavigationService, NodeRef};
use filesquid_core::{CollectionId, FileId, LibraryId};
use std::collections::{HashMap, HashSet};

/// Prefix of a file object's identifier among a journal entry's targets.
const OBJECT_PREFIX: &str = "urn:uuid:";

/// Operations that change a file without uploading content: the row says `changed`.
const CHANGING: [&str; 6] = [
    MOVE_OPERATION,
    RESTORE_OPERATION,
    RENAME_OPERATION,
    METADATA_SET_OPERATION,
    METADATA_REMOVE_OPERATION,
    COPY_OPERATION,
];

/// Operations that open a file: the row says `opened`.
const OPENING: [&str; 2] = [crate::content::ISSUED_OPERATION, FILE_DOWNLOAD_OPERATION];

/// The caller's latest action on one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecentHit {
    /// What the caller did.
    pub kind: RecentKind,
    /// When, in Unix milliseconds.
    pub time_ms: i64,
}

/// The recent files of one library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentLibrary {
    /// The library.
    pub library_id: String,
    /// The files, newest action first.
    pub files: Vec<FileId>,
    /// Each file's latest action, by file ID.
    pub hits: HashMap<String, RecentHit>,
    /// Each file's home collection, by file ID; a file registered in none has no entry.
    pub homes: HashMap<String, HomeCollection>,
}

/// Everything one Recent view covers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentScope {
    /// The libraries, in catalog order.
    pub libraries: Vec<RecentLibrary>,
    /// False when a bound left libraries or files out.
    pub complete: bool,
}

/// The kind of action the journal operation `operation` is, or `None` for an operation that
/// is no change, opening or upload of a file. Pure.
pub fn recent_kind(operation: &str) -> Option<RecentKind> {
    if operation == UPLOAD_OPERATION {
        return Some(RecentKind::Uploaded);
    }
    if OPENING.contains(&operation) {
        return Some(RecentKind::Opened);
    }
    CHANGING.contains(&operation).then_some(RecentKind::Changed)
}

/// The files `principal` changed, opened or uploaded in the scope `node` names, from their
/// own succeeded entries in `store`. May block.
///
/// # Errors
/// `not_found` for a node that is unknown or unreadable; `invalid_request` for a node that is
/// not the repository, a library or a collection; `unavailable` when the journal or the
/// catalog cannot be read.
pub fn recent_scope(
    navigation: &NavigationService,
    principal: &Principal,
    node: &NodeRef,
    store: &dyn JournalStore,
) -> Result<RecentScope, FileError> {
    navigation
        .check_readable(principal, node)
        .map_err(file_error)?;
    let (targets, mut complete) = targets(navigation, principal, node)?;
    let mut scope = RecentScope::default();
    for target in targets {
        let (hits, read_all) =
            latest_actions(store, principal.actor(), target.library_id.as_str())?;
        let (library, all) = recent_library(navigation, principal, &target, hits, MAX_VIEW_FILES)?;
        complete &= read_all && all;
        scope.libraries.push(library);
        // Keep the newest actions across the libraries read so far, so the bound never
        // prefers an earlier library's older actions to a later library's newer ones, and
        // at most twice the bound is held at once.
        complete &= retain_newest(&mut scope, MAX_VIEW_FILES);
    }
    scope.complete = complete;
    Ok(scope)
}

/// One kept action: newest first, then by library and file for a stable cut.
type Ranked = (std::cmp::Reverse<i64>, String, String);

/// Keep only the `limit` newest actions of `scope`, across its libraries, with each kept
/// file's facts and each library's own order; false when actions were left out. Pure.
fn retain_newest(scope: &mut RecentScope, limit: usize) -> bool {
    let mut ranked: Vec<Ranked> = scope
        .libraries
        .iter()
        .flat_map(|library| {
            library.hits.iter().map(|(file_id, hit)| {
                let when = std::cmp::Reverse(hit.time_ms);
                (when, library.library_id.clone(), file_id.clone())
            })
        })
        .collect();
    if ranked.len() <= limit {
        return true;
    }
    ranked.sort_unstable();
    ranked.truncate(limit);
    let kept: HashSet<(String, String)> = ranked
        .into_iter()
        .map(|(_, library_id, file_id)| (library_id, file_id))
        .collect();
    for library in &mut scope.libraries {
        let id = library.library_id.clone();
        let keeps = |file: &str| kept.contains(&(id.clone(), file.to_string()));
        library.files.retain(|file| keeps(file.as_str()));
        library.hits.retain(|file, _| keeps(file));
        library.homes.retain(|file, _| keeps(file));
    }
    false
}

/// One library a Recent view covers, narrowed to one home collection when the node is one.
struct Target {
    library_id: LibraryId,
    collection: Option<CollectionId>,
}

/// The libraries `node` names for `principal`, and whether every readable one was taken.
fn targets(
    navigation: &NavigationService,
    principal: &Principal,
    node: &NodeRef,
) -> Result<(Vec<Target>, bool), FileError> {
    let catalog = navigation.catalog().as_ref();
    let whole = |library_id: LibraryId| Target {
        library_id,
        collection: None,
    };
    match node {
        NodeRef::Repository(repository) => {
            let read = |after: Option<&_>, limit| catalog.libraries(repository, after, limit);
            let libraries = read_bounded(read, MAX_SEARCH_LIBRARIES + 1)?;
            let complete = libraries.len() <= MAX_SEARCH_LIBRARIES;
            let targets = libraries
                .into_iter()
                .take(MAX_SEARCH_LIBRARIES)
                .map(|keyed| keyed.item.library.id().clone())
                .filter(|id| navigation.may_read(principal, &NodeRef::Library(id.clone())))
                .map(whole)
                .collect();
            Ok((targets, complete))
        }
        NodeRef::Library(id) => Ok((vec![whole(id.clone())], true)),
        NodeRef::Collection(id) => {
            let collection = catalog
                .collection(id)
                .map_err(catalog_error)?
                .ok_or_else(FileError::not_found)?;
            let target = Target {
                library_id: collection.library_id().clone(),
                collection: Some(id.clone()),
            };
            Ok((vec![target], true))
        }
        _ => Err(FileError::invalid(
            "The recent view covers the repository, a library or a collection node.",
        )),
    }
}

/// The latest action of `actor` on each file object of `library_id`, newest first, from at
/// most [`MAX_JOURNAL_PAGES`] pages of the actor's own succeeded entries, and false when the
/// page bound stopped the read with older entries left unread.
fn latest_actions(
    store: &dyn JournalStore,
    actor: &str,
    library_id: &str,
) -> Result<(Vec<(String, RecentHit)>, bool), FileError> {
    let mut query = JournalQuery {
        actor: Some(actor.to_string()),
        library_id: Some(library_id.to_string()),
        outcome: Some(Outcome::Succeeded),
        descending: true,
        limit: MAX_PAGE,
        ..JournalQuery::default()
    };
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut hits: Vec<(String, RecentHit)> = Vec::new();
    let mut read_all = false;
    for _ in 0..MAX_JOURNAL_PAGES {
        let page = store
            .query(&query)
            .map_err(|_| FileError::unavailable("the journal cannot be read"))?;
        for entry in &page.entries {
            keep_latest(
                &mut seen,
                &mut hits,
                &entry.operation,
                &entry.target_ids,
                entry.time_ms,
            );
        }
        match page.next_cursor {
            Some(next) => query.after_id = Some(next),
            None => {
                read_all = true;
                break;
            }
        }
    }
    hits.sort_by_key(|hit| std::cmp::Reverse(hit.1.time_ms));
    Ok((hits, read_all))
}

/// Keep the action `operation` at `time` as the latest one on the file object `targets`
/// name, unless a later action on it is already kept. Pure.
fn keep_latest(
    seen: &mut HashMap<String, usize>,
    hits: &mut Vec<(String, RecentHit)>,
    operation: &str,
    targets: &[String],
    time: i64,
) {
    let object = targets.iter().find(|t| t.starts_with(OBJECT_PREFIX));
    let (Some(object), Some(kind)) = (object, recent_kind(operation)) else {
        return;
    };
    let hit = RecentHit {
        kind,
        time_ms: time,
    };
    match seen.get(object) {
        Some(&at) if hits[at].1.time_ms >= time => {}
        Some(&at) => hits[at].1 = hit,
        None => {
            seen.insert(object.clone(), hits.len());
            hits.push((object.clone(), hit));
        }
    }
}

/// The recent files of `target` among `hits` that the catalog lists as ordinary and
/// `principal` may read, at most `room`, and whether that was all of them.
fn recent_library(
    navigation: &NavigationService,
    principal: &Principal,
    target: &Target,
    hits: Vec<(String, RecentHit)>,
    room: usize,
) -> Result<(RecentLibrary, bool), FileError> {
    let catalog = navigation.catalog().as_ref();
    let mut entries: Vec<FileEntry> = Vec::new();
    let mut kept = HashMap::new();
    let mut complete = true;
    for (object, hit) in hits {
        let Some(entry) = listed_file(navigation, principal, target, &object)? else {
            continue;
        };
        if entries.len() == room {
            complete = false;
            break;
        }
        kept.insert(entry.file.id().as_str().to_string(), hit);
        entries.push(entry);
    }
    let homes = home_collections(catalog, &target.library_id, &entries)?;
    let library = RecentLibrary {
        library_id: target.library_id.as_str().to_string(),
        files: entries.into_iter().map(|e| e.file.id().clone()).collect(),
        hits: kept,
        homes,
    };
    Ok((library, complete))
}

/// The catalog entry of the file object `object`, when it is an ordinary file of `target`
/// that `principal` may read; `None` otherwise.
fn listed_file(
    navigation: &NavigationService,
    principal: &Principal,
    target: &Target,
    object: &str,
) -> Result<Option<FileEntry>, FileError> {
    let id = object.strip_prefix(OBJECT_PREFIX).unwrap_or(object);
    let Ok(file_id) = FileId::new(id) else {
        return Ok(None);
    };
    let catalog = navigation.catalog().as_ref();
    let Some(entry) = catalog.file(&file_id).map_err(catalog_error)? else {
        return Ok(None);
    };
    let home = entry.file.home_collection_id();
    let ordinary = !entry.state.hidden && entry.state.deletion.is_none();
    let in_library = entry.file.library_id() == &target.library_id;
    let in_collection = target.collection.as_ref().is_none_or(|c| c == home);
    let readable = navigation.may_read(principal, &NodeRef::Collection(home.clone()));
    let listed = ordinary && in_library && in_collection && readable;
    Ok(listed.then_some(entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A library `id` whose files `files` were last touched at the matching `times`.
    fn library_of(id: &str, files: &[(&str, i64)]) -> RecentLibrary {
        let hit = |time_ms| RecentHit {
            kind: RecentKind::Uploaded,
            time_ms,
        };
        RecentLibrary {
            library_id: id.to_string(),
            files: files
                .iter()
                .map(|(f, _)| FileId::new(*f).expect("id"))
                .collect(),
            hits: files
                .iter()
                .map(|(f, t)| (f.to_string(), hit(*t)))
                .collect(),
            homes: HashMap::new(),
        }
    }

    #[test]
    fn the_bound_keeps_the_newest_actions_across_libraries_not_the_first_library() {
        let mut scope = RecentScope {
            libraries: vec![
                library_of("first", &[("a", 1), ("b", 2), ("c", 3)]),
                library_of("second", &[("d", 9), ("e", 8)]),
            ],
            complete: true,
        };
        assert!(!retain_newest(&mut scope, 3), "two actions were left out");
        let first: Vec<&str> = scope.libraries[0]
            .files
            .iter()
            .map(FileId::as_str)
            .collect();
        let second: Vec<&str> = scope.libraries[1]
            .files
            .iter()
            .map(FileId::as_str)
            .collect();
        assert_eq!(first, ["c"], "the older actions of the first library go");
        assert_eq!(second, ["d", "e"], "the newer library keeps its order");
        assert!(scope.libraries[0].hits.contains_key("c"));
        assert_eq!(scope.libraries[0].hits.len(), 1);
        assert!(retain_newest(&mut scope, 3), "nothing more is left out");
    }

    /// The targets of a journal entry about file object `id`.
    fn targets_of(id: &str) -> Vec<String> {
        vec![
            "lib".to_string(),
            format!("urn:uuid:{id}"),
            "v2".to_string(),
        ]
    }

    #[test]
    fn each_operation_has_one_kind_and_others_have_none() {
        assert_eq!(recent_kind(UPLOAD_OPERATION), Some(RecentKind::Uploaded));
        assert_eq!(recent_kind(RENAME_OPERATION), Some(RecentKind::Changed));
        assert_eq!(recent_kind(MOVE_OPERATION), Some(RecentKind::Changed));
        assert_eq!(
            recent_kind(FILE_DOWNLOAD_OPERATION),
            Some(RecentKind::Opened)
        );
        for other in ["files.label", "files.view", "library.seed", ""] {
            assert_eq!(recent_kind(other), None, "{other}");
        }
    }

    #[test]
    fn the_latest_action_on_a_file_wins_whatever_the_order_read() {
        let (mut seen, mut hits) = (HashMap::new(), Vec::new());
        keep_latest(&mut seen, &mut hits, RENAME_OPERATION, &targets_of("a"), 9);
        keep_latest(&mut seen, &mut hits, UPLOAD_OPERATION, &targets_of("a"), 3);
        keep_latest(&mut seen, &mut hits, UPLOAD_OPERATION, &targets_of("b"), 5);
        keep_latest(
            &mut seen,
            &mut hits,
            FILE_DOWNLOAD_OPERATION,
            &targets_of("b"),
            7,
        );
        keep_latest(&mut seen, &mut hits, "files.label", &targets_of("c"), 8);
        keep_latest(
            &mut seen,
            &mut hits,
            UPLOAD_OPERATION,
            &["lib".to_string()],
            8,
        );
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert_eq!(hits[0].0, "urn:uuid:a");
        assert_eq!(hits[0].1.kind, RecentKind::Changed);
        assert_eq!(hits[0].1.time_ms, 9);
        assert_eq!(hits[1].1.kind, RecentKind::Opened);
        assert_eq!(hits[1].1.time_ms, 7);
    }
}
