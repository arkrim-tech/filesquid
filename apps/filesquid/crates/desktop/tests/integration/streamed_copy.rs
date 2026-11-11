//! Task API-F2.26 on the desktop (API-S-011, NFR-003, OCFL-S-026): a file above the former
//! 64 MiB copy bound, imported by the native worker, is copied through the `filesquid://`
//! scheme with the per-launch token. The copy is a new file with its source's SHA-256 and
//! size, and the staged bytes go to the library's staging area on disk, under the data
//! directory, which holds no staged file afterwards.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

#[path = "../support/import_rig.rs"]
#[allow(dead_code)]
mod import_rig;

use filesquid_api::navigation::{FolderScope, NodeRef};
use filesquid_core::CollectionId;
use import_rig::{fresh_dir, ImportRig, FOLDER, LIBRARY};
use journal::block_on;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The size of the copied file: 70 MiB, above the former 64 MiB bound.
const FILE_BYTES: u64 = 70 * 1024 * 1024;

/// Most directories [`files_under`] visits, so its walk is bounded.
const MAX_DIRECTORIES: usize = 10_000;

/// A folder `<dir>/Films` with one sparse file of [`FILE_BYTES`] zero bytes (filesystem I/O).
fn film_folder(dir: &Path) -> PathBuf {
    let folder = dir.join("Films");
    std::fs::create_dir_all(&folder).expect("folder");
    let film = std::fs::File::create(folder.join("Film.mov")).expect("film");
    film.set_len(FILE_BYTES).expect("sparse film");
    folder
}

/// The opaque handle of `node`.
fn handle(node: &NodeRef) -> String {
    node.to_handle().expect("handle")
}

/// Every regular file under `dir`, by a bounded walk; none when `dir` does not exist.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    for _ in 0..MAX_DIRECTORIES {
        let Some(next) = pending.pop() else {
            return found;
        };
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match path.is_dir() {
                true => pending.push(path),
                false => found.push(path),
            }
        }
    }
    panic!("more than {MAX_DIRECTORIES} directories under {dir:?}");
}

/// The files of [`FILE_BYTES`] under `dir`: the stored original, its copy, or a staged blob.
fn film_sized(dir: &Path) -> usize {
    files_under(dir)
        .iter()
        .filter(|path| std::fs::metadata(path).is_ok_and(|meta| meta.len() == FILE_BYTES))
        .count()
}

/// The object ID of the one imported file, read from the listing of its folder.
fn imported_object(rig: &ImportRig) -> String {
    let collection = CollectionId::new(rig.collection.clone()).expect("collection");
    let scope = FolderScope::Collection(collection);
    let films = handle(&NodeRef::folder(scope, format!("{FOLDER}/Films")));
    let (status, page) = block_on(rig.get(&format!("/api/v1/files/listing?node={films}")));
    assert_eq!(status, 200, "{page}");
    let entries = page["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1, "{page}");
    entries[0]["object_id"]
        .as_str()
        .expect("object")
        .to_string()
}

/// The stored original of `object_id`: its listing entry, with its digest and size.
fn original_of(rig: &ImportRig, object_id: &str) -> Value {
    let uri = format!(
        "/api/v1/files/listing?library_id={LIBRARY}&object_id={}&prefix=original",
        object_id.replace(':', "%3A")
    );
    let (status, page) = block_on(rig.get(&uri));
    assert_eq!(status, 200, "{page}");
    let entries = page["entries"].as_array().expect("entries").clone();
    entries
        .into_iter()
        .find(|entry| entry["size"].as_u64() == Some(FILE_BYTES))
        .expect("the original")
}

/// The head version of `object_id`, read through the versions route.
fn head_of(rig: &ImportRig, object_id: &str) -> String {
    let uri = format!(
        "/api/v1/files/versions?library_id={LIBRARY}&object_id={}",
        object_id.replace(':', "%3A")
    );
    let (status, versions) = block_on(rig.get(&uri));
    assert_eq!(status, 200, "{versions}");
    versions["head"].as_str().expect("head").to_string()
}

#[test]
fn a_copy_above_64_mib_streams_through_the_scheme() {
    let dir = fresh_dir("streamed-copy");
    let folder = film_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let (status, answer) = block_on(rig.import(LIBRARY, "op-film", "folder"));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(rig.wait_for_files(1), 1, "the film is imported");
    rig.stop_worker();
    rig.install_home_collections();
    let source = imported_object(&rig);
    let file_id = source.strip_prefix("urn:uuid:").expect("a file object");
    let body = json!({
        "envelope": {
            "schema_version": 1,
            "operation_id": "op-copy-film",
            "expected_revision": head_of(&rig, &source),
            "scope": format!("library:{LIBRARY}"),
        },
        "input": {
            "library_id": LIBRARY, "file_id": file_id, "collection_id": rig.collection,
        },
    });
    let (status, answer) = block_on(rig.command("/api/v1/files/copies", &body));
    assert_eq!(status, 200, "{answer}");
    let copy = answer["result"]["object_id"].as_str().expect("the copy");
    assert_ne!(copy, source, "a new file");
    let (original, copied) = (original_of(&rig, &source), original_of(&rig, copy));
    assert_eq!(copied["digest"], original["digest"], "the same SHA-256");
    assert_eq!(copied["size"], FILE_BYTES);
    assert_eq!(
        film_sized(&rig.dir.join("root")),
        2,
        "the source and its copy"
    );
    assert_eq!(film_sized(&rig.dir.join("data")), 0, "nothing left staged");
}
