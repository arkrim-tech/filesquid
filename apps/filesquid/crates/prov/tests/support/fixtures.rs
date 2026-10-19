//! Shared version facts for the `filesquid-prov` tests.
//!
//! The standard fixture describes version `v3` of one object whose base is
//! `v2`: one changed path, one removed path, one moved path, one added path
//! with characters that need percent-encoding, and one unchanged path.

use std::collections::BTreeMap;

use filesquid_prov::{HostFacts, VersionFacts};

/// Installation UUID (version 4) used by every fixture.
pub const INSTALLATION: &str = "0f8e2b9a-4c1d-4e7f-9a3b-5d6c7e8f9a0b";

/// OCFL object identifier of the standard fixture.
pub const OBJECT: &str = "urn:uuid:5b1c2d3e-4f50-4617-8829-3a4b5c6d7e8f";

/// Operation identifier of the standard fixture.
pub const OPERATION: &str = "op-7d1e";

/// Added logical path that needs percent-encoding.
pub const ADDED_PATH: &str = "Fotos/été 1.jpg";

/// Percent-encoded form of [`ADDED_PATH`].
pub const ADDED_PATH_ENCODED: &str = "Fotos%2F%C3%A9t%C3%A9%201.jpg";

/// A SHA-256 digest made of one repeated hexadecimal character.
pub fn digest(hex: char) -> String {
    hex.to_string().repeat(64)
}

/// Build a path-to-digest map from `(path, hex)` pairs.
pub fn state(entries: &[(&str, char)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, hex)| (path.to_string(), digest(*hex)))
        .collect()
}

/// The FileSquid host that committed the standard fixture.
pub fn host() -> HostFacts {
    HostFacts {
        product: "FileSquid".to_string(),
        distribution: "imagesquid".to_string(),
        version: "0.1.0".to_string(),
    }
}

/// Facts of version `v3` (base `v2`) with every kind of change.
pub fn standard_facts() -> VersionFacts {
    VersionFacts {
        object_id: OBJECT.to_string(),
        version: "v3".to_string(),
        base_version: Some("v2".to_string()),
        created: "2026-10-19T10:00:05Z".to_string(),
        user: None,
        message: Some("filesquid.operation.v1".to_string()),
        base_state: state(&[
            ("a.txt", '1'),
            ("b.txt", '2'),
            ("old/c.txt", '3'),
            ("keep.txt", '4'),
        ]),
        new_state: state(&[
            ("a.txt", '5'),
            ("new/c.txt", '3'),
            ("keep.txt", '4'),
            (ADDED_PATH, '6'),
        ]),
        moves: [("old/c.txt".to_string(), "new/c.txt".to_string())]
            .into_iter()
            .collect(),
        operation_id: OPERATION.to_string(),
        kind: "move".to_string(),
        started: "2026-10-19T10:00:00Z".to_string(),
        installation: Some(INSTALLATION.to_string()),
        host: Some(host()),
        journal_entries: vec!["j-2".to_string(), "j-1".to_string()],
        reconstruction: None,
    }
}

/// Facts of a first version `v1` that adds one file.
pub fn first_version_facts(object_id: &str, operation: &str, created: &str) -> VersionFacts {
    VersionFacts {
        object_id: object_id.to_string(),
        version: "v1".to_string(),
        base_version: None,
        created: created.to_string(),
        user: None,
        message: None,
        base_state: BTreeMap::new(),
        new_state: state(&[("a.txt", 'a')]),
        moves: BTreeMap::new(),
        operation_id: operation.to_string(),
        kind: "import".to_string(),
        started: created.to_string(),
        installation: Some(INSTALLATION.to_string()),
        host: Some(host()),
        journal_entries: vec!["j-1".to_string()],
        reconstruction: None,
    }
}
