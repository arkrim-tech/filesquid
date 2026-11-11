//! Step definitions for the streamed scenarios of
//! `copy_provenance_and_streamed_copies_API-S-011.feature` (Task API-F2.26; API-S-011,
//! NFR-003, OCFL-S-026): a copy of 1, 65 and 600 MiB through the router over a real OCFL
//! storage root and a staging area on the local filesystem, and a source that changes while
//! it is read. The rig's library has one collection, so the scenarios' "Work" and "Archive"
//! are both that collection. The provenance scenarios of the feature are Task API-F2.23's.

use crate::copy_fixture::{
    content_file, copy, import_source, object_count, staged_files, stored, Fill, Stored, MAX_READ,
    MAX_WRITE, MIB,
};
use crate::files_fixture::{Answer, FilesRig};
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::journal::Outcome;
use std::sync::atomic::{AtomicBool, Ordering};

/// The largest source held as a real pattern; a larger one is a sparse file of zero bytes.
const MAX_PATTERN: u64 = 128 * MIB;

/// Per-scenario state of the streamed copy steps.
#[derive(Default)]
pub struct StreamedCopyWorld {
    rig: Option<FilesRig>,
    source: Option<Stored>,
    objects: usize,
    staged_before: u64,
    answer: Option<Answer>,
}

impl std::fmt::Debug for StreamedCopyWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamedCopyWorld").finish_non_exhaustive()
    }
}

impl StreamedCopyWorld {
    /// The rig.
    fn rig(&self) -> &FilesRig {
        self.rig.as_ref().expect("a library")
    }

    /// The source file.
    fn source(&self) -> &Stored {
        self.source.as_ref().expect("a source file")
    }

    /// The copy's answer.
    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("a copy was asked for")
    }

    /// A new library holding one source file of `size` bytes.
    async fn start(&mut self, size: u64) {
        let rig = FilesRig::new().await;
        let fill = match size > MAX_PATTERN {
            true => Fill::Zeros,
            false => Fill::Pattern,
        };
        let source = import_source(&rig, "film.mov", size, fill).await;
        self.objects = object_count(&rig);
        self.staged_before = rig.api_staging.written();
        self.source = Some(source);
        self.rig = Some(rig);
    }
}

#[given(
    regex = r#"^copy-history file of (\d+) MiB in collection "Work" of a library whose staging area is on disk$"#
)]
async fn a_file_of(w: &mut MonitorWorld, mebibytes: u64) {
    w.streamed_copy.start(mebibytes * MIB).await;
}

#[given(regex = r"^copy-history file of (\d+) MiB that changes while it is read$")]
async fn a_file_that_changes(w: &mut MonitorWorld, mebibytes: u64) {
    let size = mebibytes * MIB;
    w.streamed_copy.start(size).await;
    let rig = w.streamed_copy.rig();
    let content = content_file(rig, size);
    let (root, before) = (rig.root.clone(), rig.root.chunk_reads());
    let changed = AtomicBool::new(false);
    rig.root.on_read(Box::new(move || {
        if root.chunk_reads() > before && !changed.swap(true, Ordering::SeqCst) {
            std::fs::remove_file(&content).expect("remove the stored content");
            std::fs::write(&content, vec![7u8; size as usize]).expect("other content");
        }
    }));
}

#[when(regex = r#"^copy-history caller copies it into collection "Archive"$"#)]
async fn copies_it(w: &mut MonitorWorld) {
    let world = &mut w.streamed_copy;
    let answer = copy(world.rig(), &world.source().file_id).await;
    world.answer = Some(answer);
}

#[then("copy-history copy should have the source's SHA-256 and size")]
async fn same_content(w: &mut MonitorWorld) {
    let world = &w.streamed_copy;
    let answer = world.answer();
    assert_eq!(answer.status.as_u16(), 200, "{:?}", answer.json());
    let copy_id = answer.result()["file_id"].as_str().expect("id").to_string();
    assert_ne!(copy_id, world.source().file_id, "a new file");
    let copied = stored(world.rig(), &copy_id).await;
    assert_eq!(copied.sha256, world.source().sha256);
    assert_eq!(copied.size, world.source().size);
}

#[then(
    "copy-history copy should never hold the whole file in memory, its staged bytes going to the staging area on disk"
)]
async fn streamed(w: &mut MonitorWorld) {
    let world = &w.streamed_copy;
    let rig = world.rig();
    let read = rig.root.largest_read();
    assert!(read <= MAX_READ, "a read of {read} bytes");
    for backend in [&rig.root, &rig.api_staging] {
        let write = backend.largest_write();
        assert!(write <= MAX_WRITE, "a write of {write} bytes");
    }
    let staged = rig.api_staging.written() - world.staged_before;
    assert!(staged >= world.source().size, "{staged} bytes were staged");
    assert_eq!(staged_files(rig), Vec::<std::path::PathBuf>::new());
}

#[then("copy-history copy should fail with nothing committed and nothing left staged")]
async fn failed_cleanly(w: &mut MonitorWorld) {
    let world = &w.streamed_copy;
    let (rig, answer) = (world.rig(), world.answer());
    assert_eq!(answer.status.as_u16(), 409, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], "conflict");
    assert_eq!(object_count(rig), world.objects, "no object was committed");
    assert_eq!(staged_files(rig), Vec::<std::path::PathBuf>::new());
    let journaled = rig.journaled("file.copy");
    assert_eq!(journaled.len(), 1, "journaled once");
    assert_eq!(journaled[0].outcome, Outcome::Failed);
}
