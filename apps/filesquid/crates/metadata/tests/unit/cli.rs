//! CLI help and validate.

use std::path::{Path, PathBuf};

use filesquid_metadata::cli::{run, USAGE};

#[test]
fn help_prints_commands() {
    let code = run(&["help".into()]).expect("help");
    assert_eq!(code, 0);
    assert!(USAGE.contains("extract"));
    assert!(USAGE.contains("validate"));
}

#[test]
fn validate_title_ok() {
    let code = run(&["validate".into(), "dcterms:title".into()]).expect("ok");
    assert_eq!(code, 0);
}

#[test]
fn validate_name_fails() {
    let code = run(&["validate".into(), "dcterms:name".into()]).expect("fail");
    assert_eq!(code, 1);
}

#[test]
fn unknown_command_errors() {
    let err = run(&["mods".into()]).expect_err("mods");
    assert!(err.contains("unknown command"));
}

#[test]
fn set_and_get_round_trip() {
    let dir = std::env::temp_dir().join("filesquid-metadata-cli-set");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.to_string_lossy().into_owned();
    let code = run(&[
        "set".into(),
        "dcterms:title".into(),
        "Harbour".into(),
        "--object".into(),
        object.clone(),
    ])
    .expect("set");
    assert_eq!(code, 0);
    let code = run(&[
        "get".into(),
        "dcterms:title".into(),
        "--object".into(),
        object,
    ])
    .expect("get");
    assert_eq!(code, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_csv_contains_header() {
    let dir = std::env::temp_dir().join("filesquid-metadata-cli-csv");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.to_string_lossy().into_owned();
    run(&[
        "set".into(),
        "dcterms:creator".into(),
        "George".into(),
        "--object".into(),
        object.clone(),
    ])
    .unwrap();
    let csv_path = dir.join("out.csv");
    let code = run(&[
        "export".into(),
        "csv".into(),
        csv_path.to_string_lossy().into_owned(),
        "--object".into(),
        object,
    ])
    .expect("export");
    assert_eq!(code, 0);
    let body = std::fs::read_to_string(&csv_path).unwrap();
    assert!(body.contains("dcterms:creator"));
    assert!(body.contains("George"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Convert string slices into owned CLI arguments.
fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_string()).collect()
}

/// Create an empty scratch object directory holding a `photo.jpg` logical file.
fn json_object(name: &str, photo: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "filesquid-metadata-cli-json-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("photo.jpg"), photo).unwrap();
    dir
}

/// Run `<cmd> json <path> --id <id> --file photo.jpg --object <dir>`.
fn run_json(cmd: &str, sidecar: &Path, id: &str, object: &Path) -> Result<i32, String> {
    run(&args(&[
        cmd,
        "json",
        &sidecar.to_string_lossy(),
        "--id",
        id,
        "--file",
        "photo.jpg",
        "--object",
        &object.to_string_lossy(),
    ]))
}

/// Set one statement in the object's `metadata.md` through the CLI.
fn set_in(object: &Path, curie: &str, value: &str) {
    let code = run(&args(&[
        "set",
        curie,
        value,
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect("set");
    assert_eq!(code, 0);
}

/// Remove the scratch objects and the sidecar a JSON test created.
fn cleanup(dirs: &[&Path], sidecar: &Path) {
    for dir in dirs {
        let _ = std::fs::remove_dir_all(dir);
    }
    let _ = std::fs::remove_file(sidecar);
}

#[test]
fn help_lists_json_interchange() {
    assert!(USAGE.contains("export csv|xmp|json"));
    assert!(USAGE.contains("import csv|xmp|json"));
}

#[test]
fn json_export_then_import_round_trips_through_metadata_md() {
    let source = json_object("source", b"pixels");
    set_in(&source, "dcterms:title", "Harbour, \"dawn\"");
    set_in(&source, "schema:album", "Coast");
    let sidecar = source.with_extension("json");
    assert_eq!(run_json("export", &sidecar, "urn:x:1", &source), Ok(0));
    let target = json_object("target", b"pixels");
    assert_eq!(run_json("import", &sidecar, "urn:x:1", &target), Ok(0));
    let a = std::fs::read_to_string(source.join("metadata.md")).unwrap();
    let b = std::fs::read_to_string(target.join("metadata.md")).unwrap();
    assert_eq!(a, b);
    let entries = std::fs::read_dir(&target).unwrap().count();
    assert_eq!(entries, 2, "only photo.jpg and metadata.md in the object");
    cleanup(&[&source, &target], &sidecar);
}

#[test]
fn json_import_rejects_digest_mismatch_and_keeps_metadata_md() {
    let source = json_object("mismatch-src", b"original");
    set_in(&source, "dcterms:title", "Harbour");
    let sidecar = source.with_extension("json");
    assert_eq!(run_json("export", &sidecar, "urn:x:1", &source), Ok(0));
    let target = json_object("mismatch-dst", b"edited");
    set_in(&target, "dcterms:title", "Before");
    let before = std::fs::read_to_string(target.join("metadata.md")).unwrap();
    let err = run_json("import", &sidecar, "urn:x:1", &target).expect_err("mismatch");
    assert!(err.contains("digest"), "{err}");
    let after = std::fs::read_to_string(target.join("metadata.md")).unwrap();
    assert_eq!(before, after);
    cleanup(&[&source, &target], &sidecar);
}

#[test]
fn json_import_rejects_other_object_id() {
    let object = json_object("other-id", b"pixels");
    set_in(&object, "dcterms:title", "Harbour");
    let sidecar = object.with_extension("json");
    assert_eq!(run_json("export", &sidecar, "urn:x:1", &object), Ok(0));
    let err = run_json("import", &sidecar, "urn:x:2", &object).expect_err("id");
    assert!(err.contains("object identifier"), "{err}");
    cleanup(&[&object], &sidecar);
}

#[test]
fn json_requires_id_and_file() {
    let object = json_object("flags", b"pixels");
    let out = object.join("out.json");
    let err = run(&args(&[
        "export",
        "json",
        &out.to_string_lossy(),
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect_err("flags required");
    assert!(err.contains("--id") && err.contains("--file"), "{err}");
    cleanup(&[&object], &out);
}

#[test]
fn csv_export_still_ignores_json_flags() {
    let object = json_object("csv-flags", b"pixels");
    set_in(&object, "dcterms:title", "Harbour");
    let out = object.with_extension("csv");
    let code = run(&args(&[
        "export",
        "csv",
        &out.to_string_lossy(),
        "--id",
        "urn:x:1",
        "--file",
        "photo.jpg",
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect("csv");
    assert_eq!(code, 0);
    assert!(std::fs::read_to_string(&out).unwrap().contains("Harbour"));
    cleanup(&[&object], &out);
}

#[test]
fn json_rejects_a_flag_as_the_id_value() {
    let object = json_object("flag-as-value", b"pixels");
    let out = object.join("out.json");
    let err = run(&args(&[
        "export",
        "json",
        &out.to_string_lossy(),
        "--id",
        "--file",
        "photo.jpg",
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect_err("--id has no value");
    assert!(err.contains("--id"), "{err}");
    assert!(!out.exists(), "no sidecar is written");
    cleanup(&[&object], &out);
}

#[test]
fn set_keeps_json_flag_names_as_literal_values() {
    let object = json_object("literal-flag", b"pixels");
    set_in(&object, "dcterms:title", "--file");
    let body = std::fs::read_to_string(object.join("metadata.md")).unwrap();
    assert!(body.contains("--file"), "{body}");
    cleanup(&[&object], &object.join("unused.json"));
}

#[cfg(unix)]
#[test]
fn json_rejects_a_symlink_that_leaves_the_object() {
    let object = json_object("symlink-escape", b"pixels");
    let outside = object.with_extension("outside.bin");
    std::fs::write(&outside, b"secret").unwrap();
    std::os::unix::fs::symlink(&outside, object.join("link.jpg")).unwrap();
    let out = object.join("out.json");
    let err = run(&args(&[
        "export",
        "json",
        &out.to_string_lossy(),
        "--id",
        "urn:x:1",
        "--file",
        "link.jpg",
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect_err("symlink escape");
    assert!(err.contains("outside the object"), "{err}");
    std::fs::remove_file(&outside).ok();
    cleanup(&[&object], &out);
}

/// Big-endian TIFF with one ASCII `ImageDescription` ("Cat").
const TIFF_DESCRIPTION: &[u8] = &[
    b'M', b'M', 0, 42, 0, 0, 0, 8, 0, 1, 0x01, 0x0E, 0, 2, 0, 0, 0, 4, b'C', b'a', b't', 0, 0, 0,
    0, 0,
];

#[test]
fn extract_reads_the_file_through_a_reader_into_metadata_md() {
    let dir = std::env::temp_dir().join(format!(
        "filesquid-metadata-cli-extract-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let photo = dir.join("cat.tif");
    std::fs::write(&photo, TIFF_DESCRIPTION).unwrap();
    let object = dir.join("object");
    std::fs::create_dir_all(&object).unwrap();
    let args = [
        "extract".to_string(),
        photo.to_string_lossy().into_owned(),
        "--object".to_string(),
        object.to_string_lossy().into_owned(),
    ];
    assert_eq!(run(&args).expect("extract"), 0);
    let text = std::fs::read_to_string(object.join("metadata.md")).unwrap();
    assert!(text.contains("ImageDescription"), "{text}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn extract_reports_a_missing_file() {
    let dir = std::env::temp_dir().join(format!(
        "filesquid-metadata-cli-missing-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let args = [
        "extract".to_string(),
        dir.join("absent.jpg").to_string_lossy().into_owned(),
        "--object".to_string(),
        dir.to_string_lossy().into_owned(),
    ];
    let err = run(&args).expect_err("missing file is reported");
    assert!(err.contains("absent.jpg"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}
