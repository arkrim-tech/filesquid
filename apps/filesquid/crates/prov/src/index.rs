//! In-memory history index (PROV-S-005).
//!
//! [`HistoryIndex`] holds records keyed by activity identifier and answers
//! the activities of an object, an entity, an operation, an agent or a time
//! range. Results are ordered by end time (`prov:endedAtTime`, compared as
//! instants) and then by activity identifier, and come in pages of 1 to
//! [`MAX_PAGE_SIZE`] activities linked by a [`Cursor`]. A page walk touches at
//! most the page size plus one key, so every loop is bounded by the request.
//!
//! The index is a cache: it can always be rebuilt by inserting the records
//! again, and it does no I/O.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use crate::error::IndexError;
use crate::ids::check_activity_id;
use crate::model::{Activity, Record};
use crate::scalar::{Instant, Timestamp};

/// Largest page a [`PageRequest`] may ask for.
pub const MAX_PAGE_SIZE: usize = 1000;

/// Ordering key of an activity: end instant, then activity identifier.
type OrderKey = (Instant, String);

/// The set of activities one query value names.
type KeySet = BTreeSet<OrderKey>;

/// What to list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryQuery {
    /// Activities that committed a version of this OCFL object identifier.
    Object(String),
    /// Activities that generated, used or invalidated this entity identifier.
    Entity(String),
    /// Activities of this operation identifier (one per object version it committed).
    Operation(String),
    /// Activities associated with this agent identifier.
    Agent(String),
    /// Activities that ended at or after `from` and before `to`.
    TimeRange {
        /// Inclusive lower bound of the end time.
        from: Timestamp,
        /// Exclusive upper bound of the end time.
        to: Timestamp,
    },
}

/// Position after the last activity of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    ended: Timestamp,
    activity_id: String,
}

impl Cursor {
    /// Encode the cursor as a token for transport: `<end time> <activity id>`.
    pub fn to_token(&self) -> String {
        format!("{} {}", self.ended, self.activity_id)
    }

    /// Decode a token made by [`Cursor::to_token`].
    ///
    /// # Errors
    /// [`IndexError::InvalidCursor`] when the token is not in that form.
    pub fn from_token(token: &str) -> Result<Self, IndexError> {
        let (ended, activity_id) = token.split_once(' ').ok_or(IndexError::InvalidCursor)?;
        let ended = Timestamp::parse(ended).map_err(|_| IndexError::InvalidCursor)?;
        if !check_activity_id(activity_id) {
            return Err(IndexError::InvalidCursor);
        }
        Ok(Self {
            ended,
            activity_id: activity_id.to_string(),
        })
    }

    /// The ordering key this cursor stands after.
    fn key(&self) -> OrderKey {
        (self.ended.instant(), self.activity_id.clone())
    }
}

/// A page size and an optional cursor to continue after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRequest {
    limit: usize,
    after: Option<Cursor>,
}

impl PageRequest {
    /// Ask for at most `limit` activities after `after` (from the start when `None`).
    ///
    /// # Errors
    /// [`IndexError::PageSize`] unless `1 <= limit <= 1000`.
    pub fn new(limit: usize, after: Option<Cursor>) -> Result<Self, IndexError> {
        if !(1..=MAX_PAGE_SIZE).contains(&limit) {
            return Err(IndexError::PageSize);
        }
        Ok(Self { limit, after })
    }

    /// The page size.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// The cursor the page continues after.
    pub fn after(&self) -> Option<&Cursor> {
        self.after.as_ref()
    }
}

/// One page of activities and the cursor of the next page, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    items: Vec<Activity>,
    next: Option<Cursor>,
}

impl Page {
    /// The activities of this page, in order.
    pub fn items(&self) -> &[Activity] {
        &self.items
    }

    /// The cursor of the next page; `None` on the last page.
    pub fn next(&self) -> Option<&Cursor> {
        self.next.as_ref()
    }
}

/// In-memory index of PROV-O records.
#[derive(Debug, Clone, Default)]
pub struct HistoryIndex {
    records: BTreeMap<String, Record>,
    by_time: KeySet,
    by_object: BTreeMap<String, KeySet>,
    by_entity: BTreeMap<String, KeySet>,
    by_operation: BTreeMap<String, KeySet>,
    by_agent: BTreeMap<String, KeySet>,
}

/// The set that `value` names in `map`, if any.
fn lookup<'a>(map: &'a BTreeMap<String, KeySet>, value: &str) -> Option<&'a KeySet> {
    map.get(value)
}

/// Add `key` to the set that `value` names in `map`.
fn add_key(map: &mut BTreeMap<String, KeySet>, value: &str, key: &OrderKey) {
    map.entry(value.to_string())
        .or_default()
        .insert(key.clone());
}

impl HistoryIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of indexed records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the index holds no record.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The record whose activity identifier is `activity_id`.
    pub fn record(&self, activity_id: &str) -> Option<&Record> {
        self.records.get(activity_id)
    }

    /// Index `record`. Inserting an identical record again changes nothing.
    ///
    /// # Errors
    /// [`IndexError::ConflictingRecord`] when a different record with the
    /// same activity identifier is already indexed.
    pub fn insert(&mut self, record: Record) -> Result<(), IndexError> {
        let activity = record.activity();
        if let Some(existing) = self.records.get(activity.id()) {
            return match existing == &record {
                true => Ok(()),
                false => Err(IndexError::ConflictingRecord),
            };
        }
        let key: OrderKey = (activity.ended_at().instant(), activity.id().to_string());
        self.by_time.insert(key.clone());
        add_key(
            &mut self.by_object,
            activity.object_version().object_id(),
            &key,
        );
        add_key(&mut self.by_operation, activity.operation(), &key);
        for agent in activity.associated_with() {
            add_key(&mut self.by_agent, agent, &key);
        }
        for entity in record.entities() {
            add_key(&mut self.by_entity, entity.id(), &key);
        }
        self.records.insert(key.1.clone(), record);
        Ok(())
    }

    /// One page of the activities `query` names.
    pub fn query(&self, query: &HistoryQuery, request: &PageRequest) -> Page {
        let after = request.after().map(Cursor::key);
        let (set, from, to) = match query {
            HistoryQuery::Object(id) => (lookup(&self.by_object, id), None, None),
            HistoryQuery::Entity(id) => (lookup(&self.by_entity, id), None, None),
            HistoryQuery::Operation(id) => (lookup(&self.by_operation, id), None, None),
            HistoryQuery::Agent(id) => (lookup(&self.by_agent, id), None, None),
            HistoryQuery::TimeRange { from, to } => (
                Some(&self.by_time),
                Some(from.instant()),
                Some(to.instant()),
            ),
        };
        let Some(set) = set else {
            return Page {
                items: Vec::new(),
                next: None,
            };
        };
        self.page(set, start_bound(after, from), to, request.limit())
    }

    /// Read up to `limit` activities of `set` from `start`, ending before `to`.
    fn page(
        &self,
        set: &KeySet,
        start: Bound<OrderKey>,
        to: Option<Instant>,
        limit: usize,
    ) -> Page {
        let mut keys = set
            .range((start, Bound::Unbounded))
            .take_while(|(ended, _)| to.is_none_or(|to| *ended < to));
        let items: Vec<Activity> = keys
            .by_ref()
            .take(limit)
            .filter_map(|(_, id)| self.records.get(id))
            .map(|record| record.activity().clone())
            .collect();
        let more = keys.next().is_some();
        let next = items.last().filter(|_| more).map(|last| Cursor {
            ended: last.ended_at().clone(),
            activity_id: last.id().to_string(),
        });
        Page { items, next }
    }
}

/// The first key a page may hold: after the cursor, and not before `from`.
fn start_bound(after: Option<OrderKey>, from: Option<Instant>) -> Bound<OrderKey> {
    let from_key = from.map(|instant| (instant, String::new()));
    match (after, from_key) {
        (Some(after), Some(from)) if from > after => Bound::Included(from),
        (Some(after), _) => Bound::Excluded(after),
        (None, Some(from)) => Bound::Included(from),
        (None, None) => Bound::Unbounded,
    }
}
