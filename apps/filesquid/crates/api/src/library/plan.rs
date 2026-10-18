//! What each library command means: the checks it needs and the catalog changes it makes.
//!
//! The [`Planner`] reads committed state through the injected catalog and turns one
//! validated [`LibraryCommand`] into a [`Planned`] change set. It writes nothing; the service
//! commits the result. Every referenced collection, virtual collection and file must belong
//! to the command's library, or the command is `not_found`.

use super::catalog::{CatalogChange, FileEntry, LibraryCatalog, LibraryRecord};
use super::command::{LibraryCommand, NEW_LIBRARY_REVISION};
use super::ids::mint_uuid_v4;
use super::policy::{Deletion, DeletionPolicy, FileState, PurgeConfirmation};
use super::LibraryError;
use crate::operation::CommandEnvelope;
use filesquid_core::{
    Collection, CollectionId, CollectionSort, FileId, FolderId, Library, LibraryId, RepositoryId,
    VirtualCollection, VirtualCollectionId, VirtualMembership,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Membership order keys per millisecond, so a batch keeps its order after earlier additions.
const ORDER_KEYS_PER_MS: i64 = 1_000;

/// The changes one command makes, and what it answers.
#[derive(Debug, Default)]
pub(super) struct Planned {
    /// The library changed.
    pub library_id: Option<LibraryId>,
    /// The library's revision before the command (answered when nothing changes).
    pub revision: Option<String>,
    /// The catalog changes, in order; empty when the command changes nothing.
    pub changes: Vec<CatalogChange>,
    /// Identifier the command created.
    pub created_id: Option<String>,
    /// Opaque identifiers of what was touched, for the journal.
    pub targets: Vec<String>,
    /// The deletion record of a recoverable deletion.
    pub deletion: Option<Deletion>,
}

/// Plans commands against committed catalog state at one instant.
pub(super) struct Planner<'a> {
    /// The injected catalog (read only here).
    pub catalog: &'a dyn LibraryCatalog,
    /// The command's time, Unix milliseconds (UTC).
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
        let revision = self.check_revision(library_id, expected)?;
        let mut planned = self.plan_in(library_id, command)?;
        planned.library_id = Some(library_id.clone());
        planned.revision = Some(revision);
        planned.targets.insert(0, library_id.as_str().to_string());
        Ok(planned)
    }

    /// The library's current revision, when it exists and equals `expected`.
    fn check_revision(
        &self,
        library_id: &LibraryId,
        expected: &str,
    ) -> Result<String, LibraryError> {
        let state = self
            .catalog
            .library(library_id)?
            .ok_or(LibraryError::NotFound("library"))?;
        if state.revision != expected {
            return Err(LibraryError::Conflict);
        }
        Ok(state.revision)
    }

    /// Plan a command of an existing library.
    fn plan_in(&self, lib: &LibraryId, command: &LibraryCommand) -> Result<Planned, LibraryError> {
        use LibraryCommand as C;
        match command {
            C::CreateCollection {
                title,
                folder_id,
                sort,
                ..
            } => create_collection(lib, title, folder_id.as_ref(), *sort),
            C::RenameCollection {
                collection_id,
                title,
                ..
            } => self.rename_collection(lib, collection_id, title),
            C::DeleteCollection { collection_id, .. } => self.delete_collection(lib, collection_id),
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
            other => self.change_states(lib, other),
        }
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

    /// Rename a collection of `lib`.
    fn rename_collection(
        &self,
        lib: &LibraryId,
        id: &CollectionId,
        title: &str,
    ) -> Result<Planned, LibraryError> {
        let renamed = retitled(&self.collection_in(lib, id)?, title)?;
        Ok(planned(
            vec![CatalogChange::PutCollection(renamed)],
            [id.as_str()],
        ))
    }

    /// Delete a collection of `lib`; its files stay and become unfiled.
    fn delete_collection(
        &self,
        lib: &LibraryId,
        id: &CollectionId,
    ) -> Result<Planned, LibraryError> {
        self.collection_in(lib, id)?;
        Ok(planned(
            vec![CatalogChange::RemoveCollection(id.clone())],
            [id.as_str()],
        ))
    }

    /// Rename a virtual collection of `lib`.
    fn rename_virtual(
        &self,
        lib: &LibraryId,
        id: &VirtualCollectionId,
        title: &str,
    ) -> Result<Planned, LibraryError> {
        let renamed = retitled(&self.virtual_in(lib, id)?, title)?;
        Ok(planned(
            vec![CatalogChange::PutVirtualCollection(renamed)],
            [id.as_str()],
        ))
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

    /// Add the files that are not members yet to a hand-picked virtual collection.
    fn add_members(
        &self,
        lib: &LibraryId,
        id: &VirtualCollectionId,
        file_ids: &[FileId],
    ) -> Result<Planned, LibraryError> {
        self.hand_picked_in(lib, id)?;
        let base = self.now_ms.saturating_mul(ORDER_KEYS_PER_MS);
        let mut changes = Vec::new();
        for (index, file_id) in file_ids.iter().enumerate() {
            self.file_in(lib, file_id)?;
            if self.catalog.is_virtual_member(id, file_id)? {
                continue;
            }
            let order = base.saturating_add(i64::try_from(index).unwrap_or(i64::MAX));
            let member = VirtualMembership::new(id.clone(), file_id.clone(), order);
            changes.push(CatalogChange::AddVirtualMember(member));
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
    ) -> Result<Planned, LibraryError> {
        let deletion = self.policy.deletion_at(self.now_ms);
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
        LibraryCommand::RestoreFiles { .. } => Box::new(|s| FileState {
            deletion: None,
            ..s
        }),
        _ => return Err(LibraryError::Internal),
    };
    Ok(next)
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

/// Plan a new, empty collection in `lib`.
fn create_collection(
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
        tracing::error!(error_code = "internal", detail = %err, "retitle encode failed");
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
