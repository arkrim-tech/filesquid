//! File operation routes (Task API-F1.3; API-S-004, API-S-005, API-S-008, API-S-018,
//! OCFL-S-012): browse and edit the OCFL objects of an open library like a drive.
//!
//! Every route takes the library and the OCFL object by identifier; the file and folder
//! routes also take a file or folder by its logical path, while the object-versions route is
//! object-level and takes none. Every answer carries storage facts only, never a filesystem path, storage key or
//! OCFL content path (API-S-005):
//!
//! - **Reads** (action `files.read`): a folder listing by logical-path prefix with an opaque
//!   cursor and 1 to 1,000 entries per page ([`DEFAULT_PAGE`] by default), file details,
//!   a file's version history, an object's versions, and ranged content reads with one
//!   `Range` per request, each answer capped at [`MAX_ANSWER_BYTES`] (the desktop scheme's
//!   buffer cap, DE-S-005).
//! - **Writes** (action `files.write`): upload, move or rename, remove and restore. Each one
//!   calls the IO-1.2 view API, which publishes exactly one version through the
//!   `filesquid-ocfl` change model, or is refused without publishing (nothing would change,
//!   another change is staged, the head is incomplete); there is no second write path
//!   (API-S-008). Creating a folder writes nothing: OCFL stores no empty folder, so a new
//!   folder stays pending in the client until a file is added beneath it.
//! - **Journal.** Every refusal is journaled once (an authorisation refusal as
//!   `api.auth.denied`, any other as the operation with outcome `failed` and its code), and
//!   every published version as the operation with opaque IDs only; successful reads are not
//!   journaled (API-S-017).
//!
//! The host injects each library's storage root, reservations and staging backend
//! ([`FileLibrary`]) into a [`FileService`]; this module does no storage I/O of its own
//! (API-S-006).

mod error;
mod handle;
pub mod media;
pub mod names;
mod range;
mod routes;
mod service;
mod view;

pub use error::FileError;
pub use handle::{FileFuture, FileOperations};
pub use names::{DisplayNames, NameFuture, StoredFile};
pub use range::{RangePlan, RangeRequest};
pub(crate) use routes::{answer_content, refuse as refuse_file, routes, ContentAnswerKind};
pub use service::{
    staging_prefix, ContentAnswer, ContentRequest, FileClock, FileLibrary, FileService,
};
pub use view::{
    CommitView, ContentQuery, EntryView, FileChangeView, FileDetailsView, FileQuery,
    FolderPageView, FolderRequest, FolderView, HistoryEntryView, HistoryQuery, HistoryView,
    ListingQuery, MintRequest, MintView, MoveRequest, ObjectQuery, RemoveRequest, RestoreRequest,
    UploadQuery, UserView, VersionView, VersionsView,
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

/// Most bytes of a version message.
pub const MAX_MESSAGE_BYTES: usize = 1_024;

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
/// Journal operation of a refused content read.
pub const READ_OPERATION: &str = "files.read_content";
/// Journal operation of an upload.
pub const UPLOAD_OPERATION: &str = "files.upload";
/// Journal operation of a refused folder creation (a created folder writes nothing).
pub const FOLDER_OPERATION: &str = "files.create_folder";
/// Journal operation of a move.
pub const MOVE_OPERATION: &str = "files.move";
/// Journal operation of a removal.
pub const REMOVE_OPERATION: &str = "files.remove";
/// Journal operation of a restore.
pub const RESTORE_OPERATION: &str = "files.restore";
