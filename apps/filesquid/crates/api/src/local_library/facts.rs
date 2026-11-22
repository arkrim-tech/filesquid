//! Library facts (Tasks API-F2.18 and API-F2.31; API-S-038, API-S-034): a library's name, the
//! free bytes of the volume that holds its storage root, the names of its folder and volume on
//! the desktop, and the libraries this host opened.
//!
//! A host keeps what it knows of each library it bound in a [`LibraryRegistry`]: its name, the
//! location it was opened from and when, and it answers the free space of a location. The API
//! keeps the one meaning: [`check_name`] states the name rule, [`RegistryBook`] the bounded
//! bookkeeping every host can reuse, and the routes decide readability and the 20-entry list.
//! Nothing here carries a path: a location is an opaque ID the host registered, and no answer
//! of the routes names it. The folder's own name and its volume's name ([`FolderNames`]) are
//! answered only where a host reads them (the desktop, the owner's own device; owner decision
//! 2026-11-02), and never journaled, logged or sent on the event channel.

use crate::journal::format_rfc3339_ms;
use filesquid_core::LibraryId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

/// Longest library name, in bytes (API-S-019).
pub const MAX_LIBRARY_NAME_BYTES: usize = crate::library::MAX_NAME_BYTES;

/// The name of a library first bound without one, so every library has a name.
pub const UNTITLED_LIBRARY: &str = "Untitled library";

/// Most libraries the recent route lists (API-S-038).
pub const MAX_RECENT_LIBRARIES: usize = 20;

/// Most libraries a [`RegistryBook`] remembers; the least recently opened is forgotten first.
pub const MAX_REMEMBERED_LIBRARIES: usize = 256;

/// The free space of the volume that holds a location, as a host reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FreeSpace {
    /// The bytes free to this process.
    Bytes(u64),
    /// The volume is local but could not be read (or this build does not read it).
    VolumeUnreadable,
    /// The storage is not a local volume (for example S3-compatible storage).
    NotLocal,
}

/// Why `free_bytes` is `null` (API-S-038).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreeBytesReason {
    /// The volume could not be read.
    VolumeUnreadable,
    /// The storage is not a local volume.
    NotLocal,
}

impl FreeSpace {
    /// `free_bytes` and `free_bytes_reason` of this space. Pure.
    pub fn fields(&self) -> (Option<u64>, Option<FreeBytesReason>) {
        match self {
            Self::Bytes(bytes) => (Some(*bytes), None),
            Self::VolumeUnreadable => (None, Some(FreeBytesReason::VolumeUnreadable)),
            Self::NotLocal => (None, Some(FreeBytesReason::NotLocal)),
        }
    }
}

/// The names of a chosen or library folder that the desktop answers to its owner (API-S-034
/// **Folder and volume names**): the folder's own name, never a parent folder or the path, and
/// the name of the volume that holds it, each `None` when the host cannot read it. A value is
/// only ever answered to the caller: it never reaches a journal entry, a refusal, an event or
/// a log line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderNames {
    /// The folder's own name (its last path component), after API-S-005's safe transformation.
    pub folder_name: Option<String>,
    /// The name of the volume that holds the folder, after the same transformation.
    pub volume_name: Option<String>,
}

impl FolderNames {
    /// The names a host read, each passed through API-S-005's safe transformation
    /// ([`crate::files::names::safe_name`]); a name with nothing left is `None`. Pure.
    pub fn safe(folder_name: Option<&str>, volume_name: Option<&str>) -> Self {
        Self {
            folder_name: folder_name.and_then(crate::files::names::safe_name),
            volume_name: volume_name.and_then(crate::files::names::safe_name),
        }
    }

    /// These names after the safe transformation, so a route answers only safe names whatever
    /// a host handed it; already safe names are unchanged. Pure.
    pub fn made_safe(self) -> Self {
        Self::safe(self.folder_name.as_deref(), self.volume_name.as_deref())
    }
}

/// What a host remembers of one library it bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEntry {
    /// The library.
    pub library_id: LibraryId,
    /// Its name.
    pub name: String,
    /// The opaque location it was last opened from; never answered by a route.
    pub location_id: String,
    /// When it was last created or opened (Unix milliseconds, UTC).
    pub last_opened_ms: i64,
}

/// The host port of the library facts: what a host remembers of the libraries it bound and
/// the free space of its locations. Every call is answered from memory or one volume query;
/// none may block for long.
pub trait LibraryRegistry: Send + Sync {
    /// Remember that `library_id` was created or opened from `location_id` at `opened_ms`,
    /// named `name` when one is given, and answer the name it now has: `name`, else the one
    /// remembered, else [`UNTITLED_LIBRARY`].
    fn remember(
        &self,
        library_id: &LibraryId,
        location_id: &str,
        name: Option<&str>,
        opened_ms: i64,
    ) -> String;

    /// Rename `library_id` to `name`: `None` for a library this host does not remember,
    /// `Some(false)` when it already has that name, `Some(true)` when renamed.
    fn rename(&self, library_id: &LibraryId, name: &str) -> Option<bool>;

    /// What this host remembers of `library_id`.
    fn entry(&self, library_id: &LibraryId) -> Option<RegistryEntry>;

    /// Every library this host remembers, most recently opened first.
    fn entries(&self) -> Vec<RegistryEntry>;

    /// The free space of the volume that holds `location_id`. May query the volume.
    fn free_space(&self, location_id: &str) -> FreeSpace;

    /// The names of the folder registered as `location_id` and of its volume (API-S-034),
    /// each `None` when the host cannot read it or the folder cannot be reached now. Only the
    /// desktop host reads them; the default answers none, as every other host must. May query
    /// the folder and its volume.
    fn folder_names(&self, _location_id: &str) -> FolderNames {
        FolderNames::default()
    }
}

/// The bounded, in-memory bookkeeping of a [`LibraryRegistry`]: at most
/// [`MAX_REMEMBERED_LIBRARIES`] entries, the least recently opened forgotten first. Hosts keep
/// one beside their own free-space reads.
#[derive(Debug, Default)]
pub struct RegistryBook {
    entries: Mutex<HashMap<LibraryId, RegistryEntry>>,
}

impl RegistryBook {
    /// An empty book.
    pub fn new() -> Self {
        Self::default()
    }

    /// [`LibraryRegistry::remember`].
    pub fn remember(
        &self,
        library_id: &LibraryId,
        location_id: &str,
        name: Option<&str>,
        opened_ms: i64,
    ) -> String {
        let mut entries = self.lock();
        let kept = name
            .map(str::to_string)
            .or_else(|| entries.get(library_id).map(|known| known.name.clone()))
            .unwrap_or_else(|| UNTITLED_LIBRARY.to_string());
        if !entries.contains_key(library_id) && entries.len() >= MAX_REMEMBERED_LIBRARIES {
            forget_oldest(&mut entries);
        }
        let entry = RegistryEntry {
            library_id: library_id.clone(),
            name: kept.clone(),
            location_id: location_id.to_string(),
            last_opened_ms: opened_ms,
        };
        entries.insert(library_id.clone(), entry);
        kept
    }

    /// [`LibraryRegistry::rename`].
    pub fn rename(&self, library_id: &LibraryId, name: &str) -> Option<bool> {
        let mut entries = self.lock();
        let entry = entries.get_mut(library_id)?;
        if entry.name == name {
            return Some(false);
        }
        entry.name = name.to_string();
        Some(true)
    }

    /// [`LibraryRegistry::entry`].
    pub fn entry(&self, library_id: &LibraryId) -> Option<RegistryEntry> {
        self.lock().get(library_id).cloned()
    }

    /// [`LibraryRegistry::entries`]: most recently opened first, ties by library ID.
    pub fn entries(&self) -> Vec<RegistryEntry> {
        let mut all: Vec<RegistryEntry> = self.lock().values().cloned().collect();
        all.sort_by(|a, b| {
            b.last_opened_ms
                .cmp(&a.last_opened_ms)
                .then_with(|| a.library_id.as_str().cmp(b.library_id.as_str()))
        });
        all
    }

    /// The entries; a poisoned lock still holds a consistent map, since every change is one
    /// insert, one removal or one field write.
    fn lock(&self) -> MutexGuard<'_, HashMap<LibraryId, RegistryEntry>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Forget the least recently opened entry of `entries`.
fn forget_oldest(entries: &mut HashMap<LibraryId, RegistryEntry>) {
    let oldest = entries
        .values()
        .min_by_key(|entry| entry.last_opened_ms)
        .map(|entry| entry.library_id.clone());
    if let Some(oldest) = oldest {
        entries.remove(&oldest);
    }
}

/// The free-space reader of a [`MemoryLibraryRegistry`].
type FreeSpaceFn = Box<dyn Fn(&str) -> FreeSpace + Send + Sync>;

/// The folder-names reader of a [`MemoryLibraryRegistry`].
type FolderNamesFn = Box<dyn Fn(&str) -> FolderNames + Send + Sync>;

/// A [`LibraryRegistry`] in memory whose free space a function answers, and its folder names
/// another when one is given: for tests, and for a host whose locations a function can
/// measure.
pub struct MemoryLibraryRegistry {
    book: RegistryBook,
    free: FreeSpaceFn,
    names: Option<FolderNamesFn>,
}

impl std::fmt::Debug for MemoryLibraryRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryLibraryRegistry")
            .finish_non_exhaustive()
    }
}

impl MemoryLibraryRegistry {
    /// An empty registry whose locations' free space `free` answers.
    pub fn new(free: impl Fn(&str) -> FreeSpace + Send + Sync + 'static) -> Self {
        Self {
            book: RegistryBook::new(),
            free: Box::new(free),
            names: None,
        }
    }

    /// This registry, its locations' folder names answered by `names` (a desktop host's
    /// reader); without it every location answers none. Pure.
    pub fn with_folder_names(
        mut self,
        names: impl Fn(&str) -> FolderNames + Send + Sync + 'static,
    ) -> Self {
        self.names = Some(Box::new(names));
        self
    }
}

impl LibraryRegistry for MemoryLibraryRegistry {
    fn remember(
        &self,
        library_id: &LibraryId,
        location_id: &str,
        name: Option<&str>,
        opened_ms: i64,
    ) -> String {
        self.book.remember(library_id, location_id, name, opened_ms)
    }

    fn rename(&self, library_id: &LibraryId, name: &str) -> Option<bool> {
        self.book.rename(library_id, name)
    }

    fn entry(&self, library_id: &LibraryId) -> Option<RegistryEntry> {
        self.book.entry(library_id)
    }

    fn entries(&self) -> Vec<RegistryEntry> {
        self.book.entries()
    }

    fn free_space(&self, location_id: &str) -> FreeSpace {
        (self.free)(location_id)
    }

    fn folder_names(&self, location_id: &str) -> FolderNames {
        self.names
            .as_ref()
            .map_or_else(FolderNames::default, |names| names(location_id))
    }
}

/// Whether `name` is a library name: 1 to [`MAX_LIBRARY_NAME_BYTES`] bytes, not blank, and no
/// control characters (API-S-019, API-S-005). Pure.
pub fn check_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= MAX_LIBRARY_NAME_BYTES
        && !name.chars().any(char::is_control)
}

/// The answer of the facts and rename routes: `{library_id, name, free_bytes,
/// free_bytes_reason, revision, folder_name, volume_name}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryFacts {
    /// The library.
    pub library_id: LibraryId,
    /// Its name.
    pub name: String,
    /// The free bytes of the volume that holds its storage root, or `null`.
    pub free_bytes: Option<u64>,
    /// Why `free_bytes` is `null`; `null` when it is known.
    pub free_bytes_reason: Option<FreeBytesReason>,
    /// The revision of the library while it is open on this host, the expected revision of
    /// its next copy, rebuild or rename command (Task API-F2.19); `null` for a remembered
    /// library that is not open now.
    pub revision: Option<String>,
    /// The library folder's own name on the desktop (API-S-034), `null` when unreadable, when
    /// the folder cannot be reached now, and on every other host.
    pub folder_name: Option<String>,
    /// The name of the volume that holds the library folder, under the same rules.
    pub volume_name: Option<String>,
}

/// What the facts of one library are made of besides its registry entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactsReading {
    /// The free space of its volume.
    pub space: FreeSpace,
    /// The names of its folder and volume.
    pub names: FolderNames,
}

impl LibraryFacts {
    /// The facts of `entry` with what was `read` of its folder, at `revision` when the
    /// library is open. Pure.
    pub fn of(entry: &RegistryEntry, read: &FactsReading, revision: Option<String>) -> Self {
        let (free_bytes, free_bytes_reason) = read.space.fields();
        let names = read.names.clone().made_safe();
        Self {
            library_id: entry.library_id.clone(),
            name: entry.name.clone(),
            free_bytes,
            free_bytes_reason,
            revision,
            folder_name: names.folder_name,
            volume_name: names.volume_name,
        }
    }
}

/// One library of the recent route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentLibrary {
    /// The library.
    pub library_id: LibraryId,
    /// Its name.
    pub name: String,
    /// When it was last created or opened (RFC 3339 UTC).
    pub last_opened: String,
    /// Whether it is open on this host now.
    pub open: bool,
    /// The free bytes of its volume when its folder is reachable, or `null`.
    pub free_bytes: Option<u64>,
    /// Why `free_bytes` is `null`; `null` when it is known.
    pub free_bytes_reason: Option<FreeBytesReason>,
}

impl RecentLibrary {
    /// The listing of `entry`, open or not, with `space`. Pure.
    pub fn of(entry: &RegistryEntry, open: bool, space: &FreeSpace) -> Self {
        let (free_bytes, free_bytes_reason) = space.fields();
        Self {
            library_id: entry.library_id.clone(),
            name: entry.name.clone(),
            last_opened: format_rfc3339_ms(entry.last_opened_ms),
            open,
            free_bytes,
            free_bytes_reason,
        }
    }
}

/// The answer of `GET /api/v1/libraries/recent`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentLibraries {
    /// At most [`MAX_RECENT_LIBRARIES`] libraries the caller may read, newest first.
    pub libraries: Vec<RecentLibrary>,
}

/// Body of `POST /api/v1/libraries/renames`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameLibraryRequest {
    /// A library this host opened.
    pub library_id: LibraryId,
    /// Its new name.
    pub name: String,
}
