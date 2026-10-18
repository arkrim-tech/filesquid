//! Journals the IPC refusals Tauri reports only through its own tracing (Task DE-0.9;
//! DE-S-002, DE-S-003; [ADR 002](../../../../../resources/architecture/002-tauri2-desktop-shell.md)
//! amendment of 2026-10-18).
//!
//! The host's IPC gate (`ipc_gate`) sees every call Tauri's IPC script sends through the `ipc`
//! scheme. Two paths stay inside Tauri, with no hook: wry's `postMessage` handler, which Tauri
//! installs on every webview and which a page script can call, and Tauri's invoke-key check,
//! which drops a call silently. With Tauri's `tracing` feature both report through `tracing`
//! (`tauri` 2.12):
//!
//! - the `postMessage` handler (`tauri::ipc::protocol`) opens the span `ipc::request::handle`
//!   with the command (`cmd`), and under it `ipc::request::response`, whose `response` field is
//!   `InvokeError(...)` when the call was refused;
//! - the same handler emits a TRACE event `ipc.request.error …` when a message cannot be read;
//! - `Webview::on_message` emits an ERROR event on target `tauri::webview` whose message starts
//!   with `__TAURI_INVOKE_KEY__` and names the expected key.
//!
//! [`TauriIpcObserver`] is a `tracing` layer that turns these into journal entries
//! ([`crate::ipc_gate::REFUSED_OPERATION`], actor `webview`): `acl_denied`
//! with the command's opaque id, `malformed_request` or `invalid_invoke_key`. It never reads a
//! request body or journals a message's text. A
//! layer cannot emit `tracing` events while it handles one (they would be dropped), so it
//! writes the journal entry itself and prints the redacted entry to stdout.
//!
//! Everything the observer reads (Tauri's IPC spans and events, and every ERROR event on
//! `tauri::webview`) is handled here and nowhere else: the host's stdout and journal layers
//! skip it ([`outside_observed`]), so neither a request body Tauri records in its IPC spans
//! nor the invoke key in its message reaches stdout or the journal, whatever `RUST_LOG` says.
//! The other webview errors (an invalid window `data_directory`) are journaled as
//! `tauri::webview.error` with the code `webview_error`, without their text.
//!
//! Tauri's trace does not name the webview, so the refusal entries carry the actor `webview`
//! rather than `webview:<label>`; `tauri::webview.error` entries carry the actor `system`.

use crate::ipc_gate::{command_id, Refusal, REFUSED_OPERATION};
use filesquid_api::journal::{JournalEntry, Level as JournalLevel, Outcome};
use filesquid_api::journal_writer::JournalSender;
use std::sync::{Arc, OnceLock};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Target of Tauri's IPC handling spans.
pub const TAURI_IPC_TARGET: &str = "tauri::ipc::protocol";

/// Target of Tauri's webview events, the invoke-key check included.
pub const TAURI_WEBVIEW_TARGET: &str = "tauri::webview";

/// Journal operation of a Tauri webview error other than an invoke-key refusal.
pub const WEBVIEW_ERROR_OPERATION: &str = "tauri::webview.error";

/// Journal actor of a refusal seen in Tauri's trace, which does not name the webview.
pub const OBSERVED_ACTOR: &str = "webview";

/// Span that carries the command of one IPC call.
const HANDLE_SPAN: &str = "ipc::request::handle";

/// Span that carries the answer to one IPC call.
const RESPONSE_SPAN: &str = "ipc::request::response";

/// Start of Tauri's message for a `postMessage` call it cannot read.
const UNREADABLE_MESSAGE: &str = "ipc.request.error";

/// Start of Tauri's invoke-key refusal message.
const INVOKE_KEY_MESSAGE: &str = "__TAURI_INVOKE_KEY__";

/// Start of a refused answer in the `response` field (`format!("{:?}", InvokeError)`).
const REFUSED_ANSWER: &str = "InvokeError(";

/// Where the observer prints each redacted entry as one JSON line.
pub type StdoutSink = Arc<dyn Fn(&str) + Send + Sync>;

/// Journal error code of a Tauri webview error other than an invoke-key refusal.
pub const WEBVIEW_ERROR_CODE: &str = "webview_error";

/// The `tracing` layer that journals Tauri's own IPC refusals. Clone it to keep a handle for
/// [`attach`](Self::attach) after installing it; clones share the journal.
#[derive(Clone)]
pub struct TauriIpcObserver {
    journal: Arc<OnceLock<JournalSender>>,
    stdout: StdoutSink,
}

impl Default for TauriIpcObserver {
    fn default() -> Self {
        Self {
            journal: Arc::new(OnceLock::new()),
            stdout: Arc::new(|line| println!("{line}")),
        }
    }
}

impl TauriIpcObserver {
    /// An observer that prints to the process's stdout and has no journal yet: until
    /// [`attach`](Self::attach), entries go to stdout only.
    pub fn new() -> Self {
        Self::default()
    }

    /// This observer printing its lines through `stdout` instead (the host's stdout writer).
    pub fn with_stdout(self, stdout: StdoutSink) -> Self {
        Self { stdout, ..self }
    }

    /// Attach the journal. Only the first call has an effect.
    pub fn attach(&self, sender: JournalSender) {
        let _ = self.journal.set(sender);
    }

    /// Print the redacted entry to stdout and queue it on the journal. Performs I/O.
    fn record(&self, entry: JournalEntry) {
        let entry = entry.redacted();
        let attached = self.journal.get();
        match serde_json::to_value(&entry) {
            Ok(mut line) => {
                line["journal_unavailable"] = serde_json::Value::Bool(attached.is_none());
                (self.stdout)(&line.to_string());
            }
            Err(err) => eprintln!("journal entry not printable: {err}"),
        }
        if let Some(sender) = attached {
            sender.send(entry);
        }
    }
}

/// True for the spans and events the observer reads: Tauri's IPC spans and ERROR events on
/// `tauri::webview`. Its per-layer filter.
pub fn observed(meta: &Metadata<'_>) -> bool {
    meta.target() == TAURI_IPC_TARGET || is_tauri_webview_error(meta)
}

/// False for what only the observer handles ([`observed`]); the filter of the host's stdout and
/// journal layers.
pub fn outside_observed(meta: &Metadata<'_>) -> bool {
    !observed(meta)
}

/// True for an ERROR event on `tauri::webview`.
fn is_tauri_webview_error(meta: &Metadata<'_>) -> bool {
    meta.is_event() && meta.target() == TAURI_WEBVIEW_TARGET && *meta.level() == Level::ERROR
}

/// Reads one text field of a span or event.
struct TextField {
    name: &'static str,
    value: Option<String>,
}

impl TextField {
    fn named(name: &'static str) -> Self {
        Self { name, value: None }
    }
}

impl Visit for TextField {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == self.name {
            self.value = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == self.name {
            self.value = Some(format!("{value:?}"));
        }
    }
}

/// The command of one IPC call, kept on its `ipc::request::handle` span.
struct IpcCommand(String);

/// The journal entry of an IPC refusal seen in Tauri's trace. Pure.
pub fn observed_refusal(refusal: Refusal, cmd: Option<&str>) -> JournalEntry {
    let entry = JournalEntry::new(REFUSED_OPERATION, Outcome::Failed)
        .level(JournalLevel::Warn)
        .actor(OBSERVED_ACTOR)
        .error_code(refusal.code());
    match cmd {
        Some(cmd) => entry.target(command_id(cmd)),
        None => entry,
    }
}

impl<S> Layer<S> for TauriIpcObserver
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        match attrs.metadata().name() {
            HANDLE_SPAN => remember_command(attrs, id, &ctx),
            RESPONSE_SPAN => self.check_answer(attrs, id, &ctx),
            _ => {}
        }
    }

    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut message = TextField::named("message");
        event.record(&mut message);
        let text = message.value.unwrap_or_default();
        if let Some(entry) = event_entry(event.metadata(), &text) {
            self.record(entry);
        }
    }
}

/// The journal entry for one Tauri event with message `text`, or `None` when it is not one the
/// observer journals: an unreadable `postMessage` call (`malformed_request`), an invoke-key
/// refusal, or another Tauri webview error. Pure; the text itself is never kept.
fn event_entry(meta: &Metadata<'_>, text: &str) -> Option<JournalEntry> {
    if meta.target() == TAURI_IPC_TARGET && text.starts_with(UNREADABLE_MESSAGE) {
        return Some(observed_refusal(Refusal::MalformedRequest, None));
    }
    if !is_tauri_webview_error(meta) {
        return None;
    }
    if text.starts_with(INVOKE_KEY_MESSAGE) {
        return Some(observed_refusal(Refusal::InvalidInvokeKey, None));
    }
    Some(JournalEntry::new(WEBVIEW_ERROR_OPERATION, Outcome::Failed).error_code(WEBVIEW_ERROR_CODE))
}

impl TauriIpcObserver {
    /// Journal the answer of a `ipc::request::response` span when it is a refusal, with the
    /// command of the call's `ipc::request::handle` span.
    fn check_answer<S>(&self, attrs: &Attributes<'_>, id: &Id, ctx: &Context<'_, S>)
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let mut response = TextField::named("response");
        attrs.record(&mut response);
        let refused = response
            .value
            .is_some_and(|answer| answer.starts_with(REFUSED_ANSWER));
        if refused {
            let cmd = call_command(id, ctx);
            self.record(observed_refusal(Refusal::AclDenied, cmd.as_deref()));
        }
    }
}

/// Keep the command of a new `ipc::request::handle` span on the span.
fn remember_command<S>(attrs: &Attributes<'_>, id: &Id, ctx: &Context<'_, S>)
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let mut cmd = TextField::named("cmd");
    attrs.record(&mut cmd);
    if let (Some(cmd), Some(span)) = (cmd.value, ctx.span(id)) {
        span.extensions_mut().insert(IpcCommand(cmd));
    }
}

/// The command kept on the nearest `ipc::request::handle` span above span `id`. Bounded by
/// the span tree's depth.
fn call_command<S>(id: &Id, ctx: &Context<'_, S>) -> Option<String>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    ctx.span_scope(id)?
        .find_map(|span| span.extensions().get::<IpcCommand>().map(|c| c.0.clone()))
}
