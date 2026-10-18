//! Everything stored under one object root, listed once (bounded), and the
//! storage hierarchy check of that root (E024, E090).

use std::collections::BTreeSet;

use super::report::ValidationReport;
use crate::guard::next_start;
use crate::read::small::child;
use crate::read::ReadError;
use crate::storage::{StorageBackend, StorageError, StorageKey, MAX_LIST_PAGE, MAX_WALK_ENTRIES};

/// Most listing pages read for one object: enough for [`MAX_WALK_ENTRIES`]
/// keys in full pages, plus the last one.
const MAX_PAGES: usize = MAX_WALK_ENTRIES / MAX_LIST_PAGE + 1;

/// The files of one object, as paths relative to its root.
#[derive(Debug, Clone, Default)]
pub(crate) struct Listing {
    root: String,
    files: BTreeSet<String>,
}

/// The files and directories directly in one directory.
#[derive(Debug, Clone, Default)]
pub(crate) struct Children {
    /// File names.
    pub files: BTreeSet<String>,
    /// Directory names (each holds at least one file).
    pub dirs: BTreeSet<String>,
}

impl Listing {
    /// List every key under `root` (a key prefix without a trailing `/`,
    /// empty for the backend root), at most [`MAX_WALK_ENTRIES`] keys in at
    /// most [`MAX_PAGES`] pages. Each continuation must be the page's last
    /// key and move forward, so a faulty listing cannot skip a file.
    ///
    /// # Errors
    ///
    /// [`ReadError::Storage`] for a backend error, a faulty continuation or
    /// an object above the key bound.
    pub(crate) async fn read<B: StorageBackend>(
        backend: &B,
        root: &str,
    ) -> Result<Self, ReadError> {
        let prefix = if root.is_empty() {
            String::new()
        } else {
            format!("{root}/")
        };
        let mut listing = Self {
            root: root.to_string(),
            files: BTreeSet::new(),
        };
        let mut after: Option<StorageKey> = None;
        for _ in 0..MAX_PAGES {
            let page = backend.list(&prefix, after.as_ref(), MAX_LIST_PAGE).await?;
            listing.note_page(&prefix, &page.keys)?;
            match next_start(&page, after.as_ref())? {
                None => return Ok(listing),
                Some(next) => after = Some(next),
            }
        }
        Err(over_bound("listing pages read for one object", MAX_PAGES))
    }

    /// Add one page of keys, refusing an object above the key bound.
    fn note_page(&mut self, prefix: &str, keys: &[StorageKey]) -> Result<(), ReadError> {
        let relative = keys
            .iter()
            .filter_map(|key| key.as_str().strip_prefix(prefix));
        self.files.extend(relative.map(str::to_string));
        if self.files.len() > MAX_WALK_ENTRIES {
            return Err(over_bound("keys listed for one object", MAX_WALK_ENTRIES));
        }
        Ok(())
    }

    /// The storage key of `rel` inside the object root (the root itself for
    /// an empty `rel`), for findings and reads.
    pub(crate) fn key(&self, rel: &str) -> String {
        if rel.is_empty() {
            return self.root.clone();
        }
        child(&self.root, rel)
    }

    /// Whether a file is stored at `rel`.
    pub(crate) fn contains(&self, rel: &str) -> bool {
        self.files.contains(rel)
    }

    /// Whether `rel` holds at least one file.
    pub(crate) fn has_dir(&self, rel: &str) -> bool {
        self.files_under(rel).next().is_some()
    }

    /// Every file below the directory `dir` (empty for the root), as paths
    /// relative to the object root, in order.
    pub(crate) fn files_under<'a>(&'a self, dir: &str) -> impl Iterator<Item = &'a str> + 'a {
        let start = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        self.files
            .range(start.clone()..)
            .take_while(move |path| path.starts_with(&start))
            .map(String::as_str)
    }

    /// The files and directories directly in `dir` (empty for the root).
    pub(crate) fn children(&self, dir: &str) -> Children {
        let skip = if dir.is_empty() { 0 } else { dir.len() + 1 };
        let mut children = Children::default();
        for path in self.files_under(dir) {
            match path[skip..].split_once('/') {
                Some((name, _)) => children.dirs.insert(name.to_string()),
                None => children.files.insert(path[skip..].to_string()),
            };
        }
        children
    }
}

/// The error for a listing above one of its bounds.
fn over_bound(what: &'static str, limit: usize) -> ReadError {
    StorageError::LimitExceeded {
        what,
        limit: limit as u64,
    }
    .into()
}

/// Run the backend's hierarchy check on the object root. An empty
/// directory is E024 and a soft or hard link is E090; either ends the
/// validation, since the stored files cannot be trusted to be what a
/// listing shows. Returns whether validation can go on.
///
/// # Errors
///
/// Any other backend error.
pub(crate) async fn check_hierarchy<B: StorageBackend>(
    backend: &B,
    root: &str,
    report: &mut ValidationReport,
) -> Result<bool, ReadError> {
    let fault = match backend.check_hierarchy(root).await {
        Ok(()) => return Ok(true),
        Err(StorageError::EmptyDirectoryForbidden { key }) => (
            "E024",
            key,
            "empty directories must not appear in an object",
        ),
        Err(StorageError::SymlinkForbidden { key } | StorageError::HardLinkForbidden { key }) => (
            "E090",
            key,
            "hard and soft links must not be used in a storage hierarchy",
        ),
        Err(other) => return Err(other.into()),
    };
    let (code, key, message) = fault;
    report.add(code, key, message);
    Ok(false)
}
