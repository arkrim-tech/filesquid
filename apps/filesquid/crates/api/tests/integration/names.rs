//! Integration tests of Task API-F1.13 (API-S-027) through the router, against a real OCFL
//! storage root on the local filesystem: the file history and object versions carry the
//! display name, a rename changes only the display name in one new version, a move of a
//! file object's original is refused (and only a file object's), and downloads name the file
//! by an RFC 6266 header whose ASCII fallback falls back to the FileId plus extension.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

use axum::http::{Method, StatusCode};
use files_fixture::{encode, upload_uri, Answer, FilesRig, LIBRARY};
use serde_json::{json, Value};

/// Upload `bytes` as a new file named `name` (NFC), and answer the upload view.
async fn upload(rig: &FilesRig, name: &str, bytes: &[u8]) -> Value {
    let uri = upload_uri(name, bytes.len(), "name_normalization=nfc");
    let answer = rig.call(Method::PUT, &uri, &[], bytes.to_vec()).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.json()
}

/// The query naming the file object of `upload`, then `rest`.
fn object_query(upload: &Value, rest: &str) -> String {
    let object = upload["object_id"].as_str().expect("object");
    format!("library_id={LIBRARY}&object_id={}&{rest}", encode(object))
}

/// The stored path of the original of `upload` with extension `ext`.
fn original(upload: &Value, ext: &str) -> String {
    format!("original/{}.{ext}", upload["file_id"].as_str().expect("id"))
}

/// `POST /api/v1/files/renames` of the file of `upload` to `name`.
async fn rename(rig: &FilesRig, upload: &Value, name: &str) -> Answer {
    let body = json!({
        "library_id": LIBRARY,
        "file_id": upload["file_id"],
        "display_name": name,
    });
    rig.post("/api/v1/files/renames", body).await
}

/// The `Content-Disposition` of the download of `path` in the object of `upload`.
async fn download_header(rig: &FilesRig, upload: &Value, path: &str) -> String {
    let rest = format!("path={}", encode(path));
    let uri = format!("/api/v1/files/download?{}", object_query(upload, &rest));
    let answer = rig.get(&uri).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
    answer.header("content-disposition").to_string()
}

#[tokio::test]
async fn history_and_versions_carry_the_display_name() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "Été à Nice.jpg", b"jpeg").await;
    let path = original(&upload, "jpg");
    let rest = format!("path={}", encode(&path));
    let history = rig
        .get(&format!(
            "/api/v1/files/history?{}",
            object_query(&upload, &rest)
        ))
        .await
        .json();
    assert_eq!(history["display_name"], "Été à Nice.jpg");
    assert_eq!(history["path"], path);
    let versions = rig
        .get(&format!(
            "/api/v1/files/versions?{}",
            object_query(&upload, "")
        ))
        .await
        .json();
    assert_eq!(versions["display_name"], "Été à Nice.jpg");
}

#[tokio::test]
async fn a_rename_changes_only_the_display_name_in_one_new_version() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "IMG_0001.JPG", b"jpeg").await;
    let renamed = rename(&rig, &upload, "Harbour at dawn.JPG").await;
    assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.json());
    assert_eq!(renamed.json()["version"], "v2");
    let path = original(&upload, "JPG");
    let listing = rig
        .get(&format!(
            "/api/v1/files/listing?{}",
            object_query(&upload, "prefix=original")
        ))
        .await
        .json();
    assert_eq!(listing["entries"][0]["path"], path, "the stored path stays");
    assert_eq!(listing["entries"][0]["display_name"], "Harbour at dawn.JPG");
    let header = download_header(&rig, &upload, &path).await;
    assert!(
        header.contains("filename=\"Harbour at dawn.JPG\""),
        "{header}"
    );
    let facts = rig
        .get(&format!(
            "/api/v1/files/facts?library_id={LIBRARY}&file_id={}",
            upload["file_id"].as_str().expect("id")
        ))
        .await
        .json();
    assert!(facts.to_string().contains("IMG_0001.JPG"), "{facts}");
    let journaled = rig.journaled("files.rename");
    assert_eq!(journaled.len(), 1);
    assert!(journaled[0]
        .target_ids
        .iter()
        .all(|id| filesquid_api::journal::is_opaque(id)));
}

#[tokio::test]
async fn a_rename_to_the_same_or_an_invalid_name_publishes_nothing() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "a.txt", b"a").await;
    let same = rename(&rig, &upload, "a.txt").await;
    assert_eq!(same.status.as_u16(), 409, "{:?}", same.json());
    for bad in ["", "a/b.txt", "nul\0.txt"] {
        let refused = rename(&rig, &upload, bad).await;
        assert_eq!(
            refused.status.as_u16(),
            400,
            "{bad:?}: {:?}",
            refused.json()
        );
    }
    let versions = rig
        .get(&format!(
            "/api/v1/files/versions?{}",
            object_query(&upload, "")
        ))
        .await
        .json();
    assert_eq!(versions["head"], "v1", "no version was made");
}

#[tokio::test]
async fn a_move_of_a_file_objects_original_is_refused_with_a_pointer_to_rename() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "a.txt", b"a").await;
    let from = original(&upload, "txt");
    for (from, to) in [(from.as_str(), "b.txt"), ("original", "kept")] {
        let body = json!({
            "library_id": LIBRARY,
            "object_id": upload["object_id"],
            "from": from,
            "to": to,
        });
        let answer = rig.post("/api/v1/files/moves", body).await;
        assert_eq!(answer.status.as_u16(), 400, "{:?}", answer.json());
        let message = answer.json()["message"]
            .as_str()
            .expect("message")
            .to_string();
        assert!(message.contains("/api/v1/files/renames"), "{message}");
    }
}

#[tokio::test]
async fn an_original_folder_in_an_object_that_is_not_a_file_object_still_moves() {
    let rig = FilesRig::with_files(&[("original/a.txt", b"a")]).await;
    let body = json!({
        "library_id": LIBRARY,
        "object_id": files_fixture::OBJECT,
        "from": "original/a.txt",
        "to": "kept/a.txt",
    });
    let answer = rig.post("/api/v1/files/moves", body).await;
    assert_eq!(answer.status, StatusCode::OK, "{:?}", answer.json());
}

#[tokio::test]
async fn download_names_survive_quotes_backslashes_slashes_and_controls() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "Sum\"mer\\2023.jpg", b"jpeg").await;
    let renamed = rename(&rig, &upload, "Sum\"mer\\2023;\t1.jpg").await;
    assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.json());
    let header = download_header(&rig, &upload, &original(&upload, "jpg")).await;
    assert_eq!(
        header,
        "attachment; filename=\"Sum_mer_2023_1.jpg\"; \
         filename*=UTF-8''Sum%22mer_2023%3B1.jpg"
    );
}

#[tokio::test]
async fn a_name_without_an_ascii_letter_or_digit_falls_back_to_the_file_id() {
    let rig = FilesRig::new().await;
    let upload = upload(&rig, "写真.jpg", b"jpeg").await;
    let path = original(&upload, "jpg");
    let header = download_header(&rig, &upload, &path).await;
    let id_name = path.trim_start_matches("original/");
    assert!(
        header.starts_with("attachment; filename=\"__.jpg\""),
        "an extension is an ASCII letter: {header}"
    );
    let renamed = rename(&rig, &upload, "写真 — 夏").await;
    assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.json());
    let header = download_header(&rig, &upload, &path).await;
    assert_eq!(
        header,
        format!(
            "attachment; filename=\"{id_name}\"; \
             filename*=UTF-8''%E5%86%99%E7%9C%9F%20%E2%80%94%20%E5%A4%8F"
        )
    );
}
