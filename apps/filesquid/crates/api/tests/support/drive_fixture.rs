//! Shared helpers for the drive tests (Task API-F2.2; API-S-011): a [`ScopeObjects`] that
//! maps one folder scope to the rig's object, and the preference calls through the router.
//!
//! The including test crate must declare the `files_fixture` module at its root.

#![allow(dead_code)]

use crate::files_fixture::{Answer, FilesRig, LIBRARY, OBJECT};
use axum::http::Method;
use filesquid_api::files::{ScopeObject, ScopeObjects};
use filesquid_api::navigation::FolderScope;
use filesquid_api::preferences::MemoryPreferenceStore;
use serde_json::{json, Value};
use std::sync::Arc;

/// Maps exactly one folder scope to the rig's library and object.
pub struct OneScope(pub FolderScope);

impl ScopeObjects for OneScope {
    fn object_of(&self, scope: &FolderScope) -> Option<ScopeObject> {
        (*scope == self.0).then(|| ScopeObject {
            library_id: LIBRARY.to_string(),
            object_id: OBJECT.to_string(),
        })
    }
}

/// Install a new in-memory preference store in `rig`'s API state and return it.
pub fn keep_preferences(rig: &FilesRig) -> Arc<MemoryPreferenceStore> {
    let store = Arc::new(MemoryPreferenceStore::new());
    assert!(rig.state.install_preferences(store.clone()));
    store
}

/// A complete preferences body with the defaults except `layout` grid and one pin.
pub fn grid_preferences(pin: &str) -> Value {
    json!({
        "schema": 1,
        "start_view": "personal_root",
        "sort_key": "modified_by_me",
        "sort_direction": "descending",
        "folder_placement": "mixed",
        "layout": "grid",
        "hover_card": false,
        "navigation": {
            "version": 1,
            "pinned": [{ "node": pin, "kind": "collection" }],
            "order": ["libraries", "pinned"],
            "collapsed": [pin]
        }
    })
}

/// `PUT /api/v1/preferences` with `body`, as the rig's caller.
pub async fn put_preferences(rig: &FilesRig, body: &Value) -> Answer {
    let headers = [("content-type", "application/json")];
    rig.call(
        Method::PUT,
        "/api/v1/preferences",
        &headers,
        body.to_string().into_bytes(),
    )
    .await
}

/// `GET /api/v1/preferences`, as the rig's caller.
pub async fn get_preferences(rig: &FilesRig) -> Answer {
    rig.get("/api/v1/preferences").await
}
