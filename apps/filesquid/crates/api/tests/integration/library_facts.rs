//! Integration tests of the library facts (Task API-F2.18; API-S-038) through the router: a
//! library's name at create, open and rename; `GET /api/v1/libraries/{library}/facts` with the
//! free bytes of its volume or the reason they are unknown; and `GET /api/v1/libraries/recent`,
//! readable libraries first, then at most 20 newest first, never a path or location ID.

#[path = "../support/library_routes_fixture.rs"]
mod library_routes_fixture;

use axum::http::StatusCode;
use filesquid_api::auth::Principal;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::library::{LibraryCatalog, MemoryCatalog};
use filesquid_api::local_library::{
    FreeSpace, LibraryRegistry, CREATE_OPERATION, FACTS_OPERATION, RENAME_OPERATION,
    UNTITLED_LIBRARY,
};
use filesquid_api::navigation::{
    MemoryFolderIndex, NavigationAccess, NavigationService, NodeRef, RepositoryInfo,
};
use filesquid_core::{LibraryId, RepositoryId};
use library_routes_fixture::{Answer, LibraryRig, Mount};
use serde_json::{json, Value};
use std::sync::Arc;

/// Both mounts, desktop first.
const MOUNTS: [Mount; 2] = [Mount::Desktop, Mount::Server];

/// Assert `answer` is the typed error `code` with `status`.
fn assert_error(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{}", answer.text());
    assert_eq!(answer.json()["code"], code, "{}", answer.text());
}

/// Create a library named `name` on a new location `folder`; answer its ID and location.
async fn create_named(rig: &LibraryRig, folder: &str, name: &str) -> (String, String) {
    let location = rig.locations.register(folder);
    let body = json!({ "location_id": location, "name": name });
    let answer = rig.post("/api/v1/libraries", body).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    let library = answer.json()["library_id"]
        .as_str()
        .expect("id")
        .to_string();
    (library, location)
}

/// The facts of `library`.
async fn facts(rig: &LibraryRig, library: &str) -> Answer {
    rig.get(&format!("/api/v1/libraries/{library}/facts")).await
}

/// `POST /api/v1/libraries/renames` of `library` to `name`.
async fn rename(rig: &LibraryRig, library: &str, name: &str) -> Answer {
    let body = json!({ "library_id": library, "name": name });
    rig.post("/api/v1/libraries/renames", body).await
}

/// The entries of `operation` with `outcome`.
fn entries(rig: &LibraryRig, operation: &str, outcome: Outcome) -> Vec<JournalEntry> {
    rig.journaled(operation)
        .into_iter()
        .filter(|e| e.outcome == outcome)
        .collect()
}

/// Assert no answer or journal entry names the rig directory, a location ID or `names`.
fn assert_redacted(rig: &LibraryRig, answers: &[&Answer], names: &[&str]) {
    let dir = rig.dir_text();
    let journal = serde_json::to_string(&rig.journal()).expect("journal json");
    for text in answers.iter().map(|a| a.text()) {
        assert!(!text.contains(&dir), "a path leaked: {text}");
        assert!(!text.contains("loc-"), "a location ID leaked: {text}");
    }
    assert!(!journal.contains(&dir), "a path was journaled");
    assert!(!journal.contains("loc-"), "a location ID was journaled");
    for name in names {
        assert!(!journal.contains(name), "the name {name} was journaled");
    }
}

/// Readability decided by a closure over the library node.
struct Only(Box<dyn Fn(&NodeRef) -> bool + Send + Sync>);

impl NavigationAccess for Only {
    fn may_read(&self, _principal: &Principal, node: &NodeRef) -> bool {
        (self.0)(node)
    }
}

/// Install navigation in `rig` whose caller may read every library except those `hidden`.
fn hide(rig: &LibraryRig, hidden: &[&str]) {
    let hidden: Vec<NodeRef> = hidden
        .iter()
        .map(|id| NodeRef::Library(LibraryId::new(*id).expect("id")))
        .collect();
    install(rig, Only(Box::new(move |node| !hidden.contains(node))));
}

/// Install navigation in `rig` whose readability `access` decides.
fn install(rig: &LibraryRig, access: Only) {
    let repository = RepositoryId::new("repo").expect("repository");
    let service = NavigationService::new(
        Arc::new(MemoryCatalog::new()) as Arc<dyn LibraryCatalog>,
        Arc::new(MemoryFolderIndex::new()),
        Arc::new(access),
        RepositoryInfo::new(repository, None),
    );
    assert!(rig.state.install_navigation(Arc::new(service)));
}

// ---- Name ----

#[tokio::test]
async fn a_library_is_created_with_its_name_and_the_facts_answer_it() {
    for mount in MOUNTS {
        let rig = LibraryRig::new(mount);
        let (library, _) = create_named(&rig, "family", "Family archive").await;
        let answer = facts(&rig, &library).await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
        assert_eq!(
            answer.json(),
            json!({
                "library_id": library,
                "name": "Family archive",
                "free_bytes": 1_000_000,
                "free_bytes_reason": null,
            })
        );
        assert_redacted(&rig, &[&answer], &["Family archive"]);
    }
}

#[tokio::test]
async fn a_library_created_without_a_name_is_untitled_and_open_may_name_it() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (library, location) = rig.create("plain").await;
    assert_eq!(facts(&rig, &library).await.json()["name"], UNTITLED_LIBRARY);
    let body = json!({ "location_id": location, "name": "Holidays" });
    let opened = rig.post("/api/v1/libraries/openings", body).await;
    assert_eq!(opened.status, StatusCode::OK, "{}", opened.text());
    assert_eq!(facts(&rig, &library).await.json()["name"], "Holidays");
}

#[tokio::test]
async fn a_name_that_breaks_the_rules_is_refused_before_anything_is_created() {
    let rig = LibraryRig::new(Mount::Desktop);
    let long = "x".repeat(1025);
    for (n, bad) in ["", "   ", long.as_str(), "a\u{0}b", "line\nbreak"]
        .iter()
        .enumerate()
    {
        let location = rig.locations.register(&format!("bad-{n}"));
        let body = json!({ "location_id": location, "name": bad });
        let answer = rig.post("/api/v1/libraries", body).await;
        assert_error(&answer, 400, "invalid_request");
        assert!(rig.files(&location).is_empty(), "nothing was written");
    }
    let refused = entries(&rig, CREATE_OPERATION, Outcome::Failed);
    assert_eq!(refused.len(), 5);
    assert!(refused
        .iter()
        .all(|e| e.error_code.as_deref() == Some("invalid_name")));
    let ok = "y".repeat(1024);
    create_named(&rig, "longest", &ok).await;
}

#[tokio::test]
async fn a_rename_is_journaled_with_the_library_only_and_a_same_name_changes_nothing() {
    for mount in MOUNTS {
        let rig = LibraryRig::new(mount);
        let (library, _) = create_named(&rig, "family", "Family archive").await;
        let renamed = rename(&rig, &library, "Family archive 1990s").await;
        assert_eq!(renamed.status, StatusCode::OK, "{}", renamed.text());
        assert_eq!(renamed.json()["name"], "Family archive 1990s");
        assert_eq!(
            facts(&rig, &library).await.json()["name"],
            "Family archive 1990s"
        );
        let same = rename(&rig, &library, "Family archive 1990s").await;
        assert_eq!(same.status, StatusCode::OK, "{}", same.text());
        let journaled = entries(&rig, RENAME_OPERATION, Outcome::Succeeded);
        assert_eq!(
            journaled.len(),
            1,
            "a rename that changes nothing is not journaled"
        );
        assert_eq!(journaled[0].target_ids, std::slice::from_ref(&library));
        assert_redacted(&rig, &[&renamed], &["Family archive"]);
    }
}

#[tokio::test]
async fn a_rename_of_an_unknown_library_or_to_a_bad_name_is_refused_once() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (library, _) = create_named(&rig, "family", "Family archive").await;
    assert_error(
        &rename(&rig, "7d1c2b9e-4f3a-4e61-8a0b-2c9d5e6f7a80", "X").await,
        404,
        "not_found",
    );
    assert_error(&rename(&rig, &library, " ").await, 400, "invalid_request");
    let refused = entries(&rig, RENAME_OPERATION, Outcome::Failed);
    let codes: Vec<_> = refused
        .iter()
        .filter_map(|e| e.error_code.clone())
        .collect();
    assert_eq!(codes, ["library_not_found", "invalid_name"]);
    assert_eq!(facts(&rig, &library).await.json()["name"], "Family archive");
}

// ---- Facts ----

#[tokio::test]
async fn free_bytes_are_null_with_their_reason_when_the_volume_cannot_be_read() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (unreadable, location) = create_named(&rig, "a", "A").await;
    let (remote, other) = create_named(&rig, "b", "B").await;
    {
        let mut free = rig.free.lock().expect("free");
        free.insert(location, FreeSpace::VolumeUnreadable);
        free.insert(other, FreeSpace::NotLocal);
    }
    for (library, reason) in [(unreadable, "volume_unreadable"), (remote, "not_local")] {
        let json = facts(&rig, &library).await.json();
        assert_eq!(json["free_bytes"], Value::Null);
        assert_eq!(json["free_bytes_reason"], reason);
    }
}

#[tokio::test]
async fn an_unknown_or_unreadable_library_is_not_found_and_journaled_once() {
    let rig = LibraryRig::new(Mount::Server);
    let (hidden, _) = create_named(&rig, "hidden", "Hidden").await;
    let (shown, _) = create_named(&rig, "shown", "Shown").await;
    hide(&rig, &[&hidden]);
    assert_error(&facts(&rig, &hidden).await, 404, "not_found");
    let unknown = facts(&rig, "7d1c2b9e-4f3a-4e61-8a0b-2c9d5e6f7a80").await;
    assert_error(&unknown, 404, "not_found");
    let refused = entries(&rig, FACTS_OPERATION, Outcome::Failed);
    assert_eq!(refused.len(), 2);
    // The administrator does not own the library and still reads its free bytes.
    let readable = facts(&rig, &shown).await;
    assert_eq!(readable.json()["free_bytes"], 1_000_000);
}

#[tokio::test]
async fn without_a_registry_the_facts_and_recent_routes_are_unavailable() {
    let rig = LibraryRig::without_registry(Mount::Server);
    let (library, _) = rig.create("plain").await;
    assert_error(&facts(&rig, &library).await, 503, "unavailable");
    assert_error(
        &rig.get("/api/v1/libraries/recent").await,
        503,
        "unavailable",
    );
}

// ---- Recent ----

#[tokio::test]
async fn recent_lists_at_most_20_readable_libraries_newest_first_without_paths() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (hidden, _) = create_named(&rig, "hidden-old", "Hidden old").await;
    let mut created = Vec::new();
    for n in 0..21 {
        rig.clock.advance(1_000);
        created.push(
            create_named(&rig, &format!("lib-{n}"), &format!("Library {n}"))
                .await
                .0,
        );
    }
    rig.clock.advance(1_000);
    let (newest_hidden, _) = create_named(&rig, "hidden-new", "Hidden new").await;
    hide(&rig, &[&hidden, &newest_hidden]);
    let answer = rig.get("/api/v1/libraries/recent").await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    let listed = answer.json()["libraries"]
        .as_array()
        .expect("libraries")
        .clone();
    assert_eq!(listed.len(), 20, "filtered first, then limited");
    let ids: Vec<&str> = listed
        .iter()
        .filter_map(|l| l["library_id"].as_str())
        .collect();
    let expected: Vec<&str> = created.iter().rev().take(20).map(String::as_str).collect();
    assert_eq!(ids, expected, "newest first");
    let first = &listed[0];
    assert_eq!(first["name"], "Library 20");
    assert_eq!(first["open"], true);
    assert_eq!(first["free_bytes"], 1_000_000);
    assert_eq!(first["free_bytes_reason"], Value::Null);
    let opened = first["last_opened"].as_str().expect("time");
    assert!(
        opened.ends_with('Z') && opened.contains('T'),
        "RFC 3339 UTC: {opened}"
    );
    let keys: Vec<&String> = first.as_object().expect("object").keys().collect();
    assert_eq!(
        keys,
        [
            "free_bytes",
            "free_bytes_reason",
            "last_opened",
            "library_id",
            "name",
            "open"
        ]
    );
    assert_redacted(&rig, &[&answer], &["Library 20"]);
}

#[tokio::test]
async fn a_library_this_host_remembers_but_has_not_opened_now_is_listed_as_not_open() {
    let rig = LibraryRig::new(Mount::Desktop);
    let earlier = LibraryId::new("0b6f6a58-5c1d-4b5e-9f3e-2a7d8c9e0f11").expect("id");
    let at = library_routes_fixture::START_MS - 60_000;
    rig.registry
        .remember(&earlier, "loc-elsewhere", Some("Earlier"), at);
    rig.free
        .lock()
        .expect("free")
        .insert("loc-elsewhere".into(), FreeSpace::VolumeUnreadable);
    rig.clock.advance(1_000);
    let (library, _) = create_named(&rig, "now", "Now").await;
    let answer = rig.get("/api/v1/libraries/recent").await;
    let listed = answer.json()["libraries"].clone();
    assert_eq!(listed[0]["library_id"], library.as_str());
    assert_eq!(listed[0]["open"], true);
    assert_eq!(listed[1]["library_id"], earlier.as_str());
    assert_eq!(listed[1]["open"], false);
    assert_eq!(listed[1]["last_opened"], "2026-10-17T23:59:00.000Z");
    assert_eq!(listed[1]["free_bytes"], Value::Null);
    assert_eq!(listed[1]["free_bytes_reason"], "volume_unreadable");
    assert_redacted(&rig, &[&answer], &["Earlier", "Now"]);
}

#[tokio::test]
async fn a_caller_who_may_not_read_the_repository_reads_no_library_facts() {
    let rig = LibraryRig::new(Mount::Server);
    let (library, _) = create_named(&rig, "inside", "Inside").await;
    install(
        &rig,
        Only(Box::new(|node| !matches!(node, NodeRef::Repository(_)))),
    );
    assert_error(&facts(&rig, &library).await, 404, "not_found");
    let answer = rig.get("/api/v1/libraries/recent").await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    assert_eq!(answer.json()["libraries"], json!([]));
}

#[tokio::test]
async fn a_name_given_on_open_that_replaces_another_is_journaled_as_a_rename() {
    let rig = LibraryRig::new(Mount::Desktop);
    let (library, location) = create_named(&rig, "reopened", "First name").await;
    let reopen =
        |name: &str| json!({ "location_id": location, "library_id": library, "name": name });
    let answer = rig
        .post("/api/v1/libraries/openings", reopen("First name"))
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    assert!(
        rig.journaled(RENAME_OPERATION).is_empty(),
        "same name, no rename"
    );
    let answer = rig
        .post("/api/v1/libraries/openings", reopen("Second name"))
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text());
    let renames = rig.journaled(RENAME_OPERATION);
    assert_eq!(renames.len(), 1);
    assert_eq!(renames[0].outcome, Outcome::Succeeded);
    assert_eq!(renames[0].target_ids, std::slice::from_ref(&library));
    assert_eq!(facts(&rig, &library).await.json()["name"], "Second name");
    assert_redacted(&rig, &[&answer], &["First name", "Second name"]);
}
