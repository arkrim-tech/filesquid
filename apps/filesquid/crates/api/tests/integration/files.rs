//! Integration tests of the file operation routes (Task API-F1.3; API-S-004, API-S-005,
//! API-S-008, API-S-017, OCFL-S-012) through the router, against a real OCFL storage root on
//! the local filesystem: folder listing with cursor paging, file details, version history,
//! ranged reads, upload, create folder, move, remove and restore.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

use axum::http::{Method, StatusCode};
use files_fixture::{
    digest, encode, pattern, query, upload_uri, Answer, FilesRig, LIBRARY, OBJECT,
};
use filesquid_api::files::{
    DisplayNames, NameFuture, StoredFile, DEFAULT_PAGE, MAX_ANSWER_BYTES, MAX_UPLOAD_BYTES,
};
use filesquid_api::journal::Outcome;
use serde_json::{json, Value};

/// A JSON body naming the rig's library and object plus `fields`.
fn body(fields: Value) -> Value {
    let mut all = json!({ "library_id": LIBRARY, "object_id": OBJECT });
    for (key, value) in fields.as_object().expect("fields").iter() {
        all[key] = value.clone();
    }
    all
}

/// Assert `answer` is the typed error `code`.
fn assert_error(answer: &files_fixture::Answer, status: u16, code: &str) {
    assert_eq!(answer.status.as_u16(), status, "{:?}", answer.json());
    assert_eq!(answer.json()["code"], code, "{:?}", answer.json());
}

// ---- Listing ----

#[tokio::test]
async fn the_root_lists_folders_first_then_files() {
    let rig = FilesRig::with_files(&[
        ("readme.txt", b"hello"),
        ("photos/a.jpg", b"a"),
        ("photos/2026/b.jpg", b"b"),
    ])
    .await;
    let answer = rig
        .get(&format!("/api/v1/files/listing?{}", query("")))
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let page = answer.json();
    assert_eq!(page["library_id"], LIBRARY);
    assert_eq!(page["object_id"], OBJECT);
    assert_eq!(page["version"], "v1");
    assert_eq!(page["prefix"], "");
    let entries = page["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["kind"], "folder");
    assert_eq!(entries[0]["name"], "photos");
    assert_eq!(entries[0]["path"], "photos");
    assert_eq!(entries[1]["kind"], "file");
    assert_eq!(entries[1]["path"], "readme.txt");
    assert_eq!(entries[1]["size"], 5);
    assert_eq!(entries[1]["digest"], digest(b"hello"));
    assert_eq!(entries[1]["modified_version"], "v1");
    assert_eq!(entries[1]["type_hint"], "txt");
    assert_eq!(page["next_cursor"], Value::Null);
}

#[tokio::test]
async fn a_prefix_lists_one_folder_level() {
    let rig = FilesRig::with_files(&[("photos/a.jpg", b"a"), ("photos/2026/b.jpg", b"b")]).await;
    let uri = format!("/api/v1/files/listing?{}", query("prefix=photos"));
    let page = rig.get(&uri).await.json();
    let paths: Vec<&str> = page["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|e| e["path"].as_str().expect("path"))
        .collect();
    assert_eq!(paths, ["photos/2026", "photos/a.jpg"]);
}

#[tokio::test]
async fn pages_follow_the_opaque_cursor_without_a_repeat_or_gap() {
    let rig = FilesRig::with_files(&[("a.txt", b"a"), ("b.txt", b"b"), ("c.txt", b"c")]).await;
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..5 {
        let rest = match &cursor {
            Some(c) => format!("limit=2&cursor={}", files_fixture::encode(c)),
            None => "limit=2".to_string(),
        };
        let page = rig
            .get(&format!("/api/v1/files/listing?{}", query(&rest)))
            .await
            .json();
        for entry in page["entries"].as_array().expect("entries") {
            seen.push(entry["path"].as_str().expect("path").to_string());
        }
        cursor = page["next_cursor"].as_str().map(str::to_string);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, ["a.txt", "b.txt", "c.txt"]);
}

#[tokio::test]
async fn a_listing_holds_the_default_page_of_200() {
    let names: Vec<String> = (0..201).map(|i| format!("f{i:03}.txt")).collect();
    let files: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b"x"[..])).collect();
    let rig = FilesRig::with_files(&files).await;
    let page = rig
        .get(&format!("/api/v1/files/listing?{}", query("")))
        .await
        .json();
    assert_eq!(DEFAULT_PAGE, 200);
    assert_eq!(page["entries"].as_array().expect("entries").len(), 200);
    assert!(page["next_cursor"].is_string());
}

#[tokio::test]
async fn a_limit_outside_1_to_1000_or_a_malformed_cursor_is_refused() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    for rest in ["limit=0", "limit=1001", "limit=x", "cursor=not-a-cursor"] {
        let answer = rig
            .get(&format!("/api/v1/files/listing?{}", query(rest)))
            .await;
        assert_error(&answer, 400, "invalid_request");
    }
    let refusals = rig.journaled("files.list");
    assert_eq!(refusals.len(), 4, "every refusal is journaled");
    assert!(refusals.iter().all(
        |e| e.outcome == Outcome::Failed && e.error_code.as_deref() == Some("invalid_request")
    ));
}

#[tokio::test]
async fn an_unknown_library_object_version_or_folder_is_not_found_and_journaled() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let unknown_library = "library_id=00000000-0000-4000-8000-000000000000&object_id=x";
    let cases = [
        format!("/api/v1/files/listing?{unknown_library}"),
        format!("/api/v1/files/listing?library_id={LIBRARY}&object_id=urn%3Auuid%3Aunknown"),
        format!("/api/v1/files/listing?{}", query("version=v9")),
        format!("/api/v1/files/listing?{}", query("prefix=missing")),
    ];
    for uri in &cases {
        assert_error(&rig.get(uri).await, 404, "not_found");
    }
    assert_eq!(rig.journaled("files.list").len(), cases.len());
}

// ---- Details, history and versions ----

#[tokio::test]
async fn details_answer_storage_facts_without_any_storage_path() {
    let rig = FilesRig::with_files(&[("photos/a.jpg", b"jpeg bytes")]).await;
    let uri = format!("/api/v1/files/details?{}", query("path=photos%2Fa.jpg"));
    let answer = rig.get(&uri).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let details = answer.json();
    assert_eq!(details["path"], "photos/a.jpg");
    assert_eq!(details["name"], "a.jpg");
    assert_eq!(details["version"], "v1");
    assert_eq!(details["size"], 10);
    assert_eq!(details["digest"], digest(b"jpeg bytes"));
    assert_eq!(details["digest_algorithm"], "sha256");
    assert_eq!(details["last_change"]["version"], "v1");
    assert_eq!(details["media_type"], "image/jpeg");
    let text = String::from_utf8(answer.body.clone()).expect("utf-8");
    assert!(!text.contains("content_path"), "{text}");
    assert!(!text.contains("v1/content"), "no OCFL content path: {text}");
    assert!(!text.contains("objects/holiday"), "no storage key: {text}");
    assert!(
        !text.contains(rig.dir.to_str().expect("dir")),
        "no filesystem path"
    );
}

#[tokio::test]
async fn history_lists_the_versions_that_changed_a_file() {
    let rig = FilesRig::with_files(&[("a.txt", b"one")]).await;
    rig.commit(&[("b.txt", b"other")]).await;
    rig.commit(&[("a.txt", b"two")]).await;
    let uri = format!("/api/v1/files/history?{}", query("path=a.txt"));
    let history = rig.get(&uri).await.json();
    let entries = history["entries"].as_array().expect("entries");
    let versions: Vec<&str> = entries
        .iter()
        .map(|e| e["version"]["version"].as_str().expect("version"))
        .collect();
    assert_eq!(versions, ["v1", "v3"]);
    assert_eq!(entries[0]["change"], "added");
    assert_eq!(entries[1]["change"], "changed");
    assert_eq!(entries[1]["digest"], digest(b"two"));
}

#[tokio::test]
async fn versions_list_every_version_and_the_head() {
    let rig = FilesRig::with_files(&[("a.txt", b"one")]).await;
    rig.commit(&[("b.txt", b"two")]).await;
    let answer = rig
        .get(&format!("/api/v1/files/versions?{}", query("")))
        .await
        .json();
    assert_eq!(answer["head"], "v2");
    let versions = answer["versions"].as_array().expect("versions");
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0]["version"], "v1");
    assert!(versions[0]["created"]
        .as_str()
        .expect("created")
        .ends_with('Z'));
}

// ---- Ranged reads ----

#[tokio::test]
async fn a_read_without_a_range_answers_the_whole_small_file() {
    let rig = FilesRig::with_files(&[("a.txt", b"hello world")]).await;
    let answer = rig
        .get(&format!("/api/v1/files/content?{}", query("path=a.txt")))
        .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, b"hello world");
    assert_eq!(answer.header("accept-ranges"), "bytes");
    assert_eq!(answer.header("content-type"), "text/plain");
    assert_eq!(answer.header("content-length"), "11");
}

#[tokio::test]
async fn single_ranges_answer_206_with_their_content_range() {
    let bytes = pattern(1000);
    let rig = FilesRig::with_files(&[("a.bin", &bytes)]).await;
    let uri = format!("/api/v1/files/content?{}", query("path=a.bin"));
    let cases = [
        ("bytes=0-9", 0usize, 9usize),
        ("bytes=990-", 990, 999),
        ("bytes=-10", 990, 999),
        ("bytes=500-2000", 500, 999),
    ];
    for (range, start, end) in cases {
        let answer = rig
            .call(Method::GET, &uri, &[("range", range)], Vec::new())
            .await;
        assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT, "{range}");
        assert_eq!(
            answer.header("content-range"),
            format!("bytes {start}-{end}/1000"),
            "{range}"
        );
        assert_eq!(answer.body, bytes[start..=end], "{range}");
    }
}

#[tokio::test]
async fn an_unsatisfiable_range_answers_416_with_the_length() {
    let rig = FilesRig::with_files(&[("a.bin", &pattern(100))]).await;
    let uri = format!("/api/v1/files/content?{}", query("path=a.bin"));
    for range in ["bytes=100-", "bytes=200-300", "bytes=-0"] {
        let answer = rig
            .call(Method::GET, &uri, &[("range", range)], Vec::new())
            .await;
        assert_eq!(answer.status.as_u16(), 416, "{range}");
        assert_eq!(answer.header("content-range"), "bytes */100", "{range}");
        assert_eq!(answer.json()["code"], "range_not_satisfiable", "{range}");
    }
}

#[tokio::test]
async fn a_malformed_or_multi_range_header_is_ignored() {
    let rig = FilesRig::with_files(&[("a.bin", &pattern(100))]).await;
    let uri = format!("/api/v1/files/content?{}", query("path=a.bin"));
    for range in ["bytes=0-1,5-6", "items=0-1", "bytes=x-y", "bytes=5-1"] {
        let answer = rig
            .call(Method::GET, &uri, &[("range", range)], Vec::new())
            .await;
        assert_eq!(answer.status, StatusCode::OK, "{range}");
        assert_eq!(answer.body, pattern(100), "{range}: the whole file");
    }
    let repeated = rig
        .call(
            Method::GET,
            &uri,
            &[("range", "bytes=0-1"), ("range", "bytes=5-6")],
            Vec::new(),
        )
        .await;
    assert_eq!(
        repeated.status,
        StatusCode::OK,
        "two Range fields are two ranges"
    );
    assert_eq!(repeated.body, pattern(100));
    let upper = rig
        .call(Method::GET, &uri, &[("range", "Bytes=2-5")], Vec::new())
        .await;
    assert_eq!(
        upper.status,
        StatusCode::PARTIAL_CONTENT,
        "the unit is case-insensitive"
    );
    assert_eq!(upper.header("content-range"), "bytes 2-5/100");
}

#[tokio::test]
async fn answers_are_capped_at_16_mib() {
    let len = MAX_ANSWER_BYTES + 1_000_000;
    let bytes = pattern(len);
    let rig = FilesRig::with_files(&[("big.bin", &bytes)]).await;
    let uri = format!("/api/v1/files/content?{}", query("path=big.bin"));
    let whole = rig.get(&uri).await;
    assert_eq!(whole.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(whole.body.len(), MAX_ANSWER_BYTES);
    let expected = format!("bytes 0-{}/{len}", MAX_ANSWER_BYTES - 1);
    assert_eq!(whole.header("content-range"), expected);
    assert_eq!(whole.body, bytes[..MAX_ANSWER_BYTES]);
    let open = rig
        .call(Method::GET, &uri, &[("range", "bytes=100-")], Vec::new())
        .await;
    assert_eq!(open.status, StatusCode::PARTIAL_CONTENT);
    let open_range = format!("bytes 100-{}/{len}", 100 + MAX_ANSWER_BYTES - 1);
    assert_eq!(open.header("content-range"), open_range);
    assert_eq!(open.body.len(), MAX_ANSWER_BYTES);
    assert_eq!(open.body, bytes[100..100 + MAX_ANSWER_BYTES]);
}

#[tokio::test]
async fn head_describes_the_file_without_reading_its_content() {
    let rig = FilesRig::with_files(&[("clip.mp4", &pattern(5000))]).await;
    let uri = format!("/api/v1/files/content?{}", query("path=clip.mp4"));
    let before = rig.root.content_reads();
    let answer = rig.call(Method::HEAD, &uri, &[], Vec::new()).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.is_empty());
    assert_eq!(answer.header("content-length"), "5000");
    assert_eq!(answer.header("content-type"), "video/mp4");
    assert_eq!(answer.header("accept-ranges"), "bytes");
    assert_eq!(rig.root.content_reads(), before, "HEAD reads no content");
}

#[tokio::test]
async fn head_with_a_range_describes_the_get_for_that_range() {
    let rig = FilesRig::with_files(&[("clip.mp4", &pattern(5000))]).await;
    let uri = format!("/api/v1/files/content?{}", query("path=clip.mp4"));
    let before = rig.root.content_reads();
    let part = rig
        .call(Method::HEAD, &uri, &[("range", "bytes=10-19")], Vec::new())
        .await;
    assert_eq!(part.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(part.header("content-range"), "bytes 10-19/5000");
    assert_eq!(part.header("content-length"), "10");
    assert_eq!(part.header("accept-ranges"), "bytes");
    assert_eq!(part.header("content-type"), "video/mp4");
    assert!(part.body.is_empty());
    let past = rig
        .call(Method::HEAD, &uri, &[("range", "bytes=5000-")], Vec::new())
        .await;
    assert_eq!(past.status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(past.header("content-range"), "bytes */5000");
    assert!(past.body.is_empty(), "a HEAD 416 has no body");
    assert_eq!(rig.root.content_reads(), before, "HEAD reads no content");
}

#[tokio::test]
async fn a_file_length_is_read_once_then_cached() {
    let rig = FilesRig::with_files(&[("clip.mp4", &pattern(5000))]).await;
    let uri = format!("/api/v1/files/content?{}", query("path=clip.mp4"));
    let before = rig.root.content_heads();
    for range in ["bytes=0-9", "bytes=10-19", "bytes=-5"] {
        let answer = rig
            .call(Method::GET, &uri, &[("range", range)], Vec::new())
            .await;
        assert_eq!(answer.status, StatusCode::PARTIAL_CONTENT, "{range}");
    }
    rig.call(Method::HEAD, &uri, &[], Vec::new()).await;
    let heads = rig.root.content_heads() - before;
    assert_eq!(heads, 1, "the backend head runs once per content");
}

#[tokio::test]
async fn a_read_pins_the_version_it_names() {
    let rig = FilesRig::with_files(&[("a.txt", b"first")]).await;
    rig.commit(&[("a.txt", b"second")]).await;
    let old = rig
        .get(&format!(
            "/api/v1/files/content?{}",
            query("path=a.txt&version=v1")
        ))
        .await;
    assert_eq!(old.body, b"first");
    let head = rig
        .get(&format!("/api/v1/files/content?{}", query("path=a.txt")))
        .await;
    assert_eq!(head.body, b"second");
}

// ---- Writes ----

#[tokio::test]
async fn an_upload_creates_its_own_file_object_and_is_journaled() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let uri = upload_uri("b.txt", 3, "");
    let answer = rig.call(Method::PUT, &uri, &[], b"bee".to_vec()).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    let upload = answer.json();
    let file_id = upload["file_id"].as_str().expect("file id").to_string();
    let object_id = format!("urn:uuid:{file_id}");
    assert_eq!(upload["object_id"], object_id.as_str());
    assert_eq!(upload["version"], "v1");
    assert_eq!(rig.head().await, "v1", "the other object is untouched");
    let read = format!(
        "/api/v1/files/content?library_id={LIBRARY}&object_id={}&path={}",
        encode(&object_id),
        encode(&format!("original/{file_id}.txt"))
    );
    let answer = rig.get(&read).await;
    assert_eq!(answer.body, b"bee");
    assert_eq!(answer.header("content-type"), "text/plain");
    assert!(answer
        .header("content-disposition")
        .contains("filename=\"b.txt\""));
    let entries = rig.journaled("files.upload");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
    assert_eq!(entries[0].actor, "owner");
    assert!(entries[0].target_ids.contains(&"v1".to_string()));
    assert!(entries[0].target_ids.contains(&LIBRARY.to_string()));
    let object_ref = format!("{object_id}@v1");
    assert_eq!(entries[0].object_ref.as_deref(), Some(object_ref.as_str()));
}

#[tokio::test]
async fn an_upload_whose_declared_size_differs_commits_nothing() {
    let rig = FilesRig::with_files(&[("a.txt", b"same")]).await;
    let uri = upload_uri("b.txt", 5, "");
    let answer = rig.call(Method::PUT, &uri, &[], b"four".to_vec()).await;
    assert_error(&answer, 400, "invalid_request");
    let refused = rig.journaled("files.upload");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].outcome, Outcome::Failed);
    let objects = std::fs::read_dir(rig.dir.join("root").join("files"));
    assert!(objects.is_err(), "no file object was created");
}

/// Assert `answer` refused a write that would change nothing with `status` and `code`, and
/// that `operation` was journaled exactly once, as `failed` with that code and never as a
/// success. A file write never succeeds without a change, so the no-op rule of API-S-019 and
/// ADR 033 (a command that succeeds without a change stores nothing) never applies to it.
fn assert_refused_no_op(
    rig: &FilesRig,
    answer: &Answer,
    (status, code): (u16, &str),
    operation: &str,
) {
    assert_error(answer, status, code);
    let entries = rig.journaled(operation);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].outcome, Outcome::Failed);
    assert_eq!(entries[0].error_code.as_deref(), Some(code));
}

#[tokio::test]
async fn a_move_to_the_same_path_is_refused_without_a_version() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let answer = rig
        .post(
            "/api/v1/files/moves",
            body(json!({ "from": "a.txt", "to": "a.txt" })),
        )
        .await;
    // The engine refuses a move onto a path that is taken, its own included.
    assert_refused_no_op(&rig, &answer, (400, "invalid_request"), "files.move");
    assert_eq!(rig.head().await, "v1");
}

#[tokio::test]
async fn a_restore_of_the_current_version_is_refused_without_a_version() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let answer = rig
        .post(
            "/api/v1/files/restorations",
            body(json!({ "version": "v1", "paths": ["a.txt"] })),
        )
        .await;
    assert_refused_no_op(&rig, &answer, (409, "conflict"), "files.restore");
    assert_eq!(rig.head().await, "v1");
}

#[tokio::test]
async fn an_upload_above_the_cap_is_refused() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let uri = upload_uri("big.bin", MAX_UPLOAD_BYTES + 1, "");
    let answer = rig
        .call(Method::PUT, &uri, &[], vec![0u8; MAX_UPLOAD_BYTES + 1])
        .await;
    assert_error(&answer, 400, "invalid_request");
    assert_eq!(rig.head().await, "v1");
    assert_eq!(rig.journaled("files.upload").len(), 1);
}

#[tokio::test]
async fn creating_a_folder_commits_nothing_and_stays_pending() {
    let rig = FilesRig::with_files(&[("photos/a.jpg", b"a")]).await;
    let pending = rig
        .post(
            "/api/v1/files/folders",
            body(json!({ "path": "albums/2026" })),
        )
        .await;
    assert_eq!(pending.status, StatusCode::OK, "{:?}", pending.json());
    assert_eq!(pending.json()["pending"], true);
    assert_eq!(pending.json()["path"], "albums/2026");
    let existing = rig
        .post("/api/v1/files/folders", body(json!({ "path": "photos" })))
        .await;
    assert_eq!(existing.json()["pending"], false);
    let clash = rig
        .post(
            "/api/v1/files/folders",
            body(json!({ "path": "photos/a.jpg" })),
        )
        .await;
    assert_error(&clash, 409, "conflict");
    assert_eq!(rig.head().await, "v1", "OCFL stores no empty folder");
    let listing = rig
        .get(&format!("/api/v1/files/listing?{}", query("prefix=albums")))
        .await;
    assert_error(&listing, 404, "not_found");
}

#[tokio::test]
async fn a_file_added_beneath_a_pending_folder_persists_it() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    rig.post("/api/v1/files/folders", body(json!({ "path": "albums" })))
        .await;
    rig.commit(&[("albums/b.txt", b"b")]).await;
    let listing = rig
        .get(&format!("/api/v1/files/listing?{}", query("prefix=albums")))
        .await;
    assert_eq!(listing.status, StatusCode::OK);
    let json = listing.json();
    let paths: Vec<&str> = json["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .filter_map(|e| e["path"].as_str())
        .collect();
    assert_eq!(paths, ["albums/b.txt"]);
    let read = format!("/api/v1/files/content?{}", query("path=albums%2Fb.txt"));
    assert_eq!(rig.get(&read).await.body, b"b");
    let slash = format!("/api/v1/files/listing?{}", query("prefix=albums%2F"));
    assert_eq!(
        rig.get(&slash).await.status,
        StatusCode::OK,
        "a trailing slash names the folder"
    );
}

#[tokio::test]
async fn a_move_renames_a_file_or_a_whole_folder_in_one_version() {
    let rig = FilesRig::with_files(&[
        ("a.txt", b"a"),
        ("photos/x.jpg", b"x"),
        ("photos/y.jpg", b"y"),
    ])
    .await;
    let file = rig
        .post(
            "/api/v1/files/moves",
            body(json!({ "from": "a.txt", "to": "b.txt" })),
        )
        .await;
    assert_eq!(file.json()["version"], "v2", "{:?}", file.json());
    let folder = rig
        .post(
            "/api/v1/files/moves",
            body(json!({ "from": "photos", "to": "pictures" })),
        )
        .await;
    assert_eq!(folder.json()["version"], "v3", "{:?}", folder.json());
    let listing = rig
        .get(&format!(
            "/api/v1/files/listing?{}",
            query("prefix=pictures")
        ))
        .await
        .json();
    assert_eq!(listing["entries"].as_array().expect("entries").len(), 2);
    for (moved, status) in [
        ("b.txt", 200),
        ("pictures/x.jpg", 200),
        ("a.txt", 404),
        ("photos/x.jpg", 404),
        ("photos/y.jpg", 404),
    ] {
        let uri = format!("/api/v1/files/details?{}", query(&format!("path={moved}")));
        assert_eq!(rig.get(&uri).await.status.as_u16(), status, "{moved}");
    }
    assert_eq!(rig.journaled("files.move").len(), 2);
}

#[tokio::test]
async fn the_read_routes_refuse_an_invalid_path_before_any_lookup() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let reads_before = rig.root.reads();
    for route in ["content", "details", "history"] {
        for path in ["../a.txt", "/a.txt", "a.txt/", "a//b.txt", "./a.txt"] {
            let uri = format!(
                "/api/v1/files/{route}?{}",
                query(&format!("path={}", files_fixture::encode(path)))
            );
            assert_error(&rig.get(&uri).await, 400, "invalid_request");
        }
    }
    let listing = format!("/api/v1/files/listing?{}", query("prefix=..%2Fx"));
    assert_error(&rig.get(&listing).await, 400, "invalid_request");
    let restore = rig
        .post(
            "/api/v1/files/restorations",
            body(json!({ "version": "v1", "paths": ["a.txt", "../b.txt"] })),
        )
        .await;
    assert_error(&restore, 400, "invalid_request");
    assert_eq!(rig.root.reads(), reads_before, "nothing was read");
    assert_eq!(rig.head().await, "v1");
}

#[tokio::test]
async fn a_remove_and_a_restore_each_publish_one_version() {
    let rig = FilesRig::with_files(&[("a.txt", b"a"), ("b.txt", b"b")]).await;
    let removed = rig
        .post("/api/v1/files/removals", body(json!({ "path": "a.txt" })))
        .await;
    assert_eq!(removed.json()["version"], "v2", "{:?}", removed.json());
    let gone = rig
        .get(&format!("/api/v1/files/details?{}", query("path=a.txt")))
        .await;
    assert_error(&gone, 404, "not_found");
    let restored = rig
        .post(
            "/api/v1/files/restorations",
            body(json!({ "version": "v1", "paths": ["a.txt"] })),
        )
        .await;
    assert_eq!(restored.json()["version"], "v3", "{:?}", restored.json());
    let back = rig
        .get(&format!("/api/v1/files/content?{}", query("path=a.txt")))
        .await;
    assert_eq!(back.body, b"a");
}

#[tokio::test]
async fn a_move_of_a_missing_path_is_not_found_and_publishes_nothing() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let answer = rig
        .post(
            "/api/v1/files/moves",
            body(json!({ "from": "nope.txt", "to": "b.txt" })),
        )
        .await;
    assert_error(&answer, 404, "not_found");
    assert_eq!(rig.head().await, "v1");
    let refused = rig.journaled("files.move");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].error_code.as_deref(), Some("not_found"));
}

#[tokio::test]
async fn a_request_body_with_an_unknown_field_is_invalid() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let answer = rig
        .post(
            "/api/v1/files/removals",
            body(json!({ "path": "a.txt", "force": true })),
        )
        .await;
    assert_error(&answer, 400, "invalid_request");
    let invalid_path = rig
        .post(
            "/api/v1/files/removals",
            body(json!({ "path": "../a.txt" })),
        )
        .await;
    assert_error(&invalid_path, 400, "invalid_request");
    assert_eq!(rig.head().await, "v1");
}

// ---- Authorisation ----

#[tokio::test]
async fn file_routes_need_the_token_and_refusals_are_journaled() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let uri = format!("/api/v1/files/content?{}", query("path=a.txt"));
    let anonymous = files_fixture::send(&rig.state, Method::GET, &uri, &[], Vec::new()).await;
    assert_error(&anonymous, 401, "unauthorized");
    let upload_uri = upload_uri("a.txt", 1, "");
    let upload = files_fixture::send(
        &rig.state,
        Method::PUT,
        &upload_uri,
        &[("authorization", "Bearer wrong")],
        b"x".to_vec(),
    )
    .await;
    assert_error(&upload, 403, "forbidden");
    assert_eq!(rig.head().await, "v1");
    let denied = rig.journaled("api.auth.denied");
    let targets: Vec<&str> = denied
        .iter()
        .flat_map(|e| e.target_ids.iter().map(String::as_str))
        .collect();
    assert_eq!(targets, ["files.read", "files.write"]);
}

#[tokio::test]
async fn no_answer_or_journal_entry_carries_a_filesystem_path() {
    let rig = FilesRig::with_files(&[("photos/a.jpg", b"a")]).await;
    let dir = rig.dir.to_str().expect("dir").to_string();
    let uris = [
        format!("/api/v1/files/listing?{}", query("")),
        format!("/api/v1/files/details?{}", query("path=photos%2Fa.jpg")),
        format!("/api/v1/files/history?{}", query("path=photos%2Fa.jpg")),
        format!("/api/v1/files/versions?{}", query("")),
        format!("/api/v1/files/details?{}", query("path=missing.jpg")),
    ];
    for uri in &uris {
        let answer = rig.get(uri).await;
        let text = String::from_utf8_lossy(&answer.body).to_string();
        assert!(!text.contains(&dir), "{uri}: {text}");
        assert!(!text.contains("objects/holiday"), "{uri}: {text}");
    }
    let journal = serde_json::to_string(&rig.journal()).expect("json");
    assert!(!journal.contains(&dir));
    assert!(
        !journal.contains("photos/a.jpg"),
        "logical paths stay out of the journal"
    );
}

// ---- Display names (owner requirement, 2026-10-19) ----

/// File records standing in for the host's: stored path to the owner's original name.
struct Records(Vec<(&'static str, &'static str)>);

impl DisplayNames for Records {
    fn original_name<'a>(&'a self, file: StoredFile<'a>) -> NameFuture<'a> {
        let found = self
            .0
            .iter()
            .find(|(path, _)| *path == file.path)
            .map(|(_, name)| name.to_string());
        Box::pin(async move { found })
    }
}

/// A rig whose object stores `holiday-2023.jpg`, whose original name is
/// `Été à Nice 2023.jpg`.
async fn named_rig() -> FilesRig {
    let rig =
        FilesRig::with_files(&[("photos/holiday-2023.jpg", b"jpeg"), ("notes.txt", b"n")]).await;
    let records = Records(vec![("photos/holiday-2023.jpg", "Été à Nice 2023.jpg")]);
    assert!(rig
        .service
        .install_display_names(std::sync::Arc::new(records)));
    rig
}

/// The RFC 6266 header for `Été à Nice 2023.jpg` offered as `kind`.
fn nice_header(kind: &str) -> String {
    format!(
        "{kind}; filename=\"_t_ _ Nice 2023.jpg\"; \
         filename*=UTF-8''%C3%89t%C3%A9%20%C3%A0%20Nice%202023.jpg"
    )
}

#[tokio::test]
async fn listings_and_details_show_the_original_name_not_the_stored_one() {
    let rig = named_rig().await;
    let page = rig
        .get(&format!("/api/v1/files/listing?{}", query("prefix=photos")))
        .await
        .json();
    let entry = &page["entries"][0];
    assert_eq!(entry["display_name"], "Été à Nice 2023.jpg");
    assert_eq!(
        entry["name"], "holiday-2023.jpg",
        "the stored name stays available"
    );
    let uri = format!(
        "/api/v1/files/details?{}",
        query("path=photos%2Fholiday-2023.jpg")
    );
    let details = rig.get(&uri).await.json();
    assert_eq!(details["display_name"], "Été à Nice 2023.jpg");
    let plain = rig
        .get(&format!(
            "/api/v1/files/details?{}",
            query("path=notes.txt")
        ))
        .await
        .json();
    assert_eq!(
        plain["display_name"], "notes.txt",
        "no record: the stored name"
    );
}

#[tokio::test]
async fn reads_and_downloads_are_named_by_the_original_name() {
    let rig = named_rig().await;
    let base = format!(
        "/api/v1/files/content?{}",
        query("path=photos%2Fholiday-2023.jpg")
    );
    let inline = rig.get(&base).await;
    assert_eq!(inline.header("content-disposition"), nice_header("inline"));
    let download = rig.get(&format!("{base}&download=true")).await;
    assert_eq!(
        download.header("content-disposition"),
        nice_header("attachment")
    );
    let head = rig
        .call(
            Method::HEAD,
            &format!("{base}&download=true"),
            &[],
            Vec::new(),
        )
        .await;
    assert_eq!(
        head.header("content-disposition"),
        nice_header("attachment")
    );
    let ranged = rig
        .call(Method::GET, &base, &[("range", "bytes=0-1")], Vec::new())
        .await;
    assert_eq!(ranged.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(ranged.header("content-disposition"), nice_header("inline"));
}

#[tokio::test]
async fn a_hostile_original_name_never_reaches_a_header_as_a_path() {
    let rig = FilesRig::with_files(&[("a.txt", b"a")]).await;
    let records = Records(vec![("a.txt", "../../etc/passwd\r\nX-Evil: 1")]);
    rig.service
        .install_display_names(std::sync::Arc::new(records));
    let answer = rig
        .get(&format!(
            "/api/v1/files/content?{}",
            query("path=a.txt&download=true")
        ))
        .await;
    let header = answer.header("content-disposition");
    assert!(
        header.starts_with("attachment; filename=\".._.._etc_passwdX-Evil: 1\""),
        "{header}"
    );
    assert!(!header.contains('/') && !header.contains('\r') && !header.contains('\n'));
    assert!(answer.header("x-evil").is_empty());
}
