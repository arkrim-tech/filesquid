//! Integration tests for the desktop host's other journaled events (Task DE-0.9; DE-S-002,
//! DE-S-003, DE-S-005; ADR 002 amendment of 2026-10-18): Tauri's own IPC refusals on the
//! `postMessage` path and its invoke-key check, refused new windows, permission requests and
//! downloads, window lifecycle, panics and a window that cannot be created. Each event type
//! has a redaction test: no address, path, payload, title or key reaches the journal.

use filesquid_api::journal::{
    JournalEntry, JournalQuery, JournalStore, Level, MemoryJournal, Outcome,
};
use filesquid_api::journal_writer::{JournalWriter, WriterConfig};
use filesquid_api::metrics::Metrics;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::EventBus;
use filesquid_desktop_lib::app_window::{
    create_main_window, download_allowed, host_built_windows, journal_download_finished,
    journal_window_closed, permission_id, refuse_new_window, refuse_permission,
    screen_download_request, CLOSED_OPERATION, CREATE_FAILED_OPERATION, DOWNLOAD_OPERATION,
    DOWNLOAD_REFUSED_OPERATION, NEW_WINDOW_REFUSED_OPERATION, OPENED_OPERATION,
    PERMISSION_REFUSED_OPERATION,
};
use filesquid_desktop_lib::host_subscriber;
use filesquid_desktop_lib::ipc_observer::TauriIpcObserver;
use filesquid_desktop_lib::panic_journal::{install_panic_hook, PANIC_OPERATION};
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::webview::PermissionKind;
use tauri::{Manager, Url};

#[path = "../support/journal.rs"]
#[allow(dead_code)]
mod journal;

#[path = "../support/mock_app.rs"]
#[allow(dead_code)]
mod mock_app;

use journal::{all_entries, journaled};
use mock_app::{mock_app, APP_ORIGIN};

/// Serialises the two panic tests: the hook and the journal it flushes are process-wide.
static PANIC_TESTS: Mutex<()> = Mutex::new(());

/// Captured stdout of the host subscriber.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("captured stdout")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("captured stdout").clone()).expect("UTF-8")
    }
}

/// Run `body` under the host's real subscriber stack (stdout JSON, journal layer and Tauri IPC
/// observer, with the host's filters), all attached to an in-memory journal. Returns the
/// journaled entries and the stdout text.
fn journaled_host(body: impl FnOnce()) -> (Vec<JournalEntry>, String) {
    let store = Arc::new(MemoryJournal::new("desktop"));
    let writer = JournalWriter::start(
        store.clone(),
        WriterConfig { capacity: 16 },
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let layer = JournalLayer::new();
    layer.attach(writer.sender());
    let observer = TauriIpcObserver::new();
    observer.attach(writer.sender());
    let stdout = Captured::default();
    let sink = stdout.clone();
    let filter = tracing_subscriber::EnvFilter::new("trace");
    let subscriber = host_subscriber(layer, observer, move || sink.clone(), filter);
    tracing::subscriber::with_default(subscriber, body);
    writer.sender().flush();
    let entries = all_entries(store.as_ref());
    writer.shutdown();
    (entries, stdout.text())
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

/// An invoke request with a wrong key, as a page that calls `window.ipc.postMessage` directly
/// sends it: it never carries the key Tauri hides in its own initialization script.
fn forged_request(cmd: &str) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: cmd.into(),
        callback: tauri::ipc::CallbackFn(1),
        error: tauri::ipc::CallbackFn(2),
        url: APP_ORIGIN.parse().expect("origin"),
        body: tauri::ipc::InvokeBody::Json(serde_json::json!({ "path": "/home/owner/secret.txt" })),
        headers: Default::default(),
        invoke_key: "forged-key-456".into(),
    }
}

#[test]
fn tauris_invoke_key_refusal_is_journaled_once_and_the_key_stays_out_of_the_logs() {
    let (app, _window) = mock_app();
    let real = app.invoke_key().to_string();
    let webview = app.get_webview_window("main").expect("main window");
    let (entries, stdout) = journaled_host(|| {
        webview.on_message(
            forged_request("plugin:fs|read_text_file"),
            Box::new(|_, _, _, _, _| {}),
        );
    });
    let entry = only(&entries, "desktop.ipc.refused");
    assert_eq!(entry.error_code.as_deref(), Some("invalid_invoke_key"));
    assert_eq!(entry.outcome, Outcome::Failed);
    assert_eq!(entry.level, Level::Warn);
    assert!(
        entries
            .iter()
            .all(|e| e.operation != "tauri::webview.error"),
        "{entries:?}"
    );
    for text in [as_json(&entries), stdout] {
        for secret in [real.as_str(), "forged-key-456", "/home/owner"] {
            assert!(!text.contains(secret), "{secret} leaked into {text}");
        }
    }
}

#[test]
fn a_refusal_tauri_reports_on_its_post_message_path_is_journaled_with_the_command() {
    // Tauri's `postMessage` handler (`tauri::ipc::protocol`) opens these spans; the observer
    // reads the command and the refused answer, never the request body.
    let (entries, stdout) = journaled_host(|| {
        let request = tracing::trace_span!(
            target: "tauri::ipc::protocol",
            "ipc::request",
            kind = "post-message",
            request = r#"{"path":"/home/owner/secret.txt"}"#
        )
        .entered();
        let handle = tracing::trace_span!(
            target: "tauri::ipc::protocol",
            "ipc::request::handle",
            cmd = "plugin:fs|read_text_file".to_string()
        );
        let respond =
            tracing::trace_span!(target: "tauri::ipc::protocol", parent: &handle, "ipc::request::respond")
                .entered();
        let _response = tracing::trace_span!(
            target: "tauri::ipc::protocol",
            "ipc::request::response",
            response = "InvokeError(String(\"Command plugin:fs|read_text_file not allowed by ACL\"))",
            mime_type = "application/json"
        )
        .entered();
        drop(respond);
        drop(request);
    });
    let entry = only(&entries, "desktop.ipc.refused");
    assert_eq!(entry.error_code.as_deref(), Some("acl_denied"));
    assert_eq!(
        entry.target_ids,
        vec!["plugin:fs.read_text_file".to_string()]
    );
    assert!(!as_json(&entries).contains("/home/owner"));
    assert!(!stdout.contains("/home/owner"), "{stdout}");
}

#[test]
fn a_successful_answer_on_the_post_message_path_is_not_a_refusal() {
    let (entries, _) = journaled_host(|| {
        let handle = tracing::trace_span!(
            target: "tauri::ipc::protocol",
            "ipc::request::handle",
            cmd = "plugin:event|listen".to_string()
        );
        let _respond =
            tracing::trace_span!(target: "tauri::ipc::protocol", parent: &handle, "ipc::request::respond")
                .entered();
        let _response = tracing::trace_span!(
            target: "tauri::ipc::protocol",
            "ipc::request::response",
            response = "7".to_string(),
            mime_type = "application/json"
        )
        .entered();
    });
    assert!(
        entries.iter().all(|e| e.operation != "desktop.ipc.refused"),
        "{entries:?}"
    );
}

#[test]
fn an_unreadable_post_message_call_is_journaled_without_its_text() {
    // Tauri's `postMessage` handler reports a message it cannot parse at TRACE level; the
    // parser's error may quote the body.
    let (entries, stdout) = journaled_host(|| {
        tracing::trace!(
            target: "tauri::ipc::protocol",
            "ipc.request.error expected value at line 1 column 2: /home/owner/secret.txt"
        );
    });
    let entry = only(&entries, "desktop.ipc.refused");
    assert_eq!(entry.error_code.as_deref(), Some("malformed_request"));
    assert!(!as_json(&entries).contains("/home/owner"));
    assert!(!stdout.contains("/home/owner"), "{stdout}");
}

#[test]
fn other_tauri_webview_errors_are_journaled_without_their_text() {
    let (entries, stdout) = journaled_host(|| {
        tracing::error!(
            target: "tauri::webview",
            "data_directory `/home/owner/odd` is not a relative path, ignoring config."
        );
    });
    let entry = only(&entries, "tauri::webview.error");
    assert_eq!(entry.level, Level::Error);
    assert!(!as_json(&entries).contains("/home/owner"));
    assert!(!stdout.contains("/home/owner"), "{stdout}");
}

#[test]
fn ordinary_host_events_still_reach_stdout_and_the_journal() {
    let (entries, stdout) = journaled_host(|| {
        tracing::warn!(
            operation = "desktop.test.warning",
            error_code = "x",
            "a warning"
        );
    });
    assert_eq!(only(&entries, "desktop.test.warning").level, Level::Warn);
    assert!(stdout.contains("desktop.test.warning"), "{stdout}");
}

#[test]
fn a_new_window_is_refused_and_journaled_without_its_address() {
    let entries = journaled(|| refuse_new_window("main"));
    let entry = only(&entries, NEW_WINDOW_REFUSED_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("new_window_denied"));
    assert_eq!(entry.actor, "webview:main");
    assert_eq!(entry.outcome, Outcome::Failed);
    assert_eq!(entry.level, Level::Warn);
}

#[test]
fn a_permission_request_is_refused_and_journaled_by_kind_only() {
    let entries = journaled(|| refuse_permission("main", &PermissionKind::Camera));
    let entry = only(&entries, PERMISSION_REFUSED_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("permission_denied"));
    assert_eq!(entry.target_ids, vec!["camera".to_string()]);
    assert_eq!(permission_id(&PermissionKind::Microphone), "microphone");
    assert_eq!(permission_id(&PermissionKind::Geolocation), "geolocation");
}

#[test]
fn only_app_page_blob_downloads_are_allowed() {
    let allowed = Url::parse("blob:tauri://localhost/2c1f6f3e-6a47-4d4f-9a0a-0e9d1f0d5e11")
        .expect("blob url");
    assert!(download_allowed(&allowed));
    for refused in [
        "https://example.com/file.zip",
        "blob:https://evil.example/2c1f6f3e",
        "data:text/plain,hello",
        "filesquid://localhost/api/v1/content/abc",
        "file:///home/owner/secret.txt",
    ] {
        let url = Url::parse(refused).expect("url");
        assert!(!download_allowed(&url), "{refused}");
    }
}

#[test]
fn a_download_is_journaled_without_its_address_or_file_name() {
    let app_blob = Url::parse("blob:tauri://localhost/2c1f6f3e").expect("blob url");
    let foreign = Url::parse("https://example.com/holiday-photos.zip").expect("url");
    let mut verdicts = Vec::new();
    let entries = journaled(|| {
        verdicts.push(screen_download_request("main", &app_blob));
        verdicts.push(screen_download_request("main", &foreign));
        journal_download_finished("main", true);
        journal_download_finished("main", false);
    });
    assert_eq!(verdicts, vec![true, false]);
    let refused = only(&entries, DOWNLOAD_REFUSED_OPERATION);
    assert_eq!(refused.error_code.as_deref(), Some("download_denied"));
    let outcomes: Vec<Outcome> = entries
        .iter()
        .filter(|e| e.operation == DOWNLOAD_OPERATION)
        .map(|e| e.outcome)
        .collect();
    assert_eq!(
        outcomes,
        vec![Outcome::Started, Outcome::Succeeded, Outcome::Failed]
    );
    let text = as_json(&entries);
    assert!(
        !text.contains("example.com") && !text.contains("holiday"),
        "{text}"
    );
    assert!(!text.contains("2c1f6f3e"), "{text}");
}

#[test]
fn the_host_builds_every_configured_window_itself() {
    let mut context = mock_context::<MockRuntime, _>(noop_assets());
    context.config_mut().app.windows.push(Default::default());
    host_built_windows(context.config_mut());
    assert!(context.config().app.windows.iter().all(|w| !w.create));
}

#[test]
fn the_main_window_is_built_by_the_host_and_its_opening_and_closing_journaled() {
    let mut context = mock_context(noop_assets());
    context.config_mut().app.windows.push(Default::default());
    let app = mock_builder().build(context).expect("mock app");
    let entries = journaled(|| {
        let window = create_main_window(app.handle()).expect("main window");
        assert_eq!(window.label(), "main");
        journal_window_closed("main");
    });
    assert_eq!(
        only(&entries, OPENED_OPERATION).target_ids,
        vec!["main".to_string()]
    );
    assert_eq!(
        only(&entries, CLOSED_OPERATION).target_ids,
        vec!["main".to_string()]
    );
}

#[test]
fn a_main_window_that_cannot_be_built_is_journaled() {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let mut result = None;
    let entries = journaled(|| result = Some(create_main_window(app.handle())));
    assert!(result.expect("ran").is_err());
    let entry = only(&entries, CREATE_FAILED_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("window_create_failed"));
    assert_eq!(entry.level, Level::Error);
}

#[test]
fn a_panic_is_journaled_without_its_message() {
    let _serial = PANIC_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    install_panic_hook(Arc::new(OnceLock::new()));
    let entries = journaled(|| {
        let outcome = std::panic::catch_unwind(|| panic!("Secret Title at /home/owner/x"));
        assert!(outcome.is_err());
    });
    let entry = only(&entries, PANIC_OPERATION);
    assert_eq!(entry.error_code.as_deref(), Some("panic"));
    assert_eq!(entry.level, Level::Error);
    let text = as_json(&entries);
    assert!(
        !text.contains("Secret Title") && !text.contains("/home/owner"),
        "{text}"
    );
}

#[test]
fn the_panic_hook_flushes_the_journal_it_is_given() {
    let _serial = PANIC_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let store = Arc::new(MemoryJournal::new("desktop"));
    let writer = JournalWriter::start(
        store.clone(),
        WriterConfig { capacity: 16 },
        EventBus::new(),
        Arc::new(Metrics::default()),
    );
    let layer = JournalLayer::new();
    layer.attach(writer.sender());
    let slot = Arc::new(OnceLock::new());
    let _ = slot.set(writer.sender());
    install_panic_hook(slot);
    let subscriber =
        tracing_subscriber::layer::SubscriberExt::with(tracing_subscriber::registry(), layer);
    tracing::subscriber::with_default(subscriber, || {
        let _ = std::panic::catch_unwind(|| panic!("boom"));
        // Flushed by the hook: the entry is in the store before this thread flushes anything.
        assert_eq!(store.count(&JournalQuery::default()).expect("count"), 1);
    });
    writer.shutdown();
}
