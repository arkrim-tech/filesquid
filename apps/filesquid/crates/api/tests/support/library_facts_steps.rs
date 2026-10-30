//! Step definitions for `library_facts_API-S-038.feature` (Task API-F2.18; API-S-038): library
//! names, facts and recent libraries through the router, over real OCFL roots on disk.

use crate::library_routes_fixture::{Answer, LibraryRig, Mount};
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::auth::Principal;
use filesquid_api::journal::Outcome;
use filesquid_api::library::{LibraryCatalog, MemoryCatalog};
use filesquid_api::local_library::{FreeSpace, CREATE_OPERATION, RENAME_OPERATION};
use filesquid_api::navigation::{
    MemoryFolderIndex, NavigationAccess, NavigationService, NodeRef, RepositoryInfo,
};
use filesquid_core::{LibraryId, RepositoryId};
use serde_json::{json, Value};
use std::sync::Arc;

/// What the library facts scenarios keep between steps.
#[derive(Default)]
pub struct FactsWorld {
    rig: Option<LibraryRig>,
    /// The libraries created, oldest first: (library ID, location ID).
    libraries: Vec<(String, String)>,
    /// Every library name given at create or rename, oldest first.
    names: Vec<String>,
    answer: Option<Answer>,
    location: Option<String>,
}

impl FactsWorld {
    /// The scenario's rig, made on first use.
    fn rig(&mut self) -> &LibraryRig {
        self.rig
            .get_or_insert_with(|| LibraryRig::new(Mount::Desktop))
    }

    /// The last library created.
    fn last(&self) -> (String, String) {
        self.libraries.last().cloned().expect("a library")
    }

    /// The last answer.
    fn answer(&self) -> &Answer {
        self.answer.as_ref().expect("an answer")
    }
}

/// Readability that denies the one library it holds and admits every other node.
struct Except(LibraryId);

impl NavigationAccess for Except {
    fn may_read(&self, _principal: &Principal, node: &NodeRef) -> bool {
        node != &NodeRef::Library(self.0.clone())
    }
}

/// Create a library named `name` on a new location `folder`.
async fn create(w: &mut MonitorWorld, folder: &str, name: &str) {
    let location = w.facts.rig().locations.register(folder);
    let body = json!({ "location_id": location, "name": name });
    let answer = w.facts.rig().post("/api/v1/libraries", body).await;
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    let library = answer.json()["library_id"]
        .as_str()
        .expect("id")
        .to_string();
    w.facts.libraries.push((library, location));
    w.facts.names.push(name.to_string());
}

/// Install navigation whose caller may read every library except `hidden`.
fn hide(w: &mut MonitorWorld, hidden: &str) {
    let access = Except(LibraryId::new(hidden).expect("id"));
    let service = NavigationService::new(
        Arc::new(MemoryCatalog::new()) as Arc<dyn LibraryCatalog>,
        Arc::new(MemoryFolderIndex::new()),
        Arc::new(access),
        RepositoryInfo::new(RepositoryId::new("repo").expect("repository"), None),
    );
    assert!(w.facts.rig().state.install_navigation(Arc::new(service)));
}

#[given(expr = "the owner has created the library {string}")]
async fn created(w: &mut MonitorWorld, name: String) {
    let folder = format!("lib-{}", w.facts.libraries.len());
    create(w, &folder, &name).await;
}

#[given("the library's volume is not local")]
fn not_local(w: &mut MonitorWorld) {
    let (_, location) = w.facts.last();
    let mut free = w.facts.rig().free.lock().expect("free");
    free.insert(location, FreeSpace::NotLocal);
}

#[given(expr = "the owner has opened {int} libraries one after another")]
async fn opened_many(w: &mut MonitorWorld, count: usize) {
    for n in 0..count {
        w.facts.rig().clock.advance(1_000);
        create(w, &format!("many-{n}"), &format!("Library {n}")).await;
    }
}

#[given("the caller may not read the newest one")]
#[given("the caller may not read that library")]
fn unreadable(w: &mut MonitorWorld) {
    let (library, _) = w.facts.last();
    hide(w, &library);
}

#[when("the owner reads the library's facts")]
#[when("the caller reads the library's facts")]
async fn read_facts(w: &mut MonitorWorld) {
    let (library, _) = w.facts.last();
    let uri = format!("/api/v1/libraries/{library}/facts");
    w.facts.answer = Some(w.facts.rig().get(&uri).await);
}

#[when(expr = "the owner renames the library to {string}")]
async fn rename(w: &mut MonitorWorld, name: String) {
    let (library, _) = w.facts.last();
    let body = json!({ "library_id": library, "name": name });
    let answer = w.facts.rig().post("/api/v1/libraries/renames", body).await;
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    w.facts.names.push(name);
    read_facts(w).await;
}

#[when("the caller lists the recent libraries")]
async fn list_recent(w: &mut MonitorWorld) {
    w.facts.answer = Some(w.facts.rig().get("/api/v1/libraries/recent").await);
}

#[when(expr = "the owner creates a library named with {int} bytes")]
async fn create_with_bytes(w: &mut MonitorWorld, bytes: usize) {
    create_named(w, &"x".repeat(bytes)).await;
}

#[when("the owner creates a library with a blank name")]
async fn create_blank(w: &mut MonitorWorld) {
    create_named(w, "   ").await;
}

#[when("the owner creates a library with a control character in its name")]
async fn create_with_control_character(w: &mut MonitorWorld) {
    create_named(w, "Family\u{0007}archive").await;
}

/// Ask to create a library named `name` on a new location, keeping the answer and the
/// location for the Then steps.
async fn create_named(w: &mut MonitorWorld, name: &str) {
    let location = w.facts.rig().locations.register("checked-name");
    let body = json!({ "location_id": location, "name": name });
    w.facts.answer = Some(w.facts.rig().post("/api/v1/libraries", body).await);
    w.facts.location = Some(location);
}

#[then(expr = "the facts should name it {string} with {int} free bytes")]
fn facts_named(w: &mut MonitorWorld, name: String, bytes: u64) {
    let answer = w.facts.answer();
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    let json = answer.json();
    assert_eq!(json["name"], name.as_str());
    assert_eq!(json["free_bytes"], bytes);
    assert_eq!(json["free_bytes_reason"], Value::Null);
}

#[then("the journal should record one library rename naming the library and not the name")]
fn rename_journaled(w: &mut MonitorWorld) {
    let (library, _) = w.facts.last();
    let rig = w.facts.rig();
    let renames = rig.journaled(RENAME_OPERATION);
    assert_eq!(renames.len(), 1);
    assert_eq!(renames[0].outcome, Outcome::Succeeded);
    assert_eq!(renames[0].target_ids, [library]);
    let journal = serde_json::to_string(&rig.journal()).expect("json");
    for name in &w.facts.names {
        assert!(!journal.contains(name.as_str()), "no name in the journal");
    }
}

#[then(expr = "the free bytes should be null because {string}")]
fn null_because(w: &mut MonitorWorld, reason: String) {
    let json = w.facts.answer().json();
    assert_eq!(json["free_bytes"], Value::Null);
    assert_eq!(json["free_bytes_reason"], reason.as_str());
}

#[then("20 libraries should be listed, newest readable first, with no path or location ID")]
fn listed(w: &mut MonitorWorld) {
    let answer = w.facts.answer();
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    let listed = answer.json()["libraries"]
        .as_array()
        .expect("libraries")
        .clone();
    let ids: Vec<String> = listed
        .iter()
        .filter_map(|l| l["library_id"].as_str().map(str::to_string))
        .collect();
    let all = &w.facts.libraries;
    let expected: Vec<String> = all[..all.len() - 1]
        .iter()
        .rev()
        .take(20)
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(ids, expected);
    let text = answer.text();
    let dir = w.facts.rig().dir_text();
    assert!(!text.contains(&dir) && !text.contains("loc-"), "{text}");
}

#[then(expr = "the library should be created with its {int}-byte name")]
async fn created_with_bytes(w: &mut MonitorWorld, bytes: usize) {
    let answer = w.facts.answer();
    assert_eq!(answer.status.as_u16(), 200, "{}", answer.text());
    let library = answer.json()["library_id"]
        .as_str()
        .expect("id")
        .to_string();
    let location = w.facts.location.clone().expect("location");
    w.facts.libraries.push((library, location));
    read_facts(w).await;
    let name = w.facts.answer().json()["name"].clone();
    assert_eq!(name.as_str().expect("name").len(), bytes);
}

#[then(expr = "the creation should be refused with {string} and nothing written")]
fn refused(w: &mut MonitorWorld, code: String) {
    let answer = w.facts.answer();
    assert_eq!(answer.status.as_u16(), 400, "{}", answer.text());
    let location = w.facts.location.clone().expect("location");
    let rig = w.facts.rig();
    assert!(rig.files(&location).is_empty());
    let refusals = rig.journaled(CREATE_OPERATION);
    assert_eq!(refusals.len(), 1);
    assert_eq!(refusals[0].error_code.as_deref(), Some(code.as_str()));
}

#[then("the facts should be refused as not found")]
fn not_found(w: &mut MonitorWorld) {
    let answer = w.facts.answer();
    assert_eq!(answer.status.as_u16(), 404, "{}", answer.text());
    assert_eq!(answer.json()["code"], "not_found");
}
