//! A history with one record, for tests that call every documented route (API-S-029): the
//! record of `v1` of the file [`SAMPLE_FILE`], and [`concrete`] to fill the path
//! parameters of a documented path with that file and version.

#![allow(dead_code)]

use filesquid_api::history::MemoryHistory;
use filesquid_api::ApiState;
use std::sync::Arc;

/// The file whose history [`install_sample_history`] holds.
pub const SAMPLE_FILE: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

/// The version of its one record.
pub const SAMPLE_VERSION: &str = "v1";

/// The installation that committed it.
pub const SAMPLE_INSTALLATION: &str = "0f8e2b9a-4c1d-4e7f-9a3b-5d6c7e8f9a0b";

/// A documented path with `{id}` and `{version}` filled in for the sample history.
pub fn concrete(path: &str) -> String {
    path.replace("{id}", SAMPLE_FILE)
        .replace("{version}", SAMPLE_VERSION)
}

/// The record of the first version of [`SAMPLE_FILE`]: an import of `photo.jpg`.
pub fn sample_record() -> filesquid_prov::Record {
    let facts = filesquid_prov::VersionFacts {
        object_id: format!("urn:uuid:{SAMPLE_FILE}"),
        version: SAMPLE_VERSION.into(),
        created: "2026-10-19T10:00:01Z".into(),
        started: "2026-10-19T10:00:00Z".into(),
        operation_id: "op-1".into(),
        kind: "import".into(),
        new_state: [("photo.jpg".to_string(), "a".repeat(64))].into(),
        installation: Some(SAMPLE_INSTALLATION.into()),
        journal_entries: vec!["7".into()],
        ..filesquid_prov::VersionFacts::default()
    };
    filesquid_prov::build_record(&facts).expect("record")
}

/// Install a history holding [`sample_record`] in `state`.
pub fn install_sample_history(state: &ApiState) {
    let history = MemoryHistory::new();
    history.insert(sample_record()).expect("sample record");
    assert!(state.install_history(Arc::new(history)));
}
