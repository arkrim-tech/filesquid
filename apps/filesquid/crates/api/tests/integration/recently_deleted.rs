//! Integration tests of Task API-F2.15 (API-S-011, API-S-019, API-S-021; ADR 041 Decisions 8
//! to 11) through the router, against a real OCFL storage root and a seeded catalog of two
//! libraries: the one Recently deleted node and its view across libraries, the Recently
//! deleted command route (delete, delete with the collection, restore into an "untitled
//! collection"), the settings routes and their refusals, and the permission filter.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

#[path = "../support/drive_catalog.rs"]
mod drive_catalog;

use axum::http::{Method, StatusCode};
use drive_catalog::{DriveCatalog, COLLECTION, REPOSITORY};
use files_fixture::{encode, operation_id, upload_facts, Answer, FilesRig, LIBRARY};
use filesquid_api::auth::{Action, Authenticator, Authoriser, Policy, Principal, Resource};
use filesquid_api::files::FileLibrary;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::library::{
    CatalogChange, CatalogCommit, LibraryCatalog, LibraryRecord, LibraryService, MemorySettings,
};
use filesquid_api::navigation::{
    MemoryFolderIndex, NavigationAccess, NavigationService, NodeRef, RepositoryInfo,
};
use filesquid_api::operation::{ExpectedRevision, OperationId};
use filesquid_core::{Collection, CollectionId, Library, LibraryId, RepositoryId};
use serde_json::{json, Value};
use std::sync::Arc;

/// The second library, "Studio work", with its own retention of 90 days.
const STUDIO: &str = "7a2e3d4c-5b6f-4a71-9b8c-0d1e2f3a4b5c";

/// Its collection "Clients".
const CLIENTS: &str = "8b3f4e5d-6c7a-4b82-8c9d-1e2f3a4b5c6d";

/// One day in milliseconds.
const DAY_MS: i64 = 86_400_000;

/// Every node readable but the second library.
struct NotStudio;

impl NavigationAccess for NotStudio {
    fn may_read(&self, _principal: &Principal, node: &NodeRef) -> bool {
        match node {
            NodeRef::Library(id) => id.as_str() != STUDIO,
            NodeRef::Collection(id) => id.as_str() != CLIENTS,
            _ => true,
        }
    }
}

/// The rig: the files rig's library, a second library served from the same root, the
/// library service with in-memory settings, and the navigation over `access`.
struct Trash {
    rig: FilesRig,
    drive: DriveCatalog,
    settings: Arc<MemorySettings>,
}

impl Trash {
    /// Build the rig; `access` decides what the owner may read.
    async fn new(access: Option<Arc<dyn NavigationAccess>>) -> Self {
        Self::over(FilesRig::new().await, access).await
    }

    /// Build the rig over `rig`; `access` decides what its caller may read.
    async fn over(rig: FilesRig, access: Option<Arc<dyn NavigationAccess>>) -> Self {
        let drive = DriveCatalog::new(LIBRARY);
        add_studio(&drive);
        let navigation = match access {
            Some(access) => Arc::new(NavigationService::new(
                drive.catalog.clone(),
                Arc::new(MemoryFolderIndex::new()),
                access,
                RepositoryInfo::new(RepositoryId::new(REPOSITORY).expect("repository"), None),
            )),
            None => drive.navigation.clone(),
        };
        assert!(rig.state.install_navigation(navigation));
        let studio = LibraryId::new(STUDIO).expect("library");
        let served = FileLibrary {
            backend: rig.root.clone(),
            reservations: rig.leases.clone(),
            staging: rig.staging.clone(),
            staging_prefix: "studio".to_string(),
        };
        assert!(rig.service.open_library(&studio, served));
        let settings = Arc::new(MemorySettings::new());
        let service = LibraryService::new(drive.catalog.clone(), rig.monitor.journal())
            .with_settings(settings.clone());
        assert!(rig.state.install_library_service(Arc::new(service)));
        Self {
            rig,
            drive,
            settings,
        }
    }

    /// Upload `name` and place it in `collection` of `library`; answer its file ID.
    async fn file(&self, name: &str, library: &str, collection: &str) -> String {
        let answer = self
            .rig
            .upload(upload_facts(name, 3, "name_normalization=nfc"), b"abc")
            .await;
        assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
        let id = answer.result()["file_id"].as_str().expect("id").to_string();
        if library == LIBRARY {
            self.drive.file_in_collection_id(&id, collection);
        } else {
            self.drive.file_in_library(&id, library, collection);
        }
        id
    }

    /// The current revision of `library`.
    fn revision(&self, library: &str) -> String {
        let id = LibraryId::new(library).expect("library");
        self.drive
            .catalog
            .library(&id)
            .expect("read")
            .expect("library")
            .revision
    }

    /// Send the Recently deleted command `input` of `library`.
    async fn command(&self, library: &str, input: Value) -> Answer {
        let body = json!({
            "envelope": {
                "schema_version": 1,
                "operation_id": operation_id(),
                "expected_revision": self.revision(library),
                "scope": format!("library:{library}"),
            },
            "input": input,
        });
        self.rig
            .send_command(Method::POST, "/api/v1/recently-deleted/commands", &body)
            .await
    }

    /// The handle of the repository's Recently deleted node, read from the navigation.
    async fn trash_node(&self) -> String {
        let root = self.rig.get("/api/v1/navigation").await.json();
        let repository = root["nodes"][0]["node"]
            .as_str()
            .expect("repository")
            .to_string();
        let page = self
            .rig
            .get(&format!("/api/v1/navigation?node={}", encode(&repository)))
            .await
            .json();
        let last = page["nodes"]
            .as_array()
            .expect("nodes")
            .last()
            .expect("node")
            .clone();
        assert_eq!(last["kind"], "recently_deleted", "{page}");
        last["node"].as_str().expect("handle").to_string()
    }

    /// The rows of the Recently deleted view, by display name.
    async fn trash_rows(&self) -> (Vec<Value>, Value) {
        let node = self.trash_node().await;
        let answer = self
            .rig
            .get(&format!(
                "/api/v1/files/view?node={}&sort=name",
                encode(&node)
            ))
            .await;
        assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
        let page = answer.json();
        let rows = page["rows"].as_array().expect("rows").clone();
        (rows, page)
    }
}

/// Add the library "Studio work" with its collection "Clients" and a 90-day retention.
fn add_studio(drive: &DriveCatalog) {
    let id = LibraryId::new(STUDIO).expect("library");
    let repository = RepositoryId::new(REPOSITORY).expect("repository");
    let record = LibraryRecord {
        library: Library::new(id.clone(), repository),
        name: "Studio work".into(),
    };
    let clients = Collection::new(
        CollectionId::new(CLIENTS).expect("id"),
        id.clone(),
        "Clients",
    )
    .expect("collection");
    let commit = CatalogCommit {
        operation_id: OperationId::new("seed-studio").expect("op"),
        library_id: id.clone(),
        expected_revision: ExpectedRevision::new("0").expect("revision"),
        changes: vec![
            CatalogChange::CreateLibrary(record),
            CatalogChange::PutCollection(clients),
        ],
        journal: JournalEntry::new("seed", Outcome::Succeeded),
        created_id: None,
        deletion: None,
        restored_into: Vec::new(),
    };
    drive.catalog.commit(commit).expect("seed");
    drive
        .catalog
        .set_retention_days(&id, Some(90))
        .expect("retention");
}

/// The deleted facts of the row named `name`.
fn deleted_of<'a>(rows: &'a [Value], name: &str) -> &'a Value {
    let row = rows
        .iter()
        .find(|row| row["display_name"] == name)
        .unwrap_or_else(|| panic!("no row {name}"));
    &row["deleted"]
}

#[tokio::test]
async fn recently_deleted_lists_the_deleted_files_of_every_library_with_their_own_retention() {
    let trash = Trash::new(None).await;
    let alone = trash.file("minutes.pdf", LIBRARY, COLLECTION).await;
    trash.file("site-visit.jpg", LIBRARY, COLLECTION).await;
    let brief = trash.file("brief-v1.pdf", STUDIO, CLIENTS).await;
    trash.file("kept.txt", STUDIO, CLIENTS).await;
    let deleted = trash
        .command(
            LIBRARY,
            json!({"command": "delete_files", "library_id": LIBRARY, "file_ids": [alone]}),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::OK, "{:?}", deleted.json());
    assert_eq!(deleted.json()["kind"], "committed");
    let collection = trash
        .command(LIBRARY, json!({"command": "delete_collection", "library_id": LIBRARY, "collection_id": COLLECTION}))
        .await;
    assert_eq!(collection.status, StatusCode::OK, "{:?}", collection.json());
    assert_eq!(collection.result()["deletion"]["cause"], "with_collection");
    let studio = trash
        .command(
            STUDIO,
            json!({"command": "delete_files", "library_id": STUDIO, "file_ids": [brief]}),
        )
        .await;
    assert_eq!(studio.status, StatusCode::OK, "{:?}", studio.json());
    let (rows, page) = trash.trash_rows().await;
    let names: Vec<&str> = rows
        .iter()
        .map(|r| r["display_name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        ["brief-v1.pdf", "minutes.pdf", "site-visit.jpg"],
        "no folder rows: {page}"
    );
    assert_eq!(page["complete"], true);
    let one = deleted_of(&rows, "minutes.pdf");
    assert_eq!(one["library_id"], LIBRARY);
    assert_eq!(one["library_name"], "Family archive");
    assert_eq!(one["cause"], "one_by_one");
    assert_eq!(one["home_collection_title"], "Holidays");
    let with = deleted_of(&rows, "site-visit.jpg");
    assert_eq!(with["cause"], "with_collection");
    assert_eq!(
        with["home_collection_title"], "Holidays",
        "kept after its deletion"
    );
    assert_eq!(with["home_collection_deleted"], true);
    let other = deleted_of(&rows, "brief-v1.pdf");
    assert_eq!(other["library_name"], "Studio work");
    assert_eq!(other["home_collection_title"], "Clients");
    assert_eq!(other["home_collection_deleted"], false);
    let span = |row: &Value| {
        let at = filesquid_api::journal::parse_rfc3339_ms(row["deleted_at"].as_str().expect("at"));
        let after =
            filesquid_api::journal::parse_rfc3339_ms(row["purge_after"].as_str().expect("after"));
        after.expect("after") - at.expect("at")
    };
    assert_eq!(span(one), 30 * DAY_MS, "the installation default");
    assert_eq!(span(other), 90 * DAY_MS, "its own library's retention");
    assert_ne!(one["days_left"], other["days_left"], "different days left");
}

#[tokio::test]
async fn the_node_is_refused_by_the_folder_listing_and_found_by_is_deleted() {
    let trash = Trash::new(None).await;
    let file = trash.file("minutes.pdf", LIBRARY, COLLECTION).await;
    trash
        .command(
            LIBRARY,
            json!({"command": "delete_files", "library_id": LIBRARY, "file_ids": [file]}),
        )
        .await;
    let node = trash.trash_node().await;
    let listing = trash
        .rig
        .get(&format!("/api/v1/files/listing?node={}", encode(&node)))
        .await;
    assert_eq!(
        listing.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        listing.json()
    );
    let found = trash.rig.get("/api/v1/files/search?q=is%3Adeleted").await;
    assert_eq!(found.status, StatusCode::OK, "{:?}", found.json());
    let rows = found.json()["rows"].as_array().expect("rows").clone();
    assert_eq!(rows.len(), 1);
    let unfiled = trash.rig.get("/api/v1/files/search?q=is%3Aunfiled").await;
    assert_eq!(unfiled.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_restore_whose_home_is_gone_names_the_untitled_collection_it_created() {
    let trash = Trash::new(None).await;
    let file = trash.file("site-visit.jpg", LIBRARY, COLLECTION).await;
    trash
        .command(LIBRARY, json!({"command": "delete_collection", "library_id": LIBRARY, "collection_id": COLLECTION}))
        .await;
    let restored = trash
        .command(
            LIBRARY,
            json!({"command": "restore_files", "library_id": LIBRARY, "file_ids": [file]}),
        )
        .await;
    assert_eq!(restored.status, StatusCode::OK, "{:?}", restored.json());
    let made = &restored.result()["restored_into"][0];
    assert_eq!(made["title"], "untitled collection");
    assert_eq!(made["from_collection_id"], COLLECTION);
    let (rows, _) = trash.trash_rows().await;
    assert!(rows.is_empty(), "the file left Recently deleted");
}

#[tokio::test]
async fn a_restore_with_no_free_untitled_title_carries_its_sub_code() {
    use filesquid_api::library::{MAX_UNTITLED_TITLES, UNTITLED_COLLECTION_TITLE};
    let trash = Trash::new(None).await;
    let file = trash.file("site-visit.jpg", LIBRARY, COLLECTION).await;
    trash
        .command(LIBRARY, json!({"command": "delete_collection", "library_id": LIBRARY, "collection_id": COLLECTION}))
        .await;
    let library = LibraryId::new(LIBRARY).expect("library");
    for n in 1..=MAX_UNTITLED_TITLES {
        let title = match n {
            1 => UNTITLED_COLLECTION_TITLE.to_string(),
            n => format!("{UNTITLED_COLLECTION_TITLE} {n}"),
        };
        let id = CollectionId::new(format!("taken-{n}")).expect("id");
        let taken = Collection::new(id, library.clone(), title).expect("collection");
        trash
            .drive
            .catalog
            .insert_collection(taken)
            .expect("insert");
    }
    let refused = trash
        .command(
            LIBRARY,
            json!({"command": "restore_files", "library_id": LIBRARY, "file_ids": [file]}),
        )
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{:?}", refused.json());
    assert_eq!(refused.json()["code"], "conflict");
    assert_eq!(refused.json()["sub_code"], "restore_name_exhausted");
    assert_eq!(refused.json()["kind"], "conflict");
}

#[tokio::test]
async fn the_command_route_takes_only_trash_commands_in_their_own_scope() {
    let trash = Trash::new(None).await;
    let other = trash
        .command(
            LIBRARY,
            json!({"command": "create_collection", "library_id": LIBRARY, "title": "x"}),
        )
        .await;
    assert_eq!(other.status, StatusCode::BAD_REQUEST, "{:?}", other.json());
    let body = json!({
        "envelope": {"schema_version": 1, "operation_id": operation_id(), "expected_revision": "1", "scope": format!("library:{STUDIO}")},
        "input": {"command": "delete_files", "library_id": LIBRARY, "file_ids": ["f"]},
    });
    let scope = trash
        .rig
        .send_command(Method::POST, "/api/v1/recently-deleted/commands", &body)
        .await;
    assert_eq!(scope.status, StatusCode::BAD_REQUEST, "{:?}", scope.json());
    let stale = json!({
        "envelope": {"schema_version": 1, "operation_id": operation_id(), "expected_revision": "999", "scope": format!("library:{LIBRARY}")},
        "input": {"command": "set_library_retention", "library_id": LIBRARY, "retention_days": 7},
    });
    let conflict = trash
        .rig
        .send_command(Method::POST, "/api/v1/recently-deleted/commands", &stale)
        .await;
    assert_eq!(
        conflict.status,
        StatusCode::CONFLICT,
        "{:?}",
        conflict.json()
    );
    assert_eq!(conflict.json()["kind"], "conflict");
    let journal = trash.rig.journal();
    assert!(journal
        .iter()
        .any(|e| e.operation == "recently_deleted.command" && e.outcome == Outcome::Failed));
    for entry in &journal {
        let text = serde_json::to_string(entry).expect("json");
        assert!(
            !text.contains("Holidays") && !text.contains("Clients"),
            "{text}"
        );
    }
}

/// A settings command: the envelope with `operation_id` and `scope`, and `input`.
fn settings_command(operation_id: &str, scope: &str, input: Value) -> Value {
    json!({
        "envelope": {"schema_version": 1, "operation_id": operation_id,
                     "expected_revision": "0", "scope": scope},
        "input": input,
    })
}

#[tokio::test]
async fn the_settings_answer_the_default_the_expiry_setting_and_each_library_s_period() {
    let trash = Trash::new(None).await;
    let read = trash.rig.get("/api/v1/settings/recently-deleted").await;
    assert_eq!(read.status, StatusCode::OK, "{:?}", read.json());
    let view = read.json();
    assert_eq!(view["default_retention_days"], 30);
    assert_eq!(view["expiry"], "keep_version_history");
    assert_eq!(view["may_change"], true, "the owner is the admin");
    let libraries = view["libraries"].as_array().expect("libraries");
    assert_eq!(libraries.len(), 2);
    assert_eq!(libraries[0]["retention_days"], Value::Null);
    assert_eq!(libraries[1]["retention_days"], 90);
    for (n, input) in [
        json!({"default_retention_days": 0, "expiry": "keep_version_history",
               "expiry_interval_minutes": 60}),
        json!({"default_retention_days": 3651, "expiry": "keep_version_history",
               "expiry_interval_minutes": 60}),
        json!({"default_retention_days": 30, "expiry": "forever", "expiry_interval_minutes": 60}),
        json!({"default_retention_days": 30, "expiry": "keep_version_history"}),
    ]
    .into_iter()
    .enumerate()
    {
        let bad = settings_command(&format!("op-bad-{n}"), "installation", input);
        let answer = trash
            .rig
            .send_command(Method::PUT, "/api/v1/settings/recently-deleted", &bad)
            .await;
        assert_eq!(
            answer.status,
            StatusCode::BAD_REQUEST,
            "{bad}: {:?}",
            answer.json()
        );
    }
    let input = json!({"default_retention_days": 14, "expiry": "delete_all_version_history",
                       "expiry_interval_minutes": 60});
    let set = settings_command("op-settings", "installation", input.clone());
    let answer = trash
        .rig
        .send_command(Method::PUT, "/api/v1/settings/recently-deleted", &set)
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    assert_eq!(answer.json()["operation_id"], "op-settings");
    assert_eq!(answer.json()["kind"], "committed");
    assert_eq!(answer.json()["result"]["default_retention_days"], 14);
    assert_eq!(
        answer.json()["result"]["expiry"],
        "delete_all_version_history"
    );
    let replay = trash
        .rig
        .send_command(Method::PUT, "/api/v1/settings/recently-deleted", &set)
        .await;
    assert_eq!(
        replay.json(),
        answer.json(),
        "a repeated operation_id replays"
    );
    let reused = settings_command(
        "op-settings",
        "installation",
        json!({"default_retention_days": 15, "expiry": "keep_version_history",
               "expiry_interval_minutes": 60}),
    );
    let refused = trash
        .rig
        .send_command(Method::PUT, "/api/v1/settings/recently-deleted", &reused)
        .await;
    assert_eq!(
        refused.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        refused.json()
    );
    let scoped = settings_command("op-scope", "library:x", input.clone());
    let refused = trash
        .rig
        .send_command(Method::PUT, "/api/v1/settings/recently-deleted", &scoped)
        .await;
    assert_eq!(
        refused.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        refused.json()
    );
    let bare = trash
        .rig
        .send_command(Method::PUT, "/api/v1/settings/recently-deleted", &input)
        .await;
    assert_eq!(
        bare.status,
        StatusCode::BAD_REQUEST,
        "an envelope is required"
    );
    let stored = trash.settings.journal_entries();
    assert_eq!(stored.len(), 1, "journaled once");
    assert_eq!(stored[0].operation, "settings.recently_deleted.set");
    assert!(stored[0].target_ids.is_empty(), "no value is journaled");
}

#[tokio::test]
async fn a_library_the_caller_may_not_read_is_left_out_of_recently_deleted() {
    let trash = Trash::new(Some(Arc::new(NotStudio))).await;
    let mine = trash.file("minutes.pdf", LIBRARY, COLLECTION).await;
    let hidden = trash.file("brief-v1.pdf", STUDIO, CLIENTS).await;
    trash
        .command(
            LIBRARY,
            json!({"command": "delete_files", "library_id": LIBRARY, "file_ids": [mine]}),
        )
        .await;
    let refused = trash
        .command(
            STUDIO,
            json!({"command": "delete_files", "library_id": STUDIO, "file_ids": [hidden]}),
        )
        .await;
    assert_eq!(
        refused.status,
        StatusCode::NOT_FOUND,
        "{:?}",
        refused.json()
    );
    let service = trash.rig.state.library_service().expect("service");
    let studio = LibraryId::new(STUDIO).expect("id");
    let request = filesquid_api::library::CommandRequest {
        envelope: filesquid_api::operation::CommandEnvelope::new(
            OperationId::new("direct").expect("op"),
            ExpectedRevision::new(trash.revision(STUDIO)).expect("rev"),
            filesquid_api::operation::OperationScope::new(format!("library:{STUDIO}"))
                .expect("scope"),
        ),
        command: filesquid_api::library::LibraryCommand::DeleteFiles {
            library_id: studio,
            file_ids: vec![filesquid_core::FileId::new(hidden).expect("id")],
        },
    };
    service.execute("owner", request).expect("deleted directly");
    let (rows, _) = trash.trash_rows().await;
    let names: Vec<&str> = rows
        .iter()
        .map(|r| r["display_name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["minutes.pdf"]);
    let settings = trash
        .rig
        .get("/api/v1/settings/recently-deleted")
        .await
        .json();
    assert_eq!(
        settings["libraries"].as_array().expect("libraries").len(),
        1
    );
}

/// The settings input with the defaults and `interval` (any JSON value).
fn interval_input(interval: Value) -> Value {
    json!({"default_retention_days": 30, "expiry": "keep_version_history",
           "expiry_interval_minutes": interval})
}

/// `PUT` the settings `input` under a fresh operation ID.
async fn put_settings(trash: &Trash, input: Value) -> Answer {
    let body = settings_command(&operation_id(), "installation", input);
    trash
        .rig
        .send_command(Method::PUT, "/api/v1/settings/recently-deleted", &body)
        .await
}

/// The `expiry_interval_minutes` the settings `GET` answers.
async fn read_interval(trash: &Trash) -> Value {
    let read = trash.rig.get("/api/v1/settings/recently-deleted").await;
    assert_eq!(read.status, StatusCode::OK, "{:?}", read.json());
    read.json()["expiry_interval_minutes"].clone()
}

/// Task API-F2.30 (API-S-019 **Routes and expiry runs**): the settings `GET` answers
/// `expiry_interval_minutes`, 60 by default; the admin stores 5 and 1,440; each change is
/// journaled once without its value and the current value stores nothing.
#[tokio::test]
async fn the_admin_reads_and_sets_the_expiry_interval_within_its_bounds() {
    let trash = Trash::new(None).await;
    assert_eq!(read_interval(&trash).await, 60);
    let same = put_settings(&trash, interval_input(json!(60))).await;
    assert_eq!(same.status, StatusCode::OK, "{:?}", same.json());
    assert!(
        trash.settings.journal_entries().is_empty(),
        "the current value stores nothing"
    );
    for (n, minutes) in [5, 1_440].into_iter().enumerate() {
        let answer = put_settings(&trash, interval_input(json!(minutes))).await;
        assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
        assert_eq!(answer.json()["kind"], "committed");
        assert_eq!(answer.json()["result"]["expiry_interval_minutes"], minutes);
        assert_eq!(read_interval(&trash).await, minutes);
        let stored = trash.settings.journal_entries();
        assert_eq!(stored.len(), n + 1, "journaled once");
        assert_eq!(stored[n].operation, "settings.recently_deleted.set");
        assert!(stored[n].target_ids.is_empty(), "no value is journaled");
        let text = serde_json::to_string(&stored[n]).expect("entry");
        assert!(!text.contains("expiry_interval"), "{text}");
    }
}

/// Task API-F2.30: 4, 1,441, a fraction and any value that is not a whole number are
/// refused as `invalid_request` with nothing stored, each refusal journaled once.
#[tokio::test]
async fn an_expiry_interval_outside_the_bounds_or_not_whole_is_refused_with_nothing_stored() {
    let trash = Trash::new(None).await;
    let bad = [
        json!(4),
        json!(1_441),
        json!(1.5),
        json!(-5),
        json!("60"),
        Value::Null,
    ];
    for (n, interval) in bad.into_iter().enumerate() {
        let answer = put_settings(&trash, interval_input(interval.clone())).await;
        assert_eq!(
            answer.status,
            StatusCode::BAD_REQUEST,
            "{interval}: {:?}",
            answer.json()
        );
        assert_eq!(answer.json()["code"], "invalid_request", "{interval}");
        assert_eq!(read_interval(&trash).await, 60, "nothing is stored");
        assert!(trash.settings.journal_entries().is_empty());
        let refusals = trash.rig.journaled("settings.recently_deleted.set");
        assert_eq!(refusals.len(), n + 1, "{interval}: journaled once");
        assert_eq!(refusals[n].outcome, Outcome::Failed);
    }
}

/// The token of the caller who is not the admin.
const MEMBER_TOKEN: &str = "member-token";

/// A signed-in caller who may do everything but change the installation settings: the
/// stand-in for a server member until the server has accounts.
struct Member;

impl Authenticator for Member {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        (token == MEMBER_TOKEN).then_some(Principal::Administrator)
    }
}

impl Policy for Member {
    fn permits(&self, _principal: &Principal, action: Action, _resource: &Resource) -> bool {
        action != Action::ConfigureRecentlyDeleted
    }
}

/// Task API-F2.30: a caller who is not the admin reads the interval, is told they may not
/// change it, and is refused `forbidden` with nothing stored.
#[tokio::test]
async fn a_caller_who_is_not_the_admin_may_not_change_the_expiry_interval() {
    let authoriser = Authoriser::new(Arc::new(Member), Arc::new(Member));
    let rig = FilesRig::with_authoriser(authoriser, MEMBER_TOKEN.to_string()).await;
    let trash = Trash::over(rig, None).await;
    let read = trash.rig.get("/api/v1/settings/recently-deleted").await;
    assert_eq!(read.status, StatusCode::OK, "{:?}", read.json());
    assert_eq!(read.json()["expiry_interval_minutes"], 60);
    assert_eq!(read.json()["may_change"], false);
    let refused = put_settings(&trash, interval_input(json!(30))).await;
    assert_eq!(
        refused.status,
        StatusCode::FORBIDDEN,
        "{:?}",
        refused.json()
    );
    assert_eq!(refused.json()["code"], "forbidden");
    assert_eq!(read_interval(&trash).await, 60, "nothing is stored");
    assert!(trash.settings.journal_entries().is_empty());
}
