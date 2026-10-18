//! OCFL Git-guard unit tests (OCFL-S-001, ADR 016 layout constraints).
//!
//! The engine guard runs over the in-memory backend, so these tests need no
//! Cargo features. The path-based helper is checked when `fs` is on.

use filesquid_ocfl::storage::{
    complete_immediately, ByteRange, CompletedPart, ListPage, MemoryBackend, ObjectMeta,
    Precondition, StorageBackend, StorageError, StorageKey, UploadId,
};
use filesquid_ocfl::{check_storage_root, reject_git_in_root, OcflError};

/// The OCFL 1.1 object declaration file name.
const OBJECT_DECLARATION: &str = "0=ocfl_object_1.1";

/// An in-memory storage root holding an empty object at each of `keys`.
fn root_with(keys: &[&str]) -> MemoryBackend {
    let backend = MemoryBackend::new();
    for raw in keys {
        let key = StorageKey::new(raw).expect("valid key");
        complete_immediately(backend.put(&key, b"x"))
            .expect("ready")
            .expect("put");
    }
    backend
}

/// Run the full storage-root guard on `backend`.
fn check(backend: &MemoryBackend) -> Result<(), OcflError> {
    complete_immediately(check_storage_root(backend)).expect("ready")
}

/// The error for a `.git` held by the directory `root`.
fn git_at(root: &str) -> Result<(), OcflError> {
    Err(OcflError::GitDirectoryForbidden {
        root: root.to_string(),
    })
}

/// A small valid layout: storage declaration and two objects.
const LAYOUT: [&str; 5] = [
    "0=ocfl_1.1",
    "ab/obj-1/0=ocfl_object_1.1",
    "ab/obj-1/inventory.json",
    "cd/obj-2/0=ocfl_object_1.1",
    "cd/obj-2/v1/content/a.txt",
];

#[test]
fn a_clean_storage_root_passes() {
    assert_eq!(check(&root_with(&LAYOUT)), Ok(()));
}

#[test]
fn an_empty_storage_root_passes() {
    assert_eq!(check(&MemoryBackend::new()), Ok(()));
}

#[test]
fn git_in_the_storage_root_is_rejected() {
    let mut keys = LAYOUT.to_vec();
    keys.push(".git/HEAD");
    assert_eq!(check(&root_with(&keys)), git_at(""));
}

#[test]
fn a_git_file_in_the_storage_root_is_rejected() {
    let mut keys = LAYOUT.to_vec();
    keys.push(".git");
    assert_eq!(check(&root_with(&keys)), git_at(""));
}

#[test]
fn git_in_an_object_root_is_rejected() {
    let mut keys = LAYOUT.to_vec();
    keys.push("cd/obj-2/.git/config");
    assert_eq!(check(&root_with(&keys)), git_at("cd/obj-2"));
}

#[test]
fn git_in_a_storage_hierarchy_directory_is_rejected() {
    let mut keys = LAYOUT.to_vec();
    keys.push("ab/.git/HEAD");
    assert_eq!(check(&root_with(&keys)), git_at("ab"));
}

#[test]
fn git_inside_object_content_is_preserved_content() {
    let mut keys = LAYOUT.to_vec();
    keys.push("cd/obj-2/v1/content/repo/.git/HEAD");
    assert_eq!(check(&root_with(&keys)), Ok(()));
}

#[test]
fn a_declaration_inside_content_is_not_an_object_root() {
    let nested = format!("cd/obj-2/v1/content/inner/{OBJECT_DECLARATION}");
    let mut keys = LAYOUT.to_vec();
    keys.push(&nested);
    keys.push("cd/obj-2/v1/content/inner/.git/HEAD");
    assert_eq!(check(&root_with(&keys)), Ok(()));
}

#[test]
fn reject_git_in_root_checks_one_root() {
    let backend = root_with(&["obj/.git/HEAD", "other/inventory.json"]);
    let at = |root: &str| complete_immediately(reject_git_in_root(&backend, root)).expect("ready");
    assert_eq!(at("obj"), git_at("obj"));
    assert_eq!(at("obj/"), git_at("obj"));
    assert_eq!(at("other"), Ok(()));
    assert_eq!(at(""), Ok(()));
}

#[test]
fn reject_git_in_root_refuses_an_escaping_root() {
    let backend = MemoryBackend::new();
    let result = complete_immediately(reject_git_in_root(&backend, "../up")).expect("ready");
    assert!(matches!(result, Err(OcflError::Storage(_))), "{result:?}");
}

#[test]
fn reject_git_in_root_refuses_invalid_prefixes() {
    let backend = root_with(&[".git/HEAD"]);
    for root in ["/", ".", "./", "..", "a//"] {
        let result = complete_immediately(reject_git_in_root(&backend, root)).expect("ready");
        assert!(
            matches!(
                result,
                Err(OcflError::Storage(StorageError::InvalidKey { .. }))
            ),
            "{root:?}: {result:?}"
        );
    }
}

/// How the [`Paging`] backend serves listings.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Correct pages of at most the configured size.
    Honest,
    /// Every listing returns the first key again with a continuation that
    /// never moves past it.
    Stall,
    /// The continuation jumps past keys the page never returned.
    JumpAhead,
    /// An empty page that still carries a continuation.
    EmptyWithNext,
}

/// A backend that serves listings in pages of at most `page` keys, in the
/// given [`Mode`].
struct Paging {
    inner: MemoryBackend,
    page: usize,
    mode: Mode,
}

impl StorageBackend for Paging {
    async fn put(&self, key: &StorageKey, bytes: &[u8]) -> Result<ObjectMeta, StorageError> {
        self.inner.put(key, bytes).await
    }

    async fn put_if(
        &self,
        key: &StorageKey,
        bytes: &[u8],
        precondition: &Precondition,
    ) -> Result<ObjectMeta, StorageError> {
        self.inner.put_if(key, bytes, precondition).await
    }

    async fn get_range(&self, key: &StorageKey, range: ByteRange) -> Result<Vec<u8>, StorageError> {
        self.inner.get_range(key, range).await
    }

    async fn head(&self, key: &StorageKey) -> Result<ObjectMeta, StorageError> {
        self.inner.head(key).await
    }

    async fn exists(&self, key: &StorageKey) -> Result<bool, StorageError> {
        self.inner.exists(key).await
    }

    async fn list(
        &self,
        prefix: &str,
        start_after: Option<&StorageKey>,
        limit: usize,
    ) -> Result<ListPage, StorageError> {
        let size = limit.min(self.page);
        match self.mode {
            Mode::Honest => self.inner.list(prefix, start_after, size).await,
            Mode::Stall => {
                let mut page = self.inner.list(prefix, None, 1).await?;
                page.next = page.keys.first().cloned();
                Ok(page)
            }
            Mode::JumpAhead => {
                let mut page = self.inner.list(prefix, start_after, size).await?;
                let Some(after) = page.next.clone() else {
                    return Ok(page);
                };
                let skipped = self.inner.list(prefix, Some(&after), 2).await?;
                page.next = skipped.keys.last().cloned().or(Some(after));
                Ok(page)
            }
            Mode::EmptyWithNext => {
                let mut page = self.inner.list(prefix, start_after, size).await?;
                page.next = page.keys.pop();
                page.keys.clear();
                Ok(page)
            }
        }
    }

    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        self.inner.delete(key).await
    }

    async fn create_multipart(&self, key: &StorageKey) -> Result<UploadId, StorageError> {
        self.inner.create_multipart(key).await
    }

    async fn upload_part(
        &self,
        upload: &UploadId,
        part_number: u32,
        bytes: &[u8],
    ) -> Result<CompletedPart, StorageError> {
        self.inner.upload_part(upload, part_number, bytes).await
    }

    async fn complete_multipart(
        &self,
        upload: &UploadId,
        parts: &[CompletedPart],
        precondition: Option<&Precondition>,
    ) -> Result<ObjectMeta, StorageError> {
        self.inner
            .complete_multipart(upload, parts, precondition)
            .await
    }

    async fn abort_multipart(&self, upload: &UploadId) -> Result<(), StorageError> {
        self.inner.abort_multipart(upload).await
    }
}

/// `count` keys under `obj/v1/content/`, served `page` keys at a time.
fn paged_root(count: usize, page: usize, mode: Mode) -> Paging {
    let keys: Vec<String> = (0..count)
        .map(|i| format!("obj/v1/content/f{i:05}"))
        .collect();
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    Paging {
        inner: root_with(&refs),
        page,
        mode,
    }
}

#[test]
fn short_pages_do_not_shrink_the_key_bound() {
    let backend = paged_root(1_001, 1, Mode::Honest);
    let result = complete_immediately(check_storage_root(&backend)).expect("ready");
    assert_eq!(result, Ok(()));
}

#[test]
fn a_continuation_that_makes_no_progress_is_refused() {
    let backend = paged_root(3, 1, Mode::Stall);
    let result = complete_immediately(check_storage_root(&backend)).expect("ready");
    assert!(
        matches!(
            result,
            Err(OcflError::Storage(StorageError::Backend { .. }))
        ),
        "{result:?}"
    );
}

/// The guard over `backend` must refuse its listing as a backend fault.
fn assert_listing_refused(backend: &Paging) {
    let result = complete_immediately(check_storage_root(backend)).expect("ready");
    assert!(
        matches!(
            result,
            Err(OcflError::Storage(StorageError::Backend { .. }))
        ),
        "{result:?}"
    );
}

#[test]
fn a_continuation_that_skips_keys_is_refused() {
    // The only .git sits in the keys the continuation jumps over, so a guard
    // that trusts the continuation passes the root.
    let backend = paged_root(6, 2, Mode::JumpAhead);
    let git = StorageKey::new("obj/v1/content/f00002/.git/HEAD").expect("key");
    complete_immediately(backend.inner.put(&git, b"x"))
        .expect("ready")
        .expect("put");
    assert_listing_refused(&backend);
}

#[test]
fn an_empty_page_with_a_continuation_is_refused() {
    assert_listing_refused(&paged_root(4, 2, Mode::EmptyWithNext));
}

#[test]
fn object_stores_have_no_links_or_empty_directories() {
    let backend = root_with(&LAYOUT);
    let checked = complete_immediately(backend.check_hierarchy("")).expect("ready");
    assert_eq!(checked, Ok(()));
}

#[cfg(feature = "fs")]
mod path_helper {
    use filesquid_ocfl::{reject_git_in_ocfl_root, OcflError};
    use std::fs;

    /// A scratch directory under Cargo's per-target temporary directory,
    /// removed on drop.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                .join(format!("ocfl-git-guard-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("scratch");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn forbidden(dir: &std::path::Path) -> Result<(), OcflError> {
        Err(OcflError::GitDirectoryForbidden {
            root: dir.display().to_string(),
        })
    }

    #[test]
    fn git_directory_is_rejected() {
        let scratch = Scratch::new("dir");
        fs::create_dir_all(scratch.0.join(".git")).expect("git dir");
        assert_eq!(reject_git_in_ocfl_root(&scratch.0), forbidden(&scratch.0));
    }

    #[test]
    fn git_file_is_rejected() {
        let scratch = Scratch::new("file");
        fs::write(scratch.0.join(".git"), "gitdir: ../elsewhere").expect("git file");
        assert_eq!(reject_git_in_ocfl_root(&scratch.0), forbidden(&scratch.0));
    }

    #[cfg(unix)]
    #[test]
    fn dangling_git_link_is_rejected() {
        let scratch = Scratch::new("link");
        std::os::unix::fs::symlink(scratch.0.join("missing"), scratch.0.join(".git"))
            .expect("dangling link");
        assert_eq!(reject_git_in_ocfl_root(&scratch.0), forbidden(&scratch.0));
    }

    #[test]
    fn a_root_without_git_passes() {
        let scratch = Scratch::new("clean");
        assert_eq!(reject_git_in_ocfl_root(&scratch.0), Ok(()));
    }
}
