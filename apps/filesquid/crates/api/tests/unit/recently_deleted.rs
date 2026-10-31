//! Task API-F2.15 / API-S-019: one Recently deleted, restore and retention (ADR 041
//! Decisions 8 to 10). Every test drives `LibraryService` against the in-memory
//! `MemoryCatalog`, the reference implementation of the injected `LibraryCatalog` port.

#[path = "../support/library_fixture.rs"]
mod library_fixture;

use filesquid_api::journal::Outcome;
use filesquid_api::library::{
    DeletionCause, ExpiryMode, FileEntry, FileScope, InstallationSettings, LibraryCatalog,
    LibraryCommand, LibraryError, MemorySettings, RetentionSettings, VisibilityFilter,
    MAX_RETENTION_DAYS, MAX_UNTITLED_TITLES, RECOVERABLE_DAYS, UNTITLED_COLLECTION_TITLE,
};
use filesquid_core::{Collection, CollectionId, FileId};
use library_fixture::{Fixture, DAY_MS, NOW_MS};
use std::sync::Arc;

/// The catalog entry of `id`.
fn entry(fx: &Fixture, id: &FileId) -> FileEntry {
    fx.catalog.file(id).expect("read").expect("file")
}

/// Delete `files` one by one in the fixture's library.
fn delete(fx: &Fixture, files: &[&FileId]) {
    fx.run(LibraryCommand::DeleteFiles {
        library_id: fx.library(),
        file_ids: files.iter().map(|f| (*f).clone()).collect(),
    });
}

/// Delete the collection `id` in the fixture's library.
fn delete_collection(fx: &Fixture, id: &CollectionId) {
    fx.run(LibraryCommand::DeleteCollection {
        library_id: fx.library(),
        collection_id: id.clone(),
    });
}

/// Restore `files` in the fixture's library.
fn restore(fx: &Fixture, files: &[&FileId]) -> Result<filesquid_api::library::CommandResult, LibraryError> {
    fx.try_run(LibraryCommand::RestoreFiles {
        library_id: fx.library(),
        file_ids: files.iter().map(|f| (*f).clone()).collect(),
    })
}

#[test]
fn a_file_deleted_on_its_own_is_deleted_one_by_one_under_the_default_retention() {
    let fx = Fixture::with_library();
    let docs = fx.create_collection("Documents");
    let file = fx.add_file_in("f1", &docs);
    delete(&fx, &[&file]);
    let deletion = entry(&fx, &file).state.deletion.expect("deleted");
    assert_eq!(deletion.cause, DeletionCause::OneByOne);
    assert_eq!(deletion.deleted_at_ms, NOW_MS);
    assert_eq!(
        deletion.purge_after_ms,
        NOW_MS + i64::from(RECOVERABLE_DAYS) * DAY_MS
    );
    let listed = fx.list_files(FileScope::Library, VisibilityFilter::Deleted);
    assert_eq!(listed, vec![file]);
}

#[test]
fn deleting_a_collection_deletes_its_files_with_it_and_keeps_its_title() {
    let fx = Fixture::with_library();
    let visits = fx.create_collection("Site visits");
    let earlier = fx.add_file_in("f0", &visits);
    let first = fx.add_file_in("f1", &visits);
    let second = fx.add_file_in("f2", &visits);
    let before = fx.run_at(NOW_MS - DAY_MS, LibraryCommand::DeleteFiles {
        library_id: fx.library(),
        file_ids: vec![earlier.clone()],
    });
    let earlier_times = before.deletion.expect("deleted");
    let commits = fx.catalog.commit_count();
    let result = fx.run(LibraryCommand::DeleteCollection {
        library_id: fx.library(),
        collection_id: visits.clone(),
    });
    assert_eq!(fx.catalog.commit_count(), commits + 1, "one version");
    let answered = result.deletion.expect("deletion answered");
    assert_eq!(answered.cause, DeletionCause::WithCollection);
    for file in [&first, &second] {
        let state = entry(&fx, file);
        let deletion = state.state.deletion.expect("deleted");
        assert_eq!(deletion.cause, DeletionCause::WithCollection);
        assert_eq!(deletion.deleted_at_ms, NOW_MS);
        assert_eq!(state.file.home_collection_id(), &visits, "keeps its home");
        assert_eq!(state.removed_home_title.as_deref(), Some("Site visits"));
    }
    let kept = entry(&fx, &earlier);
    assert_eq!(kept.state.deletion, Some(earlier_times), "keeps its earlier times");
    assert_eq!(kept.removed_home_title.as_deref(), Some("Site visits"));
    assert!(fx.list_files(FileScope::Library, VisibilityFilter::Ordinary).is_empty());
    assert_eq!(
        fx.list_files(FileScope::Library, VisibilityFilter::Deleted).len(),
        3
    );
}

#[test]
fn a_library_retention_sets_purge_after_and_never_moves_a_deleted_file() {
    let fx = Fixture::with_library();
    let docs = fx.create_collection("Documents");
    let old = fx.add_file_in("f1", &docs);
    let new = fx.add_file_in("f2", &docs);
    delete(&fx, &[&old]);
    let first = entry(&fx, &old).state.deletion.expect("deleted");
    fx.run(LibraryCommand::SetLibraryRetention {
        library_id: fx.library(),
        retention_days: Some(7),
    });
    assert_eq!(entry(&fx, &old).state.deletion, Some(first), "unchanged");
    delete(&fx, &[&new]);
    let later = entry(&fx, &new).state.deletion.expect("deleted");
    assert_eq!(later.purge_after_ms, NOW_MS + 7 * DAY_MS);
    let journal = fx.catalog.journal_entries();
    let set = journal
        .iter()
        .find(|e| e.operation == "library.retention.set")
        .expect("journaled");
    assert_eq!(set.target_ids, vec![fx.library().as_str().to_string()]);
}

#[test]
fn a_retention_outside_one_to_3650_days_is_refused() {
    let fx = Fixture::with_library();
    for days in [0, MAX_RETENTION_DAYS + 1] {
        let err = fx
            .try_run(LibraryCommand::SetLibraryRetention {
                library_id: fx.library(),
                retention_days: Some(days),
            })
            .expect_err("refused");
        assert_eq!(err.error_code(), "invalid_retention");
    }
    let ok = fx.try_run(LibraryCommand::SetLibraryRetention {
        library_id: fx.library(),
        retention_days: Some(MAX_RETENTION_DAYS),
    });
    assert!(ok.is_ok());
}

#[test]
fn the_installation_default_applies_to_a_library_without_its_own_period() {
    let fx = Fixture::with_library();
    let settings = Arc::new(MemorySettings::new());
    settings
        .store(
            &InstallationSettings {
                default_retention_days: 10,
                expiry: ExpiryMode::KeepVersionHistory,
            },
            filesquid_api::journal::JournalEntry::new("x", Outcome::Succeeded),
        )
        .expect("store");
    let service = fx.service.clone().with_settings(settings);
    let docs = fx.create_collection("Documents");
    let file = fx.add_file_in("f1", &docs);
    let request = fx.request(
        &fx.revision(),
        LibraryCommand::DeleteFiles {
            library_id: fx.library(),
            file_ids: vec![file.clone()],
        },
    );
    service.execute("user:ana", request).expect("delete");
    let deletion = entry(&fx, &file).state.deletion.expect("deleted");
    assert_eq!(deletion.purge_after_ms, NOW_MS + 10 * DAY_MS);
}

#[test]
fn installation_settings_default_to_30_days_and_keep_version_history() {
    let settings = MemorySettings::new();
    let read = settings.read().expect("read");
    assert_eq!(read.default_retention_days, RECOVERABLE_DAYS);
    assert_eq!(read.expiry, ExpiryMode::KeepVersionHistory);
}

#[test]
fn a_restore_into_a_home_collection_that_exists_returns_the_file_in_place() {
    let fx = Fixture::with_library();
    let docs = fx.create_collection("Documents");
    let file = fx.add_file_in("f1", &docs);
    fx.run(LibraryCommand::HideFiles {
        library_id: fx.library(),
        file_ids: vec![file.clone()],
    });
    delete(&fx, &[&file]);
    let result = restore(&fx, &[&file]).expect("restore");
    assert!(result.restored_into.is_empty());
    let state = entry(&fx, &file);
    assert!(state.state.deletion.is_none());
    assert!(state.state.hidden, "a hidden file stays hidden");
    assert_eq!(state.file.home_collection_id(), &docs);
}

#[test]
fn a_restore_whose_home_is_gone_creates_an_untitled_collection_once_per_home() {
    let fx = Fixture::with_library();
    let visits = fx.create_collection("Site visits");
    let a = fx.add_file_in("f1", &visits);
    let b = fx.add_file_in("f2", &visits);
    let c = fx.add_file_in("f3", &visits);
    delete_collection(&fx, &visits);
    let commits = fx.catalog.commit_count();
    let first = restore(&fx, &[&a, &b]).expect("restore");
    assert_eq!(fx.catalog.commit_count(), commits + 1, "one version");
    assert_eq!(first.restored_into.len(), 1, "one collection per gone home");
    let made = &first.restored_into[0];
    assert_eq!(made.title, UNTITLED_COLLECTION_TITLE);
    assert_eq!(made.from_collection_id, visits);
    for file in [&a, &b] {
        let state = entry(&fx, file);
        assert!(state.state.deletion.is_none());
        assert_eq!(state.file.home_collection_id(), &made.collection_id);
        assert!(state.removed_home_title.is_none());
    }
    let second = restore(&fx, &[&c]).expect("restore");
    assert_eq!(second.restored_into[0].title, "untitled collection 2");
    let journal = fx.catalog.journal_entries();
    let restores: Vec<_> = journal
        .iter()
        .filter(|e| e.operation == "library.file.restore")
        .collect();
    assert_eq!(restores.len(), 2, "one journaled operation per restore");
    assert!(restores[0]
        .target_ids
        .contains(&made.collection_id.as_str().to_string()));
}

#[test]
fn the_first_free_title_is_used() {
    let fx = Fixture::with_library();
    fx.create_collection("untitled collection");
    fx.create_collection("untitled collection 3");
    let visits = fx.create_collection("Site visits");
    let file = fx.add_file_in("f1", &visits);
    delete_collection(&fx, &visits);
    let result = restore(&fx, &[&file]).expect("restore");
    assert_eq!(result.restored_into[0].title, "untitled collection 2");
}

#[test]
fn a_restore_is_refused_when_every_untitled_title_is_taken() {
    let fx = Fixture::with_library();
    let visits = fx.create_collection("Site visits");
    let file = fx.add_file_in("f1", &visits);
    delete_collection(&fx, &visits);
    for n in 1..=MAX_UNTITLED_TITLES {
        let title = match n {
            1 => UNTITLED_COLLECTION_TITLE.to_string(),
            n => format!("{UNTITLED_COLLECTION_TITLE} {n}"),
        };
        let id = CollectionId::new(format!("taken-{n}")).expect("id");
        let collection = Collection::new(id, fx.library(), title).expect("collection");
        fx.catalog.insert_collection(collection).expect("insert");
    }
    let commits = fx.catalog.commit_count();
    let err = restore(&fx, &[&file]).expect_err("refused");
    assert_eq!(err.error_code(), "restore_name_exhausted");
    assert_eq!(err.api_code(), filesquid_api::ErrorCode::Conflict);
    assert_eq!(fx.catalog.commit_count(), commits, "no change");
    assert!(entry(&fx, &file).state.deletion.is_some());
    let failed = fx.failure_journal();
    let refusal = failed
        .iter()
        .find(|e| e.operation == "library.file.restore")
        .expect("journaled");
    assert_eq!(refusal.outcome, Outcome::Failed);
    assert_eq!(refusal.error_code.as_deref(), Some("restore_name_exhausted"));
}

// ---- Expiry (ADR 041 Decision 11) ----

use filesquid_api::library::{ExpiryReport, EXPIRY_OBJECT_OPERATION, EXPIRY_OPERATION};

/// A service of `fx` at `now_ms` reading `settings`.
fn service_at(fx: &Fixture, now_ms: i64, settings: Arc<MemorySettings>) -> filesquid_api::library::LibraryService {
    fx.service
        .clone()
        .with_clock(Arc::new(move || now_ms))
        .with_settings(settings)
}

/// Settings with `expiry`.
fn settings_with(expiry: ExpiryMode) -> Arc<MemorySettings> {
    let settings = Arc::new(MemorySettings::new());
    let stored = InstallationSettings {
        default_retention_days: RECOVERABLE_DAYS,
        expiry,
    };
    settings
        .store(&stored, filesquid_api::journal::JournalEntry::new("x", Outcome::Succeeded))
        .expect("store");
    settings
}

/// Expire the fixture's library 31 days from now under `expiry`.
fn expire(fx: &Fixture, expiry: ExpiryMode) -> ExpiryReport {
    let later = service_at(fx, NOW_MS + 31 * DAY_MS, settings_with(expiry));
    later
        .expire_library("system", &fx.library(), fx.catalog.as_ref())
        .expect("expire")
}

#[test]
fn expiry_removes_due_files_from_every_view_and_records_their_object_work() {
    let fx = Fixture::with_library();
    let docs = fx.create_collection("Documents");
    let due = fx.add_file_in("f1", &docs);
    let kept = fx.add_file_in("f2", &docs);
    delete(&fx, &[&due]);
    fx.run_at(NOW_MS + 20 * DAY_MS, LibraryCommand::DeleteFiles {
        library_id: fx.library(),
        file_ids: vec![kept.clone()],
    });
    let report = expire(&fx, ExpiryMode::KeepVersionHistory);
    assert_eq!(report.expired, 1);
    assert_eq!(report.objects_done, 1);
    assert!(fx.catalog.file(&due).expect("read").is_none(), "never listed again");
    assert!(fx.catalog.file(&kept).expect("read").is_some(), "not due yet");
    let done = fx.catalog.purged_objects();
    assert_eq!(done, vec![(due.clone(), ExpiryMode::KeepVersionHistory)]);
    let journal = fx.catalog.journal_entries();
    let purge = journal
        .iter()
        .find(|e| e.operation == EXPIRY_OPERATION)
        .expect("journaled with the deletion");
    assert_eq!(purge.outcome, Outcome::Succeeded);
    assert_eq!(purge.actor, "system");
    assert_eq!(
        purge.target_ids,
        vec![fx.library().as_str().to_string(), due.as_str().to_string()],
        "opaque IDs only"
    );
    assert_eq!(report.operation_id.as_deref(), purge.operation_id.as_deref());
}

#[test]
fn delete_all_version_history_asks_for_the_object_s_deletion() {
    let fx = Fixture::with_library();
    let docs = fx.create_collection("Documents");
    let due = fx.add_file_in("f1", &docs);
    delete(&fx, &[&due]);
    expire(&fx, ExpiryMode::DeleteAllVersionHistory);
    assert_eq!(
        fx.catalog.purged_objects(),
        vec![(due, ExpiryMode::DeleteAllVersionHistory)]
    );
}

#[test]
fn a_failed_object_purge_is_journaled_as_failed_and_retried_at_the_next_run() {
    let fx = Fixture::with_library();
    let docs = fx.create_collection("Documents");
    let due = fx.add_file_in("f1", &docs);
    delete(&fx, &[&due]);
    fx.catalog.fail_next_purges(1);
    let first = expire(&fx, ExpiryMode::KeepVersionHistory);
    assert_eq!((first.expired, first.objects_failed), (1, 1));
    assert!(fx.catalog.file(&due).expect("read").is_none());
    let failed = fx.failure_journal();
    let entry = failed
        .iter()
        .find(|e| e.operation == EXPIRY_OBJECT_OPERATION)
        .expect("journaled");
    assert_eq!(entry.outcome, Outcome::Failed);
    assert_eq!(entry.target_ids, vec![fx.library().as_str().to_string(), due.as_str().to_string()]);
    let second = expire(&fx, ExpiryMode::KeepVersionHistory);
    assert_eq!((second.expired, second.objects_done), (0, 1), "the pending work is retried");
    assert_eq!(fx.catalog.purged_objects().len(), 1);
}

#[test]
fn a_run_with_nothing_due_changes_and_journals_nothing() {
    let fx = Fixture::with_library();
    let docs = fx.create_collection("Documents");
    fx.add_file_in("f1", &docs);
    let commits = fx.catalog.commit_count();
    let report = expire(&fx, ExpiryMode::KeepVersionHistory);
    assert_eq!(report.expired, 0);
    assert!(report.operation_id.is_none());
    assert_eq!(fx.catalog.commit_count(), commits);
}
