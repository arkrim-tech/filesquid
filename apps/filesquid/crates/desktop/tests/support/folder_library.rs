//! A library in a folder of its own, attached to a desktop catalog as the desktop's library
//! routes attach it (Task DE-1.7, ADR 041): every library lives in a folder the owner chose,
//! and the data directory keeps only its index. The folders of a data directory `dir` live in
//! the sibling `dir.folders/`, outside it, as the host requires.

use filesquid_core::{LibraryId, RepositoryId};
use filesquid_native::catalog::LibraryFolder;
use filesquid_native::{HostLocations, SqliteCatalog, MAX_LIBRARIES};
use std::path::{Path, PathBuf};

/// The installation every desktop test library belongs to.
pub const INSTALLATION: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// Where the library folders of the data directory `data_dir` live. Pure.
pub fn folders_of(data_dir: &Path) -> PathBuf {
    data_dir.with_extension("folders")
}

/// Create a library named `name` in a new folder `<data_dir>.folders/<library-id>/` and attach
/// it to `catalog`; answer its ID and its folder (its OCFL storage root). Filesystem and
/// database I/O.
pub fn attach_new_library(
    catalog: &SqliteCatalog,
    data_dir: &Path,
    name: &str,
) -> (String, PathBuf) {
    let id = filesquid_api::library::mint_uuid_v4().expect("uuid");
    let folder = folders_of(data_dir).join(&id);
    std::fs::create_dir_all(&folder).expect("library folder");
    attach_folder(catalog, data_dir, &id, &folder, name);
    (id, folder)
}

/// Attach `folder` as library `id` named `name` to `catalog`, registered under the data
/// directory `data_dir`. Filesystem and database I/O.
pub fn attach_folder(
    catalog: &SqliteCatalog,
    data_dir: &Path,
    id: &str,
    folder: &Path,
    name: &str,
) {
    let locations = HostLocations::new(data_dir);
    let location = locations.register(folder).expect("register the folder");
    let library = LibraryFolder {
        library_id: LibraryId::new(id).expect("library"),
        repository_id: RepositoryId::new(INSTALLATION).expect("repository"),
        name: name.to_string(),
        root: locations.root_of(&location).expect("root"),
        identity: locations.identity_of(&location).expect("identity"),
    };
    catalog.attach(&library).expect("attached");
}

/// Attach every library folder of the data directory `data_dir` to `catalog` again, as the
/// owner reopens them at the next launch; at most the catalog's [`MAX_LIBRARIES`], and a
/// fixture with more fails rather than leave libraries unopened. Filesystem and database I/O.
pub fn reattach_all(catalog: &SqliteCatalog, data_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(folders_of(data_dir)) else {
        return;
    };
    let entries: Vec<_> = entries.take(MAX_LIBRARIES + 1).collect();
    assert!(
        entries.len() <= MAX_LIBRARIES,
        "more library folders than a catalog serves"
    );
    for entry in entries {
        let entry = entry.expect("entry");
        let id = entry.file_name().to_string_lossy().to_string();
        attach_folder(catalog, data_dir, &id, &entry.path(), "Family archive");
    }
}

/// Remove the data directory `data_dir` and its library folders (filesystem I/O).
pub fn remove_all(data_dir: &Path) {
    let _ = std::fs::remove_dir_all(folders_of(data_dir));
    std::fs::remove_dir_all(data_dir).expect("cleanup");
}
