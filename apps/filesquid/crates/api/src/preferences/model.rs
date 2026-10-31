//! [`Preferences`]: one user's UI preferences as the API stores them (API-S-011, SRS-F-062),
//! with the navigation tree's choices of UI-S-017, and the bounds every saved value meets.
//!
//! The value holds choices only: enumerated settings, opaque node handles with their kinds,
//! and section ids. It never holds a name, title, logical path, token or content handle
//! (UI-S-017): every node is checked to be a handle the navigation service wrote, so nothing
//! else can be stored in its place.

use crate::navigation::NodeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The format version of a stored value.
pub const PREFERENCES_SCHEMA: u32 = 1;

/// The format version of the navigation choices (UI-S-017 `version`).
pub const NAVIGATION_VERSION: u32 = 1;

/// Most pinned destinations (UI-S-017).
pub const MAX_PINNED: usize = 100;

/// Most collapsed nodes (UI-S-017).
pub const MAX_COLLAPSED: usize = 1_000;

/// Most section ids in the section order (the UI's bound).
pub const MAX_SECTIONS: usize = 16;

/// Longest section id, in characters (the UI's bound).
pub const MAX_SECTION_ID_CHARS: usize = 64;

/// Where the UI opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartView {
    /// The Home view.
    #[default]
    Home,
    /// The root of the user's personal library.
    PersonalRoot,
}

/// What a listing is sorted by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    /// The display name.
    #[default]
    Name,
    /// The size in bytes.
    Size,
    /// When anyone last changed it.
    Modified,
    /// When the user last changed it.
    ModifiedByMe,
    /// When the user last opened it.
    OpenedByMe,
}

/// The sort direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    /// Smallest, earliest or A first.
    #[default]
    Ascending,
    /// Largest, latest or Z first.
    Descending,
}

/// Where folders go in a listing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderPlacement {
    /// Folders before files.
    #[default]
    OnTop,
    /// Folders sorted among files.
    Mixed,
}

/// How a listing is laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// One row per item.
    #[default]
    List,
    /// Tiles.
    Grid,
}

/// What a pinned node stands for: the contract's `NavigationKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinKind {
    /// The host's repository.
    Repository,
    /// A library.
    Library,
    /// A collection.
    Collection,
    /// A virtual collection.
    VirtualCollection,
    /// A folder.
    Folder,
    /// The repository's one Recently deleted node (Task API-F2.15).
    RecentlyDeleted,
}

/// One pinned destination: its opaque node handle and kind, never its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedDestination {
    /// The node handle (API-S-021).
    pub node: String,
    /// What the node stands for.
    pub kind: PinKind,
}

impl PinnedDestination {
    /// True when `kind` is the kind the node handle names (false for a non-handle). Pure.
    fn kind_matches(&self) -> bool {
        let named = match NodeRef::from_handle(&self.node) {
            Some(NodeRef::Repository(_)) => PinKind::Repository,
            Some(NodeRef::Library(_)) => PinKind::Library,
            Some(NodeRef::Collection(_)) => PinKind::Collection,
            Some(NodeRef::VirtualCollection(_)) => PinKind::VirtualCollection,
            Some(NodeRef::Folder { .. }) => PinKind::Folder,
            Some(NodeRef::RecentlyDeleted(_)) => PinKind::RecentlyDeleted,
            None => return false,
        };
        named == self.kind
    }
}

/// The navigation tree's choices (UI-S-017).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationPreferences {
    /// The format version, [`NAVIGATION_VERSION`].
    pub version: u32,
    /// Pinned destinations, at most [`MAX_PINNED`], each node once.
    #[serde(default)]
    pub pinned: Vec<PinnedDestination>,
    /// Section ids in the user's order, at most [`MAX_SECTIONS`] of 1 to
    /// [`MAX_SECTION_ID_CHARS`] characters, each once.
    #[serde(default)]
    pub order: Vec<String>,
    /// Handles of collapsed nodes, at most [`MAX_COLLAPSED`], each once.
    #[serde(default)]
    pub collapsed: Vec<String>,
}

impl Default for NavigationPreferences {
    fn default() -> Self {
        Self {
            version: NAVIGATION_VERSION,
            pinned: Vec::new(),
            order: Vec::new(),
            collapsed: Vec::new(),
        }
    }
}

/// One user's UI preferences: the body of `PUT` and the answer of `GET /api/v1/preferences`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    /// The format version, [`PREFERENCES_SCHEMA`].
    pub schema: u32,
    /// Where the UI opens.
    pub start_view: StartView,
    /// What listings are sorted by.
    pub sort_key: SortKey,
    /// The sort direction.
    pub sort_direction: SortDirection,
    /// Where folders go in a listing.
    pub folder_placement: FolderPlacement,
    /// How listings are laid out.
    pub layout: Layout,
    /// Whether hovering an item shows its card.
    pub hover_card: bool,
    /// The navigation tree's choices.
    pub navigation: NavigationPreferences,
}

impl Default for Preferences {
    /// The preferences of a user who saved none.
    fn default() -> Self {
        Self {
            schema: PREFERENCES_SCHEMA,
            start_view: StartView::default(),
            sort_key: SortKey::default(),
            sort_direction: SortDirection::default(),
            folder_placement: FolderPlacement::default(),
            layout: Layout::default(),
            hover_card: true,
            navigation: NavigationPreferences::default(),
        }
    }
}

impl Preferences {
    /// Check every bound of a value about to be saved. Pure.
    ///
    /// # Errors
    ///
    /// A message for people naming the first bound the value breaks.
    pub fn check(&self) -> Result<(), &'static str> {
        if self.schema != PREFERENCES_SCHEMA {
            return Err("The preferences schema is 1.");
        }
        self.navigation.check()
    }
}

impl NavigationPreferences {
    /// Check the version, the bounds and that every node is a node handle. Pure.
    fn check(&self) -> Result<(), &'static str> {
        if self.version != NAVIGATION_VERSION {
            return Err("The navigation preferences version is 1.");
        }
        let pins = self.pinned.iter().map(|pin| pin.node.as_str());
        if self.pinned.len() > MAX_PINNED || !all_handles_once(pins) {
            return Err("Navigation keeps at most 100 pinned node handles, each once.");
        }
        if !self.pinned.iter().all(PinnedDestination::kind_matches) {
            return Err("Each pinned destination's kind is the kind its node handle names.");
        }
        let order_ok = self.order.iter().all(|id| is_section_id(id));
        if self.order.len() > MAX_SECTIONS || !order_ok || !all_once(&self.order) {
            return Err("The section order holds at most 16 section ids of 1 to 64 characters.");
        }
        let collapsed = self.collapsed.iter().map(String::as_str);
        if self.collapsed.len() > MAX_COLLAPSED || !all_handles_once(collapsed) {
            return Err("Navigation keeps at most 1,000 collapsed node handles, each once.");
        }
        Ok(())
    }
}

/// True when every item is a node handle and none repeats. Pure.
fn all_handles_once<'a>(nodes: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = HashSet::new();
    let mut nodes = nodes;
    nodes.all(|node| NodeRef::from_handle(node).is_some() && seen.insert(node))
}

/// True when no item repeats. Pure.
fn all_once(items: &[String]) -> bool {
    let unique: HashSet<&String> = items.iter().collect();
    unique.len() == items.len()
}

/// True for a section id: 1 to [`MAX_SECTION_ID_CHARS`] characters. Pure.
fn is_section_id(id: &str) -> bool {
    (1..=MAX_SECTION_ID_CHARS).contains(&id.chars().count())
}
