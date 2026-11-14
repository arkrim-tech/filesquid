//! Step definitions for `self_scoped_journal_API-S-031.feature` (Task API-F2.27; API-S-031
//! **Self-scoped journal**), driven through the router of the two-caller host of
//! [`crate::own_journal_fixture`]: "ana" is not an administrator, "ben" is.

use crate::own_journal_fixture::{
    actor_of, entries_of, entry_by, object, seeded, token_of, two_callers, OwnHost, FILE, LOST_FILE,
};
use crate::MonitorWorld;
use axum::http::StatusCode;
use cucumber::{given, then, when};
use filesquid_api::admin::JournalNames;
use filesquid_api::auth::{Authoriser, LaunchToken, Principal};
use filesquid_api::files::NameFuture;
use filesquid_api::journal::{is_opaque, JournalEntry, JournalStore, Outcome};
use serde_json::Value;
use std::sync::Arc;

/// The self-scoped journal route.
const OWN: &str = "/api/v1/journal";
/// The title of the file of the redaction scenario.
const TITLE: &str = "Grandma's birthday.jpg";
/// The path of the file of the redaction scenario.
const PATH: &str = "/home/ana/Pictures/Grandma's birthday.jpg";
/// Most pages the paging step reads before it fails.
const MAX_PAGES: usize = 10;

/// Per-scenario state of the self-scoped journal steps.
#[derive(Default)]
pub struct OwnJournalWorld {
    host: Option<OwnHost>,
    owner_token: String,
    status: Option<StatusCode>,
    page: Value,
    pages: Vec<Value>,
    refusals: Vec<(StatusCode, Value)>,
}

impl OwnJournalWorld {
    fn host(&self) -> &OwnHost {
        self.host.as_ref().expect("a host with a journal")
    }

    /// `GET uri` as the caller holding `token`; keep the status and the body.
    async fn read(&mut self, uri: &str, token: &str) {
        let (status, page) = self.host().get(uri, Some(token)).await;
        self.status = Some(status);
        self.page = page;
    }

    /// Append `entry` to the host's journal store.
    fn append(&self, entry: JournalEntry) {
        self.host().store.append(&[entry]).expect("append");
    }
}

/// Names every file but [`LOST_FILE`], which the caller may no longer read.
struct ReadableNames;

impl JournalNames for ReadableNames {
    fn display_name<'a>(&'a self, _principal: &'a Principal, object_id: &'a str) -> NameFuture<'a> {
        let name = (object_id != object(LOST_FILE)).then(|| "Lisbon 2026.jpg".to_string());
        Box::pin(async move { name })
    }
}

#[given(expr = "own-journal entries by user {string} and by user {string}")]
fn entries_by_two_users(w: &mut MonitorWorld, first: String, second: String) {
    let history = seeded();
    for name in [&first, &second] {
        let actor = actor_of(name);
        assert!(
            history.iter().any(|e| e.actor == actor),
            "entries by {name}"
        );
    }
    w.own_journal.host = Some(OwnHost::start(two_callers(), &history));
}

#[given(expr = "own-journal entries by the desktop owner and by the host itself")]
fn entries_by_owner_and_host(w: &mut MonitorWorld) {
    let token = LaunchToken::generate().expect("token");
    let history = [
        entry_by("system", "host.started", Outcome::Succeeded),
        entry_by("owner", "file.upload", Outcome::Succeeded),
        entry_by("system", "process.started", Outcome::Info),
        entry_by("owner", "file.rename", Outcome::Succeeded),
    ];
    w.own_journal.host = Some(OwnHost::start(Authoriser::desktop_owner(&token), &history));
    w.own_journal.owner_token = token.secret().to_string();
}

#[given(expr = "own-journal entry of {string} about an upload of a file with a title and a path")]
fn entry_with_title_and_path(w: &mut MonitorWorld, name: String) {
    let upload = entry_by(actor_of(&name), "file.upload", Outcome::Succeeded)
        .operation_id("op-titled")
        .object_ref(format!("{}@v1", object(FILE)))
        .target(TITLE)
        .target(PATH)
        .error_code(format!("Bearer {}", token_of(&name)));
    w.own_journal.append(upload);
}

#[given(expr = "own-journal entry of {string} about a file {string} may no longer read")]
fn entry_about_lost_file(w: &mut MonitorWorld, name: String, reader: String) {
    assert_eq!(name, reader);
    let lost = format!("{}@v1", object(LOST_FILE));
    let stored = w.own_journal.host().stored();
    let about =
        |e: &&JournalEntry| e.actor == actor_of(&name) && e.object_ref.as_ref() == Some(&lost);
    assert!(stored.iter().any(|e| about(&e)), "an entry about that file");
    let names = Arc::new(ReadableNames);
    assert!(w.own_journal.host().state.install_journal_names(names));
}

#[when(expr = "own-journal caller {string} reads the self-scoped journal")]
async fn caller_reads(w: &mut MonitorWorld, name: String) {
    w.own_journal.read(OWN, token_of(&name)).await;
}

#[when(expr = "own-journal desktop owner reads the self-scoped journal")]
async fn owner_reads(w: &mut MonitorWorld) {
    let token = w.own_journal.owner_token.clone();
    w.own_journal.read(OWN, &token).await;
}

#[when(expr = "own-journal caller {string} reads the self-scoped journal with names")]
async fn caller_reads_with_names(w: &mut MonitorWorld, name: String) {
    let uri = format!("{OWN}?names=true");
    w.own_journal.read(&uri, token_of(&name)).await;
}

#[when(
    expr = "own-journal caller {string} reads the self-scoped journal filtered by operation {string} one page of {int} at a time"
)]
async fn caller_reads_pages(w: &mut MonitorWorld, name: String, operation: String, size: usize) {
    let mut after = String::new();
    for _ in 0..MAX_PAGES {
        let uri = format!("{OWN}?operation={operation}&limit={size}{after}");
        w.own_journal.read(&uri, token_of(&name)).await;
        assert_eq!(w.own_journal.status, Some(StatusCode::OK));
        let page = w.own_journal.page.clone();
        assert!(entries_of(&page).len() <= size, "at most {size} a page");
        let cursor = page["next_cursor"].as_i64();
        w.own_journal.pages.push(page);
        let Some(cursor) = cursor else {
            return;
        };
        after = format!("&after={cursor}");
    }
    panic!("the journal did not end within {MAX_PAGES} pages");
}

#[when(
    expr = "own-journal caller {string} reads the self-scoped journal filtered by actor {string}"
)]
async fn caller_reads_by_actor(w: &mut MonitorWorld, name: String, other: String) {
    let uri = format!("{OWN}?actor={}", actor_of(&other));
    w.own_journal.read(&uri, token_of(&name)).await;
}

#[when(
    expr = "own-journal is read without a credential and with a credential the host does not accept"
)]
async fn read_without_accepted_credential(w: &mut MonitorWorld) {
    for token in [None, Some("not-a-caller")] {
        let answer = w.own_journal.host().get(OWN, token).await;
        w.own_journal.refusals.push(answer);
    }
}

#[when(expr = "own-journal caller {string} reads the admin journal")]
async fn caller_reads_admin(w: &mut MonitorWorld, name: String) {
    w.own_journal
        .read("/api/v1/admin/journal", token_of(&name))
        .await;
}

#[then(expr = "own-journal answer should hold only entries whose actor is {string}")]
fn only_entries_of(w: &mut MonitorWorld, name: String) {
    assert_eq!(w.own_journal.status, Some(StatusCode::OK));
    let entries = entries_of(&w.own_journal.page);
    let stored = w.own_journal.host().stored();
    let own = stored.iter().filter(|e| e.actor == actor_of(&name)).count();
    assert!(
        own > 0 && own < stored.len(),
        "other callers have entries too"
    );
    assert_eq!(entries.len(), own, "every entry of {name}");
    for entry in entries {
        assert_eq!(entry["actor"], actor_of(&name), "{entry}");
    }
}

#[then(expr = "own-journal answer should hold the owner's entries and no entry of the host itself")]
fn owner_entries_only(w: &mut MonitorWorld) {
    assert_eq!(w.own_journal.status, Some(StatusCode::OK));
    let entries = entries_of(&w.own_journal.page);
    assert_eq!(entries.len(), 2, "{entries:?}");
    for entry in entries {
        assert_eq!(entry["actor"], "owner", "{entry}");
    }
}

#[then(
    expr = "own-journal pages should hold only {string}'s file.upload entries, with no repeat or gap"
)]
fn pages_are_complete(w: &mut MonitorWorld, name: String) {
    let read: Vec<i64> = w
        .own_journal
        .pages
        .iter()
        .flat_map(|page| entries_of(page).iter())
        .map(|entry| {
            assert_eq!(entry["actor"], actor_of(&name), "{entry}");
            assert_eq!(entry["operation"], "file.upload", "{entry}");
            entry["id"].as_i64().expect("id")
        })
        .collect();
    let expected: Vec<i64> = w
        .own_journal
        .host()
        .stored()
        .iter()
        .filter(|e| e.actor == actor_of(&name) && e.operation == "file.upload")
        .filter_map(|e| e.id)
        .collect();
    assert!(w.own_journal.pages.len() > 1, "more than one page");
    assert_eq!(read, expected);
}

#[then(expr = "own-journal answer should be refused as forbidden and journaled as api.auth.denied")]
fn refused_and_journaled(w: &mut MonitorWorld) {
    assert_eq!(w.own_journal.status, Some(StatusCode::FORBIDDEN));
    assert_eq!(w.own_journal.page["code"], "forbidden");
    assert_eq!(w.own_journal.page["reason"], "not_permitted");
    let stored = w.own_journal.host().stored();
    let refusal = stored.last().expect("the refusal");
    assert_eq!(refusal.operation, "api.auth.denied");
    assert_eq!(refusal.actor, actor_of("ana"));
    assert_eq!(refusal.target_ids, ["monitor.read"]);
}

#[then(
    expr = "own-journal entry should carry opaque IDs only, with no title, path, content or credential, exactly as the admin journal route would answer it"
)]
async fn entry_is_redacted(w: &mut MonitorWorld) {
    let own = w.own_journal.page.clone();
    let text = own.to_string();
    for secret in [TITLE, PATH, "Grandma", "Pictures", token_of("ana")] {
        assert!(!text.contains(secret), "{secret} in {text}");
    }
    let titled = entries_of(&own).last().expect("the upload");
    assert_eq!(titled["operation_id"], "op-titled");
    for target in titled["target_ids"].as_array().expect("targets") {
        assert!(is_opaque(target.as_str().expect("text")), "{target}");
    }
    let as_admin = format!("/api/v1/admin/journal?actor={}", actor_of("ana"));
    let (status, admin) = w
        .own_journal
        .host()
        .get(&as_admin, Some(token_of("ben")))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(own, admin);
}

#[then(
    expr = "own-journal answer should be refused as an invalid request naming the actor parameter"
)]
fn refused_actor_filter(w: &mut MonitorWorld) {
    assert_eq!(w.own_journal.status, Some(StatusCode::BAD_REQUEST));
    assert_eq!(w.own_journal.page["code"], "invalid_request");
    assert_eq!(
        w.own_journal.page["message"],
        "Invalid query parameter: actor."
    );
    assert!(w.own_journal.page.get("entries").is_none());
}

#[then(expr = "own-journal answers should be unauthorized and forbidden, with no entry")]
fn refused_without_credential(w: &mut MonitorWorld) {
    let refusals = &w.own_journal.refusals;
    let expected = [
        (StatusCode::UNAUTHORIZED, "missing_credentials"),
        (StatusCode::FORBIDDEN, "invalid_credentials"),
    ];
    assert_eq!(refusals.len(), expected.len());
    for ((status, body), (code, reason)) in refusals.iter().zip(expected) {
        assert_eq!(*status, code, "{body}");
        assert_eq!(body["reason"], reason, "{body}");
        assert!(body.get("entries").is_none(), "{body}");
    }
}

#[then(expr = "own-journal entry should carry no name for that file")]
fn no_name_for_lost_file(w: &mut MonitorWorld) {
    assert_eq!(w.own_journal.status, Some(StatusCode::OK));
    let lost = format!("{}@v1", object(LOST_FILE));
    let readable = format!("{}@v1", object(FILE));
    let entries = entries_of(&w.own_journal.page);
    let about = |object_ref: &str| {
        let found = entries.iter().find(|e| e["object_ref"] == object_ref);
        found.expect("an entry about the file").clone()
    };
    assert!(about(&lost).get("display_name").is_none());
    assert_eq!(about(&readable)["display_name"], "Lisbon 2026.jpg");
}
