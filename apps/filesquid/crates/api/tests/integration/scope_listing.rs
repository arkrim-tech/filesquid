//! Task DE-1.7 (DE-S-018, API-S-011, API-S-021): `GET /api/v1/files/listing?node=` of a
//! collection or folder node answered from the host's index of registered files, one object
//! per file: the node's subfolders, then its files, each naming its own object and shown under
//! its name in the collection; paged with an opaque cursor; never a filesystem path.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/library_fixture.rs"]
mod library_fixture;

#[path = "../support/drive_catalog.rs"]
mod drive_catalog;

use axum::http::{Method, StatusCode};
use drive_catalog::DriveCatalog;
use files_fixture::{encode, upload_uri, Answer, FilesRig, LIBRARY};
use filesquid_api::files::{
    FileError, ScopeAfter, ScopeEntry, ScopeObject, ScopeObjects, ScopePage,
};
use filesquid_api::navigation::{
    FolderScope, HostAccess, MemoryFolderIndex, NavigationService, NodeRef, RepositoryInfo,
};
use filesquid_core::{CollectionId, RepositoryId};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// A host index over one collection: `files` are (collection logical path, file ID).
struct Indexed {
    collection: CollectionId,
    files: Mutex<Vec<(String, String)>>,
}

impl Indexed {
    /// The entries directly in `folder` ("" for the root), folders first, by name.
    fn entries_in(&self, folder: &str) -> (Vec<String>, Vec<(String, String)>) {
        let prefix = match folder {
            "" => String::new(),
            folder => format!("{folder}/"),
        };
        let mut folders = Vec::new();
        let mut files = Vec::new();
        for (path, id) in self.files.lock().expect("files").iter() {
            let Some(rest) = path.strip_prefix(&prefix) else {
                continue;
            };
            match rest.split_once('/') {
                Some((name, _)) => folders.push(name.to_string()),
                None => files.push((rest.to_string(), id.clone())),
            }
        }
        folders.sort();
        folders.dedup();
        files.sort();
        (folders, files)
    }
}

impl ScopeObjects for Indexed {
    fn object_of(&self, _scope: &FolderScope) -> Option<ScopeObject> {
        None
    }

    fn entries(
        &self,
        scope: &FolderScope,
        folder: &str,
        after: Option<&ScopeAfter>,
        limit: usize,
    ) -> Result<Option<ScopePage>, FileError> {
        if *scope != FolderScope::Collection(self.collection.clone()) {
            return Ok(None);
        }
        let (folders, files) = self.entries_in(folder);
        let mut entries: Vec<ScopeEntry> = Vec::new();
        let after_folder = match after {
            Some(ScopeAfter::Folder { name }) => Some(name.clone()),
            _ => None,
        };
        if !matches!(after, Some(ScopeAfter::File { .. })) {
            let folders = folders
                .into_iter()
                .filter(|name| after_folder.as_ref().is_none_or(|a| name > a))
                .map(|name| ScopeEntry::Folder { name });
            entries.extend(folders);
        }
        let after_file = match after {
            Some(ScopeAfter::File { name, file_id }) => Some((name.clone(), file_id.clone())),
            _ => None,
        };
        let files = files
            .into_iter()
            .filter(|f| after_file.as_ref().is_none_or(|a| f > a))
            .map(|(name, file_id)| ScopeEntry::File { name, file_id });
        entries.extend(files);
        entries.truncate(limit);
        Ok(Some(ScopePage {
            library_id: LIBRARY.to_string(),
            entries,
        }))
    }
}

/// Upload `bytes` as a new file named `name` (NFC) and answer its file ID.
async fn upload(rig: &FilesRig, name: &str, bytes: &[u8]) -> String {
    let uri = upload_uri(name, bytes.len(), "name_normalization=nfc");
    let answer = rig.call(Method::PUT, &uri, &[], bytes.to_vec()).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.json()["file_id"]
        .as_str()
        .expect("file id")
        .to_string()
}

/// A rig with two uploaded files registered in the drive catalog's collection at
/// `trips/beach-1.jpg` and `notes.txt`.
async fn rig() -> (FilesRig, DriveCatalog, Vec<String>) {
    // The fixture shares the files rig's library ID with the drive catalog's library.
    let rig = FilesRig::new().await;
    let drive = DriveCatalog::new(LIBRARY);
    let collection = CollectionId::new(drive_catalog::COLLECTION).expect("collection");
    // The navigation service checks a folder node against the folders committed paths
    // imply; the desktop answers them from the same index (`CatalogFolders`).
    let folders = MemoryFolderIndex::new();
    folders.add_file(
        &FolderScope::Collection(collection.clone()),
        "trips/beach-1.jpg",
    );
    let navigation = NavigationService::new(
        drive.catalog.clone(),
        Arc::new(folders),
        Arc::new(HostAccess),
        RepositoryInfo::new(
            RepositoryId::new(drive_catalog::REPOSITORY).expect("repository"),
            None,
        ),
    );
    assert!(rig.state.install_navigation(Arc::new(navigation)));
    let beach = upload(&rig, "beach.jpg", b"sand").await;
    let notes = upload(&rig, "notes.txt", b"words").await;
    drive.file_in_collection(&beach);
    drive.file_in_collection(&notes);
    let index = Indexed {
        collection,
        files: Mutex::new(vec![
            ("trips/beach-1.jpg".to_string(), beach.clone()),
            ("notes.txt".to_string(), notes.clone()),
        ]),
    };
    assert!(rig.service.install_scope_objects(Arc::new(index)));
    (rig, drive, vec![beach, notes])
}

/// `GET /api/v1/files/listing?node=<handle><rest>`.
async fn list(rig: &FilesRig, handle: &str, rest: &str) -> Answer {
    rig.get(&format!(
        "/api/v1/files/listing?node={}{rest}",
        encode(handle)
    ))
    .await
}

/// The kind and shown name of each entry of a page.
fn shown(page: &Value) -> Vec<(String, String)> {
    page["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| {
            let name = e.get("display_name").unwrap_or(&e["name"]);
            (
                e["kind"].as_str().expect("kind").to_string(),
                name.as_str().expect("name").to_string(),
            )
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_collection_node_lists_its_folders_then_its_registered_files() {
    let (rig, drive, files) = rig().await;
    let answer = list(&rig, &drive.collection_node(), "").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let page = answer.json();
    assert_eq!(
        shown(&page),
        [
            ("folder".to_string(), "trips".to_string()),
            ("file".to_string(), "notes.txt".to_string())
        ]
    );
    let file = &page["entries"][1];
    assert_eq!(file["object_id"], format!("urn:uuid:{}", files[1]));
    assert!(file["path"]
        .as_str()
        .expect("path")
        .starts_with("original/"));
    assert_eq!(file["size"], 5);
    assert_eq!(page["library_id"], LIBRARY);
    assert_eq!(page["prefix"], "");
    assert_eq!(page["next_cursor"], Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_node_lists_the_files_beneath_it_under_their_numbered_names() {
    let (rig, _drive, files) = rig().await;
    let collection = CollectionId::new(drive_catalog::COLLECTION).expect("collection");
    let folder = NodeRef::folder(FolderScope::Collection(collection), "trips")
        .to_handle()
        .expect("handle");
    let answer = list(&rig, &folder, "").await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let page = answer.json();
    assert_eq!(
        shown(&page),
        [("file".to_string(), "beach-1.jpg".to_string())]
    );
    assert_eq!(
        page["entries"][0]["object_id"],
        format!("urn:uuid:{}", files[0])
    );
    assert_eq!(page["prefix"], "trips");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scope_listing_pages_with_an_opaque_cursor() {
    let (rig, drive, _) = rig().await;
    let first = list(&rig, &drive.collection_node(), "&limit=1").await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.json());
    assert_eq!(shown(&first.json()), [("folder".into(), "trips".into())]);
    let cursor = first.json()["next_cursor"]
        .as_str()
        .expect("a next page")
        .to_string();
    assert!(!cursor.contains("trips"), "the cursor is opaque");
    let rest = format!("&limit=1&cursor={}", encode(&cursor));
    let second = list(&rig, &drive.collection_node(), &rest).await;
    assert_eq!(second.status, StatusCode::OK, "{:?}", second.json());
    assert_eq!(shown(&second.json()), [("file".into(), "notes.txt".into())]);
    assert_eq!(second.json()["next_cursor"], Value::Null);
    let bad = list(&rig, &drive.collection_node(), "&cursor=s1.zz").await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let limit = list(&rig, &drive.collection_node(), "&limit=0").await;
    assert_eq!(limit.status, StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scope_listing_names_no_filesystem_path() {
    let (rig, drive, _) = rig().await;
    let answer = list(&rig, &drive.collection_node(), "").await;
    let text = answer.json().to_string();
    let root = rig.dir.to_string_lossy().to_string();
    assert!(!text.contains(&root), "{text}");
    assert!(
        !text.contains("/tmp") && !text.contains("/dev/shm"),
        "{text}"
    );
}
