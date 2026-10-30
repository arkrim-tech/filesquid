//! Tasks DE-1.2 and DE-1.7: the desktop's import route (API-S-036, DE-S-019) through the
//! `filesquid://` scheme with the per-launch token: the owner picks a folder in the host's
//! dialog (scripted here), one durable import job runs on the native worker and commits every
//! file as its own object with its record, registered in the destination collection and folder
//! the request named, so the navigation and listing routes show it there; a request without a
//! destination is refused before any dialog; the same operation ID answers the same job; a
//! cancelled dialog makes no job; a library that is not open is refused; a job the worker could
//! not take runs at the next launch; and no answer or journal entry names a path.

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

#[path = "../support/import_rig.rs"]
#[allow(dead_code)]
mod import_rig;

use filesquid_api::imports::{DESTINATION_REQUIRED, IMPORT_PLAN_OPERATION};
use filesquid_api::navigation::{FolderScope, NodeRef};
use filesquid_core::CollectionId;
use import_rig::{fresh_dir, holiday_folder, ImportRig, LIBRARY, UNOPENED_LIBRARY};
use journal::block_on;
use serde_json::json;

#[test]
fn a_picked_folder_is_imported_as_one_job_and_a_repeat_answers_the_same_job() {
    let dir = fresh_dir("folder");
    let folder = holiday_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let (status, answer) = block_on(rig.import(LIBRARY, "op-import-1", "folder"));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["import"], "started");
    assert_eq!(
        answer["outcome"]["kind"], "pending",
        "the job's work is durable, not done"
    );
    let job = answer["job_id"].as_str().expect("job").to_string();
    let (status, again) = block_on(rig.import(LIBRARY, "op-import-1", "folder"));
    assert_eq!(status, 200, "{again}");
    assert_eq!(
        again["job_id"],
        job.as_str(),
        "the same operation answers the same job"
    );
    assert_eq!(rig.dialog.shown(), 1, "no second dialog");
    assert_eq!(rig.wait_for_files(2), 2, "each file imported");
    rig.stop_worker();
    assert_eq!(rig.imported().len(), 2, "each file imported once");
    let text = format!("{:?}", rig.journal());
    assert!(
        !text.contains("Beach day") && !text.contains("Holidays"),
        "no names"
    );
    assert_eq!(rig.file_objects(), 2, "one object per file");
}

#[test]
fn a_cancelled_dialog_or_an_unopened_library_makes_no_job() {
    let rig = block_on(ImportRig::launch(fresh_dir("cancel"), None));
    let (status, answer) = block_on(rig.import(LIBRARY, "op-cancel", "files"));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer,
        json!({
            "outcome": { "operation_id": "op-cancel", "kind": "committed", "error": null },
            "import": "cancelled"
        })
    );
    let (status, _) = block_on(rig.import(UNOPENED_LIBRARY, "op-other", "files"));
    assert_eq!(status, 404, "that library is not open");
    assert_eq!(
        rig.dialog.shown(),
        1,
        "no dialog for the library that is not open"
    );
    rig.stop_worker();
    let plans = rig
        .journal()
        .into_iter()
        .filter(|e| e.operation == IMPORT_PLAN_OPERATION)
        .count();
    assert_eq!(plans, 2, "each request journaled once (import.plan)");
    assert_eq!(rig.file_objects(), 0, "nothing imported");
}

#[test]
fn a_job_the_stopped_worker_could_not_take_runs_at_the_next_launch() {
    let dir = fresh_dir("relaunch");
    let folder = holiday_folder(&dir);
    let mut rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    rig.stop_worker();
    let (status, answer) = block_on(rig.import(LIBRARY, "op-later", "folder"));
    assert_eq!(status, 503, "{answer}");
    assert_eq!(rig.file_objects(), 0, "nothing imported yet");
    block_on(rig.relaunch());
    assert_eq!(rig.wait_for_files(2), 2, "the kept job ran at the launch");
    let (status, again) = block_on(rig.import(LIBRARY, "op-later", "folder"));
    assert_eq!(status, 200, "{again}");
    assert_eq!(again["import"], "started", "the operation names its job");
    assert_eq!(rig.dialog.shown(), 1, "no second dialog");
    rig.stop_worker();
    assert_eq!(rig.file_objects(), 2);
}

#[test]
fn an_import_without_a_destination_is_refused_before_any_dialog() {
    let dir = fresh_dir("no-destination");
    let folder = holiday_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let only_folder = json!({ "folder": "trips" });
    let (status, answer) = block_on(rig.import_into(LIBRARY, "op-nowhere", "folder", only_folder));
    assert_eq!(status, 400, "{answer}");
    assert!(
        answer["message"]
            .as_str()
            .is_some_and(|m| m.contains(DESTINATION_REQUIRED)),
        "{answer}"
    );
    assert_eq!(rig.dialog.shown(), 0, "no dialog");
    rig.stop_worker();
    let refusals: Vec<_> = rig
        .journal()
        .into_iter()
        .filter(|e| e.operation == IMPORT_PLAN_OPERATION)
        .collect();
    assert_eq!(refusals.len(), 1, "journaled once");
    assert_eq!(
        refusals[0].error_code.as_deref(),
        Some(DESTINATION_REQUIRED)
    );
    assert_eq!(
        rig.file_objects(),
        0,
        "nothing imported to the library's top level"
    );
}

/// The handle of `node`.
fn handle(node: &NodeRef) -> String {
    node.to_handle().expect("handle")
}

/// The names of the nodes or entries of `page` under `key`, shown names first.
fn names(page: &serde_json::Value, key: &str) -> Vec<String> {
    page[key]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| {
            let name = item.get("display_name").unwrap_or(&item["name"]);
            name.as_str().expect("name").to_string()
        })
        .collect()
}

#[test]
fn imported_files_appear_under_their_collection_and_folder() {
    let dir = fresh_dir("navigation");
    let folder = holiday_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let (status, answer) = block_on(rig.import(LIBRARY, "op-trip", "folder"));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(rig.wait_for_files(2), 2);
    rig.stop_worker();
    let collection = CollectionId::new(rig.collection.clone()).expect("collection");
    let scope = FolderScope::Collection(collection.clone());
    let node = handle(&NodeRef::Collection(collection));
    let (status, page) = block_on(rig.get(&format!("/api/v1/navigation?node={node}")));
    assert_eq!(status, 200, "{page}");
    assert_eq!(names(&page, "nodes"), ["trips"], "the destination folder");
    let trips = handle(&NodeRef::folder(scope.clone(), "trips"));
    let (status, page) = block_on(rig.get(&format!("/api/v1/navigation?node={trips}")));
    assert_eq!(status, 200, "{page}");
    assert_eq!(
        names(&page, "nodes"),
        ["Holidays"],
        "the picked folder kept"
    );
    let picked = handle(&NodeRef::folder(scope, "trips/Holidays"));
    let (status, page) = block_on(rig.get(&format!("/api/v1/files/listing?node={picked}")));
    assert_eq!(status, 200, "{page}");
    assert_eq!(names(&page, "entries"), ["Beach day.jpg", "Notes.txt"]);
    let objects: Vec<&str> = page["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| e["object_id"].as_str().expect("its own object"))
        .collect();
    assert_eq!(objects.len(), 2);
    assert!(objects.iter().all(|o| o.starts_with("urn:uuid:")));
    let registrations = rig
        .journal()
        .into_iter()
        .filter(|e| e.operation == "library.files.register")
        .collect::<Vec<_>>();
    assert_eq!(
        registrations.len(),
        2,
        "each registration journaled with its version"
    );
    assert!(registrations
        .iter()
        .all(|e| e.operation_id.as_deref() == Some("op-trip")));
    let text = format!("{:?}", rig.journal());
    assert!(
        !text.contains("Beach day") && !text.contains("Holidays") && !text.contains("trips/"),
        "no path, file or folder name in any journal entry"
    );
}

#[test]
fn an_operation_id_reused_for_another_import_is_refused_not_answered_with_its_job() {
    let dir = fresh_dir("reused");
    let folder = holiday_folder(&dir);
    let rig = block_on(ImportRig::launch(dir, Some(vec![folder])));
    let (status, answer) = block_on(rig.import(LIBRARY, "op-reused", "folder"));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(rig.wait_for_files(2), 2);
    let elsewhere = json!({ "collection_id": rig.collection, "folder": "Elsewhere" });
    let (status, refused) = block_on(rig.import_into(LIBRARY, "op-reused", "folder", elsewhere));
    assert_eq!(status, 409, "{refused}");
    let (status, files) = block_on(rig.import(LIBRARY, "op-reused", "files"));
    assert_eq!(status, 409, "another kind of source: {files}");
    assert_eq!(rig.dialog.shown(), 1, "no second dialog");
    rig.stop_worker();
    let reused: Vec<_> = rig
        .journal()
        .into_iter()
        .filter(|e| e.error_code.as_deref() == Some("operation_reused"))
        .collect();
    assert_eq!(reused.len(), 2, "{reused:?}");
}
