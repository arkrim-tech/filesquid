//! Unit tests for the per-file progress of import jobs (Task DE-1.8; API-S-037 **Per-file
//! progress**): the items of a job's manifest kept as queued in order, each file's state with
//! its bytes and error code, the destination from the job's plan, paging by cursor, and a
//! retry of one failed file or of every failed file that puts back only failed files.

use filesquid_api::jobs::{JobDestination, JobError, JobItemState, JobItems};
use filesquid_metadata::import::ImportMethod;
use filesquid_native::import::items::{NewItem, SqliteJobItems};
use filesquid_native::import::plans::{ImportPlan, SqliteImportPlans};
use std::path::PathBuf;

/// A store in a fresh scratch file, removed when dropped.
struct Store(PathBuf);

impl Store {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "fs-native-import-items-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        Self(dir)
    }

    fn file(&self) -> PathBuf {
        self.0.join("filesquid.sqlite")
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.0) {
            eprintln!("scratch directory not removed: {err}");
        }
    }
}

/// The items of a job `job-1` with three files, its plan kept first.
fn three_files(store: &Store) -> SqliteJobItems {
    let plans = SqliteImportPlans::open(&store.file()).expect("plans");
    let plan = ImportPlan {
        library_id: "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b".into(),
        collection_id: Some("c-1".into()),
        folder: Some("trips".into()),
        sources: vec![PathBuf::from("/picked/Trip")],
        method: ImportMethod::Folder,
    };
    plans.save("job-1", "op-1", &plan).expect("plan");
    let items = SqliteJobItems::open(&store.file()).expect("items");
    let new = |index: usize, id: &str, name: &str| NewItem {
        index,
        file_id: id.into(),
        name: name.into(),
        folder: Some("trips/Trip".into()),
        bytes_total: 10 * (index as u64 + 1),
    };
    let kept = [
        new(0, "f-a", "a.jpg"),
        new(1, "f-b", "b.jpg"),
        new(3, "f-c", "c.jpg"),
    ];
    items.keep("job-1", &kept).expect("kept");
    items
}

#[test]
fn a_manifest_is_kept_as_queued_items_in_order_with_its_destination() {
    let store = Store::new("keep");
    let items = three_files(&store);
    let page = items.page("job-1", None, 1_000).expect("page");
    let ids: Vec<&str> = page.items.iter().map(|i| i.item_id.as_str()).collect();
    assert_eq!(ids, ["f-a", "f-b", "f-c"]);
    assert!(page.items.iter().all(|i| i.state == JobItemState::Queued));
    assert_eq!(page.items[2].bytes_total, 40);
    assert_eq!(page.items[0].folder.as_deref(), Some("trips/Trip"));
    assert_eq!(page.next_cursor, None);
    let destination = items.destination("job-1").expect("destination");
    assert_eq!(
        destination,
        Some(JobDestination {
            collection_id: "c-1".into(),
            folder: Some("trips".into()),
        })
    );
    assert_eq!(items.destination("job-2").expect("read"), None);
    assert_eq!(items.waiting("job-1").expect("waiting"), [0, 1, 3]);
}

#[test]
fn keeping_a_manifest_again_changes_no_item() {
    let store = Store::new("again");
    let items = three_files(&store);
    items
        .set("job-1", 0, JobItemState::Copied, 10, None)
        .expect("copied");
    let again = NewItem {
        index: 0,
        file_id: "f-a".into(),
        name: "a.jpg".into(),
        folder: None,
        bytes_total: 10,
    };
    items.keep("job-1", &[again]).expect("kept");
    let page = items.page("job-1", None, 1).expect("page");
    assert_eq!(page.items[0].state, JobItemState::Copied);
    assert!(page.next_cursor.is_some(), "a cursor to the next page");
}

#[test]
fn a_retry_puts_back_only_failed_files_and_a_repeat_changes_nothing() {
    let store = Store::new("retry");
    let items = three_files(&store);
    items
        .set("job-1", 0, JobItemState::Copied, 10, None)
        .expect("copied");
    items
        .set("job-1", 1, JobItemState::Failed, 0, Some("read_failed"))
        .expect("failed");
    items
        .set("job-1", 3, JobItemState::Failed, 3, Some("source_changed"))
        .expect("failed");
    assert!(items.has_failed("job-1").expect("read"));
    assert_eq!(items.retry("job-1", Some("f-b")).expect("one"), ["f-b"]);
    assert_eq!(
        items.retry("job-1", Some("f-b")).expect("again"),
        Vec::<String>::new(),
        "a file that is not failed is left as it is"
    );
    assert_eq!(
        items.retry("job-1", Some("f-a")).expect("copied"),
        Vec::<String>::new()
    );
    assert_eq!(items.retry("job-1", Some("f-x")), Err(JobError::NotFound));
    assert_eq!(items.waiting("job-1").expect("waiting"), [1]);
    assert_eq!(items.retry("job-1", None).expect("all"), ["f-c"]);
    assert!(!items.has_failed("job-1").expect("read"));
    let page = items.page("job-1", None, 1_000).expect("page");
    let retried = &page.items[2];
    assert_eq!(
        (
            retried.state,
            retried.bytes_done,
            retried.error_code.as_deref()
        ),
        (JobItemState::Queued, 0, None)
    );
}
