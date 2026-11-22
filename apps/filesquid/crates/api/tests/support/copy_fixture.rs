//! Fixture of the streamed copy tests (Task API-F2.26; API-S-011, NFR-003, OCFL-S-026): a
//! source file of any size in a [`FilesRig`]'s library, made the way a host imports one (a
//! sparse file of zero bytes, or a patterned one, staged through the streamed import, since
//! the upload route takes at most 64 MiB), the copy command, and what a copy left behind:
//! the stored original of a file and the files in the service's staging area on disk.
//!
//! The including test crate MUST declare `mod files_fixture` (`support/files_fixture.rs`) at
//! its root.

#![allow(dead_code)]

use crate::files_fixture::{
    encode, Answer, FilesRig, API_STAGING, API_STAGING_PARTS, HOME_COLLECTION, LIBRARY,
};
use filesquid_api::files::SourceImport;
use filesquid_metadata::fact::{Fact, Reason};
use filesquid_metadata::import::{
    Acquisition, ByteStorage, Host, ImportEntry, ImportMethod, NameForm, UploadFacts,
};
use filesquid_metadata::raw::Raw;
use filesquid_ocfl::storage::FsBackend;
use serde_json::{json, Value};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// One mebibyte.
pub const MIB: u64 = 1024 * 1024;

/// The largest ranged read the streamed stage makes: 8 MiB (`MAX_CHUNK_BYTES`).
pub const MAX_READ: u64 = filesquid_ocfl::storage::MAX_CHUNK_BYTES;

/// The largest single write the streamed stage makes: one multipart part of 64 MiB
/// (`MAX_PART_BYTES`).
pub const MAX_WRITE: u64 = filesquid_ocfl::storage::MAX_PART_BYTES;

/// The key of a source file in its own folder.
const SOURCE_KEY: &str = "source.bin";

/// Most 1 MiB blocks [`write_source`] writes (16 GiB), so its loop is bounded.
const MAX_BLOCKS: u64 = 16 * 1024;

/// Most directories [`files_under`] visits, so its walk is bounded.
const MAX_DIRECTORIES: usize = 10_000;

/// Sources made so far in this process, so each gets its own file ID and folder.
static SOURCES: AtomicU64 = AtomicU64::new(0);

/// What a source file holds.
#[derive(Clone, Copy, Debug)]
pub enum Fill {
    /// Zero bytes, written as a sparse file: nothing is held in memory or written out.
    Zeros,
    /// Byte `i` is `i mod 251`, written in 1 MiB blocks.
    Pattern,
}

/// The facts of an import of a file named `name` declaring `size` bytes as the new file
/// `file_id`: what an upload would record, outside any job. Pure.
fn source_import(file_id: &str, name: &str, size: u64) -> SourceImport {
    let not_read = || Fact::unavailable(Reason::NotSupportedBySource);
    let host = Host {
        os_name: not_read(),
        os_version: not_read(),
        os_build: not_read(),
        architecture: not_read(),
        host_name: not_read(),
        account: not_read(),
        filesquid_version: "0.1.0".to_string(),
        filesquid_build: "test".to_string(),
        distribution: "filesquid".to_string(),
    };
    let acquisition = Acquisition {
        recorded_at: "2026-10-18T00:00:00Z".to_string(),
        started_at: "2026-10-18T00:00:00Z".to_string(),
        finished_at: "2026-10-18T00:00:00Z".to_string(),
        method: ImportMethod::FilePicker,
        storage: ByteStorage::Copied,
        job_id: not_read(),
        operation_id: not_read(),
        object_id: format!("urn:uuid:{file_id}"),
        object_version: "v1".to_string(),
        logical_path: None,
        sha256: String::new(),
        size,
    };
    let facts = ImportEntry::for_upload(UploadFacts {
        acquisition,
        name: Raw::from_bytes(name.as_bytes()),
        name_normalization: NameForm::Nfc,
        reported_size: size,
        last_modified: None,
        media_type: Some("application/octet-stream".to_string()),
        host,
    });
    SourceImport {
        library_id: LIBRARY.to_string(),
        file_id: file_id.to_string(),
        method: ImportMethod::FilePicker,
        job_id: None,
        operation_id: None,
        started_at: "2026-10-18T00:00:00Z".to_string(),
        source: facts.source,
        volume: facts.volume,
        attributes: facts.attributes,
        host: facts.host,
    }
}

/// Write a file of `size` bytes of `fill` at `path`. Filesystem I/O.
fn write_source(path: &Path, size: u64, fill: Fill) {
    let mut file = std::fs::File::create(path).expect("source file");
    if matches!(fill, Fill::Zeros) {
        file.set_len(size).expect("sparse source");
        return;
    }
    let mut written = 0u64;
    for _ in 0..MAX_BLOCKS {
        if written >= size {
            break;
        }
        let take = (size - written).min(MIB);
        let block: Vec<u8> = (written..written + take).map(|i| (i % 251) as u8).collect();
        file.write_all(&block).expect("source block");
        written += take;
    }
    assert_eq!(written, size, "the whole source is written");
}

/// A file of `rig`'s library.
#[derive(Clone, Debug)]
pub struct Stored {
    /// The file's ID.
    pub file_id: String,
    /// The SHA-256 of its original, lowercase hexadecimal.
    pub sha256: String,
    /// The size of its original in bytes.
    pub size: u64,
}

/// Import a new file named `name` of `size` bytes of `fill` into `rig`'s library through the
/// streamed import, and answer it. The source file is removed afterwards. Storage I/O.
pub async fn import_source(rig: &FilesRig, name: &str, size: u64, fill: Fill) -> Stored {
    let n = SOURCES.fetch_add(1, Ordering::SeqCst);
    let file_id = format!("0b1c2d3e-4f5a-4b6c-8d7e-{n:012x}");
    let folder = rig.dir.join(format!("source-{n}"));
    std::fs::create_dir_all(&folder).expect("source folder");
    write_source(&folder.join(SOURCE_KEY), size, fill);
    let parts = rig.dir.join(format!("source-parts-{n}"));
    let source = FsBackend::new(&folder, &parts).expect("source backend");
    let import = source_import(&file_id, name, size);
    let imported = rig
        .service
        .import_original(import, &source, SOURCE_KEY, "owner")
        .await
        .expect("the source is imported")
        .view;
    for dir in [folder, parts] {
        let _ = std::fs::remove_dir_all(dir);
    }
    assert_eq!(imported.size, size, "the imported size");
    Stored {
        file_id,
        sha256: imported.sha256,
        size,
    }
}

/// The body of a copy of `file_id` into the rig's home collection.
pub fn copy_input(file_id: &str) -> Value {
    json!({ "library_id": LIBRARY, "file_id": file_id, "collection_id": HOME_COLLECTION })
}

/// `POST /api/v1/files/copies` of `file_id` into the rig's home collection, at its current
/// revision under a fresh operation ID.
pub async fn copy(rig: &FilesRig, file_id: &str) -> Answer {
    rig.command("/api/v1/files/copies", copy_input(file_id))
        .await
}

/// The stored original of `file_id`, read through the listing route: its content digest and
/// size.
pub async fn stored(rig: &FilesRig, file_id: &str) -> Stored {
    let uri = format!(
        "/api/v1/files/listing?library_id={LIBRARY}&object_id={}&prefix=original",
        encode(&format!("urn:uuid:{file_id}"))
    );
    let listing = rig.get(&uri).await.json();
    let entries = listing["entries"].as_array().expect("entries").clone();
    let original = entries
        .iter()
        .find(|entry| {
            let path = entry["path"].as_str().unwrap_or_default();
            !path.ends_with(".metadata.md")
        })
        .expect("the original");
    Stored {
        file_id: file_id.to_string(),
        sha256: original["digest"].as_str().expect("digest").to_string(),
        size: original["size"].as_u64().expect("size"),
    }
}

/// Every regular file under `dir`, by a bounded walk; none when `dir` does not exist.
pub fn files_under(dir: &Path) -> Vec<PathBuf> {
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

/// The files the service under test left in its staging area on disk, parts included. The
/// filesystem backend's own key locks (empty files under `locks`) are not staged bytes and
/// are left out.
pub fn staged_files(rig: &FilesRig) -> Vec<PathBuf> {
    let mut files = files_under(&rig.dir.join(API_STAGING));
    files.extend(files_under(&rig.dir.join(API_STAGING_PARTS)));
    files.retain(|path| !path.components().any(|part| part.as_os_str() == "locks"));
    files
}

/// The OCFL objects of the rig's storage root: the directories that hold an `inventory.json`.
pub fn object_count(rig: &FilesRig) -> usize {
    files_under(&rig.dir.join("root"))
        .iter()
        .filter(|path| {
            let inventory = path
                .file_name()
                .is_some_and(|name| name == "inventory.json");
            let version = path
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('v'));
            inventory && !version
        })
        .count()
}

/// The stored content file of `size` bytes in the rig's storage root: the original of the
/// one file of that size.
pub fn content_file(rig: &FilesRig, size: u64) -> PathBuf {
    files_under(&rig.dir.join("root"))
        .into_iter()
        .find(|path| {
            let content = path.components().any(|part| part.as_os_str() == "content");
            content && std::fs::metadata(path).is_ok_and(|meta| meta.len() == size)
        })
        .expect("the stored content file")
}
