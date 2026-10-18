//! Integration tests for the host's IPC gate (Task DE-0.9; DE-S-002, DE-S-003; ADR 002
//! amendment of 2026-10-18): the host owns Tauri's `ipc` URI scheme, refuses every call the
//! policy does not allow before Tauri sees it, hands allowed calls to Tauri with a responder
//! that sees Tauri's own access-control refusals, and journals every refusal with whitelisted,
//! redacted fields only. The Tauri parts run on the mock runtime, so `Webview::on_message` and
//! the runtime authority are Tauri's own code.

use filesquid_api::journal::{JournalEntry, Level, Outcome};
use filesquid_desktop_lib::capability_policy::GRANTED_PERMISSIONS;
use filesquid_desktop_lib::ipc_gate::{
    command_id, decide, forward, handle, refusal_response, Gate, Refusal, Reply, ALLOWED_COMMANDS,
    FAILED_OPERATION, IPC_SCHEME, REFUSED_OPERATION, RESPONSE_HEADER,
};
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

#[path = "../support/mock_app.rs"]
#[allow(dead_code)]
mod mock_app;

use journal::journaled;
use mock_app::{ipc_request, mock_app, APP_ORIGIN};

/// A payload a refused call might carry: a path and a title that must never be journaled.
const SECRET_PAYLOAD: &str = r#"{"path":"/home/owner/Private/holiday.jpg","title":"Secret Title"}"#;

/// Where a test keeps the gate's answer.
type Slot = Arc<Mutex<Option<http::Response<Vec<u8>>>>>;

/// A reply slot and the [`Reply`] that fills it.
fn reply_slot() -> (Slot, Reply) {
    let slot = Arc::new(Mutex::new(None));
    let filler = slot.clone();
    let reply: Reply = Box::new(move |response| {
        *filler.lock().expect("reply slot") = Some(response);
    });
    (slot, reply)
}

/// The answer in `slot`; panics when the gate has not replied.
fn answered(slot: &Slot) -> http::Response<Vec<u8>> {
    slot.lock()
        .expect("reply slot")
        .take()
        .expect("the gate replied")
}

/// The body of `response` as text.
fn body_text(response: &http::Response<Vec<u8>>) -> String {
    String::from_utf8(response.body().clone()).expect("UTF-8 body")
}

/// Every journaled entry as JSON text, for redaction checks.
fn as_json(entries: &[JournalEntry]) -> String {
    serde_json::to_string(entries).expect("entries serialize")
}

/// The single entry with `operation` in `entries`.
fn only(entries: &[JournalEntry], operation: &str) -> JournalEntry {
    let found: Vec<&JournalEntry> = entries
        .iter()
        .filter(|e| e.operation == operation)
        .collect();
    assert_eq!(found.len(), 1, "{entries:?}");
    found[0].clone()
}

#[test]
fn the_gate_owns_the_scheme_tauri_sends_every_invoke_through() {
    assert_eq!(IPC_SCHEME, "ipc");
}

#[test]
fn the_ipc_allowlist_is_exactly_the_granted_event_permissions() {
    let granted: Vec<String> = ALLOWED_COMMANDS
        .iter()
        .map(|cmd| {
            let (plugin, command) = cmd
                .strip_prefix("plugin:")
                .and_then(|rest| rest.split_once('|'))
                .expect("plugin command");
            format!("core:{plugin}:allow-{command}")
        })
        .collect();
    assert_eq!(granted, GRANTED_PERMISSIONS);
}

#[test]
fn an_allowed_call_with_the_invoke_key_is_forwarded_to_tauri() {
    let request = ipc_request("plugin:event|listen", "key-1", "{}");
    match decide(&request, "key-1") {
        Gate::Forward(invocation) => assert_eq!(invocation.cmd, "plugin:event|listen"),
        other => panic!("expected a forward, got {other:?}"),
    }
}

#[test]
fn a_cors_preflight_is_answered_without_a_check() {
    let request = http::Request::builder()
        .method("OPTIONS")
        .uri("ipc://localhost/plugin%3Aevent%7Clisten")
        .body(Vec::new())
        .expect("preflight");
    assert!(matches!(decide(&request, "key-1"), Gate::Preflight));
}

#[test]
fn every_refusal_reason_is_decided_before_tauri_sees_the_call() {
    let get = http::Request::builder()
        .method("GET")
        .uri("ipc://localhost/plugin%3Aevent%7Clisten")
        .header("Origin", APP_ORIGIN)
        .body(Vec::new())
        .expect("GET");
    let mut foreign = ipc_request("plugin:event|listen", "key-1", "{}");
    foreign.headers_mut().insert(
        "Origin",
        http::HeaderValue::from_static("https://evil.example"),
    );
    let mut no_callback = ipc_request("plugin:event|listen", "key-1", "{}");
    no_callback.headers_mut().remove("Tauri-Callback");
    let cases = [
        (get, Refusal::MethodNotAllowed),
        (foreign, Refusal::ForeignOrigin),
        (
            ipc_request("plugin:event|listen", "wrong", "{}"),
            Refusal::InvalidInvokeKey,
        ),
        (no_callback, Refusal::MalformedRequest),
        (
            ipc_request("plugin:fs|read_text_file", "key-1", "{}"),
            Refusal::CommandNotAllowed,
        ),
        (
            ipc_request("plugin:event|emit", "key-1", "{}"),
            Refusal::CommandNotAllowed,
        ),
        (
            ipc_request("health_ping", "key-1", "{}"),
            Refusal::CommandNotAllowed,
        ),
    ];
    for (request, expected) in cases {
        match decide(&request, "key-1") {
            Gate::Refuse(refusal, _) => assert_eq!(refusal, expected, "{}", request.uri()),
            other => panic!("expected {expected:?} for {}, got {other:?}", request.uri()),
        }
    }
}

#[test]
fn refusal_codes_are_opaque_and_distinct() {
    let all = [
        Refusal::MethodNotAllowed,
        Refusal::MalformedRequest,
        Refusal::ForeignOrigin,
        Refusal::InvalidInvokeKey,
        Refusal::CommandNotAllowed,
        Refusal::UnknownWebview,
        Refusal::AclDenied,
        Refusal::NoResponse,
    ];
    let codes: Vec<&str> = all.iter().map(|r| r.code()).collect();
    let mut unique = codes.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), codes.len());
    assert!(
        codes.iter().all(|c| filesquid_api::journal::is_opaque(c)),
        "{codes:?}"
    );
}

#[test]
fn a_command_name_becomes_an_opaque_journal_id_or_unrecognised() {
    assert_eq!(
        command_id("plugin:fs|read_text_file"),
        "plugin:fs.read_text_file"
    );
    assert_eq!(command_id("health_ping"), "health_ping");
    assert_eq!(command_id("../etc/passwd now"), "unrecognised");
    assert_eq!(command_id(&"a".repeat(200)), "unrecognised");
}

#[test]
fn a_refusal_is_answered_in_tauris_wire_format_and_says_not_allowed() {
    let response = refusal_response(Refusal::CommandNotAllowed, Some("plugin:fs|read_text_file"));
    assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
    assert_eq!(response.headers()[RESPONSE_HEADER], "error");
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_eq!(response.headers()["access-control-allow-origin"], "*");
    let message: String = serde_json::from_slice(response.body()).expect("JSON string body");
    assert!(message.contains("not allowed"), "{message}");
    assert!(message.contains("plugin:fs|read_text_file"), "{message}");
}

#[test]
fn a_refused_command_is_journaled_without_its_payload_or_the_invoke_key() {
    let (app, _window) = mock_app();
    let key = app.invoke_key().to_string();
    let (slot, reply) = reply_slot();
    let entries = journaled(|| {
        let request = ipc_request("plugin:fs|read_text_file", &key, SECRET_PAYLOAD);
        handle(app.handle(), "main", request, reply);
    });
    let response = answered(&slot);
    assert_eq!(response.headers()[RESPONSE_HEADER], "error");
    assert!(body_text(&response).contains("not allowed"));
    let entry = only(&entries, REFUSED_OPERATION);
    assert_eq!(entry.outcome, Outcome::Failed);
    assert_eq!(entry.level, Level::Warn);
    assert_eq!(entry.actor, "webview:main");
    assert_eq!(entry.error_code.as_deref(), Some("command_not_allowed"));
    assert_eq!(
        entry.target_ids,
        vec!["plugin:fs.read_text_file".to_string()]
    );
    let text = as_json(&entries);
    for secret in ["/home/owner", "holiday", "Secret Title", key.as_str()] {
        assert!(!text.contains(secret), "{secret} leaked into {text}");
    }
}

#[test]
fn a_wrong_invoke_key_is_journaled_without_either_key() {
    let (app, _window) = mock_app();
    let real = app.invoke_key().to_string();
    let (slot, reply) = reply_slot();
    let entries = journaled(|| {
        let request = ipc_request("plugin:event|listen", "presented-key-123", "{}");
        handle(app.handle(), "main", request, reply);
    });
    assert_eq!(answered(&slot).headers()[RESPONSE_HEADER], "error");
    let entry = only(&entries, REFUSED_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("invalid_invoke_key"));
    let text = as_json(&entries);
    assert!(
        !text.contains("presented-key-123") && !text.contains(&real),
        "{text}"
    );
}

#[test]
fn a_call_from_a_foreign_origin_is_journaled_without_the_address() {
    let (app, _window) = mock_app();
    let key = app.invoke_key().to_string();
    let (slot, reply) = reply_slot();
    let entries = journaled(|| {
        let mut request = ipc_request("plugin:event|listen", &key, "{}");
        request.headers_mut().insert(
            "Origin",
            http::HeaderValue::from_static("https://evil.example"),
        );
        handle(app.handle(), "main", request, reply);
    });
    assert_eq!(answered(&slot).status(), http::StatusCode::FORBIDDEN);
    let entry = only(&entries, REFUSED_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("foreign_origin"));
    assert!(!as_json(&entries).contains("evil.example"));
}

#[test]
fn tauris_own_access_control_refusal_of_a_forwarded_call_is_journaled() {
    // The mock context grants nothing, so Tauri refuses even the event commands the host
    // forwards: the refusal Tauri used to return to the page unjournaled.
    let (app, _window) = mock_app();
    let key = app.invoke_key().to_string();
    let (slot, reply) = reply_slot();
    let body = r#"{"event":"filesquid-event","target":{"kind":"Any"},"handler":3}"#;
    let entries = journaled(|| {
        handle(
            app.handle(),
            "main",
            ipc_request("plugin:event|listen", &key, body),
            reply,
        );
    });
    let response = answered(&slot);
    assert_eq!(response.headers()[RESPONSE_HEADER], "error");
    assert!(
        body_text(&response).contains("not allowed"),
        "{}",
        body_text(&response)
    );
    let entry = only(&entries, REFUSED_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("acl_denied"));
    assert_eq!(entry.target_ids, vec!["plugin:event.listen".to_string()]);
    assert!(!as_json(&entries).contains("filesquid-event"));
}

#[test]
fn a_call_for_an_unknown_webview_is_refused_and_journaled() {
    let (app, _window) = mock_app();
    let key = app.invoke_key().to_string();
    let (slot, reply) = reply_slot();
    let entries = journaled(|| {
        handle(
            app.handle(),
            "ghost",
            ipc_request("plugin:event|listen", &key, "{}"),
            reply,
        );
    });
    assert_eq!(answered(&slot).headers()[RESPONSE_HEADER], "error");
    let entry = only(&entries, REFUSED_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("unknown_webview"));
    assert_eq!(entry.actor, "webview:ghost");
}

#[test]
fn a_call_tauri_never_answers_is_journaled_and_answered() {
    // Tauri drops the responder without calling it when the invoke key does not match; the
    // gate checks the key first, so only a direct forward can reach that path.
    let (app, window) = mock_app();
    let (slot, reply) = reply_slot();
    let entries = journaled(|| {
        let request = tauri::webview::InvokeRequest {
            cmd: "plugin:event|listen".into(),
            callback: tauri::ipc::CallbackFn(1),
            error: tauri::ipc::CallbackFn(2),
            url: APP_ORIGIN.parse().expect("origin"),
            body: tauri::ipc::InvokeBody::default(),
            headers: Default::default(),
            invoke_key: "not-the-key".into(),
        };
        let webview = app.get_webview_window("main").expect("main window");
        forward(&webview, request, reply);
    });
    drop(window);
    assert_eq!(answered(&slot).headers()[RESPONSE_HEADER], "error");
    let entry = only(&entries, REFUSED_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("no_response"));
}

#[test]
fn a_malformed_json_body_is_refused_and_journaled() {
    let (app, _window) = mock_app();
    let key = app.invoke_key().to_string();
    let (slot, reply) = reply_slot();
    let entries = journaled(|| {
        handle(
            app.handle(),
            "main",
            ipc_request("plugin:event|listen", &key, "{not json"),
            reply,
        );
    });
    assert_eq!(answered(&slot).status(), http::StatusCode::BAD_REQUEST);
    let entry = only(&entries, REFUSED_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("malformed_request"));
}

#[test]
fn the_failed_operation_names_errors_that_are_not_refusals() {
    assert_eq!(REFUSED_OPERATION, "desktop.ipc.refused");
    assert_eq!(FAILED_OPERATION, "desktop.ipc.failed");
}
