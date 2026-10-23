//! A `tracing` subscriber for tests that records the fields of every
//! WARN-level event, so a test can assert what an engine warning names
//! (OCFL-S-025). It uses `tracing` alone: spans are accepted and ignored.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

/// The fields of one recorded warning, by name, each as its text (`%`
/// values) or debug text (`?` values); the message is under `message`.
pub type Fields = BTreeMap<String, String>;

/// Records every WARN event's target and fields.
#[derive(Clone, Default)]
pub struct WarningCapture {
    warnings: Arc<Mutex<Vec<(String, Fields)>>>,
}

impl WarningCapture {
    /// Run `body` with this capture as the thread's default subscriber and
    /// return its result.
    pub fn run<T>(&self, body: impl FnOnce() -> T) -> T {
        tracing::subscriber::with_default(self.clone(), body)
    }

    /// Every recorded warning whose message is `message`, as its fields.
    pub fn with_message(&self, message: &str) -> Vec<Fields> {
        let warnings = self.warnings.lock().expect("capture lock");
        warnings
            .iter()
            .map(|(_, fields)| fields)
            .filter(|fields| fields.get("message").map(String::as_str) == Some(message))
            .cloned()
            .collect()
    }

    /// The targets of every recorded warning, in order.
    pub fn targets(&self) -> Vec<String> {
        let warnings = self.warnings.lock().expect("capture lock");
        warnings.iter().map(|(target, _)| target.clone()).collect()
    }
}

/// Collects one event's fields.
struct Collect<'f>(&'f mut Fields);

impl Visit for Collect<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

impl Subscriber for WarningCapture {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.is_span() || *metadata.level() == Level::WARN
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields::new();
        event.record(&mut Collect(&mut fields));
        let target = event.metadata().target().to_string();
        self.warnings
            .lock()
            .expect("capture lock")
            .push((target, fields));
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}
