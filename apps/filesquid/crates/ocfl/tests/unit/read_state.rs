//! Pure reads over one parsed inventory (OCFL-S-004, T-039): logical state
//! at a caller-pinned version through `state` and `manifest`, version
//! history, and the diff of two versions. No storage is touched.

use filesquid_ocfl::inventory::{Inventory, VersionNum};
use filesquid_ocfl::read::{diff_versions, resolve_state, version_history, Change, ReadError};
use serde_json::{json, Value};

#[path = "../support/inventory_example.rs"]
mod example;
use example::{base, A, C, EMPTY};

/// Parse a JSON inventory.
fn parse(value: &Value) -> Inventory {
    Inventory::parse(&serde_json::to_vec(value).expect("serialise")).expect("valid inventory")
}

/// A version number from its name.
fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version")
}

#[test]
fn state_is_resolved_through_the_manifest_at_the_pinned_version() {
    let inventory = parse(&base());
    let state = resolve_state(&inventory, &v("v1")).expect("v1");
    assert_eq!(state.object_id(), "ark:/12345/bcd987");
    assert_eq!(state.version(), &v("v1"));
    assert_eq!(
        state.paths().collect::<Vec<_>>(),
        ["empty.txt", "foo/bar.xml"]
    );
    let bar = state.get("foo/bar.xml").expect("bar");
    assert_eq!(bar.digest, A);
    assert_eq!(bar.content_path, "v1/content/foo/bar.xml");
}

#[test]
fn unchanged_bytes_resolve_to_the_older_version_directory() {
    let inventory = parse(&base());
    let state = resolve_state(&inventory, &v("v2")).expect("v2");
    assert_eq!(
        state.paths().collect::<Vec<_>>(),
        ["empty.txt", "empty2.txt", "foo/bar.xml", "image.tiff"]
    );
    let located = |path: &str| state.get(path).expect(path).content_path.clone();
    assert_eq!(located("foo/bar.xml"), "v1/content/foo/bar.xml");
    assert_eq!(located("empty2.txt"), "v1/content/empty.txt");
    assert_eq!(located("image.tiff"), "v2/content/image.tiff");
    assert_eq!(state.len(), 4);
    assert!(!state.is_empty());
}

#[test]
fn every_file_is_enumerated_with_its_digest_and_content_path() {
    let inventory = parse(&base());
    let state = resolve_state(&inventory, &v("v2")).expect("v2");
    let rows: Vec<(&str, &str, &str)> = state
        .files()
        .map(|f| (f.path.as_str(), f.digest.as_str(), f.content_path.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("empty.txt", EMPTY, "v1/content/empty.txt"),
            ("empty2.txt", EMPTY, "v1/content/empty.txt"),
            ("foo/bar.xml", A, "v1/content/foo/bar.xml"),
            ("image.tiff", C, "v2/content/image.tiff"),
        ]
    );
}

#[test]
fn a_version_the_inventory_does_not_list_is_refused() {
    let inventory = parse(&base());
    for name in ["v3", "v02"] {
        let err = resolve_state(&inventory, &v(name)).expect_err(name);
        assert!(
            matches!(&err, ReadError::VersionNotFound { version } if version == name),
            "{err:?}"
        );
        assert_eq!(err.code(), None);
    }
}

#[test]
fn a_state_digest_missing_from_the_manifest_is_e050() {
    let mut inventory = parse(&base());
    inventory.manifest.remove(C);
    let err = resolve_state(&inventory, &v("v2")).expect_err("no manifest entry");
    assert_eq!(err.code(), Some("E050"));
}

#[test]
fn history_lists_every_version_in_order_with_its_metadata() {
    let inventory = parse(&base());
    let history = version_history(&inventory);
    let names: Vec<String> = history.iter().map(|h| h.version.to_string()).collect();
    assert_eq!(names, ["v1", "v2"]);
    assert_eq!(history[0].created, "2018-01-01T01:01:01Z");
    assert_eq!(history[0].message.as_deref(), Some("Initial import"));
    let user = history[0].user.as_ref().expect("user");
    assert_eq!(user.name, "A Person");
    assert_eq!(user.address.as_deref(), Some("mailto:a@example.org"));
    assert_eq!(history[1].user.as_ref().expect("user").address, None);
}

#[test]
fn diff_reports_added_and_deleted_paths() {
    let inventory = parse(&base());
    let forward = diff_versions(&inventory, &v("v1"), &v("v2")).expect("diff");
    assert_eq!(
        forward,
        [
            Change::Added {
                path: "empty2.txt".to_string(),
                digest: EMPTY.to_string()
            },
            Change::Added {
                path: "image.tiff".to_string(),
                digest: C.to_string()
            },
        ]
    );
    let back = diff_versions(&inventory, &v("v2"), &v("v1")).expect("diff");
    assert_eq!(
        back,
        [
            Change::Deleted {
                path: "empty2.txt".to_string(),
                digest: EMPTY.to_string()
            },
            Change::Deleted {
                path: "image.tiff".to_string(),
                digest: C.to_string()
            },
        ]
    );
    assert!(diff_versions(&inventory, &v("v2"), &v("v2"))
        .expect("diff")
        .is_empty());
}

/// The example with `foo/bar.xml` renamed to `foo/baz.xml` and `empty.txt`
/// replaced by new bytes in v2.
fn renamed_and_modified() -> Value {
    let mut value = base();
    value["versions"]["v2"]["state"] = json!({
        A: ["foo/baz.xml"],
        C: ["empty.txt"],
    });
    value
}

#[test]
fn diff_reports_renames_and_modifications() {
    let inventory = parse(&renamed_and_modified());
    let changes = diff_versions(&inventory, &v("v1"), &v("v2")).expect("diff");
    assert_eq!(
        changes,
        [
            Change::Modified {
                path: "empty.txt".to_string(),
                from: EMPTY.to_string(),
                to: C.to_string()
            },
            Change::Renamed {
                from: "foo/bar.xml".to_string(),
                to: "foo/baz.xml".to_string(),
                digest: A.to_string()
            },
        ]
    );
    assert_eq!(changes[1].path(), "foo/baz.xml");
}

#[test]
fn diff_pairs_renames_of_one_digest_in_path_order() {
    let mut value = base();
    value["versions"]["v1"]["state"] = json!({EMPTY: ["a", "b"], A: ["foo/bar.xml"]});
    value["versions"]["v2"]["state"] =
        json!({EMPTY: ["c", "d", "e"], A: ["foo/bar.xml"], C: ["x"]});
    let inventory = parse(&value);
    let changes = diff_versions(&inventory, &v("v1"), &v("v2")).expect("diff");
    let renamed = |from: &str, to: &str| Change::Renamed {
        from: from.to_string(),
        to: to.to_string(),
        digest: EMPTY.to_string(),
    };
    let added = |path: &str, digest: &str| Change::Added {
        path: path.to_string(),
        digest: digest.to_string(),
    };
    assert_eq!(
        changes,
        [
            renamed("a", "c"),
            renamed("b", "d"),
            added("e", EMPTY),
            added("x", C)
        ]
    );
}

#[test]
fn diff_of_an_unknown_version_is_refused() {
    let inventory = parse(&base());
    let err = diff_versions(&inventory, &v("v1"), &v("v9")).expect_err("v9");
    assert!(matches!(err, ReadError::VersionNotFound { .. }), "{err:?}");
}
