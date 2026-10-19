//! [`ChangeModel`]: stage, status, diff, log and checkout for one object.
//! Commit, revert and reinstate are in [`super::commit`].

use std::collections::BTreeSet;

use super::area::{list_all, StagingArea};
use super::delta::{compute_delta, diff_versions, manifest_digests, state_digests};
use super::delta::{PathChange, WorkingState};
use super::error::ChangeError;
use super::index::{check_logical_path, path_conflict, Staged, StagingIndex};
use crate::digest::DigestAlgorithm;
use crate::guard::reject_git_in_root;
use crate::inventory::{Inventory, User, VersionNum};
use crate::read::copy_guard::refuse_copy_into_ocfl_root;
use crate::read::small::root_dir;
use crate::read::{version_history, Object, ReadError, VersionInfo};
use crate::storage::{StorageBackend, StorageKey};
use crate::write::base::read_base;
use crate::write::{CommitError, ObjectWriter, VersionReservations};

/// The version metadata a commit, revert or reinstate records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionMeta {
    /// The version's `created` value: RFC 3339 UTC ending in `Z`, such as
    /// [`rfc3339_utc`](crate::write::rfc3339_utc) writes.
    pub created: String,
    /// The version's `message`, when given.
    pub message: Option<String>,
    /// The version's `user`, when given.
    pub user: Option<User>,
}

impl VersionMeta {
    /// Metadata with `created` and no message or user.
    pub fn new(created: impl Into<String>) -> Self {
        Self {
            created: created.into(),
            message: None,
            user: None,
        }
    }

    /// The same metadata with `message`.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// The same metadata with `user`.
    pub fn with_user(mut self, user: User) -> Self {
        self.user = Some(user);
        self
    }
}

/// What is staged, against which base, and where the object's head is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// The object identifier.
    pub object_id: String,
    /// The head the changes were staged against (`None`: no version).
    pub base: Option<VersionNum>,
    /// The object's committed head now. When it differs from `base`, the
    /// commit rebases or reports a conflict (OCFL-S-016).
    pub head: Option<VersionNum>,
    /// The staged change set against `base`, ordered by
    /// [`PathChange::path`].
    pub changes: Vec<PathChange>,
}

/// The git-like change model over one object (OCFL-S-015): the engine's
/// only way to make a version, on every backend and target.
///
/// Stage changes into a [`StagingArea`], inspect them with
/// [`status`](Self::status) and [`diff`](Self::diff), publish them as one
/// version with [`commit`](Self::commit), and read history with
/// [`log`](Self::log) and [`checkout`](Self::checkout).
/// [`revert`](Self::revert) and [`reinstate`](Self::reinstate) publish a
/// new version from existing content. Every publication goes through
/// [`ObjectWriter::commit`]: the fenced `vN` reservation, write order and
/// head compare-and-swap of OCFL-S-018.
pub struct ChangeModel<'a, B, R, S> {
    pub(super) storage: &'a B,
    pub(super) writer: ObjectWriter<'a, B, R>,
    pub(super) root: String,
    pub(super) object_id: String,
    pub(super) staging: StagingArea<'a, S>,
}

impl<'a, B, R, S> ChangeModel<'a, B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// The change model of the object `object_id` at `root` of `storage`,
    /// writing as `holder` under `reservations` and staging in `staging`.
    pub fn new(
        storage: &'a B,
        reservations: &'a R,
        holder: impl Into<String>,
        root: &str,
        object_id: impl Into<String>,
        staging: StagingArea<'a, S>,
    ) -> Self {
        Self {
            storage,
            writer: ObjectWriter::new(storage, reservations, holder),
            root: root.to_string(),
            object_id: object_id.into(),
            staging,
        }
    }

    /// The object identifier.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// The staging area.
    pub fn staging(&self) -> &StagingArea<'a, S> {
        &self.staging
    }

    /// Stage `bytes` as the content of the logical file `path`.
    ///
    /// # Errors
    ///
    /// [`ChangeError::InvalidPath`], [`ChangeError::StagingMismatch`],
    /// [`ChangeError::Read`] for a staging area inside an OCFL root, or a
    /// read or backend error.
    pub async fn stage_bytes(&self, path: &str, bytes: &[u8]) -> Result<(), ChangeError> {
        valid_path(path)?;
        let (inventory, mut index) = self.load().await?;
        let base = base_state(inventory.as_ref(), &index)?;
        let digest = DigestAlgorithm::ENGINE.digest_hex(bytes);
        index.set(&base, path, Staged::File(digest));
        check_working(&index, &base)?;
        self.staging
            .put_bytes(bytes, &known(inventory.as_ref()))
            .await?;
        self.save(&index, &base).await
    }

    /// Stage the removal of the logical file `path`.
    ///
    /// # Errors
    ///
    /// [`ChangeError::PathNotFound`] when the working state has no `path`,
    /// or as [`stage_bytes`](Self::stage_bytes).
    pub async fn stage_remove(&self, path: &str) -> Result<(), ChangeError> {
        valid_path(path)?;
        let (inventory, mut index) = self.load().await?;
        let base = base_state(inventory.as_ref(), &index)?;
        if !index.working(&base).contains_key(path) {
            return Err(not_staged(path));
        }
        index.set(&base, path, Staged::Removed);
        self.save(&index, &base).await
    }

    /// Stage the rename of the logical file `from` to `to`. No bytes are
    /// staged: `to` points at `from`'s content.
    ///
    /// # Errors
    ///
    /// [`ChangeError::PathNotFound`] when the working state has no `from`,
    /// [`ChangeError::InvalidPath`] for an invalid `from` or `to`, or a `to`
    /// the working state already has, or as [`stage_bytes`](Self::stage_bytes).
    pub async fn stage_rename(&self, from: &str, to: &str) -> Result<(), ChangeError> {
        valid_path(from)?;
        valid_path(to)?;
        let (inventory, mut index) = self.load().await?;
        let base = base_state(inventory.as_ref(), &index)?;
        let working = index.working(&base);
        let digest = working.get(from).cloned().ok_or_else(|| not_staged(from))?;
        if working.contains_key(to) {
            return Err(invalid_path(to, "the working state already has this path"));
        }
        index.set(&base, from, Staged::Removed);
        index.set(&base, to, Staged::File(digest));
        self.save(&index, &base).await
    }

    /// Stage a whole working state: every file under `prefix` of `source`
    /// (a directory on [`FsBackend`](crate::storage::FsBackend), or any
    /// backend) becomes the logical file at its key relative to `prefix`,
    /// and every other logical path is removed. Only bytes the object does
    /// not hold yet are copied into the staging area.
    ///
    /// # Errors
    ///
    /// [`ChangeError::InvalidPath`] for a key that is not a valid logical
    /// path, [`ChangeError::SourceChanged`] for a file that changed while
    /// staged, or as [`stage_bytes`](Self::stage_bytes).
    pub async fn stage_tree<D: StorageBackend>(
        &self,
        source: &D,
        prefix: &str,
    ) -> Result<(), ChangeError> {
        let (inventory, mut index) = self.load().await?;
        let known = known(inventory.as_ref());
        let dir = match prefix.trim_end_matches('/') {
            "" => String::new(),
            trimmed => format!("{trimmed}/"),
        };
        let keys = list_all(source, &dir).await?;
        let paths: Vec<&str> = keys.iter().map(|k| &k.as_str()[dir.len()..]).collect();
        for path in &paths {
            valid_path(path)?;
        }
        if let Some(path) = path_conflict(paths.iter().copied()) {
            return Err(conflict_error(path));
        }
        let mut working = WorkingState::new();
        for (key, path) in keys.iter().zip(paths) {
            let digest = self.staging.put_file(source, key, &known).await?;
            working.insert(path.to_string(), digest);
        }
        let base = base_state(inventory.as_ref(), &index)?;
        index.paths.clear();
        index.snapshot = true;
        for path in base.keys().filter(|p| !working.contains_key(*p)) {
            index.set(&base, path, Staged::Removed);
        }
        for (path, digest) in working {
            index.set(&base, &path, Staged::File(digest));
        }
        self.save(&index, &base).await
    }

    /// Discard everything staged.
    ///
    /// # Errors
    ///
    /// A backend error.
    pub async fn reset(&self) -> Result<(), ChangeError> {
        self.staging.clear().await
    }

    /// The staged change set against its base, and the object's head now
    /// (OCFL-S-015 "diff staged against head").
    ///
    /// # Errors
    ///
    /// [`ChangeError::StagingCorrupt`], [`ChangeError::StagingMismatch`],
    /// [`ChangeError::WrongObject`], or a read or backend error.
    pub async fn status(&self) -> Result<Status, ChangeError> {
        let (inventory, index) = self.load().await?;
        let working = index.working(&base_state(inventory.as_ref(), &index)?);
        let changes = compute_delta(inventory.as_ref(), index.base.as_ref(), &working)?;
        Ok(Status {
            object_id: self.object_id.clone(),
            base: index.base,
            head: inventory.map(|inv| inv.head),
            changes,
        })
    }

    /// The changes from committed version `from` to committed version
    /// `to`, classified as a delta (content a version before `to` held is
    /// reinstated, not added).
    ///
    /// # Errors
    ///
    /// [`ChangeError::Read`] with
    /// [`ReadError::VersionNotFound`](crate::read::ReadError::VersionNotFound)
    /// for a version the object lacks, or a read or backend error.
    pub async fn diff(
        &self,
        from: &VersionNum,
        to: &VersionNum,
    ) -> Result<Vec<PathChange>, ChangeError> {
        let inventory = self.committed().await?;
        let Some(inventory) = inventory else {
            return Err(ReadError::VersionNotFound {
                version: from.to_string(),
            }
            .into());
        };
        diff_versions(&inventory, from, to)
    }

    /// Every committed version, newest first, as the inventory's `versions`
    /// record them. An object with no version has an empty log.
    ///
    /// # Errors
    ///
    /// [`ChangeError::WrongObject`], or a read or backend error.
    pub async fn log(&self) -> Result<Vec<VersionInfo>, ChangeError> {
        let inventory = self.committed().await?;
        let mut log = inventory.as_ref().map(version_history).unwrap_or_default();
        log.reverse();
        Ok(log)
    }

    /// Write the logical state of `version` to `dest` under `prefix`: each
    /// logical file at `<prefix>/<logical path>`, checked against its
    /// digest, and return the keys written in logical-path order. A
    /// destination inside an OCFL object root or storage root is refused
    /// before anything is written.
    ///
    /// # Errors
    ///
    /// [`ChangeError::Read`] with
    /// [`ReadError::CopyIntoOcflRoot`](crate::read::ReadError::CopyIntoOcflRoot)
    /// for such a destination, with `VersionNotFound` for an unknown
    /// version, or another read or backend error.
    pub async fn checkout<D: StorageBackend>(
        &self,
        version: &VersionNum,
        dest: &D,
        prefix: &str,
    ) -> Result<Vec<StorageKey>, ChangeError> {
        let object = Object::open(self.storage, &self.root).await?;
        self.check_id(object.id())?;
        let state = object.state(version)?;
        let prefix = prefix.trim_end_matches('/');
        let mut keys = Vec::with_capacity(state.len());
        for file in state.files() {
            let key = match prefix {
                "" => StorageKey::new(&file.path)?,
                prefix => StorageKey::new(&format!("{prefix}/{}", file.path))?,
            };
            refuse_copy_into_ocfl_root(dest, &key).await?;
            keys.push(key);
        }
        for (file, key) in state.files().zip(&keys) {
            object
                .copy_file(version, &file.path, dest, key, None)
                .await?;
        }
        Ok(keys)
    }

    /// The committed inventory of the object, or `None` when no version is
    /// committed. When the object root holds an inventory, the Git and
    /// hierarchy guards (OCFL-S-001) run on it first.
    ///
    /// # Errors
    ///
    /// [`ChangeError::WrongObject`] when the root holds another object (an
    /// uncommitted first version included), [`ChangeError::Commit`] with
    /// [`CommitError::Guard`] for a refused root, or a read or backend
    /// error.
    pub(crate) async fn committed(&self) -> Result<Option<Inventory>, ChangeError> {
        let root = root_dir(&self.root)?;
        let base = read_base(self.storage, &root).await?;
        if base.root_tag.is_some() {
            reject_git_in_root(self.storage, &root)
                .await
                .map_err(CommitError::from)?;
            self.storage.check_hierarchy(&root).await?;
        }
        if let Some(pending) = &base.pending_id {
            self.check_id(pending)?;
        }
        let inventory = base.committed.map(|committed| committed.inventory);
        if let Some(inventory) = &inventory {
            self.check_id(&inventory.id)?;
        }
        Ok(inventory)
    }

    /// Write `index` after checking that its working state over `base` has
    /// no two logical paths where one is the directory of the other (E095).
    ///
    /// # Errors
    ///
    /// [`ChangeError::InvalidPath`], or as
    /// [`StagingArea::write_index`].
    async fn save(&self, index: &StagingIndex, base: &WorkingState) -> Result<(), ChangeError> {
        check_working(index, base)?;
        self.staging.write_index(index).await
    }

    /// The committed inventory and the staging index, which records the
    /// current head as its base when nothing is staged yet.
    ///
    /// # Errors
    ///
    /// As [`committed`](Self::committed), [`ChangeError::StagingCorrupt`]
    /// or [`ChangeError::StagingMismatch`].
    /// The staging index as stored, or `None` when nothing is staged.
    ///
    /// # Errors
    ///
    /// [`ChangeError::StagingCorrupt`], or a backend error.
    pub(crate) async fn staged_index(&self) -> Result<Option<StagingIndex>, ChangeError> {
        self.staging.read_index().await
    }

    pub(super) async fn load(&self) -> Result<(Option<Inventory>, StagingIndex), ChangeError> {
        let inventory = self.committed().await?;
        let index = match self.staging.read_index().await? {
            Some(index) if index.object_id != self.object_id => {
                return Err(ChangeError::StagingMismatch {
                    expected: self.object_id.clone(),
                    found: index.object_id,
                })
            }
            Some(index) => index,
            None => StagingIndex::new(&self.object_id, inventory.as_ref().map(|i| i.head.clone())),
        };
        Ok((inventory, index))
    }

    /// Refuse an object root that holds another object.
    fn check_id(&self, found: &str) -> Result<(), ChangeError> {
        if found == self.object_id {
            return Ok(());
        }
        Err(ChangeError::WrongObject {
            expected: self.object_id.clone(),
            found: found.to_string(),
        })
    }
}

/// The digests the object's manifest holds (none for a new object).
pub(super) fn known(inventory: Option<&Inventory>) -> BTreeSet<String> {
    inventory.map(manifest_digests).unwrap_or_default()
}

/// The logical state of the index's base version.
///
/// # Errors
///
/// [`ChangeError::Read`] when the base is not a committed version.
pub(super) fn base_state(
    inventory: Option<&Inventory>,
    index: &StagingIndex,
) -> Result<WorkingState, ChangeError> {
    match &index.base {
        Some(version) => state_digests(inventory, version),
        None => Ok(WorkingState::new()),
    }
}

/// Refuse an index whose working state over `base` has a logical path that
/// is the directory of another (E095), naming that path.
fn check_working(index: &StagingIndex, base: &WorkingState) -> Result<(), ChangeError> {
    let working = index.working(base);
    match path_conflict(working.keys().map(String::as_str)) {
        Some(path) => Err(conflict_error(path)),
        None => Ok(()),
    }
}

/// The E095 error for `path`.
fn conflict_error(path: &str) -> ChangeError {
    invalid_path(
        path,
        "a logical path of the working state is the directory of another (E095)",
    )
}

/// Refuse a logical path that breaks the OCFL 1.1 path rules.
fn valid_path(path: &str) -> Result<(), ChangeError> {
    check_logical_path(path).map_err(|reason| invalid_path(path, &reason))
}

/// An invalid-path error.
fn invalid_path(path: &str, reason: &str) -> ChangeError {
    ChangeError::InvalidPath {
        path: path.chars().take(256).collect(),
        reason: reason.to_string(),
    }
}

/// The error for a path the working state does not hold.
fn not_staged(path: &str) -> ChangeError {
    ChangeError::PathNotFound {
        path: path.to_string(),
        version: None,
    }
}
