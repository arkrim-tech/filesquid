//! [`VersionDraft`]: the next version of an object as the caller describes
//! it, before anything is written.
//!
//! A draft names the version's whole logical state. Each logical path is
//! either [`DraftFile::Staged`], bytes the caller placed on its staging
//! backend outside the storage root, or [`DraftFile::Existing`], a digest
//! already in the object's manifest (an unchanged, renamed or reinstated
//! file). Paths the draft leaves out are deleted from the new version's
//! state; their bytes stay for history.

use std::collections::BTreeMap;

use crate::inventory::User;
use crate::storage::StorageKey;

/// Where one logical file of a draft gets its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftFile {
    /// Bytes at this key of the caller's staging backend.
    Staged(StorageKey),
    /// Bytes already in the object, named by their manifest digest.
    Existing(String),
}

/// The next version of one object: its logical state and version metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDraft {
    object_id: String,
    created: String,
    message: Option<String>,
    user: Option<User>,
    files: BTreeMap<String, DraftFile>,
}

impl VersionDraft {
    /// An empty draft of a version of `object_id`, created at `created`
    /// (RFC 3339 UTC ending in `Z`, such as
    /// [`rfc3339_utc`](super::rfc3339_utc) writes).
    pub fn new(object_id: impl Into<String>, created: impl Into<String>) -> Self {
        Self {
            object_id: object_id.into(),
            created: created.into(),
            message: None,
            user: None,
            files: BTreeMap::new(),
        }
    }

    /// Set `path` to the bytes staged at `key`.
    pub fn stage(&mut self, path: impl Into<String>, key: StorageKey) -> &mut Self {
        self.files.insert(path.into(), DraftFile::Staged(key));
        self
    }

    /// Set `path` to bytes the object already holds under `digest`.
    pub fn keep(&mut self, path: impl Into<String>, digest: impl Into<String>) -> &mut Self {
        self.files
            .insert(path.into(), DraftFile::Existing(digest.into()));
        self
    }

    /// Set the version's `message`.
    pub fn set_message(&mut self, message: impl Into<String>) -> &mut Self {
        self.message = Some(message.into());
        self
    }

    /// Set the version's `user`.
    pub fn set_user(&mut self, user: User) -> &mut Self {
        self.user = Some(user);
        self
    }

    /// The object identifier.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// The version's `created` value.
    pub fn created(&self) -> &str {
        &self.created
    }

    /// The version's `message`, when set.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The version's `user`, when set.
    pub fn user(&self) -> Option<&User> {
        self.user.as_ref()
    }

    /// Every logical path with its source, in path order.
    pub fn files(&self) -> &BTreeMap<String, DraftFile> {
        &self.files
    }
}
