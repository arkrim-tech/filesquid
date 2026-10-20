//! The `navigation.change` event (API-S-021): after a committed change adds, removes, renames
//! or moves a node, one event names the parents whose children changed, as handles (1 to 64),
//! or `all` when more than 64 changed. Events are hints: a client rereads the named parents'
//! levels, or the whole visible tree on `all` or `events.resync`.
//!
//! The bus is unscoped, so a host forwards each subscriber's events through
//! [`crate::events::EventBus::subscription_filtered`] with
//! [`super::NavigationService::event_filter`], which leaves only the handles that subscriber
//! may read and drops an event that names none.

use super::node::NodeRef;
use crate::events::{topic, ApiEvent, EventBus};
use serde::{Deserialize, Serialize};

/// Most parents one event names; past it the event says `all` instead.
pub const MAX_CHANGE_PARENTS: usize = 64;

/// The payload of a `navigation.change` event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NavigationChange {
    /// The handles (1 to 64) of the nodes whose children changed.
    Parents {
        /// The changed parents.
        parents: Vec<String>,
    },
    /// So many changed (or the changed parents are not known) that a client rereads its
    /// whole visible tree.
    All {
        /// Always `true`: any other value does not decode.
        all: AlwaysTrue,
    },
}

/// A boolean that is always `true` on the wire and refuses to decode as anything else, so an
/// `all` change can never say `{"all": false}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlwaysTrue;

impl Serialize for AlwaysTrue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for AlwaysTrue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match bool::deserialize(deserializer)? {
            true => Ok(Self),
            false => Err(serde::de::Error::custom("`all` is only ever true")),
        }
    }
}

impl NavigationChange {
    /// The change naming `handles`: `all` when there are none or more than
    /// [`MAX_CHANGE_PARENTS`]. Repeated handles are named once.
    pub fn parents(mut handles: Vec<String>) -> Self {
        let mut seen = std::collections::HashSet::new();
        handles.retain(|handle| seen.insert(handle.clone()));
        if handles.is_empty() || handles.len() > MAX_CHANGE_PARENTS {
            return Self::everything();
        }
        Self::Parents { parents: handles }
    }

    /// The change that makes every client reread its whole visible tree.
    pub fn everything() -> Self {
        Self::All { all: AlwaysTrue }
    }

    /// The change naming `nodes` as parents. A node with no handle (a folder too deep for
    /// one) cannot be named, so the change becomes `all`.
    pub fn of(nodes: &[NodeRef]) -> Self {
        let handles: Vec<String> = nodes.iter().filter_map(NodeRef::to_handle).collect();
        if handles.len() < nodes.len() {
            return Self::everything();
        }
        Self::parents(handles)
    }

    /// This change as an event.
    pub fn to_event(&self) -> Option<ApiEvent> {
        let data = serde_json::to_value(self).ok()?;
        Some(ApiEvent::new(topic::NAVIGATION_CHANGE, data))
    }
}

/// Publishes `navigation.change` events on the host's event bus after a committed change.
#[derive(Clone, Debug)]
pub struct NavigationPublisher {
    events: EventBus,
}

impl NavigationPublisher {
    /// A publisher on `events`.
    pub fn new(events: EventBus) -> Self {
        Self { events }
    }

    /// Publish one event naming `parents`, the nodes whose children changed.
    pub fn publish(&self, parents: &[NodeRef]) {
        self.send(&NavigationChange::of(parents));
    }

    /// Publish one event that makes clients reread their whole visible tree.
    pub fn publish_all(&self) {
        self.send(&NavigationChange::everything());
    }

    /// Put `change` on the bus. A subscriber-less bus is not an error.
    fn send(&self, change: &NavigationChange) {
        match change.to_event() {
            Some(event) => {
                self.events.publish(event);
            }
            None => tracing::info!(target: super::LOG_TARGET, "navigation change not encodable"),
        }
    }
}
