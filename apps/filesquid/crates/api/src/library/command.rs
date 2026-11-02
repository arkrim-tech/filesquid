//! Versioned library commands (API-S-019).
//!
//! A [`CommandRequest`] is a `filesquid-core` [`CommandEnvelope`] (schema version, operation
//! ID as idempotency key, expected revision, scope) plus one [`LibraryCommand`]. Commands use
//! the internal nouns of F-049: library, collection, virtual collection and file. Every
//! command except [`LibraryCommand::CreateLibrary`] names its library, and everything it
//! references must belong to that library. Commands carry opaque IDs and names only; never a
//! filesystem path or SQL.

use super::policy::{is_retention, Deletion, PurgeConfirmation};
use super::LibraryError;
use crate::operation::{CommandEnvelope, CommandOutcome, MAX_PAGE_SIZE};
use filesquid_core::{
    CollectionId, CollectionSort, FileId, FolderId, LibraryId, RepositoryId, VirtualCollectionId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Expected revision of a library that does not exist yet; [`LibraryCommand::CreateLibrary`]
/// MUST send it. A created library starts at revision `1`.
pub const NEW_LIBRARY_REVISION: &str = "0";

/// Longest library name, collection title or virtual-collection title, in bytes.
pub const MAX_NAME_BYTES: usize = 1024;

/// Most files one command may name: the portable page bound (CORE-S-005).
pub const MAX_FILES_PER_COMMAND: usize = MAX_PAGE_SIZE as usize;

/// Longest rule expression of a rule-based virtual collection, in bytes.
pub const MAX_EXPRESSION_BYTES: usize = 4096;

/// Title of the collection a restore creates when a file's home collection is gone (owner
/// decision 2026-10-22, ADR 041 Decision 9); later ones are numbered from 2.
pub const UNTITLED_COLLECTION_TITLE: &str = "untitled collection";

/// Most titles a restore tries for that collection: "untitled collection" to "untitled
/// collection 9999" (owner decision 2026-10-22, "Caps: 9999 for both").
pub const MAX_UNTITLED_TITLES: u32 = 9_999;

/// One library command. Serialized with its kind in the `command` member, in `snake_case`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum LibraryCommand {
    /// Create a library on `repository_id`. Needs [`NEW_LIBRARY_REVISION`]; answers the new
    /// library's UUID v4 identifier.
    CreateLibrary {
        /// Repository (FileSquid installation) that hosts the library.
        repository_id: RepositoryId,
        /// Name shown to people.
        name: String,
    },
    /// Create an empty collection; answers its UUID v4 identifier.
    CreateCollection {
        /// Owning library.
        library_id: LibraryId,
        /// Title shown to people.
        title: String,
        /// Folder of the same library that holds the collection, if any; a folder that does
        /// not exist or belongs to another library is `not_found`.
        #[serde(default)]
        folder_id: Option<FolderId>,
        /// Stored sort order.
        #[serde(default)]
        sort: CollectionSort,
    },
    /// Rename a collection.
    RenameCollection {
        /// Owning library.
        library_id: LibraryId,
        /// Collection to rename.
        collection_id: CollectionId,
        /// New title.
        title: String,
    },
    /// Delete a collection. Its files that are not deleted yet are deleted with it, in place
    /// (ADR 041 Decision 8).
    DeleteCollection {
        /// Owning library.
        library_id: LibraryId,
        /// Collection to delete.
        collection_id: CollectionId,
    },
    /// Create a virtual collection: rule-based with `expression`, hand-picked without.
    CreateVirtualCollection {
        /// Owning library.
        library_id: LibraryId,
        /// Title shown to people.
        title: String,
        /// Versioned Any/All rule of a rule-based virtual collection.
        #[serde(default)]
        expression: Option<String>,
    },
    /// Rename a virtual collection.
    RenameVirtualCollection {
        /// Owning library.
        library_id: LibraryId,
        /// Virtual collection to rename.
        virtual_collection_id: VirtualCollectionId,
        /// New title.
        title: String,
    },
    /// Delete a virtual collection. Its member files and their collections are retained.
    DeleteVirtualCollection {
        /// Owning library.
        library_id: LibraryId,
        /// Virtual collection to delete.
        virtual_collection_id: VirtualCollectionId,
    },
    /// Add files to a hand-picked virtual collection; members already present are kept.
    AddToVirtualCollection {
        /// Owning library.
        library_id: LibraryId,
        /// Hand-picked virtual collection.
        virtual_collection_id: VirtualCollectionId,
        /// Files to add (1 to [`MAX_FILES_PER_COMMAND`], distinct).
        file_ids: Vec<FileId>,
    },
    /// Remove files from a hand-picked virtual collection; their collection is unchanged.
    RemoveFromVirtualCollection {
        /// Owning library.
        library_id: LibraryId,
        /// Hand-picked virtual collection.
        virtual_collection_id: VirtualCollectionId,
        /// Files to remove (1 to [`MAX_FILES_PER_COMMAND`], distinct).
        file_ids: Vec<FileId>,
    },
    /// Hide files from ordinary browsing.
    HideFiles {
        /// Owning library.
        library_id: LibraryId,
        /// Files to hide.
        file_ids: Vec<FileId>,
    },
    /// Show hidden files again.
    UnhideFiles {
        /// Owning library.
        library_id: LibraryId,
        /// Files to unhide.
        file_ids: Vec<FileId>,
    },
    /// Delete files recoverably; they can be restored until their purge-after time.
    DeleteFiles {
        /// Owning library.
        library_id: LibraryId,
        /// Files to delete.
        file_ids: Vec<FileId>,
    },
    /// Restore recoverably deleted files into their original object and logical path; a
    /// hidden file stays hidden. A file whose home collection is gone is restored into a new
    /// "untitled collection" (ADR 041 Decision 9).
    RestoreFiles {
        /// Owning library.
        library_id: LibraryId,
        /// Files to restore.
        file_ids: Vec<FileId>,
    },
    /// Permanently delete recoverably deleted files after an explicit scope confirmation.
    PurgeFiles {
        /// Owning library.
        library_id: LibraryId,
        /// Recoverably deleted files to purge.
        file_ids: Vec<FileId>,
        /// The library and exact file count the owner confirmed.
        confirmation: PurgeConfirmation,
    },
    /// Set the library's own retention period of deleted files, or clear it so the
    /// installation default applies (ADR 041 Decision 10). It never changes the
    /// `purge_after` of a file already deleted.
    SetLibraryRetention {
        /// Owning library.
        library_id: LibraryId,
        /// Days from 1 to 3,650, or `None` for the installation default.
        #[serde(default)]
        retention_days: Option<u32>,
    },
}

impl LibraryCommand {
    /// Dotted journal operation name, for example `library.collection.rename`.
    pub fn operation(&self) -> &'static str {
        match self {
            Self::CreateLibrary { .. } => "library.create",
            Self::CreateCollection { .. } => "library.collection.create",
            Self::RenameCollection { .. } => "library.collection.rename",
            Self::DeleteCollection { .. } => "library.collection.delete",
            Self::CreateVirtualCollection { .. } => "library.virtual_collection.create",
            Self::RenameVirtualCollection { .. } => "library.virtual_collection.rename",
            Self::DeleteVirtualCollection { .. } => "library.virtual_collection.delete",
            Self::AddToVirtualCollection { .. } => "library.virtual_collection.add",
            Self::RemoveFromVirtualCollection { .. } => "library.virtual_collection.remove",
            Self::HideFiles { .. } => "library.file.hide",
            Self::UnhideFiles { .. } => "library.file.unhide",
            Self::DeleteFiles { .. } => "library.file.delete",
            Self::RestoreFiles { .. } => "library.file.restore",
            Self::PurgeFiles { .. } => "library.file.purge",
            Self::SetLibraryRetention { .. } => "library.retention.set",
        }
    }

    /// The library the command changes; `None` for [`Self::CreateLibrary`].
    pub fn library_id(&self) -> Option<&LibraryId> {
        match self {
            Self::CreateLibrary { .. } => None,
            Self::CreateCollection { library_id, .. }
            | Self::RenameCollection { library_id, .. }
            | Self::DeleteCollection { library_id, .. }
            | Self::CreateVirtualCollection { library_id, .. }
            | Self::RenameVirtualCollection { library_id, .. }
            | Self::DeleteVirtualCollection { library_id, .. }
            | Self::AddToVirtualCollection { library_id, .. }
            | Self::RemoveFromVirtualCollection { library_id, .. }
            | Self::HideFiles { library_id, .. }
            | Self::UnhideFiles { library_id, .. }
            | Self::DeleteFiles { library_id, .. }
            | Self::RestoreFiles { library_id, .. }
            | Self::PurgeFiles { library_id, .. }
            | Self::SetLibraryRetention { library_id, .. } => Some(library_id),
        }
    }

    /// Check the rules that need no catalog: names, expressions and file batches.
    ///
    /// # Errors
    ///
    /// Returns an `invalid_request` [`LibraryError`] naming the broken rule.
    pub fn validate(&self) -> Result<(), LibraryError> {
        match self {
            Self::CreateLibrary { name, .. } => check_name(name),
            Self::CreateCollection { title, .. }
            | Self::RenameCollection { title, .. }
            | Self::RenameVirtualCollection { title, .. } => check_name(title),
            Self::CreateVirtualCollection {
                title, expression, ..
            } => check_name(title).and_then(|()| check_expression(expression.as_deref())),
            Self::DeleteCollection { .. } | Self::DeleteVirtualCollection { .. } => Ok(()),
            Self::AddToVirtualCollection { file_ids, .. }
            | Self::RemoveFromVirtualCollection { file_ids, .. }
            | Self::HideFiles { file_ids, .. }
            | Self::UnhideFiles { file_ids, .. }
            | Self::DeleteFiles { file_ids, .. }
            | Self::RestoreFiles { file_ids, .. }
            | Self::PurgeFiles { file_ids, .. } => check_files(file_ids),
            Self::SetLibraryRetention { retention_days, .. } => check_retention(*retention_days),
        }
    }

    /// The files a file-batch command names; empty for every other command.
    pub fn file_ids(&self) -> &[FileId] {
        match self {
            Self::AddToVirtualCollection { file_ids, .. }
            | Self::RemoveFromVirtualCollection { file_ids, .. }
            | Self::HideFiles { file_ids, .. }
            | Self::UnhideFiles { file_ids, .. }
            | Self::DeleteFiles { file_ids, .. }
            | Self::RestoreFiles { file_ids, .. }
            | Self::PurgeFiles { file_ids, .. } => file_ids,
            Self::CreateLibrary { .. }
            | Self::CreateCollection { .. }
            | Self::RenameCollection { .. }
            | Self::DeleteCollection { .. }
            | Self::CreateVirtualCollection { .. }
            | Self::RenameVirtualCollection { .. }
            | Self::DeleteVirtualCollection { .. }
            | Self::SetLibraryRetention { .. } => &[],
        }
    }
}

/// A name or title is non-empty and at most [`MAX_NAME_BYTES`] bytes.
fn check_name(name: &str) -> Result<(), LibraryError> {
    if name.trim().is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(LibraryError::invalid(
            "invalid_name",
            "A name must not be empty or longer than 1,024 bytes.",
        ));
    }
    Ok(())
}

/// A rule expression, when present, is non-empty and at most [`MAX_EXPRESSION_BYTES`].
fn check_expression(expression: Option<&str>) -> Result<(), LibraryError> {
    match expression {
        Some(rule) if rule.trim().is_empty() || rule.len() > MAX_EXPRESSION_BYTES => Err(
            LibraryError::invalid("invalid_expression", "The rule is empty or too long."),
        ),
        _ => Ok(()),
    }
}

/// A retention period, when set, is 1 to 3,650 days.
fn check_retention(days: Option<u32>) -> Result<(), LibraryError> {
    match days {
        Some(days) if !is_retention(days) => Err(LibraryError::invalid(
            "invalid_retention",
            "Choose a period between 1 and 3,650 days.",
        )),
        _ => Ok(()),
    }
}

/// A file batch names 1 to [`MAX_FILES_PER_COMMAND`] distinct files.
fn check_files(file_ids: &[FileId]) -> Result<(), LibraryError> {
    if file_ids.is_empty() || file_ids.len() > MAX_FILES_PER_COMMAND {
        return Err(LibraryError::invalid(
            "invalid_file_count",
            "Choose between 1 and 1,000 files.",
        ));
    }
    let distinct: BTreeSet<&str> = file_ids.iter().map(FileId::as_str).collect();
    if distinct.len() != file_ids.len() {
        return Err(LibraryError::invalid(
            "duplicate_file",
            "A file is named more than once.",
        ));
    }
    Ok(())
}

/// A versioned library command: the core envelope plus the command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRequest {
    /// Schema version, operation (idempotency) ID, expected revision and scope.
    pub envelope: CommandEnvelope,
    /// The command.
    pub command: LibraryCommand,
}

/// The answer to a library command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    /// `committed` (with the operation ID) when the change is durable in the catalog, or
    /// when the command changes nothing (then nothing is stored and a retry is planned
    /// again).
    pub outcome: CommandOutcome,
    /// The library the command changed.
    pub library_id: Option<LibraryId>,
    /// The library's revision after the command.
    pub revision: Option<String>,
    /// Identifier of the library, collection or virtual collection the command created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_id: Option<String>,
    /// Deletion and purge-after times of the files this command newly deleted; a replay
    /// answers the first times. A file in the batch that was already deleted keeps its
    /// earlier times, which the deleted view reports per file. Absent when the command newly
    /// deleted no file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion: Option<Deletion>,
    /// The collections a restore created because files' home collections were gone, one per
    /// gone home collection (ADR 041 Decision 9); empty for every other command and absent
    /// from the wire then. A replay of the operation ID answers it without this list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restored_into: Vec<RestoredInto>,
}

/// A collection a restore created for the files of one gone home collection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoredInto {
    /// The gone home collection the files came from.
    pub from_collection_id: CollectionId,
    /// The new collection.
    pub collection_id: CollectionId,
    /// Its title: "untitled collection" or "untitled collection N".
    pub title: String,
}
