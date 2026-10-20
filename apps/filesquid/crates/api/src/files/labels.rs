//! Version labels (API-S-014, SRS-F-065): a user-chosen name for one OCFL version of a file.
//!
//! A label set at commit time is the `label` member of the version's
//! `filesquid.operation.v1` message (ADR 040). A label set later is application data in the
//! object itself, so it is recovered with the file from an ordinary copy: the logical file
//! [`LABELS_PATH`], `{"schema":1,"labels":{"v3":"Final draft","v5":null}}`, committed by a
//! `label` operation in its own version. An entry overrides the commit-time label of its
//! version, and `null` clears it. Labels never influence retention.
//!
//! [`APP_DATA_FOLDER`] is reserved for such application data, and [`PROVENANCE_FOLDER`] for
//! each version's PROV-O records (ADR 040): listings leave both out, and uploads, moves,
//! removals, restores and new folders there are refused. Pure; no I/O.

use super::error::FileError;
use filesquid_core::{OperationMessage, MAX_LABEL_BYTES};
use filesquid_ocfl::inventory::VersionNum;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The logical folder reserved for FileSquid's application data in an object.
pub const APP_DATA_FOLDER: &str = ".filesquid";

/// The logical folder that holds each version's PROV-O records (ADR 040).
pub const PROVENANCE_FOLDER: &str = "provenance";

/// The top-level folders FileSquid keeps its own files in: left out of listings and closed
/// to writes through the file routes.
pub const RESERVED_FOLDERS: [&str; 2] = [APP_DATA_FOLDER, PROVENANCE_FOLDER];

/// The logical file that holds the labels set after commit.
pub const LABELS_PATH: &str = ".filesquid/labels.json";

/// Most versions one object's labels file names.
pub const MAX_LABELS: usize = 1_000;

/// Largest labels file read, in bytes: [`MAX_LABELS`] entries of [`MAX_LABEL_BYTES`] fit.
pub const MAX_LABELS_FILE_BYTES: u64 = 1024 * 1024;

/// The schema of the labels file.
const SCHEMA: u32 = 1;

/// Whether `path` is the application-data folder or lies inside it.
pub fn is_reserved(path: &str) -> bool {
    in_folder(path, APP_DATA_FOLDER)
}

/// Whether `path` is one of the [`RESERVED_FOLDERS`] or lies inside one.
pub fn in_reserved_folder(path: &str) -> bool {
    RESERVED_FOLDERS
        .iter()
        .any(|folder| in_folder(path, folder))
}

/// Whether `path` is `folder` or lies inside it.
fn in_folder(path: &str, folder: &str) -> bool {
    path == folder
        || path
            .strip_prefix(folder)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Refuse a path in one of the [`RESERVED_FOLDERS`].
///
/// # Errors
///
/// `invalid_request` when `path` is reserved.
pub fn refuse_reserved(path: &str) -> Result<(), FileError> {
    match in_reserved_folder(path) {
        true => Err(FileError::invalid(
            "The .filesquid and provenance folders hold FileSquid's own data; choose another path.",
        )),
        false => Ok(()),
    }
}

/// Refuse a label that is blank, over [`MAX_LABEL_BYTES`] or holds a control character.
///
/// # Errors
///
/// `invalid_request` naming the rule.
pub fn check_label(label: &str) -> Result<(), FileError> {
    let ok = !label.trim().is_empty()
        && label.len() <= MAX_LABEL_BYTES
        && !label.chars().any(char::is_control);
    match ok {
        true => Ok(()),
        false => Err(FileError::invalid(format!(
            "A label is 1 to {MAX_LABEL_BYTES} bytes, not blank, without control characters."
        ))),
    }
}

/// The commit-time label of a version whose message is `message`: the `label` member of a
/// `filesquid.operation.v1` message, else none.
pub fn commit_label(message: Option<&str>) -> Option<String> {
    let parsed = OperationMessage::parse(message?).ok()??;
    parsed.label().map(str::to_string)
}

/// The labels file: each named version's later label, `None` for a cleared one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Labels {
    schema: u32,
    labels: BTreeMap<String, Option<String>>,
}

impl Default for Labels {
    /// No later label, in the current schema.
    fn default() -> Self {
        Self::new()
    }
}

impl Labels {
    /// No later label.
    pub fn new() -> Self {
        Self {
            schema: SCHEMA,
            labels: BTreeMap::new(),
        }
    }

    /// The labels file in `bytes`.
    ///
    /// # Errors
    ///
    /// `internal` for a file that is not a labels file of this schema within the bounds.
    pub fn parse(bytes: &[u8]) -> Result<Self, FileError> {
        let labels: Self = serde_json::from_slice(bytes)
            .map_err(|_| FileError::internal("a labels file is not valid JSON of schema 1"))?;
        let valid = labels.schema == SCHEMA
            && labels.labels.len() <= MAX_LABELS
            && labels
                .labels
                .keys()
                .all(|version| VersionNum::parse(version).is_ok())
            && labels
                .labels
                .values()
                .flatten()
                .all(|label| check_label(label).is_ok());
        match valid {
            true => Ok(labels),
            false => Err(FileError::internal("a labels file breaks its bounds")),
        }
    }

    /// The file's bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The file's bytes, refused unless [`Labels::parse`] reads them back: within
    /// [`MAX_LABELS_FILE_BYTES`] and with every label valid. A written file is always one
    /// that reads back.
    ///
    /// # Errors
    ///
    /// `conflict` when the file would be too large; `invalid_request` when a label breaks
    /// the label rules.
    pub fn to_bounded_bytes(&self) -> Result<Vec<u8>, FileError> {
        let bytes = self.to_bytes();
        if bytes.len() as u64 > MAX_LABELS_FILE_BYTES {
            return Err(FileError::conflict(
                "This object's labels would exceed their size bound; clear some labels first.",
            ));
        }
        for label in self.labels.values().flatten() {
            check_label(label)?;
        }
        Self::parse(&bytes)?;
        Ok(bytes)
    }

    /// The label `version` shows: its later label when the file names it, else
    /// `commit_label`.
    pub fn effective(&self, version: &str, commit_label: Option<String>) -> Option<String> {
        match self.labels.get(version) {
            Some(later) => later.clone(),
            None => commit_label,
        }
    }

    /// Give `version`, whose commit-time label is `commit_label`, the label `label` (`None`
    /// clears it). An entry equal to the commit-time label is dropped, so the file holds
    /// only overrides.
    ///
    /// # Errors
    ///
    /// `conflict` when the shown label would not change, or when the file already names
    /// [`MAX_LABELS`] versions.
    pub fn set(
        &mut self,
        version: &str,
        commit_label: Option<String>,
        label: Option<String>,
    ) -> Result<(), FileError> {
        if self.effective(version, commit_label.clone()) == label {
            return Err(FileError::conflict(
                "That version already shows this label, so no version was made.",
            ));
        }
        if label == commit_label {
            self.labels.remove(version);
            return Ok(());
        }
        if !self.labels.contains_key(version) && self.labels.len() >= MAX_LABELS {
            return Err(FileError::conflict(format!(
                "An object keeps at most {MAX_LABELS} labels set after commit."
            )));
        }
        self.labels.insert(version.to_string(), label);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_labels_file_is_the_current_schema_and_reads_back() {
        let labels = Labels::default();
        assert_eq!(labels, Labels::new());
        let bytes = labels.to_bounded_bytes().expect("bytes");
        assert_eq!(Labels::parse(&bytes).expect("parse"), labels);
    }

    #[test]
    fn a_labels_file_naming_something_other_than_a_version_is_refused() {
        let bad = br#"{"schema":1,"labels":{"draft":"A"}}"#;
        assert!(Labels::parse(bad).is_err());
        let good = br#"{"schema":1,"labels":{"v12":"A","v3":null}}"#;
        assert!(Labels::parse(good).is_ok());
    }

    #[test]
    fn the_reserved_folder_is_the_folder_and_its_contents_only() {
        assert!(is_reserved(".filesquid"));
        assert!(is_reserved(".filesquid/labels.json"));
        assert!(!is_reserved(".filesquidx/a"));
        assert!(!is_reserved("photos/.filesquid"));
        assert!(!is_reserved("provenance/v1.jsonld"));
        assert!(in_reserved_folder("provenance"));
        assert!(in_reserved_folder("provenance/v1.jsonld"));
        assert!(in_reserved_folder(".filesquid/labels.json"));
        assert!(!in_reserved_folder("provenances/a"));
        assert!(!in_reserved_folder("photos/provenance"));
    }

    #[test]
    fn an_override_equal_to_the_commit_label_is_dropped() {
        let mut labels = Labels::new();
        labels
            .set("v2", Some("A".into()), Some("B".into()))
            .expect("set");
        labels
            .set("v2", Some("A".into()), Some("A".into()))
            .expect("back");
        assert_eq!(labels, Labels::new());
    }

    /// Labels with `count` entries `v1`... set to `label`.
    fn filled(count: usize, label: &str) -> Labels {
        let mut labels = Labels::new();
        for n in 1..=count {
            labels
                .set(&format!("v{n}"), None, Some(label.to_string()))
                .expect("set");
        }
        labels
    }

    #[test]
    fn the_thousand_and_first_labelled_version_is_refused() {
        let mut labels = filled(MAX_LABELS, "Kept");
        let refused = labels.set("v1001", None, Some("One more".into()));
        let err = refused.expect_err("over the cap");
        assert_eq!(err.code(), crate::ErrorCode::Conflict);
        labels
            .set("v1", None, Some("Renamed".into()))
            .expect("an existing entry still changes");
        labels.set("v2", None, None).expect("and clears");
    }

    #[test]
    fn the_largest_allowed_labels_file_stays_within_its_read_bound() {
        let label = "\\\"".repeat(MAX_LABEL_BYTES / 2);
        let labels = filled(MAX_LABELS, &label);
        let bytes = labels.to_bounded_bytes().expect("within the bound");
        assert!(Labels::parse(&bytes).is_ok());
    }

    #[test]
    fn a_file_with_an_invalid_label_is_never_written() {
        let mut labels = Labels::new();
        labels
            .set("v1", None, Some(" ".into()))
            .expect("set does not check");
        let err = labels.to_bounded_bytes().expect_err("refused");
        assert_eq!(err.code(), crate::ErrorCode::InvalidRequest);
    }

    #[test]
    fn the_file_round_trips() {
        let mut labels = Labels::new();
        labels.set("v1", None, Some("One".into())).expect("set");
        labels.set("v2", Some("Two".into()), None).expect("clear");
        let parsed = Labels::parse(&labels.to_bytes()).expect("parse");
        assert_eq!(parsed.effective("v1", None), Some("One".into()));
        assert_eq!(parsed.effective("v2", Some("Two".into())), None);
        assert_eq!(
            parsed.effective("v3", Some("Three".into())),
            Some("Three".into())
        );
    }
}
