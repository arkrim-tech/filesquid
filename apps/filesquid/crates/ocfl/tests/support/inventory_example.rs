//! A valid two-version OCFL 1.1 inventory using sha256, modelled on the
//! specification's examples, shared by the unit tests and the BDD steps.

#![allow(dead_code)]

use serde_json::{json, Value};

/// SHA-256 of the empty string.
pub const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
/// Stand-in SHA-256 digest (valid hex of the right length).
pub const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
/// Stand-in SHA-256 digest (valid hex of the right length).
pub const C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

/// The example inventory as JSON.
pub fn base() -> Value {
    json!({
        "id": "ark:/12345/bcd987",
        "type": "https://ocfl.io/1.1/spec/#inventory",
        "digestAlgorithm": "sha256",
        "head": "v2",
        "contentDirectory": "content",
        "manifest": {
            A: ["v1/content/foo/bar.xml"],
            EMPTY: ["v1/content/empty.txt"],
            C: ["v2/content/image.tiff"]
        },
        "versions": {
            "v1": {
                "created": "2018-01-01T01:01:01Z",
                "message": "Initial import",
                "user": {"name": "A Person", "address": "mailto:a@example.org"},
                "state": {A: ["foo/bar.xml"], EMPTY: ["empty.txt"]}
            },
            "v2": {
                "created": "2018-02-02T02:02:02.5+01:00",
                "message": "Fix bar.xml, add image",
                "user": {"name": "A Person"},
                "state": {A: ["foo/bar.xml"], EMPTY: ["empty.txt", "empty2.txt"], C: ["image.tiff"]}
            }
        },
        "fixity": {
            "md5": {"184f84e28cbe75e050e9c25ea7f2e939": ["v1/content/foo/bar.xml"]},
            "crc32": {"0f1e2d3c": ["v1/content/empty.txt"]}
        }
    })
}
