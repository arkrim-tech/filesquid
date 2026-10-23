//! Task DE-1.7: collection logical paths (API-S-026) and the numbering of a taken path
//! (API-S-011 **Same logical path**; ADR 041 Decision 12), in
//! `filesquid_api::canonical::collection_path`. Every function under test is pure.

use filesquid_api::canonical::collection_path::{
    check_collection_path, check_folder, free_path, join, numbered, safe_segment, ItemKind,
    PathError, MAX_PATH_BYTES, MAX_SEGMENT_BYTES, MAX_SUFFIX,
};
use std::collections::HashSet;

/// A `taken` predicate over `paths`, as a collection's index answers it.
fn taken_in(paths: &[&str]) -> impl FnMut(&str) -> Result<bool, ()> {
    let set: HashSet<String> = paths.iter().map(|p| p.to_string()).collect();
    move |path: &str| Ok(set.contains(path))
}

#[test]
fn the_caps_are_the_owner_s() {
    assert_eq!(MAX_SUFFIX, 9_999);
    assert_eq!(MAX_SEGMENT_BYTES, 255);
    assert_eq!(MAX_PATH_BYTES, 1_024);
}

#[test]
fn a_folder_takes_the_number_at_its_end() {
    assert_eq!(numbered("directoryY", 1, ItemKind::Folder), "directoryY-1");
    assert_eq!(numbered("directoryY", 2, ItemKind::Folder), "directoryY-2");
    assert_eq!(numbered("trip.2024", 3, ItemKind::Folder), "trip.2024-3");
}

#[test]
fn a_file_takes_the_number_before_its_first_dot() {
    assert_eq!(numbered("photo.jpg", 1, ItemKind::File), "photo-1.jpg");
    assert_eq!(numbered("photo.jpg", 2, ItemKind::File), "photo-2.jpg");
    assert_eq!(
        numbered("archive.tar.gz", 1, ItemKind::File),
        "archive-1.tar.gz"
    );
    assert_eq!(numbered("README", 1, ItemKind::File), "README-1");
}

#[test]
fn a_dotfile_keeps_its_leading_dot_in_its_first_part() {
    assert_eq!(numbered(".env", 1, ItemKind::File), ".env-1");
    assert_eq!(
        numbered(".config.json", 1, ItemKind::File),
        ".config-1.json"
    );
    assert_eq!(numbered("..double", 1, ItemKind::File), "..double-1");
}

#[test]
fn a_free_path_is_answered_as_it_is() {
    let mut taken = taken_in(&["trips/a.jpg"]);
    let free = free_path("trips/b.jpg", ItemKind::File, &mut taken).unwrap();
    assert_eq!(free, Ok("trips/b.jpg".to_string()));
}

#[test]
fn a_taken_path_gets_the_first_free_number_beside_it() {
    let mut taken = taken_in(&["trips/photo.jpg", "trips/photo-1.jpg"]);
    let free = free_path("trips/photo.jpg", ItemKind::File, &mut taken).unwrap();
    assert_eq!(free, Ok("trips/photo-2.jpg".to_string()));
    let mut taken = taken_in(&["filepathX/directoryY"]);
    let free = free_path("filepathX/directoryY", ItemKind::Folder, &mut taken).unwrap();
    assert_eq!(free, Ok("filepathX/directoryY-1".to_string()));
}

#[test]
fn the_search_stops_after_9999_numbers() {
    let mut asked = 0u32;
    let mut always = |_: &str| -> Result<bool, ()> {
        asked += 1;
        Ok(true)
    };
    let free = free_path("a.txt", ItemKind::File, &mut always).unwrap();
    assert_eq!(free, Err(PathError::SuffixExhausted));
    assert_eq!(
        PathError::SuffixExhausted.error_code(),
        "path_suffix_exhausted"
    );
    assert_eq!(asked, 1 + MAX_SUFFIX, "the path itself, then -1 to -9999");
}

#[test]
fn the_9999th_number_is_still_tried() {
    let mut taken = |path: &str| -> Result<bool, ()> { Ok(path != "a-9999.txt") };
    let free = free_path("a.txt", ItemKind::File, &mut taken).unwrap();
    assert_eq!(free, Ok("a-9999.txt".to_string()));
}

#[test]
fn a_numbered_segment_over_255_bytes_is_name_too_long() {
    let long = format!("{}.jpg", "n".repeat(MAX_SEGMENT_BYTES - 4));
    assert_eq!(long.len(), MAX_SEGMENT_BYTES);
    let mut taken = taken_in(&[long.as_str()]);
    let free = free_path(&long, ItemKind::File, &mut taken).unwrap();
    assert_eq!(free, Err(PathError::NameTooLong));
    assert_eq!(PathError::NameTooLong.error_code(), "name_too_long");
}

#[test]
fn a_store_failure_stops_the_search() {
    let mut failing = |_: &str| -> Result<bool, &'static str> { Err("down") };
    assert_eq!(
        free_path("a.txt", ItemKind::File, &mut failing),
        Err("down")
    );
}

#[test]
fn a_safe_segment_is_the_api_s_005_name_in_nfc() {
    assert_eq!(safe_segment(" a/b\\c\u{7}.txt ").unwrap(), "a_b_c.txt");
    // "e" + combining acute (NFD) becomes the composed "é" (NFC).
    assert_eq!(safe_segment("Cafe\u{301}.jpg").unwrap(), "Caf\u{e9}.jpg");
    assert_eq!(safe_segment("\u{7}"), Err(PathError::Invalid));
    assert_eq!(safe_segment("."), Err(PathError::Invalid));
    assert_eq!(safe_segment(".."), Err(PathError::Invalid));
}

#[test]
fn a_name_over_255_bytes_after_the_safe_transformation_is_name_too_long() {
    let name = "\u{e9}".repeat(128); // 256 bytes
    assert_eq!(safe_segment(&name), Err(PathError::NameTooLong));
    assert!(safe_segment(&"\u{e9}".repeat(127)).is_ok());
}

#[test]
fn collection_paths_follow_the_profile_rule() {
    for good in ["a", "a/b.jpg", "Caf\u{e9}/x", ".env", "a b/c d.txt"] {
        assert_eq!(check_collection_path(good), Ok(()), "{good}");
    }
    let too_long = vec!["s".repeat(200); 6].join("/");
    assert!(too_long.len() > MAX_PATH_BYTES);
    let bad = [
        "",
        "/a",
        "a/",
        "a//b",
        "./a",
        "a/../b",
        "a\\b",
        "a\u{0}b",
        "a\u{7}b",
        "Cafe\u{301}",
        too_long.as_str(),
    ];
    for path in bad {
        assert_eq!(
            check_collection_path(path),
            Err(PathError::Invalid),
            "{path:?}"
        );
    }
    let segment = "s".repeat(MAX_SEGMENT_BYTES + 1);
    assert_eq!(check_collection_path(&segment), Err(PathError::NameTooLong));
}

#[test]
fn a_destination_folder_is_a_collection_path_outside_the_reserved_folders() {
    assert_eq!(check_folder("trips/2024"), Ok(()));
    assert_eq!(check_folder("provenance"), Err(PathError::Invalid));
    assert_eq!(check_folder(".filesquid/x"), Err(PathError::Invalid));
    assert_eq!(check_folder("a/../b"), Err(PathError::Invalid));
    assert_eq!(PathError::Invalid.error_code(), "invalid_path");
}

#[test]
fn join_puts_a_path_beneath_its_destination_folder() {
    assert_eq!(join(None, "a.jpg"), "a.jpg");
    assert_eq!(join(Some("trips"), "x/a.jpg"), "trips/x/a.jpg");
}
