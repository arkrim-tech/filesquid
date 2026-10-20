//! `tracing` layer that journals warnings and errors, plus INFO events emitted under the
//! `filesquid::activity` target, as structured [`JournalEntry`]s.
//!
//! Only whitelisted fields are copied (`operation`, `outcome`, `actor`, `process_id`,
//! `operation_id`, `object_ref`,
//! `target_id`, `error_code`, `duration_ms`). The free-text message and every other field
//! are never journaled; they appear on stdout only. This is the structural redaction rule of
//! FileSquid T-055.
//!
//! **Activity spans and engine warnings** (API-S-031). Code that drives the OCFL engine
//! enters an [`activity_span`] naming the operation ID and, when known, the object
//! reference; an event inside it that names neither takes them from the nearest enclosing
//! activity span. A warning or error of the OCFL engine (a `filesquid_ocfl` target, such as
//! its Git, copy and storage-hierarchy guards and lease takeovers) is journaled as
//! `ocfl.engine.<module path>` (for example `ocfl.engine.write.lease`) unless it names an
//! `operation`; its `object` field becomes a target and, with its `version` field, the
//! object reference `<object>@<version>`. Its other fields (paths, holders) stay on stdout.

use crate::journal::{is_object_ref, is_opaque, JournalEntry, Level, Outcome};
use crate::journal_writer::JournalSender;
use std::sync::{Arc, OnceLock};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Target prefix for INFO-level activity events that must be journaled.
pub const ACTIVITY_TARGET: &str = "filesquid::activity";

/// Target prefix of the OCFL engine's events.
pub const ENGINE_TARGET: &str = "filesquid_ocfl";

/// Operation prefix of journaled OCFL engine warnings and errors.
pub const ENGINE_OPERATION_PREFIX: &str = "ocfl.engine";

/// A span under the activity target that names `operation_id` and, when known,
/// `object_ref` (`<object-id>@vN`) for the journal entries of the events inside it. Entering
/// the span journals nothing by itself.
pub fn activity_span(operation_id: &str, object_ref: Option<&str>) -> tracing::Span {
    tracing::info_span!(
        target: ACTIVITY_TARGET,
        "activity",
        operation_id,
        object_ref = object_ref.unwrap_or_default()
    )
}

/// The operation ID and object reference an activity span names, kept in its extensions.
#[derive(Clone, Debug, Default)]
struct SpanLinks {
    operation_id: Option<String>,
    object_ref: Option<String>,
}

impl Visit for SpanLinks {
    fn record_str(&mut self, field: &Field, value: &str) {
        let value = (!value.is_empty()).then(|| value.to_string());
        match field.name() {
            "operation_id" => self.operation_id = value,
            "object_ref" => self.object_ref = value,
            _ => {}
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}
}

/// Layer that forwards journal-worthy events to the journal writer once attached.
#[derive(Clone, Default)]
pub struct JournalLayer {
    sender: Arc<OnceLock<JournalSender>>,
}

impl JournalLayer {
    /// Create a layer with no sender yet (events before [`attach`](Self::attach) go to stdout only).
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach the journal sender. Only the first call has an effect.
    pub fn attach(&self, sender: JournalSender) {
        let _ = self.sender.set(sender);
    }
}

/// Collects the whitelisted fields of one event.
#[derive(Default)]
struct Whitelist {
    operation: Option<String>,
    outcome: Option<String>,
    actor: Option<String>,
    process_id: Option<String>,
    target_id: Option<String>,
    error_code: Option<String>,
    operation_id: Option<String>,
    object_ref: Option<String>,
    duration_ms: Option<u64>,
    /// The engine's `object` field (engine targets only).
    object: Option<String>,
    /// The engine's `version` field (engine targets only).
    version: Option<String>,
    /// Whether the event comes from the OCFL engine, whose `object` and `version` are read.
    engine: bool,
}

impl Whitelist {
    /// The slot for a whitelisted text field, or `None` for every other field.
    fn slot(&mut self, field: &Field) -> Option<&mut Option<String>> {
        match field.name() {
            "operation" => Some(&mut self.operation),
            "outcome" => Some(&mut self.outcome),
            "actor" => Some(&mut self.actor),
            "process_id" => Some(&mut self.process_id),
            "target_id" => Some(&mut self.target_id),
            "error_code" => Some(&mut self.error_code),
            "operation_id" => Some(&mut self.operation_id),
            "object_ref" => Some(&mut self.object_ref),
            "object" if self.engine => Some(&mut self.object),
            "version" if self.engine => Some(&mut self.version),
            _ => None,
        }
    }

    /// The engine event's object reference `<object>@<version>`, when both are well formed.
    fn engine_object_ref(&self) -> Option<String> {
        let reference = format!("{}@{}", self.object.as_deref()?, self.version.as_deref()?);
        is_object_ref(&reference).then_some(reference)
    }
}

/// The journal operation of an engine event at `target`: `ocfl.engine.<module path>`.
fn engine_operation(target: &str) -> String {
    let path = target
        .strip_prefix(ENGINE_TARGET)
        .unwrap_or(target)
        .trim_start_matches("::")
        .replace("::", ".");
    if path.is_empty() {
        ENGINE_OPERATION_PREFIX.to_string()
    } else {
        format!("{ENGINE_OPERATION_PREFIX}.{path}")
    }
}

impl Visit for Whitelist {
    fn record_str(&mut self, field: &Field, value: &str) {
        if let Some(slot) = self.slot(field) {
            *slot = Some(value.to_string());
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "duration_ms" {
            self.duration_ms = Some(value);
        }
    }

    /// `%value` and `?value` fields arrive here. Whitelisted ones are kept as their
    /// formatted text; the journal's redaction check still applies to the result.
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if let Some(slot) = self.slot(field) {
            *slot = Some(format!("{value:?}"));
        }
    }
}

impl<S> Layer<S> for JournalLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if !attrs.metadata().target().starts_with(ACTIVITY_TARGET) {
            return;
        }
        let mut links = SpanLinks::default();
        attrs.record(&mut links);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(links);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut extensions = span.extensions_mut();
        if let Some(links) = extensions.get_mut::<SpanLinks>() {
            values.record(links);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();
        let level = match *meta.level() {
            tracing::Level::ERROR => Level::Error,
            tracing::Level::WARN => Level::Warn,
            tracing::Level::INFO if meta.target().starts_with(ACTIVITY_TARGET) => Level::Info,
            _ => return,
        };
        let Some(sender) = self.sender.get() else {
            return;
        };
        let mut fields = Whitelist {
            engine: meta.target().starts_with(ENGINE_TARGET),
            ..Whitelist::default()
        };
        event.record(&mut fields);
        let links = enclosing_links(event, &ctx);
        sender.send(entry_of(fields, links, meta.target(), level));
    }
}

/// The links of the nearest enclosing activity span of `event` (each field from the nearest
/// span that names it). Bounded by the span depth.
fn enclosing_links<S>(event: &Event<'_>, ctx: &Context<'_, S>) -> SpanLinks
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let mut found = SpanLinks::default();
    let Some(scope) = ctx.event_scope(event) else {
        return found;
    };
    for span in scope {
        if let Some(links) = span.extensions().get::<SpanLinks>() {
            found.operation_id = found.operation_id.or_else(|| links.operation_id.clone());
            found.object_ref = found.object_ref.or_else(|| links.object_ref.clone());
        }
    }
    found
}

/// The journal entry of one event: its whitelisted `fields`, completed by the enclosing
/// span's `links` and, for an engine event, its engine operation and object. Pure.
fn entry_of(fields: Whitelist, links: SpanLinks, target: &str, level: Level) -> JournalEntry {
    let default_outcome = if level == Level::Error {
        Outcome::Failed
    } else {
        Outcome::Info
    };
    let outcome = fields
        .outcome
        .as_deref()
        .and_then(Outcome::parse)
        .unwrap_or(default_outcome);
    let engine_ref = fields.engine_object_ref();
    let operation = match (&fields.operation, fields.engine) {
        (Some(operation), _) => operation.clone(),
        (None, true) => engine_operation(target),
        (None, false) => format!("{}.{}", target, level.as_str()),
    };
    let mut entry = JournalEntry::new(operation, outcome).level(level);
    entry.actor = fields.actor.unwrap_or_else(|| "system".to_string());
    entry.process_id = fields.process_id;
    entry.target_ids = fields.target_id.into_iter().collect();
    if let Some(object) = fields.object.filter(|o| is_opaque(o)) {
        entry.target_ids.push(object);
    }
    entry.error_code = fields.error_code;
    entry.operation_id = fields.operation_id.or(links.operation_id);
    entry.object_ref = fields.object_ref.or(engine_ref).or(links.object_ref);
    entry.duration_ms = fields.duration_ms;
    entry
}
