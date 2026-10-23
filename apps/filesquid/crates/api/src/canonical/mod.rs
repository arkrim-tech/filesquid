//! The FileSquid OCFL application profile: how a library's organisation and files become
//! canonical OCFL objects, and how they are read back (Task DE-0.3; API-S-008, API-S-026;
//! [ADR 005](../../../../../../resources/architecture/005-sqlite-local-postgres-hosted.md),
//! [ADR 010](../../../../../../resources/architecture/010-ocfl-1-1-dual-profile.md),
//! [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)).
//!
//! The meaning lives here once, for every host: the layout ([`layout`]), the entity encoding
//! ([`entity`]), the operation message that carries a receipt ([`message`]), a file's original
//! and its metadata record holding its original name ([`original`]), the name a sharing
//! or publication output gives a file ([`outgoing`]), the profile
//! transform seam ([`ProfileTransform`]), and commits and reads through the `filesquid-ocfl`
//! change model, generic over the host's storage backend ([`store`]). A host coordinates the
//! sequence of ADR 005 around it: record intent, commit the snapshot version, then update its
//! catalog from the published receipt, and reconcile from the snapshot after a crash.

pub mod collection_path;
pub mod copy;
pub mod entity;
mod error;
pub mod layout;
pub mod message;
pub mod original;
pub mod outgoing;
pub mod store;
mod transform;
mod unregistered;
pub mod upload;

pub use copy::{copy_record, source_reference, CopyInput, SOURCE_TERM};
pub use entity::{LibraryHead, SnapshotEntity, SnapshotFile, VirtualEntry, ENTITY_SCHEMA};
pub use error::CanonicalError;
pub use layout::{
    file_object_id, file_object_root, snapshot_object_id, touched, SnapshotKey, SNAPSHOT_ROOT,
};
pub use message::{operation_kind, operation_message, parse_operation_message, OperationRecord};
pub use original::{FileOriginal, COMPANION_SUFFIX};
pub use outgoing::{outgoing_name, OutgoingNames};
pub use store::{
    commit_file_original, commit_snapshot, commit_snapshot_with, create_storage_root,
    read_commit_journal, read_file_original, read_object_journal, read_snapshot,
    read_version_journal, snapshot_library_id, LibrarySnapshot, SnapshotDelta, HISTORY_PREFIX,
    JOURNAL_SUFFIX, MAX_JOURNAL_LINES,
};
pub use transform::{OwnerReadable, ProfileTransform};
pub use unregistered::{unregistered_file_objects, FILES_ROOT};
pub use upload::{ascii_name_form, new_upload_record, reupload_record, UploadInput};
