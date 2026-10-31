//! [`MemoryFolderIndex`]: the in-memory reference [`FolderIndex`], for tests and for hosts
//! without a file index. It keeps nothing across restarts.

use super::error::NavigationError;
use super::node::FolderScope;
use super::port::FolderIndex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

/// The files of one scope: every logical path, and those that are hidden or deleted.
#[derive(Default)]
struct Files {
    all: BTreeSet<String>,
    out_of_view: BTreeSet<String>,
}

/// The in-memory folder index.
#[derive(Default)]
pub struct MemoryFolderIndex {
    scopes: Mutex<BTreeMap<String, Files>>,
}

impl std::fmt::Debug for MemoryFolderIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryFolderIndex").finish_non_exhaustive()
    }
}

/// The map key of `scope`.
fn key(scope: &FolderScope) -> String {
    match scope {
        FolderScope::Collection(id) => format!("c:{}", id.as_str()),
    }
}

impl MemoryFolderIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Run `f` on the locked scopes.
    fn with<T>(
        &self,
        f: impl FnOnce(&mut BTreeMap<String, Files>) -> T,
    ) -> Result<T, NavigationError> {
        let mut scopes = self
            .scopes
            .lock()
            .map_err(|_| NavigationError::internal())?;
        Ok(f(&mut scopes))
    }

    /// Add an ordinary file at logical `path` to `scope`. A poisoned lock drops the call, so
    /// tests that need to know use the `FolderIndex` methods, which report it.
    pub fn add_file(&self, scope: &FolderScope, path: &str) {
        let added = self.with(|scopes| {
            scopes
                .entry(key(scope))
                .or_default()
                .all
                .insert(path.to_string());
        });
        debug_assert!(added.is_ok(), "memory folder index lock poisoned");
    }

    /// Make the file at `path` ordinary (`true`) or hidden or deleted (`false`).
    pub fn set_ordinary(&self, scope: &FolderScope, path: &str, ordinary: bool) {
        let set = self.with(|scopes| {
            let files = scopes.entry(key(scope)).or_default();
            if ordinary {
                files.out_of_view.remove(path);
            } else {
                files.out_of_view.insert(path.to_string());
            }
        });
        debug_assert!(set.is_ok(), "memory folder index lock poisoned");
    }

    /// The names of the folders directly below `parent` in `scope`, from the ordinary files
    /// under it only (the sorted set is probed from the prefix onward).
    fn folder_names(
        &self,
        scope: &FolderScope,
        parent: &str,
    ) -> Result<BTreeSet<String>, NavigationError> {
        let lead = if parent.is_empty() {
            String::new()
        } else {
            format!("{parent}/")
        };
        self.with(|scopes| {
            let Some(files) = scopes.get(&key(scope)) else {
                return BTreeSet::new();
            };
            files
                .all
                .range(lead.clone()..)
                .take_while(|path| path.starts_with(&lead))
                .filter(|path| !files.out_of_view.contains(*path))
                .filter_map(|path| {
                    path[lead.len()..]
                        .split_once('/')
                        .map(|(first, _)| first.to_string())
                })
                .collect()
        })
    }
}

impl FolderIndex for MemoryFolderIndex {
    fn subfolders(
        &self,
        scope: &FolderScope,
        parent: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, NavigationError> {
        Ok(self
            .folder_names(scope, parent)?
            .into_iter()
            .filter(|name| after.is_none_or(|after| name.as_str() > after))
            .take(limit)
            .collect())
    }

    fn folder_exists(&self, scope: &FolderScope, path: &str) -> Result<bool, NavigationError> {
        let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
        Ok(self.folder_names(scope, parent)?.contains(name))
    }
}
