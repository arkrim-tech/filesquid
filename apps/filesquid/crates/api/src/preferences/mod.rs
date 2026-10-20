//! Per-user UI preferences (Task API-F2.2; API-S-011, SRS-F-062, UI-S-017).
//!
//! `GET /api/v1/preferences` answers the caller's saved [`Preferences`], or the defaults when
//! they saved none; `PUT` replaces them with a checked value. Both take the action
//! `preferences.manage`, which reaches only the caller's own preferences. The value holds the
//! start view, sort key and direction, folder placement, layout and hover-card setting, and
//! the navigation tree's pinned destinations, section order and collapsed nodes as opaque
//! node handles; never a name, title, logical path, token or content handle (UI-S-017).
//!
//! The host injects where preferences are kept ([`PreferenceStore`]) with
//! [`crate::ApiState::install_preferences`]; the routes answer `unavailable` until then.

mod model;
mod routes;
mod store;

pub use model::{
    FolderPlacement, Layout, NavigationPreferences, PinKind, PinnedDestination, Preferences,
    SortDirection, SortKey, StartView, MAX_COLLAPSED, MAX_PINNED, MAX_SECTIONS,
    MAX_SECTION_ID_CHARS, NAVIGATION_VERSION, PREFERENCES_SCHEMA,
};
pub(crate) use routes::routes;
pub use store::{MemoryPreferenceStore, PreferenceStore, PreferenceStoreError, MAX_MEMORY_USERS};

/// `tracing` target of this module's stdout-only diagnostics.
const LOG_TARGET: &str = "filesquid::preferences";

/// Journal operation of a refused preference read.
pub const READ_OPERATION: &str = "preferences.read";

/// Journal operation of a preference save.
pub const UPDATE_OPERATION: &str = "preferences.update";
