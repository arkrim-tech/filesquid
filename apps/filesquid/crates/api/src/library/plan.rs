//! What each library command means: the checks it needs and the catalog changes it makes.
//!
//! The [`Planner`] reads committed state through the injected catalog and turns one
//! validated [`LibraryCommand`] into a [`Planned`] change set. It writes nothing; the service
//! commits the result. Every referenced folder, collection, virtual collection and file must
//! belong to the command's library, or the command is `not_found`. A command whose result
//! equals the stored state plans no change.

use super::catalog::{
    CatalogChange, FileEntry, FileListing, FileScope, LibraryCatalog, LibraryRecord,
};
use super::command::{
    LibraryCommand, RestoredInto, MAX_UNTITLED_TITLES, NEW_LIBRARY_REVISION,
    UNTITLED_COLLECTION_TITLE,
};
use super::ids::mint_uuid_v4;
use super::policy::{Deletion, DeletionPolicy, FileState, PurgeConfirmation, VisibilityFilter};
use super::LibraryError;
use crate::operation::CommandEnvelope;
use filesquid_core::{
    Collection, CollectionId, CollectionSort, FileId, FolderId, Library, LibraryId, RepositoryId,
    VirtualCollection, VirtualCollectionId, VirtualMembership,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

/// Most collections of one library a restore reads for the titles already taken. A library
/// with more is read up to here only (AGENTS.md §22); a title past the bound may then be
/// used twice, which titles allow.
const MAX_TITLE_SCAN: usize = 100_000;

/// Collections read per page of that scan.
const TITLE_PAGE: usize = 1_000;

/// The changes one command makes, and what it answers.
#[derive(Debug, Default)]
pub(super) struct Planned {
    /// The library changed.
    pub library_id: Option<LibraryId>,
    /// The catalog changes, in order; empty when the command changes nothing.
    pub changes: Vec<CatalogChange>,
    /// Identifier the command created.
    pub created_id: Option<String>,
    /// Opaque identifiers of what was touched, for the journal.
    pub targets: Vec<String>,
    /// The deletion record of the files a recoverable deletion newly deletes; files already
    /// deleted keep their earlier record.
    pub deletion: Option<Deletion>,
    /// The collections a restore creates for files whose home collection is gone.
    pub restored_into: Vec<RestoredInto>,
}

/// Plans commands against committed catalog state at one instant.
pub(super) struct Planner<'a> {
    /// The injected catalog (read only here).
    pub catalog: &'a dyn LibraryCatalog,
    /// The command's time, Unix milliseconds (UTC), for deletion times.
    pub now_ms: i64,
    /// The recoverable-deletion policy.
    pub policy: DeletionPolicy,
}

impl Planner<'_> {
    /// Plan `command` sent with `envelope`.
    pub fn plan(
        &self,
        envelope: &CommandEnvelope,
        command: &LibraryCommand,
    ) -> Result<Planned, LibraryError> {
        let expected = envelope.expected_revision().as_str();
        if let LibraryCommand::CreateLibrary {
            repository_id,
            name,
        } = command
        {
            return create_library(expected, repository_id, name);
        }
        let library_id = command.library_id().ok_or(LibraryError::Internal)?;
        let retention = self.check_revision(library_id, expected)?;
        if let LibraryCommand::SetLibraryRetention { retention_days, .. } = command {
            let changes = (*retention_days != retention)
                .then_some(CatalogChange::SetRetention(*retention_days))
                .into_iter()
                .collect();
            let mut planned = planned(changes, [library_id.as_str()]);
            planned.library_id = Some(library_id.clone());
            return Ok(planned);
        }
        let policy = match retention {
            Some(recoverable_days) => DeletionPolicy { recoverable_days },
            None => self.policy,
        };
        let mut planned = self.plan_in(library_id, command, policy)?;
        planned.library_id = Some(library_id.clone());
        planned.targets.insert(0, library_id.as_str().to_string());
        Ok(planned)
    }

    /// The library must exist at revision `expected`; answers its own retention period, if
    /// it set one.
    fn check_revision(
        &self,
        library_id: &LibraryId,
        expected: &str,
    ) -> Result<Option<u32>, LibraryError> {
        let state = self
            .catalog
            .library(library_id)?
            .ok_or(LibraryError::NotFound("library"))?;
        if state.revision != expected {
            return Err(LibraryError::Conflict);
        }
        Ok(state.retention_days)
    }

    /// Plan a command of an existing library whose deleted files stay recoverable as `policy`
    /// says (the library's own retention, else the installation default).
    fn plan_in(
        &self,
        lib: &LibraryId,
        command: &LibraryCommand,
        policy: DeletionPolicy,
    ) -> Result<Planned, LibraryError> {
        use LibraryCommand as C;
        match command {
            C::CreateCollection {
                title,
                folder_id,
                sort,
                ..
            } => self.create_collection(lib, title, folder_id.as_ref(), *sort),
            C::RenameCollection {
                collection_id,
                title,
                ..
            } => self.rename_collection(lib, collection_id, title),
            C::DeleteCollection { collection_id, .. } => {
                self.delete_collection(lib, collection_id, policy)
            }
            C::CreateVirtualCollection {
                title, expression, ..
            } => create_virtual(lib, title, expression.as_deref()),
            C::RenameVirtualCollection {
                virtual_collection_id,
                title,
                ..
            } => self.rename_virtual(lib, virtual_collection_id, title),
            C::DeleteVirtualCollection {
                virtual_collection_id,
                ..
            } => self.delete_virtual(lib, virtual_collection_id),
            C::AddToVirtualCollection {
                virtual_collection_id,
                file_ids,
                ..
            } => self.add_members(lib, virtual_collection_id, file_ids),
            C::RemoveFromVirtualCollection {
                virtual_collection_id,
                file_ids,
                ..
            } => self.remove_members(lib, virtual_collection_id, file_ids),
            C::PurgeFiles {
                file_ids,
                confirmation,
                ..
            } => self.purge(lib, file_ids, confirmation),
            C::RestoreFiles { file_ids, .. } => self.restore(lib, file_ids),
            other => self.change_states(lib, other, policy),
        }
    }

    /// The folder `id`, when it belongs to `lib`.
    fn folder_in(&self, lib: &LibraryId, id: &FolderId) -> Result<(), LibraryError> {
        self.catalog
            .folder(id)?
            .filter(|f| f.library_id() == lib)
            .map(|_| ())
            .ok_or(LibraryError::NotFound("folder"))
    }

    /// The collection `id`, when it belongs to `lib`.
    fn collection_in(
        &self,
        lib: &LibraryId,
        id: &CollectionId,
    ) -> Result<Collection, LibraryError> {
        self.catalog
            .collection(id)?
            .filter(|c| c.library_id() == lib)
            .ok_or(LibraryError::NotFound("collection"))
    }

    /// The virtual collection `id`, when it belongs to `lib`.
    fn virtual_in(
        &self,
        lib: &LibraryId,
        id: &VirtualCollectionId,
    ) -> Result<VirtualCollection, LibraryError> {
        self.catalog
            .virtual_collection(id)?
            .filter(|v| v.library_id() == lib)
            .ok_or(LibraryError::NotFound("virtual_collection"))
    }

    /// The hand-picked virtual collection `id` of `lib`; a rule-based one is refused.
    fn hand_picked_in(
        &self,
        lib: &LibraryId,
        id: &VirtualCollectionId,
    ) -> Result<VirtualCollection, LibraryError> {
        let vc = self.virtual_in(lib, id)?;
        if vc.expression().is_some() {
            return Err(LibraryError::invalid(
                "rule_based_virtual_collection",
                "This virtual collection is rule-based; change its rule instead.",
            ));
        }
        Ok(vc)
    }

    /// The file `id` with its state, when it belongs to `lib`.
    fn file_in(&self, lib: &LibraryId, id: &FileId) -> Result<FileEntry, LibraryError> {
        self.catalog
            .file(id)?
            .filter(|f| f.file.library_id() == lib)
            .ok_or(LibraryError::NotFound("file"))
    }

    /// Plan a new, empty collection in `lib`, in a folder of `lib` when one is named.
    fn create_collection(
        &self,
        lib: &LibraryId,
        title: &str,
        folder_id: Option<&FolderId>,
        sort: CollectionSort,
    ) -> Result<Planned, LibraryError> {
        if let Some(folder) = folder_id {
            self.folder_in(lib, folder)?;
        }
        new_collection(lib, title, folder_id, sort)
    }

    /// Rename a collection of `lib`; the same title changes nothing.
    fn rename_collection(
        &self,
        lib: &LibraryId,
        id: &CollectionId,
        title: &str,
    ) -> Result<Planned, LibraryError> {
        let current = self.collection_in(lib, id)?;
        let renamed = retitled(&current, title)?;
        let changes = (renamed != current)
            .then_some(CatalogChange::PutCollection(renamed))
            .into_iter()
            .collect();
        Ok(planned(changes, [id.as_str()]))
    }

    /// Delete a collection of `lib` and, in the same commit, every one of its files that is
    /// not already deleted, as `policy` deletes a file but with the cause "with its
    /// collection" (owner decisions 2026-10-22, ADR 041 Decision 8); every file of the
    /// collection keeps its title. A collection without files is only removed. A file
    /// registered after this plan makes the catalog refuse the removal rather than leave it
    /// undeleted. Nothing is moved: each file keeps its home collection and logical path. The
    /// answer carries the deletion times when the collection held a file that was not deleted.
    fn delete_collection(
        &self,
        lib: &LibraryId,
        id: &CollectionId,
        policy: DeletionPolicy,
    ) -> Result<Planned, LibraryError> {
        let collection = self.collection_in(lib, id)?;
        let deletion = policy.deletion_at(self.now_ms).with_collection();
        let holds = self.holds(
            lib,
            id,
            &[VisibilityFilter::Ordinary, VisibilityFilter::Hidden],
        )?;
        let any = holds || self.holds(lib, id, &[VisibilityFilter::Deleted])?;
        let files = any.then(|| CatalogChange::DeleteCollectionFiles {
            collection_id: id.clone(),
            deletion,
            title: collection.title().to_string(),
        });
        let changes = files
            .into_iter()
            .chain([CatalogChange::RemoveCollection(id.clone())])
            .collect();
        let mut out = planned(changes, [id.as_str()]);
        out.deletion = holds.then_some(deletion);
        Ok(out)
    }

    /// Whether the collection `id` of `lib` holds a file in one of `views`: one bounded page
    /// of each.
    fn holds(
        &self,
        lib: &LibraryId,
        id: &CollectionId,
        views: &[VisibilityFilter],
    ) -> Result<bool, LibraryError> {
        for visibility in views {
            let listing = FileListing {
                library_id: lib.clone(),
                scope: FileScope::Collection {
                    collection_id: id.clone(),
                },
                visibility: *visibility,
            };
            if !self.catalog.files(&listing, None, 1)?.is_empty() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Restore recoverably deleted files of `lib` into their own object and logical path:
    /// in their home collection when it exists, else into one new "untitled collection" per
    /// gone home collection, all in one commit (ADR 041 Decision 9). A file that is not
    /// deleted changes nothing; a hidden file stays hidden.
    fn restore(&self, lib: &LibraryId, file_ids: &[FileId]) -> Result<Planned, LibraryError> {
        let mut changes = Vec::new();
        let mut gone: BTreeMap<String, Vec<FileId>> = BTreeMap::new();
        let mut gone_order: Vec<CollectionId> = Vec::new();
        for file_id in file_ids {
            let entry = self.file_in(lib, file_id)?;
            if entry.state.deletion.is_none() {
                continue;
            }
            let home = entry.file.home_collection_id().clone();
            let exists = self
                .catalog
                .collection(&home)?
                .is_some_and(|c| c.library_id() == lib);
            if !exists {
                let group = gone.entry(home.as_str().to_string()).or_default();
                if group.is_empty() {
                    gone_order.push(home);
                }
                group.push(file_id.clone());
            }
            let state = FileState {
                deletion: None,
                ..entry.state
            };
            changes.push(CatalogChange::SetFileState {
                file_id: file_id.clone(),
                state,
            });
        }
        let made = self.untitled_collections(lib, &gone_order)?;
        let mut out = planned_files(Vec::new(), None, file_ids);
        for (from, collection) in gone_order.iter().zip(made) {
            let new_id = collection.id().clone();
            out.targets.push(new_id.as_str().to_string());
            out.restored_into.push(RestoredInto {
                from_collection_id: from.clone(),
                collection_id: new_id.clone(),
                title: collection.title().to_string(),
            });
            out.changes.push(CatalogChange::PutCollection(collection));
            for file_id in gone.get(from.as_str()).into_iter().flatten() {
                out.changes.push(CatalogChange::RehomeFile {
                    file_id: file_id.clone(),
                    collection_id: new_id.clone(),
                });
            }
        }
        out.changes.extend(changes);
        Ok(out)
    }

    /// One new collection of `lib` for each of `count` gone home collections, titled with the
    /// first free "untitled collection" titles in order.
    ///
    /// # Errors
    /// `restore_name_exhausted` (`409 conflict`) when the 9,999 titles run out.
    fn untitled_collections(
        &self,
        lib: &LibraryId,
        gone: &[CollectionId],
    ) -> Result<Vec<Collection>, LibraryError> {
        if gone.is_empty() {
            return Ok(Vec::new());
        }
        let mut taken = self.collection_titles(lib)?;
        let mut made = Vec::with_capacity(gone.len());
        for _ in gone {
            let title = first_untitled(&taken).ok_or(LibraryError::Refused {
                code: "restore_name_exhausted",
                message: "Every untitled collection name is taken. Rename or delete one, then restore again.",
            })?;
            taken.insert(title.clone());
            let id = mint_uuid_v4()?;
            let collection_id = CollectionId::new(id).map_err(|_| LibraryError::Internal)?;
            let collection = Collection::new(collection_id, lib.clone(), title)
                .map_err(|_| LibraryError::Internal)?;
            made.push(collection);
        }
        Ok(made)
    }

    /// The titles of the collections of `lib`, read page by page up to [`MAX_TITLE_SCAN`].
    fn collection_titles(&self, lib: &LibraryId) -> Result<HashSet<String>, LibraryError> {
        let mut titles = HashSet::new();
        let mut after: Option<super::catalog::PageKey> = None;
        for _ in 0..MAX_TITLE_SCAN / TITLE_PAGE {
            let page = self.catalog.collections(lib, after.as_ref(), TITLE_PAGE)?;
            let full = page.len() == TITLE_PAGE;
            for keyed in page {
                titles.insert(keyed.item.title().to_string());
                after = Some(keyed.key);
            }
            if !full {
                return Ok(titles);
            }
        }
        tracing::info!(target: super::LOG_TARGET, "collection title scan stopped at its bound");
        Ok(titles)
    }

    /// Rename a virtual collection of `lib`; the same title changes nothing.
    fn rename_virtual(
        &self,
        lib: &LibraryId,
        id: &VirtualCollectionId,
        title: &str,
    ) -> Result<Planned, LibraryError> {
        let current = self.virtual_in(lib, id)?;
        let renamed = retitled(&current, title)?;
        let changes = (renamed != current)
            .then_some(CatalogChange::PutVirtualCollection(renamed))
            .into_iter()
            .collect();
        Ok(planned(changes, [id.as_str()]))
    }

    /// Delete a virtual collection of `lib`; its member files stay.
    fn delete_virtual(
        &self,
        lib: &LibraryId,
        id: &VirtualCollectionId,
    ) -> Result<Planned, LibraryError> {
        self.virtual_in(lib, id)?;
        let change = CatalogChange::RemoveVirtualCollection(id.clone());
        Ok(planned(vec![change], [id.as_str()]))
    }

    /// Add the files that are not members yet to a hand-picked virtual collection, ordered
    /// after its current last member in the order named.
    fn add_members(
        &self,
        lib: &LibraryId,
        id: &VirtualCollectionId,
        file_ids: &[FileId],
    ) -> Result<Planned, LibraryError> {
        self.hand_picked_in(lib, id)?;
        let mut next = first_member_order(self.catalog.last_member_order(id)?)?;
        let mut changes = Vec::new();
        for file_id in file_ids {
            self.file_in(lib, file_id)?;
            if self.catalog.is_virtual_member(id, file_id)? {
                continue;
            }
            let member = VirtualMembership::new(id.clone(), file_id.clone(), next);
            changes.push(CatalogChange::AddVirtualMember(member));
            next = next.checked_add(1).ok_or_else(order_exhausted)?;
        }
        Ok(planned_files(changes, Some(id.as_str()), file_ids))
    }

    /// Remove member files from a hand-picked virtual collection; non-members are skipped.
    fn remove_members(
        &self,
        lib: &LibraryId,
        id: &VirtualCollectionId,
        file_ids: &[FileId],
    ) -> Result<Planned, LibraryError> {
        self.hand_picked_in(lib, id)?;
        let mut changes = Vec::new();
        for file_id in file_ids {
            self.file_in(lib, file_id)?;
            if self.catalog.is_virtual_member(id, file_id)? {
                changes.push(CatalogChange::RemoveVirtualMember {
                    virtual_collection_id: id.clone(),
                    file_id: file_id.clone(),
                });
            }
        }
        Ok(planned_files(changes, Some(id.as_str()), file_ids))
    }

    /// Hide, unhide, delete or restore files: one state change per file that changes.
    fn change_states(
        &self,
        lib: &LibraryId,
        command: &LibraryCommand,
        policy: DeletionPolicy,
    ) -> Result<Planned, LibraryError> {
        let deletion = policy.deletion_at(self.now_ms);
        let next = state_transition(command, deletion)?;
        let mut changes = Vec::new();
        for file_id in command.file_ids() {
            let current = self.file_in(lib, file_id)?.state;
            let state = next(current);
            if state != current {
                changes.push(CatalogChange::SetFileState {
                    file_id: file_id.clone(),
                    state,
                });
            }
        }
        let mut out = planned_files(changes, None, command.file_ids());
        let deletes = matches!(command, LibraryCommand::DeleteFiles { .. });
        out.deletion = (deletes && !out.changes.is_empty()).then_some(deletion);
        Ok(out)
    }

    /// Permanently delete recoverably deleted files after an exact scope confirmation.
    fn purge(
        &self,
        lib: &LibraryId,
        file_ids: &[FileId],
        confirmation: &PurgeConfirmation,
    ) -> Result<Planned, LibraryError> {
        if !confirmation.matches(lib, file_ids.len()) {
            return Err(LibraryError::invalid(
                "purge_confirmation_mismatch",
                "The confirmation does not match the files chosen for permanent deletion.",
            ));
        }
        for file_id in file_ids {
            if self.file_in(lib, file_id)?.state.deletion.is_none() {
                return Err(LibraryError::invalid(
                    "not_recoverably_deleted",
                    "Only files in Recently Deleted can be deleted permanently.",
                ));
            }
        }
        let changes = file_ids
            .iter()
            .cloned()
            .map(CatalogChange::PurgeFile)
            .collect();
        Ok(planned_files(changes, None, file_ids))
    }
}

/// The state change a hide, unhide, delete or restore command applies to each file.
fn state_transition(
    command: &LibraryCommand,
    deletion: Deletion,
) -> Result<Box<dyn Fn(FileState) -> FileState>, LibraryError> {
    let next: Box<dyn Fn(FileState) -> FileState> = match command {
        LibraryCommand::HideFiles { .. } => Box::new(|s| FileState { hidden: true, ..s }),
        LibraryCommand::UnhideFiles { .. } => Box::new(|s| FileState { hidden: false, ..s }),
        LibraryCommand::DeleteFiles { .. } => Box::new(move |s| FileState {
            deletion: s.deletion.or(Some(deletion)),
            ..s
        }),
        _ => return Err(LibraryError::Internal),
    };
    Ok(next)
}

/// The first "untitled collection" title that `taken` does not hold: the plain title, then
/// "untitled collection 2" and so on, at most [`MAX_UNTITLED_TITLES`] tries. Pure.
fn first_untitled(taken: &HashSet<String>) -> Option<String> {
    (1..=MAX_UNTITLED_TITLES)
        .map(|n| match n {
            1 => UNTITLED_COLLECTION_TITLE.to_string(),
            n => format!("{UNTITLED_COLLECTION_TITLE} {n}"),
        })
        .find(|title| !taken.contains(title))
}

/// Plan a new library on `repository_id`; the expected revision must be
/// [`NEW_LIBRARY_REVISION`].
fn create_library(
    expected: &str,
    repository_id: &RepositoryId,
    name: &str,
) -> Result<Planned, LibraryError> {
    if expected != NEW_LIBRARY_REVISION {
        return Err(LibraryError::invalid(
            "invalid_revision",
            "A new library must be created with expected revision 0.",
        ));
    }
    let id = mint_uuid_v4()?;
    let library_id = LibraryId::new(id.clone()).map_err(|_| LibraryError::Internal)?;
    let record = LibraryRecord {
        library: Library::new(library_id.clone(), repository_id.clone()),
        name: name.to_string(),
    };
    let mut out = planned(vec![CatalogChange::CreateLibrary(record)], [id.as_str()]);
    out.library_id = Some(library_id);
    out.created_id = Some(id);
    Ok(out)
}

/// The order key of the first member added after `last` (the current last member's key).
fn first_member_order(last: Option<i64>) -> Result<i64, LibraryError> {
    match last {
        None => Ok(0),
        Some(key) => key.checked_add(1).ok_or_else(order_exhausted),
    }
}

/// The error when a virtual collection has used every membership order key.
fn order_exhausted() -> LibraryError {
    LibraryError::invalid(
        "membership_order_exhausted",
        "This virtual collection cannot take more members.",
    )
}

/// Plan a new, empty collection in `lib` (the folder, if any, is already checked).
fn new_collection(
    lib: &LibraryId,
    title: &str,
    folder_id: Option<&FolderId>,
    sort: CollectionSort,
) -> Result<Planned, LibraryError> {
    let id = mint_uuid_v4()?;
    let collection_id = CollectionId::new(id.clone()).map_err(|_| LibraryError::Internal)?;
    let mut collection = Collection::new(collection_id, lib.clone(), title)
        .map_err(|_| LibraryError::invalid("invalid_name", "A name must not be empty."))?
        .with_sort(sort);
    if let Some(folder) = folder_id {
        collection = collection.with_folder(folder.clone());
    }
    let mut out = planned(
        vec![CatalogChange::PutCollection(collection)],
        [id.as_str()],
    );
    out.created_id = Some(id);
    Ok(out)
}

/// Plan a new virtual collection in `lib`: rule-based with `expression`, else hand-picked.
fn create_virtual(
    lib: &LibraryId,
    title: &str,
    expression: Option<&str>,
) -> Result<Planned, LibraryError> {
    let id = mint_uuid_v4()?;
    let vc_id = VirtualCollectionId::new(id.clone()).map_err(|_| LibraryError::Internal)?;
    let mut vc = VirtualCollection::new(vc_id, lib.clone(), title)
        .map_err(|_| LibraryError::invalid("invalid_name", "A name must not be empty."))?;
    if let Some(rule) = expression {
        vc = vc.with_expression(rule);
    }
    let mut out = planned(vec![CatalogChange::PutVirtualCollection(vc)], [id.as_str()]);
    out.created_id = Some(id);
    Ok(out)
}

/// `item` with its `title` replaced and every other field kept, through its wire form (the
/// core types validate the title again on the way back).
fn retitled<T: Serialize + DeserializeOwned>(item: &T, title: &str) -> Result<T, LibraryError> {
    let mut wire = serde_json::to_value(item).map_err(|err| {
        // Stdout only: the service journals the command's failure once (`internal`).
        tracing::info!(target: super::LOG_TARGET, error_code = "internal", detail = %err, "retitle encode failed");
        LibraryError::Internal
    })?;
    wire["title"] = serde_json::Value::String(title.to_string());
    serde_json::from_value(wire)
        .map_err(|_| LibraryError::invalid("invalid_name", "A name must not be empty."))
}

/// A plan of `changes` touching `targets`.
fn planned<'t>(changes: Vec<CatalogChange>, targets: impl IntoIterator<Item = &'t str>) -> Planned {
    Planned {
        changes,
        targets: targets.into_iter().map(str::to_string).collect(),
        ..Planned::default()
    }
}

/// A plan of `changes` touching `container` (if any) and `file_ids`.
fn planned_files(
    changes: Vec<CatalogChange>,
    container: Option<&str>,
    file_ids: &[FileId],
) -> Planned {
    let files = file_ids.iter().map(FileId::as_str);
    planned(changes, container.into_iter().chain(files))
}
