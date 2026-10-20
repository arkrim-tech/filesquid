//! Shared fixture for the navigation listing tests (Task API-F1.11; API-S-021): a library
//! catalog with library "Family archive", collection "Photos" and virtual collection "Best
//! of 2025", a real OCFL storage root per folder scope behind [`OcflFolders`] (a
//! [`FolderIndex`] over the API-F1.3 folder listing by logical-path prefix), and a
//! [`NavigationService`] installed in the API state of the Photos rig.
//!
//! The including test crate must declare the `files_fixture` and `library_fixture` modules at
//! its root, as `tests/features.rs` and `tests/integration/navigation.rs` do.

#![allow(dead_code)]

use crate::files_fixture::{FilesRig, LIBRARY, OBJECT};
use crate::library_fixture::Fixture;
use filesquid_api::auth::Principal;
use filesquid_api::files::ListingQuery;
use filesquid_api::library::LibraryCatalog;
use filesquid_api::navigation::{
    FolderIndex, FolderScope, NavigationAccess, NavigationError, NavigationPage, NavigationRequest,
    NavigationService, NodeRef, RepositoryInfo,
};
use filesquid_core::{CollectionId, LibraryId, VirtualCollectionId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Deepest folder the walk of [`OcflFolders`] follows (AGENTS.md section 22).
const MAX_DEPTH: usize = 16;

/// Most listing pages one folder walk reads.
const MAX_PAGES: usize = 100;

/// The text key of a folder scope.
fn scope_key(scope: &FolderScope) -> String {
    match scope {
        FolderScope::Collection(id) => format!("c:{}", id.as_str()),
        FolderScope::Unfiled(id) => format!("u:{}", id.as_str()),
    }
}

/// A [`FolderIndex`] over real OCFL objects: each scope's files live in the one object of a
/// [`FilesRig`], and folders come from that rig's folder listing by logical-path prefix. A
/// path under a hidden or deleted prefix is left out, as the ordinary view of the catalog
/// would leave it out.
pub struct OcflFolders {
    runtime: tokio::runtime::Handle,
    roots: Mutex<HashMap<String, Arc<FilesRig>>>,
    hidden: Mutex<Vec<(String, String)>>,
}

impl OcflFolders {
    /// An index with no scope, reading on `runtime`. Its methods block, so call them from a
    /// blocking thread.
    pub fn new(runtime: tokio::runtime::Handle) -> Self {
        Self {
            runtime,
            roots: Mutex::new(HashMap::new()),
            hidden: Mutex::new(Vec::new()),
        }
    }

    /// Serve `scope` from the OCFL root of `rig`.
    pub fn serve(&self, scope: &FolderScope, rig: Arc<FilesRig>) {
        self.roots
            .lock()
            .expect("roots")
            .insert(scope_key(scope), rig);
    }

    /// Hide (or delete) every file of `scope` under `prefix`.
    pub fn hide(&self, scope: &FolderScope, prefix: &str) {
        self.hidden
            .lock()
            .expect("hidden")
            .push((scope_key(scope), prefix.to_string()));
    }

    /// Every ordinary file path of `scope`, by walking the folder listing.
    fn ordinary_paths(&self, scope: &FolderScope) -> Result<Vec<String>, NavigationError> {
        let key = scope_key(scope);
        let rig = self.roots.lock().expect("roots").get(&key).cloned();
        let Some(rig) = rig else {
            return Ok(Vec::new());
        };
        let mut files = Vec::new();
        self.walk(&rig, String::new(), 0, &mut files)?;
        let hidden = self.hidden.lock().expect("hidden").clone();
        files.retain(|path| {
            !hidden
                .iter()
                .any(|(k, prefix)| *k == key && path.starts_with(&format!("{prefix}/")))
        });
        Ok(files)
    }

    /// Collect the file paths below `prefix` into `files`.
    fn walk(
        &self,
        rig: &FilesRig,
        prefix: String,
        depth: usize,
        files: &mut Vec<String>,
    ) -> Result<(), NavigationError> {
        if depth > MAX_DEPTH {
            return Err(NavigationError::internal());
        }
        let mut cursor = None;
        for _ in 0..MAX_PAGES {
            let query = ListingQuery {
                library_id: LIBRARY.to_string(),
                object_id: OBJECT.to_string(),
                version: None,
                prefix: (!prefix.is_empty()).then(|| prefix.clone()),
                cursor: cursor.clone(),
                limit: Some(1000),
            };
            let page = self
                .runtime
                .block_on(rig.service.listing(query))
                .map_err(|_| NavigationError::unavailable())?;
            let page = serde_json::to_value(&page).map_err(|_| NavigationError::internal())?;
            for entry in page["entries"].as_array().into_iter().flatten() {
                let path = entry["path"].as_str().unwrap_or_default().to_string();
                if entry["kind"] == "folder" {
                    self.walk(rig, path, depth + 1, files)?;
                } else {
                    files.push(path);
                }
            }
            cursor = page["next_cursor"].as_str().map(str::to_string);
            if cursor.is_none() {
                return Ok(());
            }
        }
        Err(NavigationError::internal())
    }
}

impl FolderIndex for OcflFolders {
    fn subfolders(
        &self,
        scope: &FolderScope,
        parent: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, NavigationError> {
        let lead = if parent.is_empty() {
            String::new()
        } else {
            format!("{parent}/")
        };
        let mut names: Vec<String> = self
            .ordinary_paths(scope)?
            .iter()
            .filter_map(|path| path.strip_prefix(&lead))
            .filter_map(|rest| rest.split_once('/').map(|(first, _)| first.to_string()))
            .collect();
        names.sort();
        names.dedup();
        names.retain(|name| after.is_none_or(|after| name.as_str() > after));
        names.truncate(limit);
        Ok(names)
    }

    fn folder_exists(&self, scope: &FolderScope, path: &str) -> Result<bool, NavigationError> {
        let lead = format!("{path}/");
        Ok(self
            .ordinary_paths(scope)?
            .iter()
            .any(|file| file.starts_with(&lead)))
    }
}

/// Readability decided by a closure, so a test names exactly what a user may read.
pub struct FnAccess(pub Box<dyn Fn(&NodeRef) -> bool + Send + Sync>);

impl NavigationAccess for FnAccess {
    fn may_read(&self, _principal: &Principal, node: &NodeRef) -> bool {
        (self.0)(node)
    }
}

/// The catalog, two OCFL roots and the navigation service over them.
pub struct NavRig {
    /// The library catalog and its commands.
    pub lib: Fixture,
    /// The library "Family archive".
    pub library: LibraryId,
    /// Collection "Photos".
    pub photos: CollectionId,
    /// Virtual collection "Best of 2025".
    pub best_of: VirtualCollectionId,
    /// The OCFL root of the files of "Photos"; its API state serves the router.
    pub photos_rig: Arc<FilesRig>,
    /// The OCFL root of the library's unfiled files.
    pub unfiled_rig: Arc<FilesRig>,
    /// The folder index over both roots.
    pub folders: Arc<OcflFolders>,
    /// The service the router calls (the owner reads everything).
    pub service: Arc<NavigationService>,
}

impl NavRig {
    /// Build the standard fixture of the feature: "Photos" holds `2025/summer/beach.jpg` and
    /// `2025/winter/snow.jpg`; the library holds the unfiled file `inbox/note.txt`. Call it
    /// from a multi-thread Tokio test.
    pub async fn new() -> Self {
        let lib = Fixture::with_library();
        let library = lib.library();
        let photos = lib.create_collection("Photos");
        let best_of = lib.create_virtual_collection("Best of 2025", None);
        let photos_rig = Arc::new(
            FilesRig::with_files(&[
                ("2025/summer/beach.jpg", b"beach"),
                ("2025/winter/snow.jpg", b"snow"),
            ])
            .await,
        );
        let unfiled_rig = Arc::new(FilesRig::with_files(&[("inbox/note.txt", b"note")]).await);
        let folders = Arc::new(OcflFolders::new(tokio::runtime::Handle::current()));
        folders.serve(&FolderScope::Collection(photos.clone()), photos_rig.clone());
        folders.serve(&FolderScope::Unfiled(library.clone()), unfiled_rig.clone());
        let service = Arc::new(NavigationService::new(
            lib.catalog.clone() as Arc<dyn LibraryCatalog>,
            folders.clone(),
            Arc::new(filesquid_api::navigation::HostAccess),
            RepositoryInfo::new(lib.repository.clone(), None),
        ));
        assert!(photos_rig.state.install_navigation(service.clone()));
        Self {
            lib,
            library,
            photos,
            best_of,
            photos_rig,
            unfiled_rig,
            folders,
            service,
        }
    }

    /// A service over the same catalog and folders for a user who may read what `allow` says.
    pub fn user(
        &self,
        allow: impl Fn(&NodeRef) -> bool + Send + Sync + 'static,
    ) -> NavigationService {
        NavigationService::new(
            self.lib.catalog.clone() as Arc<dyn LibraryCatalog>,
            self.folders.clone(),
            Arc::new(FnAccess(Box::new(allow))),
            RepositoryInfo::new(self.lib.repository.clone(), None),
        )
    }

    /// The handle of `node`.
    pub fn handle(&self, node: &NodeRef) -> String {
        self.service.handle_of(node).expect("handle")
    }

    /// The handle of folder `path` in the Photos collection.
    pub fn photos_folder(&self, path: &str) -> String {
        self.handle(&NodeRef::folder(
            FolderScope::Collection(self.photos.clone()),
            path,
        ))
    }

    /// `GET /api/v1/navigation` with `query` through the router, as the owner.
    pub async fn get(&self, query: &str) -> crate::files_fixture::Answer {
        let uri = if query.is_empty() {
            "/api/v1/navigation".to_string()
        } else {
            format!("/api/v1/navigation?{query}")
        };
        self.photos_rig.get(&uri).await
    }

    /// List through `service` for the owner, on a blocking thread (the folder index blocks).
    pub async fn list(
        service: Arc<NavigationService>,
        request: NavigationRequest,
    ) -> Result<NavigationPage, NavigationError> {
        tokio::task::spawn_blocking(move || service.list(&Principal::Owner, &request))
            .await
            .expect("list task")
    }
}
