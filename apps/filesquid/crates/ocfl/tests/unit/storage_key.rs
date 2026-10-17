//! Storage key and range validation (OCFL-S-007): keys are `/`-separated
//! relative paths that can never escape the storage root, and every ranged
//! read is bounded.

use filesquid_ocfl::storage::{
    ByteRange, ETag, StorageError, StorageKey, MAX_CHUNK_BYTES, MAX_LIST_PAGE, MAX_OBJECT_BYTES,
    MAX_PARTS, MAX_PUT_BYTES, MAX_WALK_DEPTH, MAX_WALK_ENTRIES,
};

#[test]
fn relative_slash_separated_keys_are_accepted() {
    for key in [
        "inventory.json",
        "objects/ab/cd/v1/content/a file.txt",
        "0=ocfl_1.1",
        "extensions/0004-hashed-n-tuple-storage-layout/config.json",
    ] {
        let parsed = StorageKey::new(key).expect("valid key");
        assert_eq!(parsed.as_str(), key);
    }
}

#[test]
fn keys_that_could_escape_the_root_are_rejected() {
    for key in [
        "",
        "/etc/passwd",
        "../outside.txt",
        "objects/../../outside.txt",
        "objects/./inventory.json",
        "objects//inventory.json",
        "objects/",
        "objects\\inventory.json",
        "objects/in\0ventory.json",
        ".",
        "..",
    ] {
        let err = StorageKey::new(key).expect_err(key);
        assert!(
            matches!(err, StorageError::InvalidKey { .. }),
            "{key:?} gave {err:?}"
        );
    }
}

#[test]
fn key_segments_are_exposed_in_order() {
    let key = StorageKey::new("objects/a/v1/content/f.txt").expect("valid");
    let segments: Vec<&str> = key.segments().collect();
    assert_eq!(segments, ["objects", "a", "v1", "content", "f.txt"]);
}

#[test]
fn byte_ranges_are_bounded_by_the_chunk_limit() {
    assert!(ByteRange::new(0, 1).is_ok());
    assert!(ByteRange::new(7, MAX_CHUNK_BYTES).is_ok());
    assert!(matches!(
        ByteRange::new(0, 0),
        Err(StorageError::InvalidArgument { .. })
    ));
    assert!(matches!(
        ByteRange::new(0, MAX_CHUNK_BYTES + 1),
        Err(StorageError::LimitExceeded { .. })
    ));
    assert!(matches!(
        ByteRange::new(u64::MAX, 2),
        Err(StorageError::InvalidArgument { .. })
    ));
}

#[test]
fn limits_match_the_plan_constraints() {
    assert_eq!(MAX_CHUNK_BYTES, 8 * 1024 * 1024);
    assert_eq!(MAX_PUT_BYTES, 8 * 1024 * 1024);
    assert_eq!(MAX_PARTS, 10_000);
    assert_eq!(MAX_LIST_PAGE, 1_000);
    assert_eq!(MAX_WALK_DEPTH, 64);
    assert_eq!(MAX_WALK_ENTRIES, 1_000_000);
    assert_eq!(MAX_OBJECT_BYTES, 5 * 1024 * 1024 * 1024 * 1024);
}

#[test]
fn entity_tag_is_lowercase_hex_sha256() {
    let tag = ETag::sha256_of(b"abc");
    assert_eq!(
        tag.as_str(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
