//! Unit tests for the self-scoped journal route `GET /api/v1/journal` (Task API-F2.27;
//! API-S-031 **Self-scoped journal**, ADR 033): only the caller's own entries, with the
//! filters, cursor paging and redaction of the admin journal route, names only for files
//! the caller may read, and no admin route.

#[path = "../support/own_journal_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use axum::http::StatusCode;
use filesquid_api::admin::JournalNames;
use filesquid_api::auth::{Action, Authoriser, DenyAll, LaunchToken, Principal, Resource};
use filesquid_api::files::NameFuture;
use filesquid_api::journal::{is_opaque, JournalEntry, Outcome};
use filesquid_api::{route_actions, ApiState};
use fixture::{
    actor_of, entries_of, entry_by, object, seeded, two_callers, OwnHost, ANA_TOKEN, BEN_TOKEN,
    FILE, LOST_FILE,
};
use serde_json::Value;
use std::sync::Arc;

/// The self-scoped journal route.
const OWN: &str = "/api/v1/journal";

/// The ids of a page's entries, in page order.
fn ids(page: &Value) -> Vec<i64> {
    entries_of(page)
        .iter()
        .map(|e| e["id"].as_i64().expect("id"))
        .collect()
}

/// A host with the two callers and the seeded history.
fn host() -> OwnHost {
    OwnHost::start(two_callers(), &seeded())
}

#[test]
fn the_route_performs_journal_read_own() {
    assert_eq!(Action::ReadOwnJournal.as_str(), "journal.read_own");
    assert!(Action::ALL.contains(&Action::ReadOwnJournal));
    assert!(route_actions().contains(&(OWN.to_string(), Action::ReadOwnJournal)));
}

#[test]
fn journal_read_own_is_granted_to_every_authenticated_principal_whatever_the_policy() {
    let authoriser = Authoriser::new(Arc::new(DenyAll), Arc::new(DenyAll));
    for principal in [Principal::Owner, Principal::Administrator] {
        assert_eq!(
            authoriser.authorise(&principal, Action::ReadOwnJournal, &Resource::Host),
            Ok(())
        );
        assert!(authoriser
            .authorise(&principal, Action::ReadMonitor, &Resource::Host)
            .is_err());
    }
}

#[tokio::test]
async fn a_caller_reads_only_their_own_entries() {
    let host = host();
    let (status, page) = host.get(OWN, Some(ANA_TOKEN)).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(ids(&page), [1, 3, 6, 9]);
    for entry in entries_of(&page) {
        assert_eq!(entry["actor"], actor_of("ana"), "{entry}");
    }
    let (status, page) = host.get(OWN, Some(BEN_TOKEN)).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(ids(&page), [2, 5, 7]);
    for entry in entries_of(&page) {
        assert_eq!(entry["actor"], actor_of("ben"), "{entry}");
    }
}

#[tokio::test]
async fn the_route_needs_an_accepted_credential() {
    let host = host();
    let (status, body) = host.get(OWN, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["reason"], "missing_credentials");
    let (status, body) = host.get(OWN, Some("wrong")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["reason"], "invalid_credentials");
}

#[tokio::test]
async fn the_admin_filters_apply_within_the_callers_entries() {
    let host = host();
    let read = |query: &'static str| {
        let host = &host;
        async move {
            let (status, page) = host.get(&format!("{OWN}?{query}"), Some(ANA_TOKEN)).await;
            assert_eq!(status, StatusCode::OK, "{query}: {page}");
            ids(&page)
        }
    };
    assert_eq!(read("operation=file.upload").await, [1, 3, 9]);
    assert_eq!(read("outcome=failed").await, [6]);
    assert_eq!(read("level=info").await, [1, 3, 9]);
    assert_eq!(read("level=error").await, [6]);
    assert_eq!(read("library=lib-b").await, [9]);
    assert_eq!(read("operation_id=op-3").await, [3]);
    assert_eq!(read("operation_id=op-2").await, [] as [i64; 0]);
    let by_file = format!("{OWN}?file={FILE}");
    let (_, page) = host.get(&by_file, Some(ANA_TOKEN)).await;
    assert_eq!(ids(&page), [1, 6]);
    let by_object = format!("{OWN}?object={}", object(LOST_FILE));
    let (_, page) = host.get(&by_object, Some(ANA_TOKEN)).await;
    assert_eq!(ids(&page), [3]);
    let future = format!("{OWN}?from=2999-01-01T00:00:00Z");
    let (_, page) = host.get(&future, Some(ANA_TOKEN)).await;
    assert_eq!(ids(&page), [] as [i64; 0]);
    let past = format!("{OWN}?to=2999-01-01T00:00:00Z&order=desc");
    let (_, page) = host.get(&past, Some(ANA_TOKEN)).await;
    assert_eq!(ids(&page), [9, 6, 3, 1]);
}

#[tokio::test]
async fn cursor_paging_walks_the_callers_entries_without_repeat_or_gap() {
    let host = host();
    let first = format!("{OWN}?operation=file.upload&limit=2");
    let (_, page) = host.get(&first, Some(ANA_TOKEN)).await;
    assert_eq!(ids(&page), [1, 3]);
    let cursor = page["next_cursor"].as_i64().expect("cursor");
    let second = format!("{first}&after={cursor}");
    let (_, page) = host.get(&second, Some(ANA_TOKEN)).await;
    assert_eq!(ids(&page), [9]);
    assert_eq!(page["next_cursor"], Value::Null);
}

#[tokio::test]
async fn an_actor_filter_is_refused() {
    let host = host();
    for actor in [actor_of("ben"), actor_of("ana"), "system"] {
        let uri = format!("{OWN}?actor={actor}");
        let (status, body) = host.get(&uri, Some(ANA_TOKEN)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{actor}");
        assert_eq!(body["code"], "invalid_request");
        assert_eq!(body["message"], "Invalid query parameter: actor.");
        assert!(body.get("entries").is_none(), "no entries: {body}");
    }
}

#[tokio::test]
async fn invalid_filter_values_answer_invalid_request_as_on_the_admin_route() {
    let host = host();
    for query in [
        "limit=0",
        "limit=1001",
        "order=sideways",
        "outcome=maybe",
        "level=loud",
        "names=yes",
        "from=yesterday",
        "file=not-a-uuid",
        "after=x",
    ] {
        let uri = format!("{OWN}?{query}");
        let (status, body) = host.get(&uri, Some(ANA_TOKEN)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
        assert_eq!(body["code"], "invalid_request", "{query}");
    }
}

#[tokio::test]
async fn the_route_grants_no_admin_route() {
    let host = host();
    let entry = "/api/v1/admin/journal/entries/2";
    let admin_paths: Vec<String> = route_actions()
        .into_iter()
        .filter(|(_, action)| *action == Action::ReadMonitor)
        .map(|(path, _)| path.replace("/entries/{entry}", "/entries/2"))
        .collect();
    assert!(admin_paths.contains(&entry.to_string()));
    assert!(admin_paths.len() >= 7, "{admin_paths:?}");
    for path in &admin_paths {
        let (status, body) = host.get(path, Some(ANA_TOKEN)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(body["reason"], "not_permitted", "{path}");
        let (status, body) = host.get(path, Some(BEN_TOKEN)).await;
        assert!(
            body.get("reason").is_none(),
            "{path}: the administrator is not refused"
        );
        assert_ne!(status, StatusCode::FORBIDDEN, "{path}");
    }
    let denied: Vec<JournalEntry> = host
        .stored()
        .into_iter()
        .filter(|e| e.operation == "api.auth.denied" && e.actor == actor_of("ana"))
        .collect();
    assert_eq!(denied.len(), admin_paths.len());
    for refusal in &denied {
        assert_eq!(refusal.error_code.as_deref(), Some("not_permitted"));
        assert_eq!(refusal.target_ids, ["monitor.read"]);
    }
}

#[tokio::test]
async fn a_successful_read_journals_nothing() {
    let host = host();
    let before = host.stored().len();
    let (status, _) = host
        .get(&format!("{OWN}?names=true"), Some(ANA_TOKEN))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(host.stored().len(), before);
}

/// The whitelisted members of a journal entry view.
const MEMBERS: [&str; 15] = [
    "id",
    "time",
    "host",
    "process_id",
    "actor",
    "operation",
    "outcome",
    "level",
    "target_ids",
    "duration_ms",
    "error_code",
    "operation_id",
    "object_ref",
    "client_event_id",
    "provenance",
];

#[tokio::test]
async fn the_answer_keeps_the_journals_redaction() {
    let title = "Grandma's birthday.jpg";
    let path = "/home/ana/Pictures/Grandma's birthday.jpg";
    let upload = entry_by(actor_of("ana"), "file.upload", Outcome::Succeeded)
        .operation_id("op-10")
        .object_ref(format!("{}@v1", object(FILE)))
        .target(title)
        .target(path)
        .target("lib-a")
        .error_code("Bearer ana-token");
    let mut history = seeded();
    history.push(upload);
    let host = OwnHost::start(two_callers(), &history);
    let (_, own) = host.get(OWN, Some(ANA_TOKEN)).await;
    let text = own.to_string();
    for secret in [title, path, "Grandma", "Pictures", ANA_TOKEN] {
        assert!(!text.contains(secret), "{secret} in {text}");
    }
    let last = entries_of(&own).last().expect("the upload");
    assert_eq!(
        last["target_ids"],
        serde_json::json!(["redacted", "redacted", "lib-a"])
    );
    assert_eq!(last["error_code"], "redacted");
    for entry in entries_of(&own) {
        for (member, _) in entry.as_object().expect("entry") {
            assert!(MEMBERS.contains(&member.as_str()), "unexpected {member}");
        }
        for target in entry["target_ids"].as_array().expect("targets") {
            assert!(is_opaque(target.as_str().expect("text")), "{target}");
        }
    }
    let as_admin = format!("/api/v1/admin/journal?actor={}", actor_of("ana"));
    let (_, admin) = host.get(&as_admin, Some(BEN_TOKEN)).await;
    assert_eq!(own, admin, "exactly as the admin journal route answers it");
}

/// Names [`FILE`] for "ana" only; it names [`LOST_FILE`] for nobody but the administrator.
struct ReadableNames;

impl JournalNames for ReadableNames {
    fn display_name<'a>(&'a self, principal: &'a Principal, object_id: &'a str) -> NameFuture<'a> {
        let readable = match principal {
            Principal::Administrator => true,
            _ => object_id == object(FILE),
        };
        let name = readable.then(|| format!("name of {object_id}"));
        Box::pin(async move { name })
    }
}

#[tokio::test]
async fn names_are_resolved_on_request_and_only_for_files_the_caller_may_read() {
    let host = host();
    assert!(host.state.install_journal_names(Arc::new(ReadableNames)));
    let (_, plain) = host.get(OWN, Some(ANA_TOKEN)).await;
    assert!(!plain.to_string().contains("display_name"), "{plain}");
    let (_, named) = host
        .get(&format!("{OWN}?names=true"), Some(ANA_TOKEN))
        .await;
    let name = |id: i64| {
        let entry = entries_of(&named).iter().find(|e| e["id"] == id);
        entry.expect("entry")["display_name"].clone()
    };
    assert_eq!(name(1), format!("name of {}", object(FILE)));
    assert_eq!(name(6), format!("name of {}", object(FILE)));
    assert_eq!(name(3), Value::Null, "a file ana may no longer read");
    assert_eq!(name(9), Value::Null, "no object");
    let stored = serde_json::to_string(&host.stored()).expect("json");
    assert!(!stored.contains("name of"), "a name is never stored");
}

#[tokio::test]
async fn the_desktop_owner_reads_the_owners_entries_and_none_of_the_hosts() {
    let token = LaunchToken::generate().expect("token");
    let history = [
        entry_by("system", "host.started", Outcome::Succeeded),
        entry_by("owner", "file.upload", Outcome::Succeeded),
        entry_by("system", "process.started", Outcome::Info),
        entry_by("owner", "file.rename", Outcome::Succeeded),
    ];
    let host = OwnHost::start(Authoriser::desktop_owner(&token), &history);
    let (status, page) = host.get(OWN, Some(token.secret())).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(ids(&page), [2, 4]);
    for entry in entries_of(&page) {
        assert_eq!(entry["actor"], "owner");
    }
    let (status, _) = host
        .get("/api/v1/admin/journal", Some(token.secret()))
        .await;
    assert_eq!(status, StatusCode::OK, "the owner keeps the admin routes");
}

#[tokio::test]
async fn the_route_reports_unavailable_before_the_monitor_starts() {
    let state = ApiState::new();
    state.install_authoriser(two_callers());
    let request = axum::http::Request::get(OWN)
        .header("authorization", format!("Bearer {ANA_TOKEN}"))
        .body(axum::body::Body::empty())
        .expect("request");
    let response = tower::ServiceExt::oneshot(filesquid_api::router(state), request)
        .await
        .expect("answer");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
