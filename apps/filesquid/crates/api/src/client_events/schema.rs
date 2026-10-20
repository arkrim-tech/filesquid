//! The client-events request schema (API-S-030): parse and check one batch. Pure; no I/O.
//!
//! A request body is `{"events": [...]}`. A console event is
//! `{"kind": "console", "id", "level", "component", "error_code", "ids"?, "time"?,
//! "principal"?}`; an overflow event is `{"kind": "overflow", "id", "component", "count",
//! "principal"?}` and nothing else. Every identifier is checked to be an opaque token, the
//! ID to be a UUID v4. A console event's other fields (a message, URL, file name or stack)
//! are never read; an overflow event with any other field is refused.

use serde_json::{Map, Value};

use super::{
    ERROR_OPERATION, MAX_CLOCK_SKEW_MS, MAX_EVENTS, MAX_EVENT_IDS, MAX_OVERFLOW_COUNT,
    OVERFLOW_OPERATION, WARN_OPERATION,
};
use crate::journal::{is_opaque, parse_rfc3339_ms, JournalEntry, Level, Outcome};

/// Fields an overflow event may carry.
const OVERFLOW_FIELDS: [&str; 5] = ["kind", "id", "component", "count", "principal"];

/// Level of a console event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleLevel {
    /// A console warning (`ui.console.warn`).
    Warn,
    /// A console error (`ui.console.error`).
    Error,
}

/// One checked client event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientEvent {
    /// A console warning or error.
    Console {
        /// Client event ID (UUID v4).
        id: String,
        /// Warning or error.
        level: ConsoleLevel,
        /// The fixed identifier of the UI component that raised it.
        component: String,
        /// Machine-readable error code.
        error_code: String,
        /// Opaque IDs of what it concerns.
        ids: Vec<String>,
        /// When it was raised (Unix ms, UTC), when the client said.
        time_ms: Option<i64>,
        /// The principal that raised it, when the client stored it.
        principal: Option<String>,
    },
    /// The page-close count of events the client could not keep.
    Overflow {
        /// Client event ID (UUID v4).
        id: String,
        /// The fixed identifier of the client's queue component.
        component: String,
        /// Events not kept, 1 to [`MAX_OVERFLOW_COUNT`].
        count: u64,
        /// The principal that raised it, when the client stored it.
        principal: Option<String>,
    },
}

impl ClientEvent {
    /// The client event ID.
    pub fn id(&self) -> &str {
        match self {
            ClientEvent::Console { id, .. } | ClientEvent::Overflow { id, .. } => id,
        }
    }

    /// The principal that raised the event, when the client named it.
    pub fn principal(&self) -> Option<&str> {
        match self {
            ClientEvent::Console { principal, .. } | ClientEvent::Overflow { principal, .. } => {
                principal.as_deref()
            }
        }
    }

    /// The journal entry of this event, recorded for `actor` at `now_ms` unless the event
    /// carries its own time. One entry per event; never merged with another.
    pub fn to_entry(&self, actor: &str, now_ms: i64) -> JournalEntry {
        match self {
            ClientEvent::Console {
                id,
                level,
                component,
                error_code,
                ids,
                time_ms,
                ..
            } => console_entry(*level, component, ids)
                .actor(actor)
                .at_ms(time_ms.unwrap_or(now_ms))
                .error_code(error_code.as_str())
                .client_event(id.as_str()),
            ClientEvent::Overflow {
                id,
                component,
                count,
                ..
            } => JournalEntry::new(OVERFLOW_OPERATION, Outcome::Info)
                .level(Level::Warn)
                .actor(actor)
                .at_ms(now_ms)
                .target(format!("component:{component}"))
                .target(format!("count:{count}"))
                .client_event(id.as_str()),
        }
    }
}

/// A console entry without actor, time, error code and ID.
fn console_entry(level: ConsoleLevel, component: &str, ids: &[String]) -> JournalEntry {
    let (operation, level) = match level {
        ConsoleLevel::Warn => (WARN_OPERATION, Level::Warn),
        ConsoleLevel::Error => (ERROR_OPERATION, Level::Error),
    };
    let entry = JournalEntry::new(operation, Outcome::Info)
        .level(level)
        .target(format!("component:{component}"));
    ids.iter()
        .fold(entry, |entry, id| entry.target(id.as_str()))
}

/// One checked request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    /// The events, in request order.
    pub events: Vec<ClientEvent>,
}

impl Batch {
    /// True when the request carries only one overflow event: it is exempt from the rate
    /// bound.
    pub fn is_lone_overflow(&self) -> bool {
        matches!(self.events.as_slice(), [ClientEvent::Overflow { .. }])
    }
}

/// Why a batch was refused; the message names the field, never its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchError(pub String);

impl BatchError {
    fn field(index: usize, name: &str) -> Self {
        Self(format!("Event {index} has an invalid {name}."))
    }
}

/// Parse and check `body` at host time `now_ms`. Pure.
///
/// # Errors
/// [`BatchError`] for malformed JSON, no events or more than [`MAX_EVENTS`], or any event
/// that breaks its schema.
pub fn parse_batch(body: &[u8], now_ms: i64) -> Result<Batch, BatchError> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| BatchError("The request body is not a JSON object.".into()))?;
    let events = value
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| BatchError("The request has no events array.".into()))?;
    if events.is_empty() || events.len() > MAX_EVENTS {
        return Err(BatchError(format!(
            "A request carries 1 to {MAX_EVENTS} events."
        )));
    }
    let events = events
        .iter()
        .enumerate()
        .map(|(index, event)| parse_event(index, event, now_ms))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Batch { events })
}

/// Check one event.
fn parse_event(index: usize, value: &Value, now_ms: i64) -> Result<ClientEvent, BatchError> {
    let object = value
        .as_object()
        .ok_or_else(|| BatchError::field(index, "shape"))?;
    match object.get("kind").and_then(Value::as_str) {
        Some("console") => parse_console(index, object, now_ms),
        Some("overflow") => parse_overflow(index, object),
        _ => Err(BatchError::field(index, "kind")),
    }
}

/// Check a console event; fields outside its schema are not read.
fn parse_console(
    index: usize,
    object: &Map<String, Value>,
    now_ms: i64,
) -> Result<ClientEvent, BatchError> {
    let level = match object.get("level").and_then(Value::as_str) {
        Some("warn") => ConsoleLevel::Warn,
        Some("error") => ConsoleLevel::Error,
        _ => return Err(BatchError::field(index, "level")),
    };
    Ok(ClientEvent::Console {
        id: event_id(index, object)?,
        level,
        component: opaque(index, object, "component")?,
        error_code: opaque(index, object, "error_code")?,
        ids: opaque_ids(index, object)?,
        time_ms: event_time(index, object, now_ms)?,
        principal: principal(index, object)?,
    })
}

/// Check an overflow event: its own schema and nothing else.
fn parse_overflow(index: usize, object: &Map<String, Value>) -> Result<ClientEvent, BatchError> {
    if object
        .keys()
        .any(|k| !OVERFLOW_FIELDS.contains(&k.as_str()))
    {
        return Err(BatchError::field(index, "overflow field"));
    }
    let count = object
        .get("count")
        .and_then(Value::as_u64)
        .filter(|n| (1..=MAX_OVERFLOW_COUNT).contains(n))
        .ok_or_else(|| BatchError::field(index, "count"))?;
    Ok(ClientEvent::Overflow {
        id: event_id(index, object)?,
        component: opaque(index, object, "component")?,
        count,
        principal: principal(index, object)?,
    })
}

/// The required opaque string field `name`.
fn opaque(index: usize, object: &Map<String, Value>, name: &str) -> Result<String, BatchError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|v| is_opaque(v))
        .map(str::to_string)
        .ok_or_else(|| BatchError::field(index, name))
}

/// The required client event ID: a UUID v4.
fn event_id(index: usize, object: &Map<String, Value>) -> Result<String, BatchError> {
    object
        .get("id")
        .and_then(Value::as_str)
        .filter(|v| is_uuid_v4(v))
        .map(str::to_string)
        .ok_or_else(|| BatchError::field(index, "id"))
}

/// The optional opaque IDs, at most [`MAX_EVENT_IDS`].
fn opaque_ids(index: usize, object: &Map<String, Value>) -> Result<Vec<String>, BatchError> {
    let Some(value) = object.get("ids") else {
        return Ok(Vec::new());
    };
    let ids = value
        .as_array()
        .filter(|ids| ids.len() <= MAX_EVENT_IDS)
        .ok_or_else(|| BatchError::field(index, "ids"))?;
    ids.iter()
        .map(|id| id.as_str().filter(|v| is_opaque(v)).map(str::to_string))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| BatchError::field(index, "ids"))
}

/// The optional RFC 3339 event time, at most [`MAX_CLOCK_SKEW_MS`] ahead of `now_ms`.
fn event_time(
    index: usize,
    object: &Map<String, Value>,
    now_ms: i64,
) -> Result<Option<i64>, BatchError> {
    let Some(value) = object.get("time") else {
        return Ok(None);
    };
    value
        .as_str()
        .and_then(parse_rfc3339_ms)
        .filter(|t| *t <= now_ms + MAX_CLOCK_SKEW_MS)
        .map(Some)
        .ok_or_else(|| BatchError::field(index, "time"))
}

/// The optional opaque principal.
fn principal(index: usize, object: &Map<String, Value>) -> Result<Option<String>, BatchError> {
    match object.get("principal") {
        None => Ok(None),
        Some(_) => opaque(index, object, "principal").map(Some),
    }
}

/// True for a UUID in its hyphenated form: 8-4-4-4-12 hexadecimal digits. Pure.
pub fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    let hyphens = [8, 13, 18, 23];
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| {
            if hyphens.contains(&i) {
                *b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

/// True for a UUID v4 in its hyphenated form ([`is_uuid`]) with version 4 and the RFC 9562
/// variant (`8`, `9`, `a` or `b`). Pure.
pub fn is_uuid_v4(text: &str) -> bool {
    let bytes = text.as_bytes();
    is_uuid(text)
        && bytes[14] == b'4'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_v4_shapes() {
        assert!(is_uuid_v4("5f0c3e44-2a7b-4c1d-9e8f-0123456789ab"));
        assert!(is_uuid_v4("5F0C3E44-2A7B-4C1D-AE8F-0123456789AB"));
        assert!(
            !is_uuid_v4("5f0c3e44-2a7b-1c1d-9e8f-0123456789ab"),
            "version 1"
        );
        assert!(
            !is_uuid_v4("5f0c3e44-2a7b-4c1d-7e8f-0123456789ab"),
            "variant"
        );
        assert!(
            !is_uuid_v4("5f0c3e442a7b4c1d9e8f0123456789ab"),
            "no hyphens"
        );
        assert!(
            !is_uuid_v4("5f0c3e44-2a7b-4c1d-9e8f-0123456789ag"),
            "not hex"
        );
    }

    #[test]
    fn a_lone_overflow_is_recognised() {
        let id = "5f0c3e44-2a7b-4c1d-9e8f-0123456789ab";
        let overflow = format!(
            r#"{{"events":[{{"kind":"overflow","id":"{id}","component":"q","count":2}}]}}"#
        );
        assert!(parse_batch(overflow.as_bytes(), 0)
            .expect("batch")
            .is_lone_overflow());
    }
}
