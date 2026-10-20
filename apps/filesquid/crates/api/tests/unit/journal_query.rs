//! Unit tests for the journal query and export routes (API-S-031): the library, object, file
//! and operation ID filters, one entry by ID, the PROV-O link and journal-only marking, the
//! display-name lookup, and the JSON Lines and CSV export journaled as `journal.export`.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use filesquid_api::admin::{JournalNames, EXPORT_OPERATION};
use filesquid_api::auth::{Authoriser, LaunchToken, Principal};
use filesquid_api::files::NameFuture;
use filesquid_api::journal::csv::{csv_field, csv_row, CSV_HEADER};
use filesquid_api::journal::{
    JournalEntry, JournalQuery, JournalStore, Level, MemoryJournal, Outcome,
};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::{router, ApiState};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

const FILE: &str = "5f0c3e44-2a7b-4c1d-9e8f-0123456789ab";
const OTHER: &str = "6a1d4f55-3b8c-4d2e-8f90-123456789abc";

fn object() -> String {
    format!("urn:uuid:{FILE}")
}

struct Host {
    state: ApiState,
    monitor: Monitor,
    token: String,
}

/// The seeded journal: ids 1 to 6.
fn seed(store: &MemoryJournal) {
    let entries = vec![
        // 1: a committed import of FILE in lib-a, operation op-1.
        JournalEntry::new("ocfl.commit", Outcome::Succeeded)
            .operation_id("op-1")
            .object_ref(format!("{}@v1", object()))
            .target("lib-a")
            .target(object()),
        // 2: a metadata edit of FILE, operation op-2.
        JournalEntry::new("metadata.edit", Outcome::Succeeded)
            .operation_id("op-2")
            .object_ref(format!("{}@v2", object()))
            .target("lib-a"),
        // 3: a failed move of FILE, operation op-3.
        JournalEntry::new("file.move", Outcome::Failed)
            .operation_id("op-3")
            .object_ref(format!("{}@v2", object()))
            .target("lib-a")
            .error_code("conflict"),
        // 4: a single-file download of FILE.
        JournalEntry::new("file.download", Outcome::Succeeded)
            .operation_id("op-4")
            .object_ref(format!("{}@v2", object()))
            .target("lib-a"),
        // 5: a commit of OTHER in lib-b.
        JournalEntry::new("ocfl.commit", Outcome::Succeeded)
            .operation_id("op-5")
            .object_ref(format!("urn:uuid:{OTHER}@v1"))
            .target("lib-b")
            .target(format!("urn:uuid:{OTHER}")),
        // 6: a host event with no object.
        JournalEntry::new("host.started", Outcome::Succeeded),
    ];
    store.append(&entries).expect("seed");
}

impl Host {
    fn start() -> Self {
        let state = ApiState::new();
        let token = LaunchToken::generate().expect("token");
        state.install_authoriser(Authoriser::desktop_owner(&token));
        let store = MemoryJournal::new("host-a");
        seed(&store);
        let monitor = Monitor::start(
            Arc::new(store),
            Arc::new(StaticHostChecks::healthy()),
            MonitorConfig::new("host-a"),
            state.events.clone(),
        );
        state.install_monitor(monitor.clone());
        Self {
            state,
            monitor,
            token: token.secret().to_string(),
        }
    }

    async fn get(&self, uri: &str) -> (StatusCode, Option<String>, String) {
        let request = Request::get(uri)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .body(Body::empty())
            .expect("request");
        let response = router(self.state.clone())
            .oneshot(request)
            .await
            .expect("answer");
        let status = response.status();
        let ctype = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().expect("ascii").to_string());
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        (
            status,
            ctype,
            String::from_utf8(bytes.to_vec()).expect("utf-8"),
        )
    }

    async fn ids(&self, query: &str) -> Vec<i64> {
        let (status, _, body) = self.get(&format!("/api/v1/admin/journal?{query}")).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        let page: Value = serde_json::from_str(&body).expect("json");
        page["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .map(|e| e["id"].as_i64().expect("id"))
            .collect()
    }
}

#[tokio::test]
async fn the_journal_filters_by_library() {
    let host = Host::start();
    assert_eq!(host.ids("library=lib-a").await, [1, 2, 3, 4]);
    assert_eq!(host.ids("library=lib-b").await, [5]);
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_journal_filters_by_object_id_and_by_file() {
    let host = Host::start();
    let by_object = host.ids(&format!("object={}", object())).await;
    assert_eq!(by_object, [1, 2, 3, 4]);
    assert_eq!(host.ids(&format!("file={FILE}")).await, by_object);
    assert_eq!(host.ids(&format!("file={OTHER}")).await, [5]);
    let upper = FILE.to_ascii_uppercase();
    assert_eq!(
        host.ids(&format!("file={upper}")).await,
        by_object,
        "a UUID in capitals"
    );
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_journal_filters_by_operation_id() {
    let host = Host::start();
    assert_eq!(host.ids("operation_id=op-2").await, [2]);
    assert_eq!(host.ids("operation_id=op-404").await, Vec::<i64>::new());
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_new_filters_combine_with_the_existing_ones() {
    let host = Host::start();
    assert_eq!(host.ids("library=lib-a&operation=ocfl.commit").await, [1]);
    assert_eq!(host.ids("library=lib-a&outcome=failed").await, [3]);
    assert_eq!(host.ids("library=lib-a&level=error").await, [3]);
    assert_eq!(
        host.ids(&format!("file={FILE}&order=desc")).await,
        [4, 3, 2, 1]
    );
    host.monitor.shutdown();
}

#[tokio::test]
async fn invalid_filter_values_are_refused() {
    let host = Host::start();
    for query in [
        "library=lib%20a",
        "object=a/b",
        "file=not-a-uuid",
        "operation_id=op%201",
        &format!("file={FILE}&object={}", object()),
    ] {
        let (status, _, body) = host.get(&format!("/api/v1/admin/journal?{query}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {body}");
        let error: Value = serde_json::from_str(&body).expect("json");
        assert_eq!(error["code"], "invalid_request");
    }
    host.monitor.shutdown();
}

#[tokio::test]
async fn one_entry_is_returned_by_id() {
    let host = Host::start();
    let (status, _, body) = host.get("/api/v1/admin/journal/entries/2").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let entry: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(entry["id"], 2);
    assert_eq!(entry["operation"], "metadata.edit");
    let (status, _, body) = host.get("/api/v1/admin/journal/entries/two").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(error["message"], "The entry ID must be a number.");
    let (status, _, body) = host.get("/api/v1/admin/journal/entries/99").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        serde_json::from_str::<Value>(&body).expect("json")["code"],
        "not_found"
    );
    let (status, _, _) = host.get("/api/v1/admin/journal/entries/two").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    host.monitor.shutdown();
}

#[tokio::test]
async fn committed_entries_link_to_their_prov_o_activity_and_others_are_journal_only() {
    let host = Host::start();
    let (_, _, body) = host.get("/api/v1/admin/journal").await;
    let page: Value = serde_json::from_str(&body).expect("json");
    let entries = page["entries"].as_array().expect("entries");
    let provenance =
        |id: i64| entries.iter().find(|e| e["id"] == id).expect("entry")["provenance"].clone();
    let commit = provenance(1);
    assert_eq!(commit["journal_only"], false);
    assert_eq!(commit["operation_id"], "op-1");
    assert_eq!(commit["object_ref"], format!("{}@v1", object()));
    assert_eq!(
        commit["record"],
        format!("/api/v1/files/{FILE}/versions/v1/prov")
    );
    assert_eq!(commit["activities"], "/api/v1/history?operation=op-1");
    assert_eq!(provenance(2)["journal_only"], false);
    for journal_only in [3, 4, 6] {
        let link = provenance(journal_only);
        assert_eq!(link["journal_only"], true, "entry {journal_only}");
        assert!(link.get("record").is_none(), "entry {journal_only}");
    }
    let (_, _, one) = host.get("/api/v1/admin/journal/entries/3").await;
    let one: Value = serde_json::from_str(&one).expect("json");
    assert_eq!(one["provenance"]["journal_only"], true);
    host.monitor.shutdown();
}

/// Names FILE for the owner only.
struct OwnerNames;

impl JournalNames for OwnerNames {
    fn display_name<'a>(&'a self, principal: &'a Principal, object_id: &'a str) -> NameFuture<'a> {
        let name = (*principal == Principal::Owner && object_id == object())
            .then(|| "Grandma's birthday.jpg".to_string());
        Box::pin(async move { name })
    }
}

#[tokio::test]
async fn display_names_are_resolved_at_read_time_on_request_only() {
    let host = Host::start();
    assert!(host.state.install_journal_names(Arc::new(OwnerNames)));
    let (_, _, plain) = host.get("/api/v1/admin/journal").await;
    assert!(!plain.contains("Grandma"), "names only on request");
    let (_, _, named) = host.get("/api/v1/admin/journal?names=true").await;
    let page: Value = serde_json::from_str(&named).expect("json");
    let entries = page["entries"].as_array().expect("entries");
    let name =
        |id: i64| entries.iter().find(|e| e["id"] == id).expect("entry")["display_name"].clone();
    assert_eq!(name(1), "Grandma's birthday.jpg");
    assert_eq!(name(4), "Grandma's birthday.jpg");
    assert_eq!(name(5), Value::Null, "the resolver did not name OTHER");
    assert_eq!(name(6), Value::Null, "no object");
    let (_, _, one) = host.get("/api/v1/admin/journal/entries/1?names=true").await;
    assert!(one.contains("Grandma's birthday.jpg"));
    host.monitor.journal().flush();
    let stored = host
        .monitor
        .store()
        .query(&JournalQuery::default())
        .expect("query");
    let text = serde_json::to_string(&stored.entries).expect("json");
    assert!(!text.contains("Grandma"), "the name is never stored");
    host.monitor.shutdown();
}

#[tokio::test]
async fn without_a_name_resolver_no_names_are_returned() {
    let host = Host::start();
    let (status, _, body) = host.get("/api/v1/admin/journal?names=true").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("display_name"));
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_export_takes_the_same_filters_and_never_carries_names() {
    let host = Host::start();
    host.state.install_journal_names(Arc::new(OwnerNames));
    let (status, ctype, body) = host
        .get(&format!(
            "/api/v1/admin/journal/export?file={FILE}&outcome=succeeded&names=true"
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ctype.as_deref(), Some("application/x-ndjson"));
    let ids: Vec<i64> = body
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).expect("line")["id"]
                .as_i64()
                .expect("id")
        })
        .collect();
    assert_eq!(ids, [1, 2, 4]);
    assert!(!body.contains("Grandma") && !body.contains("display_name"));
    let (_, _, by_op) = host
        .get("/api/v1/admin/journal/export?operation_id=op-5&format=jsonl")
        .await;
    assert_eq!(by_op.lines().count(), 1);
    host.monitor.shutdown();
}

#[tokio::test]
async fn the_csv_export_is_rfc_4180_with_one_row_per_entry() {
    let host = Host::start();
    let (status, ctype, body) = host
        .get("/api/v1/admin/journal/export?format=csv&library=lib-a")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ctype.as_deref(), Some("text/csv; charset=utf-8"));
    assert!(body.ends_with("\r\n"));
    let rows: Vec<&str> = body.split_terminator("\r\n").collect();
    assert_eq!(rows[0], CSV_HEADER);
    assert_eq!(rows.len(), 1 + 4, "header and one row per entry");
    assert!(rows[1].starts_with("1,"), "{}", rows[1]);
    assert!(rows[1].contains(",ocfl.commit,succeeded,info,"));
    assert!(rows[3].contains(",conflict,"));
    host.monitor.shutdown();
}

#[test]
fn csv_fields_with_commas_quotes_or_line_breaks_are_quoted() {
    assert_eq!(csv_field("plain"), "plain");
    assert_eq!(csv_field(""), "");
    assert_eq!(csv_field("a,b"), "\"a,b\"");
    assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
    assert_eq!(csv_field("cr\rhere"), "\"cr\rhere\"");
}

#[test]
fn a_csv_row_names_every_column_of_the_header_in_order() {
    let entry = JournalEntry::new("ocfl.commit", Outcome::Succeeded)
        .at_ms(0)
        .actor("owner")
        .target("lib-a")
        .target("urn:uuid:x")
        .operation_id("op-1")
        .object_ref("urn:uuid:x@v1")
        .duration_ms(5)
        .level(Level::Info);
    let mut entry = entry;
    entry.id = Some(9);
    entry.host = "host-a".into();
    entry.client_event_id = Some("00000001-0000-4000-8000-000000000000".into());
    let row = csv_row(&entry);
    let columns = CSV_HEADER.split(',').count();
    assert_eq!(row.split(',').count(), columns, "{row}");
    assert_eq!(
        row,
        "9,1970-01-01T00:00:00.000Z,host-a,,owner,ocfl.commit,succeeded,info,lib-a urn:uuid:x,5,,op-1,urn:uuid:x@v1,00000001-0000-4000-8000-000000000000"
    );
}

#[tokio::test]
async fn every_export_is_journaled_as_journal_export() {
    let host = Host::start();
    host.get("/api/v1/admin/journal/export?format=csv").await;
    host.get("/api/v1/admin/journal/export").await;
    host.monitor.journal().flush();
    let query = JournalQuery {
        operation: Some(EXPORT_OPERATION.to_string()),
        ..JournalQuery::default()
    };
    let exports = host.monitor.store().query(&query).expect("query").entries;
    assert_eq!(exports.len(), 2);
    assert_eq!(exports[0].actor, "owner");
    assert_eq!(exports[0].outcome, Outcome::Succeeded);
    assert!(exports[0].target_ids.contains(&"format:csv".to_string()));
    assert!(exports[1].target_ids.contains(&"format:jsonl".to_string()));
    assert!(exports[0].target_ids.contains(&"entries:6".to_string()));
    for entry in &exports {
        assert_eq!(
            entry,
            &entry.clone().redacted(),
            "journal.export is redacted"
        );
    }
    host.monitor.shutdown();
}

#[tokio::test]
async fn an_unknown_export_format_is_refused() {
    let host = Host::start();
    let (status, _, _) = host.get("/api/v1/admin/journal/export?format=xml").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    host.monitor.shutdown();
}

#[test]
fn the_store_query_filters_match_the_route_filters() {
    let store = MemoryJournal::new("host-a");
    seed(&store);
    let ids = |query: JournalQuery| -> Vec<i64> {
        store
            .query(&query)
            .expect("query")
            .entries
            .iter()
            .filter_map(|e| e.id)
            .collect()
    };
    let library = JournalQuery {
        library_id: Some("lib-b".into()),
        ..JournalQuery::default()
    };
    assert_eq!(ids(library), [5]);
    let object = JournalQuery {
        object_id: Some(object()),
        operation_id: Some("op-3".into()),
        ..JournalQuery::default()
    };
    assert_eq!(ids(object), [3]);
    assert_eq!(
        store.entry(4).expect("entry").expect("found").operation,
        "file.download"
    );
    assert!(store.entry(77).expect("entry").is_none());
}
