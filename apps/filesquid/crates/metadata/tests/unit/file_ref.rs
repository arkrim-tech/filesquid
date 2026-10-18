//! `FileRef` is opaque and rejects logical paths that could escape a root.

use filesquid_metadata::file_ref::{FileRef, FileRefError};

#[test]
fn accessors_return_what_the_caller_supplied() {
    let file = FileRef::new("urn:example:a", "photos/harbour.jpg")
        .unwrap()
        .with_version("v2");
    assert_eq!(file.object_id(), "urn:example:a");
    assert_eq!(file.version(), Some("v2"));
    assert_eq!(file.logical_path(), "photos/harbour.jpg");
}

#[test]
fn version_is_optional_and_object_id_may_be_empty() {
    let file = FileRef::new("", "a.jpg").unwrap();
    assert_eq!(file.version(), None);
    assert_eq!(file.object_id(), "");
}

#[test]
fn escaping_or_empty_logical_paths_are_rejected() {
    for bad in [
        "", "/a", "a//b", "a/", "./a", "../a", "a/../b", "a\\b", "a\0b", "D:/a", "C:a",
    ] {
        assert_eq!(
            FileRef::new("o", bad),
            Err(FileRefError::InvalidLogicalPath),
            "{bad:?}"
        );
    }
}
