//! The activity journal: FileSquid's complete, append-only record of its own activity
//! ([ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
//!
//! Every domain operation, process state change, authentication event, warning and error is
//! recorded as a [`JournalEntry`] in a [`JournalStore`]. The journal holds **structured,
//! whitelisted fields only** (operation, outcome, actor, opaque IDs, error codes); free-text
//! messages never enter it, which is how redaction is enforced structurally.
//!
//! This crate holds the [`JournalStore`] trait and the in-memory [`MemoryJournal`]. Durable
//! stores do storage I/O and live with the hosts ([API-S-006](../../resources/srs.md)): the
//! SQLite store is `filesquid_native::SqliteJournal`.

pub mod commit;
mod memory;

pub use memory::MemoryJournal;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default page size for journal queries.
pub const DEFAULT_PAGE: usize = 100;
/// Largest page size a query may request: the portable bound
/// [`filesquid_core::MAX_PAGE_SIZE`] (CORE-S-005), so one bound applies to every query.
pub const MAX_PAGE: usize = filesquid_core::MAX_PAGE_SIZE as usize;
/// Milliseconds in a day, used by retention.
pub const DAY_MS: i64 = 86_400_000;

/// Severity of a journal entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Normal activity.
    Info,
    /// Something needs attention but the operation continued.
    Warn,
    /// An operation failed.
    Error,
}

impl Level {
    /// Lowercase wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }

    /// Parse a lowercase wire name.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "info" => Some(Level::Info),
            "warn" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        }
    }
}

/// Result of the activity an entry records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// A fact without a success or failure, such as progress or a warning.
    Info,
    /// A process or operation began.
    Started,
    /// An operation or process finished successfully.
    Succeeded,
    /// An operation or process failed.
    Failed,
    /// A process missed its heartbeat deadline.
    Stalled,
    /// A stalled process reported again.
    Resumed,
    /// A process ended before finishing.
    Cancelled,
}

impl Outcome {
    /// Lowercase wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Info => "info",
            Outcome::Started => "started",
            Outcome::Succeeded => "succeeded",
            Outcome::Failed => "failed",
            Outcome::Stalled => "stalled",
            Outcome::Resumed => "resumed",
            Outcome::Cancelled => "cancelled",
        }
    }

    /// Parse a lowercase wire name.
    pub fn parse(text: &str) -> Option<Self> {
        [
            Outcome::Info,
            Outcome::Started,
            Outcome::Succeeded,
            Outcome::Failed,
            Outcome::Stalled,
            Outcome::Resumed,
            Outcome::Cancelled,
        ]
        .into_iter()
        .find(|o| o.as_str() == text)
    }
}

/// One journal record. `time` serializes as RFC 3339 UTC with milliseconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Store-assigned, strictly increasing identifier (absent before the entry is stored).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Event time in Unix milliseconds (UTC).
    #[serde(rename = "time", with = "rfc3339_ms")]
    pub time_ms: i64,
    /// Host that recorded the entry (filled in by the store when empty).
    #[serde(default)]
    pub host: String,
    /// Process or job the entry belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_id: Option<String>,
    /// Who acted: `system`, or an opaque user reference such as `user:<id>`.
    pub actor: String,
    /// Dotted operation name, for example `ocfl.commit` or `process.started`.
    pub operation: String,
    /// Result of the activity.
    pub outcome: Outcome,
    /// Severity.
    pub level: Level,
    /// Opaque identifiers of what was touched. Never titles, paths or contents.
    #[serde(default)]
    pub target_ids: Vec<String>,
    /// Duration of the operation, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Machine-readable error code for failures and warnings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// The operation the entry belongs to: shared by every entry and every
    /// OCFL version one operation produced, so journal rows join PROV-O
    /// records and OCFL versions without timestamps (ADR 040 Decision 6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// The OCFL object version the entry is about, `<object-id>@vN`
    /// (`urn:uuid:<uuid>@vN` for FileSquid objects).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_ref: Option<String>,
}

impl JournalEntry {
    /// Create an entry for `operation` with `outcome`, timestamped now, actor `system`.
    /// A `Failed` outcome defaults to level `error`; everything else to `info`.
    pub fn new(operation: impl Into<String>, outcome: Outcome) -> Self {
        Self {
            id: None,
            time_ms: now_ms(),
            host: String::new(),
            process_id: None,
            actor: "system".to_string(),
            operation: operation.into(),
            outcome,
            level: if outcome == Outcome::Failed {
                Level::Error
            } else {
                Level::Info
            },
            target_ids: Vec::new(),
            duration_ms: None,
            error_code: None,
            operation_id: None,
            object_ref: None,
        }
    }

    /// Set the operation ID the entry belongs to.
    pub fn operation_id(mut self, id: impl Into<String>) -> Self {
        self.operation_id = Some(id.into());
        self
    }

    /// Set the OCFL object version the entry is about (`<object-id>@vN`).
    pub fn object_ref(mut self, object_ref: impl Into<String>) -> Self {
        self.object_ref = Some(object_ref.into());
        self
    }

    /// Set the actor (opaque user reference or `system`).
    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = actor.into();
        self
    }

    /// Add an opaque target identifier.
    pub fn target(mut self, id: impl Into<String>) -> Self {
        self.target_ids.push(id.into());
        self
    }

    /// Set the process or job identifier.
    pub fn process(mut self, id: impl Into<String>) -> Self {
        self.process_id = Some(id.into());
        self
    }

    /// Set the event time (Unix milliseconds, UTC).
    pub fn at_ms(mut self, ms: i64) -> Self {
        self.time_ms = ms;
        self
    }

    /// Set the severity.
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Set a machine-readable error code.
    pub fn error_code(mut self, code: impl Into<String>) -> Self {
        self.error_code = Some(code.into());
        self
    }

    /// Set the duration in milliseconds.
    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// This entry with every identifier field that is not an opaque token replaced by
    /// [`REDACTED`]. Every write path applies it, so a title, path or caption passed by
    /// mistake never reaches the store, the event channel or stdout. The entry itself is
    /// kept, because the journal never drops events.
    pub fn redacted(mut self) -> Self {
        let scrub = |v: &mut String| {
            if !is_opaque(v) {
                *v = REDACTED.to_string();
            }
        };
        scrub(&mut self.actor);
        scrub(&mut self.operation);
        if !self.host.is_empty() {
            scrub(&mut self.host);
        }
        self.process_id.iter_mut().for_each(scrub);
        self.error_code.iter_mut().for_each(scrub);
        self.target_ids.iter_mut().for_each(scrub);
        self.operation_id.iter_mut().for_each(scrub);
        if let Some(object_ref) = &mut self.object_ref {
            if !is_object_ref(object_ref) {
                *object_ref = REDACTED.to_string();
            }
        }
        self
    }
}

/// Most ids one [`JournalStore::allocate_ids`] call hands out.
pub const MAX_ALLOCATED_IDS: usize = 1024;

/// Refuse an allocation above [`MAX_ALLOCATED_IDS`].
///
/// # Errors
/// [`JournalError`] naming the bound.
pub fn check_allocation(count: usize) -> Result<i64, JournalError> {
    if count > MAX_ALLOCATED_IDS {
        return Err(JournalError(format!(
            "at most {MAX_ALLOCATED_IDS} journal ids per allocation"
        )));
    }
    Ok(count as i64)
}

/// Stored in place of a field value that is not an opaque token.
pub const REDACTED: &str = "redacted";

/// Longest identifier kept as is.
pub const MAX_OPAQUE_LEN: usize = 128;

/// True when `value` has the shape of an opaque identifier or code: 1 to
/// [`MAX_OPAQUE_LEN`] ASCII letters, digits, `.`, `_`, `-` or `:`. Titles, captions and file
/// paths contain spaces, slashes or other characters and fail this check.
pub fn is_opaque(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OPAQUE_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
}

/// True when `value` is an object reference `<object-id>@vN`: an opaque
/// object ID ([`is_opaque`]) and an OCFL version name, joined by the one
/// `@` that [`is_opaque`] itself never allows.
pub fn is_object_ref(value: &str) -> bool {
    match filesquid_core::VersionRef::parse_object_ref(value) {
        Ok(reference) => value.len() <= MAX_OPAQUE_LEN && is_opaque(reference.object_id()),
        Err(_) => false,
    }
}

/// Current time in Unix milliseconds (UTC).
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Format Unix milliseconds as RFC 3339 UTC with millisecond precision.
pub fn format_rfc3339_ms(ms: i64) -> String {
    let nanos = i128::from(ms) * 1_000_000;
    let format = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
    );
    time::OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .ok()
        .and_then(|t| t.format(&format).ok())
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string())
}

/// Parse an RFC 3339 timestamp into Unix milliseconds.
pub fn parse_rfc3339_ms(text: &str) -> Option<i64> {
    time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|t| (t.unix_timestamp_nanos() / 1_000_000) as i64)
}

/// Serde adapter: Unix milliseconds <-> RFC 3339 string.
mod rfc3339_ms {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(ms: &i64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::format_rfc3339_ms(*ms))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        let text = String::deserialize(d)?;
        super::parse_rfc3339_ms(&text)
            .ok_or_else(|| serde::de::Error::custom("invalid RFC 3339 time"))
    }
}

/// Filter and page for journal queries. Every filter is optional; `limit` 0 means the default.
#[derive(Clone, Debug, Default)]
pub struct JournalQuery {
    /// Earliest time (inclusive), Unix ms.
    pub from_ms: Option<i64>,
    /// Latest time (inclusive), Unix ms.
    pub to_ms: Option<i64>,
    /// Exact actor.
    pub actor: Option<String>,
    /// Exact operation.
    pub operation: Option<String>,
    /// Exact outcome.
    pub outcome: Option<Outcome>,
    /// Exact level.
    pub level: Option<Level>,
    /// Exact process identifier.
    pub process_id: Option<String>,
    /// Cursor from the previous page: entries after this id in the chosen order
    /// (greater ids when ascending, smaller ids when descending).
    pub after_id: Option<i64>,
    /// Newest first when true (the Monitor dashboard); oldest first otherwise (exports).
    pub descending: bool,
    /// Page size (0 = [`DEFAULT_PAGE`], capped at [`MAX_PAGE`]).
    pub limit: usize,
}

impl JournalQuery {
    /// Page size after applying the default and the cap.
    pub fn effective_limit(&self) -> usize {
        match self.limit {
            0 => DEFAULT_PAGE,
            n => n.min(MAX_PAGE),
        }
    }

    /// True when `entry` lies beyond the cursor in the query's order.
    pub fn past_cursor(&self, entry: &JournalEntry) -> bool {
        match (self.after_id, entry.id) {
            (Some(cursor), Some(id)) if self.descending => id < cursor,
            (Some(cursor), Some(id)) => id > cursor,
            _ => true,
        }
    }

    /// True when `entry` passes every filter except the cursor and limit.
    pub fn matches(&self, entry: &JournalEntry) -> bool {
        self.from_ms.is_none_or(|t| entry.time_ms >= t)
            && self.to_ms.is_none_or(|t| entry.time_ms <= t)
            && self.actor.as_ref().is_none_or(|a| &entry.actor == a)
            && self
                .operation
                .as_ref()
                .is_none_or(|o| &entry.operation == o)
            && self.outcome.is_none_or(|o| entry.outcome == o)
            && self.level.is_none_or(|l| entry.level == l)
            && self
                .process_id
                .as_ref()
                .is_none_or(|p| entry.process_id.as_ref() == Some(p))
    }
}

/// One page of query results.
#[derive(Clone, Debug, Serialize)]
pub struct JournalPage {
    /// Entries in ascending id order.
    pub entries: Vec<JournalEntry>,
    /// Cursor for the next page, when more entries match.
    pub next_cursor: Option<i64>,
}

impl JournalPage {
    /// Build a page from up to `limit + 1` matching entries.
    pub fn from_overfetch(mut entries: Vec<JournalEntry>, limit: usize) -> Self {
        let more = entries.len() > limit;
        entries.truncate(limit);
        let next_cursor = if more {
            entries.last().and_then(|e| e.id)
        } else {
            None
        };
        Self {
            entries,
            next_cursor,
        }
    }
}

/// How long journal entries are kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetentionPolicy {
    /// Keep every entry (the default).
    KeepAll,
    /// Delete entries older than this many days.
    MaxAgeDays(u32),
}

/// Error from a journal store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalError(pub String);

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "journal error: {}", self.0)
    }
}

impl std::error::Error for JournalError {}

/// Durable store for journal entries. Implementations MUST be append-only except for retention.
pub trait JournalStore: Send + Sync {
    /// Host name recorded on entries that do not carry one.
    fn host(&self) -> &str;
    /// Append entries atomically; returns their ids in order. An entry that carries an
    /// `id` (one [`allocate_ids`](Self::allocate_ids) handed out) is stored under that id,
    /// and stored once: appending an id the store already holds stores nothing and returns
    /// the id, so replaying a commit's entries never duplicates them.
    fn append(&self, entries: &[JournalEntry]) -> Result<Vec<i64>, JournalError>;
    /// [`append`](Self::append), also answering for each entry whether this call stored it
    /// (`false` for a preset id the store already held), so a caller announces a replayed
    /// entry once. The default reports every entry as newly stored.
    ///
    /// # Errors
    /// As [`append`](Self::append).
    fn append_new(&self, entries: &[JournalEntry]) -> Result<Vec<(i64, bool)>, JournalError> {
        let ids = self.append(entries)?;
        Ok(ids.into_iter().map(|id| (id, true)).collect())
    }
    /// Hand out `count` ids (at most [`MAX_ALLOCATED_IDS`]), in increasing order, that no
    /// stored entry has and [`append`](Self::append) never assigns, so a caller can name a
    /// commit's entries before writing them (API-S-028; ADR 040 Decision 6). The default
    /// refuses: a store that cannot reserve ids says so.
    ///
    /// # Errors
    /// [`JournalError`] when the store cannot reserve ids or `count` is too large.
    fn allocate_ids(&self, count: usize) -> Result<Vec<i64>, JournalError> {
        let _ = count;
        Err(JournalError(
            "this journal store cannot allocate ids".into(),
        ))
    }
    /// One page of entries matching `query`, in ascending id order.
    fn query(&self, query: &JournalQuery) -> Result<JournalPage, JournalError>;
    /// Number of entries matching `query` (cursor and limit ignored).
    fn count(&self, query: &JournalQuery) -> Result<u64, JournalError>;
    /// Delete entries older than the policy allows and journal that deletion.
    /// Returns the number of entries deleted.
    fn apply_retention(&self, policy: RetentionPolicy, now_ms: i64) -> Result<u64, JournalError>;
    /// Check that the store is reachable.
    fn ping(&self) -> Result<(), JournalError>;
}

/// Cutoff time for a retention policy, or `None` to keep everything.
pub fn retention_cutoff(policy: RetentionPolicy, now_ms: i64) -> Option<i64> {
    match policy {
        RetentionPolicy::KeepAll => None,
        RetentionPolicy::MaxAgeDays(days) => Some(now_ms - i64::from(days) * DAY_MS),
    }
}

/// The entry recorded when retention deletes rows.
pub fn retention_entry(deleted: u64, now_ms: i64) -> JournalEntry {
    JournalEntry::new("journal.retention", Outcome::Succeeded)
        .at_ms(now_ms)
        .target(format!("deleted:{deleted}"))
}
