//! Enumerating the committed logical state of a whole storage root
//! (OCFL-S-004, T-039;
//! [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)
//! engine core item 6;
//! [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md)
//! "Read and browse").
//!
//! [`StorageRoot::enumerate`] walks every object discovered when the
//! storage root was opened and yields [`CommittedEvent`]s: one
//! [`CommittedObject`] per object, one [`CommittedVersion`] per enumerated
//! version, and one [`CommittedFile`] per logical file of that version. A
//! file event carries what T-039 names: the object identifier, the version,
//! the logical path, the digest and a content locator (the storage key of
//! the bytes). A caller can rebuild its own indexes from the storage root
//! alone, or project the events to another format; the enumerator needs no
//! database and no application type.
//!
//! Everything comes from the committed root inventory, read by
//! [`Object::open`]: logical state resolves through `state` and `manifest`
//! only, so bytes kept in an earlier version directory are found and files
//! the inventory does not list are not enumerated, and a version above the
//! committed `head` is never enumerated. Opening each object runs the Git
//! and hierarchy guards and the sidecar and head-inventory checks
//! (OCFL-S-001, OCFL-S-004).
//!
//! The enumerator is a pull cursor: each [`CommittedState::next_event`]
//! call does one step (open one object, resolve one version, or yield one
//! file), so it holds at most one object's inventory and one version's
//! state. Content bytes are not read: a caller that needs them streams them
//! with [`Object::stream_file`], which checks them against their digest.

use std::collections::btree_map::IntoValues;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use thiserror::Error;

use super::error::ReadError;
use super::object::Object;
use super::root::StorageRoot;
use super::state::LogicalFile;
use crate::digest::DigestAlgorithm;
use crate::inventory::VersionNum;
use crate::storage::{StorageBackend, StorageKey};

/// Which committed versions of each object to enumerate. The caller pins
/// the versions it reads; every event names its version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Versions {
    /// Only each object's committed `head`.
    #[default]
    Head,
    /// Every committed version, `v1` to `head`, oldest first.
    All,
}

/// An object the enumerator opened. It comes before the object's versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedObject {
    /// The object root, as a key prefix in the storage root, without a
    /// trailing `/`.
    pub root: String,
    /// The object identifier.
    pub id: String,
    /// The most recent committed version.
    pub head: VersionNum,
    /// The algorithm of every digest of this object.
    pub digest_algorithm: DigestAlgorithm,
    /// The head the root inventory names when its sidecar does not commit
    /// it yet (a version being published, or left unpublished by a crash).
    /// It is never enumerated.
    pub unpublished_head: Option<VersionNum>,
}

/// One enumerated version of the current object. It comes before that
/// version's files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedVersion {
    /// The object identifier.
    pub object_id: String,
    /// The version.
    pub version: VersionNum,
    /// How many logical files the version has (and how many file events
    /// follow).
    pub files: usize,
}

/// One logical file of one committed version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedFile {
    /// The object identifier.
    pub object_id: String,
    /// The version.
    pub version: VersionNum,
    /// The logical path, `/`-separated.
    pub path: String,
    /// The algorithm of [`digest`](Self::digest).
    pub digest_algorithm: DigestAlgorithm,
    /// The content digest, as the inventory writes it.
    pub digest: String,
    /// The content locator: the storage key, from the storage root, of the
    /// bytes the manifest lists for the digest. Unchanged bytes keep the key
    /// in the version directory where they were first stored.
    pub content_key: StorageKey,
}

/// One event of the committed-state enumerator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommittedEvent {
    /// An object was opened; its versions follow.
    Object(CommittedObject),
    /// A version of the current object; its files follow.
    Version(CommittedVersion),
    /// A logical file of the current version.
    File(CommittedFile),
}

/// An object that could not be enumerated: the root it was discovered at
/// and why. The enumeration goes on with the next object.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("the object at {root:?} cannot be enumerated: {source}")]
pub struct EnumerateError {
    /// The object root as discovered in the storage root.
    pub root: String,
    /// Why it could not be read.
    pub source: ReadError,
}

impl EnumerateError {
    /// The OCFL 1.1 validation code (`E###`) when an OCFL rule is broken,
    /// otherwise `None`.
    pub fn code(&self) -> Option<&'static str> {
        self.source.code()
    }
}

impl<'a, B: StorageBackend> StorageRoot<'a, B> {
    /// Enumerate the committed logical state of every object discovered
    /// when this storage root was opened, in object-root order, with
    /// `versions` of each.
    ///
    /// Objects added after the storage root was opened are not seen; open
    /// it again to include them. Nothing is read until the first
    /// [`CommittedState::next_event`].
    pub fn enumerate(&self, versions: Versions) -> CommittedState<'_, 'a, B> {
        CommittedState {
            storage_root: self,
            versions,
            next_object: 0,
            current: None,
        }
    }
}

/// The committed-state enumerator of one storage root, from
/// [`StorageRoot::enumerate`]. A pull cursor: call
/// [`next_event`](Self::next_event) until it returns `None`.
///
/// Events come in a fixed order: objects in object-root order; for each,
/// its [`CommittedObject`], then its versions oldest first, each
/// [`CommittedVersion`] followed by its files in logical-path order.
pub struct CommittedState<'r, 'a, B: StorageBackend> {
    storage_root: &'r StorageRoot<'a, B>,
    versions: Versions,
    next_object: usize,
    current: Option<ObjectCursor<'a, B>>,
}

impl<B: StorageBackend> CommittedState<'_, '_, B> {
    /// The next event, or `None` when every object has been enumerated.
    ///
    /// Each call does one step: it yields the current version's next file,
    /// or resolves the object's next version, or opens the next object.
    ///
    /// # Errors
    ///
    /// An [`EnumerateError`] item for an object that cannot be opened (any
    /// error of [`Object::open`]: a guard refusal, a broken declaration,
    /// a missing root inventory, a sidecar or head-inventory mismatch, an
    /// engine bound or a backend error) or whose state cannot be resolved.
    /// The rest of that object is skipped and the next call continues with
    /// the next object, so one broken object does not hide the others.
    pub async fn next_event(&mut self) -> Option<Result<CommittedEvent, EnumerateError>> {
        if let Some(item) = self.step_current() {
            return Some(item);
        }
        self.open_next_object().await
    }

    /// Step the open object, if any; `None` once it is done (or there is
    /// none), which also closes it.
    fn step_current(&mut self) -> Option<Result<CommittedEvent, EnumerateError>> {
        let cursor = self.current.as_mut()?;
        let Some(step) = cursor.step() else {
            self.current = None;
            return None;
        };
        match step {
            Ok(event) => Some(Ok(event)),
            Err(source) => {
                let closed = self.current.take();
                let root = closed.map(|c| c.object.root().to_string());
                Some(Err(object_error(root.unwrap_or_default(), source)))
            }
        }
    }

    /// Open the next discovered object and yield its object event, or the
    /// error that stopped it; `None` when no object is left.
    async fn open_next_object(&mut self) -> Option<Result<CommittedEvent, EnumerateError>> {
        let root = self.storage_root.objects().get(self.next_object)?.clone();
        self.next_object += 1;
        match self.storage_root.open_object(&root).await {
            Ok(object) => {
                let event = object_event(&object);
                self.current = Some(ObjectCursor::new(object, self.versions));
                Some(Ok(CommittedEvent::Object(event)))
            }
            Err(source) => Some(Err(object_error(root, source))),
        }
    }
}

impl<B: StorageBackend> fmt::Debug for CommittedState<'_, '_, B> {
    /// Show the cursor's position; the backend need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommittedState")
            .field("versions", &self.versions)
            .field("next_object", &self.next_object)
            .field("objects", &self.storage_root.objects().len())
            .field("current", &self.current.as_ref().map(|c| c.object.root()))
            .finish_non_exhaustive()
    }
}

/// Log and wrap the error that stops one object.
fn object_error(root: String, source: ReadError) -> EnumerateError {
    tracing::warn!(object = %root, error = %source, "an OCFL object cannot be enumerated");
    EnumerateError { root, source }
}

/// The object event of an opened object.
fn object_event<B: StorageBackend>(object: &Object<'_, B>) -> CommittedObject {
    CommittedObject {
        root: object.root().to_string(),
        id: object.id().to_string(),
        head: object.head().clone(),
        digest_algorithm: object.inventory().digest_algorithm,
        unpublished_head: object.unpublished_head().cloned(),
    }
}

/// Where the enumerator is inside one open object.
struct ObjectCursor<'a, B: StorageBackend> {
    object: Object<'a, B>,
    pending: VecDeque<VersionNum>,
    version: VersionNum,
    files: IntoValues<String, LogicalFile>,
}

impl<'a, B: StorageBackend> ObjectCursor<'a, B> {
    /// A cursor before the first of `versions` of `object`.
    fn new(object: Object<'a, B>, versions: Versions) -> Self {
        let head = object.head().clone();
        let pending = match versions {
            Versions::Head => VecDeque::from([head.clone()]),
            Versions::All => object.inventory().versions.keys().cloned().collect(),
        };
        Self {
            object,
            pending,
            version: head,
            files: BTreeMap::new().into_values(),
        }
    }

    /// The next file of the current version, else the next version's event;
    /// `None` when the object is done.
    fn step(&mut self) -> Option<Result<CommittedEvent, ReadError>> {
        if let Some(file) = self.files.next() {
            return Some(self.file_event(file));
        }
        let version = self.pending.pop_front()?;
        Some(self.start_version(version))
    }

    /// Resolve `version` through the inventory and yield its version event.
    fn start_version(&mut self, version: VersionNum) -> Result<CommittedEvent, ReadError> {
        let state = self.object.state(&version)?;
        let event = CommittedVersion {
            object_id: self.object.id().to_string(),
            version: version.clone(),
            files: state.len(),
        };
        self.files = state.into_files();
        self.version = version;
        Ok(CommittedEvent::Version(event))
    }

    /// The file event of `file` in the current version.
    fn file_event(&self, file: LogicalFile) -> Result<CommittedEvent, ReadError> {
        let content_key = self.object.content_key(&file)?;
        Ok(CommittedEvent::File(CommittedFile {
            object_id: self.object.id().to_string(),
            version: self.version.clone(),
            path: file.path,
            digest_algorithm: self.object.inventory().digest_algorithm,
            digest: file.digest,
            content_key,
        }))
    }
}
