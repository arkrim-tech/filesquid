//! File operation routes (Task API-F1.3; API-S-004, API-S-005, API-S-008, API-S-018,
//! OCFL-S-012): browse and edit the OCFL objects of an open library like a drive.
//!
//! The object routes take the library and the OCFL object by identifier; the file and folder
//! routes also take a file or folder by its logical path, while the object-versions route is
//! object-level and takes none. The metadata routes and copy take a file by its FileId, the
//! view by navigation node and storage use per collection by collection ID. Every answer
//! carries storage facts only, never a filesystem path, storage key or OCFL content path
//! (API-S-005):
//!
//! - **Reads** (action `files.read`): a folder listing by logical-path prefix with an opaque
//!   cursor and 1 to 1,000 entries per page ([`DEFAULT_PAGE`] by default), file details,
//!   a file's version history, an object's versions, and ranged content reads with one
//!   `Range` per request, each answer capped at [`MAX_ANSWER_BYTES`] (the desktop scheme's
//!   buffer cap, DE-S-005). A listing names its folder by library, object and prefix, or by a
//!   collection or folder navigation node readable to the caller (Task API-F2.2), and leaves
//!   out the [`RESERVED_FOLDERS`]. On `filesquid-server` the whole-file download streams one
//!   file version of any size in ranged reads as an attachment; the desktop scheme caps every
//!   answer at [`MAX_ANSWER_BYTES`], so the desktop saves a larger file with ranged content
//!   reads. A refusal, or a failure after the answer started, is journaled once as
//!   [`DOWNLOAD_OPERATION`].
//! - **Writes** (action `files.write`): upload, move, rename, remove and restore. A rename
//!   sets only the display name in the file's metadata record (Task API-F1.13, API-S-027),
//!   and a file object's stored original never moves. Each one
//!   publishes exactly one version through the `filesquid-ocfl` change model, or is refused
//!   without publishing (nothing would change, another change is staged, the head is
//!   incomplete); there is no second write path (API-S-008). Move and remove stage their
//!   renames and removals on the change model themselves rather than through
//!   `filesquid_ocfl::view`, because their commit writes version-bound files (the PROV-O
//!   record and journal entries of API-S-028), which the view writes do not take. An upload creates the file's own
//!   object `files/<file-id>` with its original and metadata record, or a new version of it
//!   (Task API-F1.4, API-S-026). Creating a folder writes nothing: OCFL stores no empty folder, so a new
//!   folder stays pending in the client until a file is added beneath it.
//! - **Drive** (Task API-F2.2 part 2, API-S-011; [`drive`]): the view operation
//!   (`GET /files/view`, `files.read`) of a library, collection or virtual-collection node
//!   with stable cursor paging; storage use per collection
//!   (`GET /files/collection-history-size`, `files.read`); and copy
//!   (`POST /files/copies`, `files.write`), a new file object with a new FileId whose record
//!   names its source.
//! - **Search** (Task API-F2.3, API-S-012; [`search`]): `GET /files/search` (`files.read`)
//!   answers the readable files and folder rows matching a query of the published grammar
//!   ([`search_query`]), permission-filtered before ranking, paged like the view.
//! - **Metadata properties** (Task API-F1.4, API-S-035; [`statements`]): reads of a file's
//!   statements, of the add order of a bare term name, of the library's namespace registry and
//!   of the read-only import-record projection take `files.read`; setting or removing a
//!   statement and registering a namespace take `files.write`, each one version through the
//!   metadata module's `ocfl` store.
//! - **Version history** (Task API-F2.5, API-S-014): file history entries carry each
//!   version's size and label, object versions carry labels, a restore is one `revert` or
//!   `reinstate` version that may carry a label, a label can be set or cleared later as
//!   application data in the object ([`labels`]), and history sizes are reported per file
//!   and per library. No route deletes, expires or thins a version.
//! - **Journal.** Every refusal is journaled once (an authorisation refusal as
//!   `api.auth.denied`, any other as the operation with outcome `failed` and its code), and
//!   every published version as `ocfl.commit` and the operation with opaque IDs only, plus
//!   `metadata.edit` for a write of a metadata record (API-S-028, [`Committed`]); successful
//!   reads are not journaled (API-S-017).
//!
//! The host injects each library's storage root, reservations and staging backend
//! ([`FileLibrary`]) into a [`FileService`]; this module does no storage I/O of its own
//! (API-S-006).

pub mod command;
pub mod copy;
mod download;
pub mod drive;
mod drive_facts;
mod drive_routes;
pub mod drive_scope;
mod error;
mod handle;
mod homing;
mod import;
pub mod labels;
pub mod media;
mod metadata_routes;
pub mod names;
mod nodes;
mod provenance;
mod range;
pub mod receipts;
mod records;
mod routes;
pub mod scope_listing;
pub mod search;
pub mod search_match;
pub mod search_query;
pub mod search_scope;
mod service;
pub mod statements;
mod upload;
mod versions;
mod view;

pub use command::{CommandAnswer, ConflictAnswer, FileCommand, NEW_FILE_REVISION};
pub use download::{Download, DownloadStream};
pub use drive_facts::{CollectionHistorySizeView, CollectionSizeQuery, FactsAnswer};
pub use error::FileError;
pub use handle::{FileFuture, FileOperations};
pub use import::SourceImport;
pub use labels::{APP_DATA_FOLDER, LABELS_PATH, MAX_LABELS, PROVENANCE_FOLDER, RESERVED_FOLDERS};
pub use names::{DisplayNames, NameFuture, StoredFile};
pub use nodes::{Listing, ScopeObject, ScopeObjects};
pub use provenance::{Caller, Committed, FileProvenance};
pub use range::{RangePlan, RangeRequest};
pub use receipts::{CommandReceipts, MAX_RECEIPTS};
pub(crate) use routes::{answer_content, refuse as refuse_file, routes, ContentAnswerKind};
pub use scope_listing::{ScopeAfter, ScopeEntry, ScopeListing, ScopePage};
pub use service::{
    staging_prefix, ContentAnswer, ContentRequest, FileClock, FileLibrary, FileService,
};
pub use versions::{MAX_SIZE_CONTENTS, MAX_SIZE_OBJECTS};
pub use view::{
    CommitView, ContentQuery, EntryView, FileChangeView, FileDetailsView, FileHistorySizeView,
    FileQuery, FolderPageView, FolderRequest, FolderView, HistoryEntryView, HistoryQuery,
    HistoryView, LabelRequest, LibraryHistorySizeView, LibraryQuery, ListingQuery, ListingRequest,
    MintRequest, MintView, MoveRequest, ObjectQuery, RemoveRequest, RestoreRequest, UploadQuery,
    UploadView, UserView, VersionView, VersionsView,
};

/// `tracing` target of this module's stdout-only diagnostics, outside `filesquid::activity`
/// so the journal records each outcome once, through the routes.
const LOG_TARGET: &str = "filesquid::files";

/// Entries in a listing page when the request names no limit.
pub const DEFAULT_PAGE: usize = 200;

/// Most bytes one content answer carries (16 MiB): the desktop scheme buffers whole answers
/// and refuses larger ones (DE-S-005), so every host answers the same capped ranges.
pub const MAX_ANSWER_BYTES: usize = 16 * 1024 * 1024;

/// Most bytes one upload carries (64 MiB). The body is held in memory while it is staged.
pub const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

/// Most bytes of a caller's version message. The message is the version label, the `label`
/// member of the version's `filesquid.operation.v1` message (API-S-028, PRD F-065), so it
/// has the label's bound.
pub const MAX_MESSAGE_BYTES: usize = filesquid_core::MAX_LABEL_BYTES;

/// Most files or folders one restore names.
pub const MAX_RESTORE_PATHS: usize = 1_000;

/// The writer name the API's commits reserve versions as.
pub const HOLDER: &str = "filesquid-api";

/// Journal operation of a refused listing.
pub const LIST_OPERATION: &str = "files.list";
/// Journal operation of a refused details read.
pub const DETAILS_OPERATION: &str = "files.details";
/// Journal operation of a refused history read.
pub const HISTORY_OPERATION: &str = "files.history";
/// Journal operation of a refused versions read.
pub const VERSIONS_OPERATION: &str = "files.versions";
/// Journal operation of a refused or failed whole-file download.
pub const DOWNLOAD_OPERATION: &str = "files.download";
/// Journal operation of a refused statements read.
pub const METADATA_READ_OPERATION: &str = "files.metadata.read";
/// Journal operation of a rename (the display name only).
pub const RENAME_OPERATION: &str = "files.rename";
/// Journal operation of a copy (Task API-F2.2).
pub const COPY_OPERATION: &str = "file.copy";
/// Journal operation of a refused view (Task API-F2.2).
pub const VIEW_OPERATION: &str = "files.view";
/// Journal operation of a refused search (Task API-F2.3).
pub const SEARCH_OPERATION: &str = "files.search";
/// Journal operation of a refused storage-use count of a collection (Task API-F2.2).
pub const COLLECTION_HISTORY_SIZE_OPERATION: &str = "files.collection_history_size";
/// Journal operation of a statement set.
pub const METADATA_SET_OPERATION: &str = "files.metadata.set";
/// Journal operation of a statement removal.
pub const METADATA_REMOVE_OPERATION: &str = "files.metadata.remove";
/// Journal operation of a refused term lookup.
pub const TERMS_OPERATION: &str = "files.metadata.terms";
/// Journal operation of a refused registry read.
pub const NAMESPACES_OPERATION: &str = "files.metadata.namespaces";
/// Journal operation of a namespace registration.
pub const NAMESPACE_OPERATION: &str = "files.metadata.register";
/// Journal operation every write of a metadata record adds beside its route's entry: a
/// statement set or removal, a namespace registration and a rename (API-S-028).
pub const METADATA_EDIT_OPERATION: &str = "metadata.edit";
/// Journal operation of a refused import-record read.
pub const FACTS_OPERATION: &str = "files.facts";
/// Journal operation of a refused content read.
pub const READ_OPERATION: &str = "files.read_content";
/// Journal operation of an upload.
pub const UPLOAD_OPERATION: &str = "file.upload";
/// Journal operation of a refused folder creation (a created folder writes nothing).
pub const FOLDER_OPERATION: &str = "files.create_folder";
/// Journal operation of a move.
pub const MOVE_OPERATION: &str = "file.move";
/// Journal operation of a removal.
pub const REMOVE_OPERATION: &str = "files.remove";
/// Journal operation of a restore.
pub const RESTORE_OPERATION: &str = "files.restore";
/// Journal operation of a label set or cleared after commit.
pub const LABEL_OPERATION: &str = "files.label";
/// Journal operation of a refused file history-size read.
pub const HISTORY_SIZE_OPERATION: &str = "files.history_size";
/// Journal operation of a refused library history-size read.
pub const LIBRARY_HISTORY_SIZE_OPERATION: &str = "files.library_history_size";
