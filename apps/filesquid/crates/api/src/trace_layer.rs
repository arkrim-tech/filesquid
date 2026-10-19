//! `tracing` layer that journals warnings and errors, plus INFO events emitted under the
//! `filesquid::activity` target, as structured [`JournalEntry`]s.
//!
//! Only whitelisted fields are copied (`operation`, `outcome`, `actor`, `process_id`,
//! `operation_id`, `object_ref`,
//! `target_id`, `error_code`, `duration_ms`). The free-text message and every other field
//! are never journaled; they appear on stdout only. This is the structural redaction rule of
//! FileSquid T-055.

use crate::journal::{JournalEntry, Level, Outcome};
use crate::journal_writer::JournalSender;
use std::sync::{Arc, OnceLock};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// Target prefix for INFO-level activity events that must be journaled.
pub const ACTIVITY_TARGET: &str = "filesquid::activity";

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
            _ => None,
        }
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

impl<S: Subscriber> Layer<S> for JournalLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
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
        let mut fields = Whitelist::default();
        event.record(&mut fields);
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
        let operation = fields
            .operation
            .unwrap_or_else(|| format!("{}.{}", meta.target(), level.as_str()));
        let mut entry = JournalEntry::new(operation, outcome).level(level);
        entry.actor = fields.actor.unwrap_or_else(|| "system".to_string());
        entry.process_id = fields.process_id;
        entry.target_ids = fields.target_id.into_iter().collect();
        entry.error_code = fields.error_code;
        entry.operation_id = fields.operation_id;
        entry.object_ref = fields.object_ref;
        entry.duration_ms = fields.duration_ms;
        sender.send(entry);
    }
}
