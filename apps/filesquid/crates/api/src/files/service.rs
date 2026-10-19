//! [`FileService`]: the file operations of the libraries a host opens, over the storage it
//! injects. Reads go through the IO-1.2 view API on an opened [`Object`]; every write goes
//! through the view API's version-producing calls on a [`ChangeModel`], so each publishes
//! exactly one OCFL version and there is no second write path (API-S-008, OCFL-S-016). This
//! module does no storage I/O of its own: every byte moves through the injected backends
//! (API-S-006).

use super::error::FileError;
use super::names::{safe_name, stored_name, DisplayNames, StoredFile};
use super::range::{RangePlan, RangeRequest};
use super::view::EntryView;
use super::view::{
    CommitView, FileDetailsView, FileQuery, FolderPageView, FolderRequest, FolderView,
    HistoryQuery, HistoryView, ListingQuery, MoveRequest, ObjectQuery, RemoveRequest,
    RestoreRequest, UploadQuery, VersionView, VersionsView,
};
use super::{media, DEFAULT_PAGE, HOLDER, MAX_ANSWER_BYTES, MAX_MESSAGE_BYTES, MAX_RESTORE_PATHS};
use crate::content::ContentTarget;
use filesquid_core::LibraryId;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::inventory::{User, VersionNum};
use filesquid_ocfl::read::{Object, StorageRoot};
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::view::{self, MAX_PAGE_ENTRIES, MAX_RANGE_BYTES};
use filesquid_ocfl::write::{rfc3339_utc, CommitReceipt, VersionReservations};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// The clock commits are dated by: Unix milliseconds (UTC).
pub type FileClock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// What a host injects for one library: the OCFL storage root, the version reservations its
/// writers share, and a staging backend outside the root where the API stages each object's
/// change under `staging_prefix` ([`staging_prefix`]).
pub struct FileLibrary<B, R, S> {
    /// The library's OCFL storage root.
    pub backend: Arc<B>,
    /// The fenced `vN` reservations every writer of the root uses.
    pub reservations: Arc<R>,
    /// Where changes are staged before they commit; never inside the root.
    pub staging: Arc<S>,
    /// The key prefix of this library's staging areas on `staging`.
    pub staging_prefix: String,
}

/// The staging prefix of `object_id` under `base`: one staging area per object, named by a
/// digest so no identifier reaches a storage key.
pub fn staging_prefix(base: &str, object_id: &str) -> String {
    let mut name = DigestAlgorithm::Sha256.digest_hex(object_id.as_bytes());
    name.truncate(32);
    format!("{}/{name}", base.trim_end_matches('/'))
}

/// Most content lengths one library keeps; the cache starts over when it is full.
const MAX_CACHED_SIZES: usize = 4_096;

/// One open library: what the host injected, where each object lives, the lengths of the
/// contents read so far, and the lock that serialises the API's writes to it.
struct Library<B, R, S> {
    files: FileLibrary<B, R, S>,
    roots: Mutex<HashMap<String, String>>,
    /// Content length by content digest. Stored content never changes, so an entry never
    /// goes stale; it spares each ranged read the backend's `head`, which may read the whole
    /// file (the filesystem backend hashes it for its entity tag).
    sizes: Mutex<HashMap<String, u64>>,
    writes: tokio::sync::Mutex<()>,
}

impl<B: StorageBackend, R, S> Library<B, R, S> {
    /// The storage-root path of `object_id`, from the cache or, on a miss, by one scan of the
    /// root.
    async fn locate(&self, object_id: &str) -> Result<String, FileError> {
        if let Some(root) = self.cached(object_id) {
            return Ok(root);
        }
        self.scan().await?;
        self.cached(object_id).ok_or_else(FileError::not_found)
    }

    /// The cached root of `object_id`.
    fn cached(&self, object_id: &str) -> Option<String> {
        lock(&self.roots).get(object_id).cloned()
    }

    /// Read every object root the storage root holds and cache the ones not cached yet. An
    /// object that cannot be opened is left out; a storage outage ends the scan.
    async fn scan(&self) -> Result<(), FileError> {
        let backend = self.files.backend.as_ref();
        let roots = StorageRoot::open(backend).await?.objects().to_vec();
        for root in roots {
            if lock(&self.roots).values().any(|known| *known == root) {
                continue;
            }
            match Object::open(backend, &root).await {
                Ok(object) => {
                    lock(&self.roots).insert(object.id().to_string(), root);
                }
                Err(err) => skip_unreadable(err)?,
            }
        }
        Ok(())
    }

    /// The length of the logical file `path` of `version`, whose content digest is
    /// `digest`: from the cache, or read once from the backend.
    async fn size_of(
        &self,
        object: &Object<'_, B>,
        version: &VersionNum,
        path: &str,
        digest: &str,
    ) -> Result<u64, FileError> {
        if let Some(size) = lock(&self.sizes).get(digest) {
            return Ok(*size);
        }
        let size = view::file_details(object, version, path).await?.size;
        let mut sizes = lock(&self.sizes);
        if sizes.len() >= MAX_CACHED_SIZES {
            sizes.clear();
        }
        sizes.insert(digest.to_string(), size);
        Ok(size)
    }

    /// The staging area of `object_id`.
    fn staging(&self, object_id: &str) -> Result<StagingArea<'_, S>, FileError>
    where
        S: StorageBackend,
    {
        let prefix = staging_prefix(&self.files.staging_prefix, object_id);
        Ok(StagingArea::new(self.files.staging.as_ref(), &prefix)?)
    }
}

/// Keep scanning past an object that cannot be opened, unless the storage is unreachable.
fn skip_unreadable(err: filesquid_ocfl::read::ReadError) -> Result<(), FileError> {
    match FileError::from(err) {
        unavailable if unavailable.code() == crate::ErrorCode::Unavailable => Err(unavailable),
        _ => Ok(()),
    }
}

/// The open libraries by identifier.
type Libraries<B, R, S> = Mutex<HashMap<String, Arc<Library<B, R, S>>>>;

/// The file operations of every library a host opened. Cheap to clone.
pub struct FileService<B, R, S> {
    libraries: Arc<Libraries<B, R, S>>,
    clock: FileClock,
    names: Arc<OnceLock<Arc<dyn DisplayNames>>>,
}

impl<B, R, S> Clone for FileService<B, R, S> {
    fn clone(&self) -> Self {
        Self {
            libraries: self.libraries.clone(),
            clock: self.clock.clone(),
            names: self.names.clone(),
        }
    }
}

impl<B, R, S> Default for FileService<B, R, S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B, R, S> FileService<B, R, S> {
    /// A service with no library open, dating commits by the system clock.
    pub fn new() -> Self {
        Self::with_clock(Arc::new(crate::journal::now_ms))
    }

    /// A service dating commits by `clock`.
    pub fn with_clock(clock: FileClock) -> Self {
        Self {
            libraries: Arc::new(Mutex::new(HashMap::new())),
            clock,
            names: Arc::new(OnceLock::new()),
        }
    }

    /// Take the owner's original file names from `names` (the host's file records). Returns
    /// false, keeping the first, when names were already installed. Without them, a file's
    /// display name is its stored name.
    pub fn install_display_names(&self, names: Arc<dyn DisplayNames>) -> bool {
        self.names.set(names).is_ok()
    }

    /// The name users see for `file`: the owner's original name from the installed
    /// [`DisplayNames`], made safe, or else the stored name.
    pub async fn display_name(&self, file: StoredFile<'_>) -> String {
        let original = match self.names.get() {
            Some(names) => names.original_name(file).await,
            None => None,
        };
        original
            .as_deref()
            .and_then(safe_name)
            .unwrap_or_else(|| stored_name(file.path).to_string())
    }

    /// Fill in the display name of every file entry of `page`.
    async fn name_entries(&self, page: &mut FolderPageView) {
        for entry in &mut page.entries {
            if let EntryView::File {
                path, display_name, ..
            } = entry
            {
                let file = StoredFile {
                    library_id: &page.library_id,
                    object_id: &page.object_id,
                    version: &page.version,
                    path,
                };
                *display_name = self.display_name(file).await;
            }
        }
    }

    /// Serve `library` as `library_id`. Returns false, keeping the open one, when that
    /// library is already open.
    pub fn open_library(&self, library_id: &LibraryId, library: FileLibrary<B, R, S>) -> bool {
        let mut libraries = lock(&self.libraries);
        if libraries.contains_key(library_id.as_str()) {
            return false;
        }
        let opened = Library {
            files: library,
            roots: Mutex::new(HashMap::new()),
            sizes: Mutex::new(HashMap::new()),
            writes: tokio::sync::Mutex::new(()),
        };
        libraries.insert(library_id.as_str().to_string(), Arc::new(opened));
        true
    }

    /// Stop serving `library_id`. Returns false when it was not open.
    pub fn close_library(&self, library_id: &LibraryId) -> bool {
        lock(&self.libraries).remove(library_id.as_str()).is_some()
    }

    /// The open library `library_id`.
    fn library(&self, library_id: &str) -> Result<Arc<Library<B, R, S>>, FileError> {
        lock(&self.libraries)
            .get(library_id)
            .cloned()
            .ok_or_else(FileError::not_found)
    }

    /// Now as RFC 3339 UTC, for a version's `created`.
    fn created(&self) -> String {
        rfc3339_utc(u64::try_from((self.clock)()).unwrap_or(0))
    }
}

impl<B: StorageBackend, R, S> FileService<B, R, S> {
    /// One page of the folder `prefix` of a version.
    pub async fn listing(&self, query: ListingQuery) -> Result<FolderPageView, FileError> {
        let limit = page_size(query.limit)?;
        let library = self.library(&query.library_id)?;
        let root = library.locate(&query.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        let version = resolve(&object, query.version.as_deref())?;
        let prefix = query.prefix.as_deref().unwrap_or("");
        let cursor = query.cursor.as_deref();
        let page = view::list_folder(&object, &version, prefix, cursor, limit).await?;
        let mut view = FolderPageView::of(&query.library_id, page);
        self.name_entries(&mut view).await;
        Ok(view)
    }

    /// Storage facts about one file of a version.
    pub async fn details(&self, query: FileQuery) -> Result<FileDetailsView, FileError> {
        let library = self.library(&query.library_id)?;
        let root = library.locate(&query.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        let version = resolve(&object, query.version.as_deref())?;
        let details = view::file_details(&object, &version, &query.path).await?;
        let mut view = FileDetailsView::of(&query.library_id, details);
        let file = StoredFile {
            library_id: &view.library_id,
            object_id: &view.object_id,
            version: &view.version,
            path: &view.path,
        };
        view.display_name = self.display_name(file).await;
        Ok(view)
    }

    /// The versions that changed one file.
    pub async fn history(&self, query: HistoryQuery) -> Result<HistoryView, FileError> {
        let library = self.library(&query.library_id)?;
        let root = library.locate(&query.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        let entries = view::file_history(&object, &query.path)?;
        if entries.is_empty() {
            return Err(FileError::not_found());
        }
        let ids = (&query.library_id, &query.object_id, &query.path);
        Ok(HistoryView::of(ids.0, ids.1, ids.2, entries))
    }

    /// Every version of an object.
    pub async fn versions(&self, query: ObjectQuery) -> Result<VersionsView, FileError> {
        let library = self.library(&query.library_id)?;
        let root = library.locate(&query.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        Ok(VersionsView {
            library_id: query.library_id,
            object_id: query.object_id,
            head: object.inventory().head.to_string(),
            versions: view::object_versions(&object)
                .iter()
                .map(VersionView::of)
                .collect(),
        })
    }

    /// The bytes one content answer carries: the file's length (cached by content digest)
    /// and media type, and for a `GET` the planned range, read in at most
    /// [`MAX_RANGE_BYTES`] pieces.
    pub async fn content(&self, request: ContentRequest) -> Result<ContentAnswer, FileError> {
        let library = self.library(&request.library_id)?;
        let root = library.locate(&request.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        let version = resolve(&object, request.version.as_deref())?;
        let state = object.state(&version)?;
        let file = state.get(&request.path).ok_or_else(FileError::not_found)?;
        let total = library
            .size_of(&object, &version, &request.path, &file.digest)
            .await?;
        let media_type = media::media_type(&request.path);
        let version_name = version.to_string();
        let file_name = self
            .display_name(StoredFile {
                library_id: &request.library_id,
                object_id: &request.object_id,
                version: &version_name,
                path: &request.path,
            })
            .await;
        let plan = match request.head_only {
            true => RangePlan::Whole { len: total },
            false => request.range.plan(total, MAX_ANSWER_BYTES as u64),
        };
        let bytes = match (request.head_only, plan) {
            (false, RangePlan::Whole { len }) => {
                read(&object, &version, &request.path, 0, len).await?
            }
            (false, RangePlan::Part { start, len }) => {
                read(&object, &version, &request.path, start, len).await?
            }
            _ => Vec::new(),
        };
        Ok(ContentAnswer {
            plan,
            total,
            media_type,
            file_name,
            bytes,
        })
    }

    /// Whether `target` names a file of an existing version: `not_found` otherwise. Reads
    /// the inventory only, never the content.
    pub async fn check_target(&self, target: &ContentTarget) -> Result<(), FileError> {
        let version = VersionNum::parse(&target.version)
            .map_err(|_| FileError::invalid("A content handle names one version, such as v1."))?;
        let library = self.library(&target.library_id)?;
        let root = library.locate(&target.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        let state = object.state(&version)?;
        state
            .get(&target.path)
            .map(|_| ())
            .ok_or_else(FileError::not_found)
    }

    /// Whether the folder `request.path` exists in the head (`pending` false) or would exist
    /// only once a file is added beneath it (`pending` true). Writes nothing: OCFL stores no
    /// empty folder. A file at the path or at one of its parents is a `conflict`.
    pub async fn create_folder(&self, request: FolderRequest) -> Result<FolderView, FileError> {
        if request.path.is_empty() {
            return Err(FileError::invalid("A folder needs a logical path."));
        }
        let library = self.library(&request.library_id)?;
        let root = library.locate(&request.object_id).await?;
        let object = Object::open(library.files.backend.as_ref(), &root).await?;
        let head = object.inventory().head.clone();
        let state = object.state(&head)?;
        let pending = match view::list_folder(&object, &head, &request.path, None, 1).await {
            Ok(_) => false,
            Err(view::ViewError::FolderNotFound { .. }) => true,
            Err(err) => return Err(err.into()),
        };
        if prefixes(&request.path).any(|p| state.get(p).is_some()) {
            return Err(FileError::conflict(
                "A file already has that path or a parent of it.",
            ));
        }
        Ok(FolderView {
            library_id: request.library_id,
            object_id: request.object_id,
            path: request.path,
            pending,
        })
    }
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// Add or replace one file in one new version.
    pub async fn upload(
        &self,
        query: UploadQuery,
        bytes: &[u8],
        actor: &str,
    ) -> Result<CommitView, FileError> {
        let meta = self.meta(actor, query.message.as_deref())?;
        let library = self.library(&query.library_id)?;
        let _writing = library.writes.lock().await;
        let root = library.locate(&query.object_id).await?;
        let staging = library.staging(&query.object_id)?;
        let model = change_model(&library, &root, &query.object_id, staging);
        let receipt = view::add_file(&model, &query.path, bytes, meta).await?;
        Ok(commit_view(&query.library_id, receipt))
    }

    /// Move or rename one file or folder in one new version.
    pub async fn move_path(
        &self,
        request: MoveRequest,
        actor: &str,
    ) -> Result<CommitView, FileError> {
        let meta = self.meta(actor, request.message.as_deref())?;
        let library = self.library(&request.library_id)?;
        let _writing = library.writes.lock().await;
        let root = library.locate(&request.object_id).await?;
        let staging = library.staging(&request.object_id)?;
        let model = change_model(&library, &root, &request.object_id, staging);
        let receipt = view::move_path(&model, &request.from, &request.to, meta).await?;
        Ok(commit_view(&request.library_id, receipt))
    }

    /// Remove one file or folder in one new version; its bytes stay for history.
    pub async fn remove(
        &self,
        request: RemoveRequest,
        actor: &str,
    ) -> Result<CommitView, FileError> {
        let meta = self.meta(actor, request.message.as_deref())?;
        let library = self.library(&request.library_id)?;
        let _writing = library.writes.lock().await;
        let root = library.locate(&request.object_id).await?;
        let staging = library.staging(&request.object_id)?;
        let model = change_model(&library, &root, &request.object_id, staging);
        let receipt = view::remove_path(&model, &request.path, meta).await?;
        Ok(commit_view(&request.library_id, receipt))
    }

    /// Restore files or folders from an earlier version in one new version.
    pub async fn restore(
        &self,
        request: RestoreRequest,
        actor: &str,
    ) -> Result<CommitView, FileError> {
        let meta = self.meta(actor, request.message.as_deref())?;
        let version = restore_version(&request)?;
        let library = self.library(&request.library_id)?;
        let _writing = library.writes.lock().await;
        let root = library.locate(&request.object_id).await?;
        let staging = library.staging(&request.object_id)?;
        let model = change_model(&library, &root, &request.object_id, staging);
        let paths: Vec<&str> = request.paths.iter().map(String::as_str).collect();
        let receipt = view::restore(&model, &version, &paths, meta).await?;
        Ok(commit_view(&request.library_id, receipt))
    }

    /// The version record of a write by `actor` with `message`.
    fn meta(&self, actor: &str, message: Option<&str>) -> Result<VersionMeta, FileError> {
        let user = User {
            name: actor.to_string(),
            address: None,
        };
        let meta = VersionMeta::new(self.created()).with_user(user);
        match message {
            None => Ok(meta),
            Some(text) if !text.trim().is_empty() && text.len() <= MAX_MESSAGE_BYTES => {
                Ok(meta.with_message(text))
            }
            Some(_) => Err(FileError::invalid(format!(
                "A version message is 1 to {MAX_MESSAGE_BYTES} bytes and not blank."
            ))),
        }
    }
}

/// The change model of `object_id` at `root` of `library`, staging in `staging`.
fn change_model<'a, B, R, S>(
    library: &'a Library<B, R, S>,
    root: &str,
    object_id: &str,
    staging: StagingArea<'a, S>,
) -> ChangeModel<'a, B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    let files = &library.files;
    ChangeModel::new(
        files.backend.as_ref(),
        files.reservations.as_ref(),
        HOLDER,
        root,
        object_id,
        staging,
    )
}

/// The checked version and path count of a restore.
fn restore_version(request: &RestoreRequest) -> Result<VersionNum, FileError> {
    if request.paths.is_empty() || request.paths.len() > MAX_RESTORE_PATHS {
        return Err(FileError::invalid(format!(
            "A restore names 1 to {MAX_RESTORE_PATHS} files or folders."
        )));
    }
    VersionNum::parse(&request.version)
        .map_err(|_| FileError::invalid("A restore names one earlier version, such as v1."))
}

/// The answer of a published version.
fn commit_view(library_id: &str, receipt: CommitReceipt) -> CommitView {
    CommitView {
        library_id: library_id.to_string(),
        object_id: receipt.object_id,
        version: receipt.version.to_string(),
        created: receipt.created,
    }
}

/// The version `text` names, the head when absent or `head`; `not_found` for a version the
/// object does not have.
fn resolve<B: StorageBackend>(
    object: &Object<'_, B>,
    text: Option<&str>,
) -> Result<VersionNum, FileError> {
    let inventory = object.inventory();
    let version = match text {
        None | Some("head") => return Ok(inventory.head.clone()),
        Some(text) => VersionNum::parse(text)
            .map_err(|_| FileError::invalid("A version is head or one such as v1."))?,
    };
    match inventory.versions.contains_key(&version) {
        true => Ok(version),
        false => Err(FileError::not_found()),
    }
}

/// The checked page size: [`DEFAULT_PAGE`] when absent, else 1 to [`MAX_PAGE_ENTRIES`].
fn page_size(limit: Option<usize>) -> Result<usize, FileError> {
    match limit.unwrap_or(DEFAULT_PAGE) {
        limit @ 1..=MAX_PAGE_ENTRIES => Ok(limit),
        _ => Err(FileError::invalid(format!(
            "A page holds 1 to {MAX_PAGE_ENTRIES} entries."
        ))),
    }
}

/// `path` and each of its parent folders, longest first.
fn prefixes(path: &str) -> impl Iterator<Item = &str> {
    let cuts = path.match_indices('/').map(|(i, _)| i);
    std::iter::once(path).chain(cuts.map(|i| &path[..i]))
}

/// Read `len` bytes of `path` of `version` from `start`, in pieces of at most
/// [`MAX_RANGE_BYTES`]; at most `len / MAX_RANGE_BYTES + 1` pieces (AGENTS.md §22).
async fn read<B: StorageBackend>(
    object: &Object<'_, B>,
    version: &VersionNum,
    path: &str,
    start: u64,
    len: u64,
) -> Result<Vec<u8>, FileError> {
    let mut bytes = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
    for _ in 0..=len / MAX_RANGE_BYTES {
        let done = bytes.len() as u64;
        if done >= len {
            break;
        }
        let piece = (len - done).min(MAX_RANGE_BYTES);
        let chunk = view::read_range(object, version, path, start + done, piece).await?;
        if chunk.is_empty() {
            return Err(FileError::internal(
                "a stored file ended before its recorded size",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// What one content answer needs: which file of which version, the requested range, and
/// whether only the description is wanted (`HEAD`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRequest {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The version; the head when absent.
    pub version: Option<String>,
    /// The file's logical path.
    pub path: String,
    /// The requested range.
    pub range: RangeRequest,
    /// True for `HEAD`: no content is read.
    pub head_only: bool,
}

impl ContentRequest {
    /// The request for the one file version a content handle names.
    pub fn of_target(target: ContentTarget, range: RangeRequest, head_only: bool) -> Self {
        Self {
            library_id: target.library_id,
            object_id: target.object_id,
            version: Some(target.version),
            path: target.path,
            range,
            head_only,
        }
    }
}

/// The bytes and facts of one content answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentAnswer {
    /// Which bytes it carries.
    pub plan: RangePlan,
    /// The file's length.
    pub total: u64,
    /// The media type it carries.
    pub media_type: &'static str,
    /// The display name the answer names the file by ([`super::names`]).
    pub file_name: String,
    /// The bytes (empty for `HEAD` and `416`).
    pub bytes: Vec<u8>,
}

/// `mutex`'s guard, recovering from a poisoned lock: every update leaves the map consistent.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
