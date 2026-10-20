//! The metadata commands of `fsquid`: help and validate, and every command
//! that reads or writes `metadata.md` through `filesquid-ocfl`, one new OCFL
//! version per edit (MD-0.2; MD-S-005, MD-S-006, MD-S-008, MD-S-010).

use std::path::{Path, PathBuf};

use filesquid_metadata::cli::{run, USAGE};
use filesquid_metadata::markdown::to_markdown;
use filesquid_metadata::store::ocfl::LocalObject;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{complete_immediately, FsBackend, MemoryBackend};
use filesquid_ocfl::write::{rfc3339_utc, LeaseConfig, LeaseStore, SystemClock};

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

/// Convert string slices into owned CLI arguments.
fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_string()).collect()
}

/// A fresh scratch directory named after the test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("md-0.2-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Commit `files` as v1 of a new OCFL object `id` in `dir` through the
/// `filesquid-ocfl` change model.
fn seed_object(dir: &Path, id: &str, files: &[(&str, &[u8])]) {
    let storage = FsBackend::with_default_staging(dir).expect("object backend");
    let leases = LeaseStore::new(
        MemoryBackend::new(),
        "leases",
        SystemClock,
        LeaseConfig::default(),
    )
    .expect("leases");
    let staging_backend = MemoryBackend::new();
    let staging = StagingArea::new(&staging_backend, "seed").expect("staging");
    let model = ChangeModel::new(&storage, &leases, "seeder", "", id, staging);
    for (path, bytes) in files {
        ready(model.stage_bytes(path, bytes)).expect("stage");
    }
    let meta = VersionMeta::new(rfc3339_utc(1_791_590_400_000));
    ready(model.commit(meta)).expect("seed");
}

/// Drive a future that completes on its first poll.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("ready")
}

/// A scratch OCFL object `id` in its own directory holding `photo.jpg`.
fn photo_object(name: &str, id: &str, photo: &[u8]) -> (PathBuf, PathBuf) {
    let dir = scratch(name);
    let object = dir.join("object");
    seed_object(&object, id, &[("photo.jpg", photo)]);
    (dir, object)
}

/// The canonical `metadata.md` text of the object's head.
fn metadata_md(object: &Path) -> String {
    let local = LocalObject::open(object, None).expect("open object");
    to_markdown(&local.load().expect("load"))
}

/// The versions of the object, oldest first.
fn versions(object: &Path) -> Vec<String> {
    let storage = FsBackend::with_default_staging(object).expect("backend");
    let object = ready(Object::open(&storage, "")).expect("open");
    object
        .history()
        .into_iter()
        .map(|info| info.version.to_string())
        .collect()
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

#[test]
fn set_on_a_new_object_needs_an_identifier() {
    let dir = scratch("set-new");
    let object = dir.join("object");
    std::fs::create_dir_all(&object).unwrap();
    let err = run(&args(&[
        "set",
        "dcterms:title",
        "Harbour",
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect_err("no --id");
    assert!(err.contains("--id"), "{err}");
    let code = run(&args(&[
        "set",
        "dcterms:title",
        "Harbour",
        "--id",
        "urn:x:1",
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect("set with --id");
    assert_eq!(code, 0);
    assert_eq!(versions(&object), ["v1"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn set_and_get_round_trip_with_one_version_per_edit() {
    let (dir, object) = photo_object("set-get", "urn:x:1", b"pixels");
    set_in(&object, "dcterms:title", "Harbour");
    set_in(&object, "dcterms:creator", "George");
    assert_eq!(versions(&object), ["v1", "v2", "v3"]);
    let code = run(&args(&[
        "get",
        "dcterms:title",
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect("get");
    assert_eq!(code, 0);
    assert!(
        !object.join("metadata.md").exists(),
        "no file outside a version"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn export_csv_contains_header() {
    let (dir, object) = photo_object("csv", "urn:x:1", b"pixels");
    set_in(&object, "dcterms:creator", "George");
    let csv_path = dir.join("out.csv");
    let code = run(&args(&[
        "export",
        "csv",
        &csv_path.to_string_lossy(),
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect("export");
    assert_eq!(code, 0);
    let body = std::fs::read_to_string(&csv_path).unwrap();
    assert!(body.contains("dcterms:creator"));
    assert!(body.contains("George"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn import_csv_and_xmp_commit_new_versions() {
    let (dir, object) = photo_object("import", "urn:x:1", b"pixels");
    let csv = dir.join("in.csv");
    std::fs::write(&csv, "dcterms:title\nHarbour\n").unwrap();
    let xmp = dir.join("in.xmp");
    let mut record = filesquid_metadata::record::Record::default();
    record
        .set(
            filesquid_metadata::MetadataNamespace::Dcterms,
            "creator",
            "George",
        )
        .unwrap();
    std::fs::write(&xmp, filesquid_metadata::xmp::to_xmp(&record)).unwrap();
    let object_arg = object.to_string_lossy().into_owned();
    for (fmt, path) in [("csv", &csv), ("xmp", &xmp)] {
        let code = run(&args(&[
            "import",
            fmt,
            &path.to_string_lossy(),
            "--object",
            &object_arg,
        ]))
        .expect("import");
        assert_eq!(code, 0);
    }
    assert_eq!(versions(&object), ["v1", "v2", "v3"]);
    let text = metadata_md(&object);
    assert!(
        text.contains("Harbour") && text.contains("George"),
        "{text}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn export_xmp_writes_the_head_record() {
    let (dir, object) = photo_object("xmp-export", "urn:x:1", b"pixels");
    set_in(&object, "dcterms:title", "Harbour");
    let out = dir.join("out.xmp");
    let code = run(&args(&[
        "export",
        "xmp",
        &out.to_string_lossy(),
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect("export xmp");
    assert_eq!(code, 0);
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(
        text.contains("<dcterms:title>Harbour</dcterms:title>"),
        "{text}"
    );
    assert_eq!(versions(&object), ["v1", "v2"], "an export commits nothing");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn extract_refuses_a_path_and_a_logical_file_together() {
    let (dir, object) = photo_object("extract-both", "urn:x:1", b"pixels");
    let err = run(&args(&[
        "extract",
        &dir.join("cat.tif").to_string_lossy(),
        "--file",
        "photo.jpg",
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect_err("both inputs");
    assert!(err.contains("not both"), "{err}");
    assert_eq!(versions(&object), ["v1"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn help_lists_json_interchange() {
    assert!(USAGE.contains("export csv|xmp|json"));
    assert!(USAGE.contains("import csv|xmp|json"));
}

#[test]
fn json_export_then_import_round_trips_through_metadata_md() {
    let (source_dir, source) = photo_object("json-src", "urn:x:1", b"pixels");
    set_in(&source, "dcterms:title", "Harbour, \"dawn\"");
    set_in(&source, "schema:album", "Coast");
    let sidecar = source_dir.join("sidecar.json");
    assert_eq!(run_json("export", &sidecar, "urn:x:1", &source), Ok(0));
    let (target_dir, target) = photo_object("json-dst", "urn:x:1", b"pixels");
    assert_eq!(run_json("import", &sidecar, "urn:x:1", &target), Ok(0));
    assert_eq!(metadata_md(&source), metadata_md(&target));
    assert_eq!(versions(&target), ["v1", "v2"]);
    std::fs::remove_dir_all(&source_dir).unwrap();
    std::fs::remove_dir_all(&target_dir).unwrap();
}

#[test]
fn json_import_rejects_digest_mismatch_and_publishes_nothing() {
    let (source_dir, source) = photo_object("mismatch-src", "urn:x:1", b"original");
    set_in(&source, "dcterms:title", "Harbour");
    let sidecar = source_dir.join("sidecar.json");
    assert_eq!(run_json("export", &sidecar, "urn:x:1", &source), Ok(0));
    let (target_dir, target) = photo_object("mismatch-dst", "urn:x:1", b"edited");
    set_in(&target, "dcterms:title", "Before");
    let before = metadata_md(&target);
    let err = run_json("import", &sidecar, "urn:x:1", &target).expect_err("mismatch");
    assert!(err.contains("digest"), "{err}");
    assert_eq!(metadata_md(&target), before);
    assert_eq!(versions(&target), ["v1", "v2"]);
    std::fs::remove_dir_all(&source_dir).unwrap();
    std::fs::remove_dir_all(&target_dir).unwrap();
}

#[test]
fn json_import_rejects_other_object_id() {
    let (dir, object) = photo_object("other-id", "urn:x:1", b"pixels");
    set_in(&object, "dcterms:title", "Harbour");
    let sidecar = dir.join("sidecar.json");
    assert_eq!(run_json("export", &sidecar, "urn:x:1", &object), Ok(0));
    let err = run_json("import", &sidecar, "urn:x:2", &object).expect_err("id");
    assert!(err.contains("object identifier"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn json_requires_file() {
    let (dir, object) = photo_object("flags", "urn:x:1", b"pixels");
    let out = dir.join("out.json");
    let err = run(&args(&[
        "export",
        "json",
        &out.to_string_lossy(),
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect_err("flags required");
    assert!(err.contains("--file"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn json_rejects_a_file_the_object_does_not_hold() {
    let (dir, object) = photo_object("absent-file", "urn:x:1", b"pixels");
    std::fs::write(object.join("loose.jpg"), b"not committed").unwrap();
    let out = dir.join("out.json");
    let err = run(&args(&[
        "export",
        "json",
        &out.to_string_lossy(),
        "--file",
        "loose.jpg",
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect_err("not a logical file");
    assert!(err.contains("loose.jpg"), "{err}");
    assert!(!out.exists(), "no sidecar is written");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn csv_export_still_ignores_json_flags() {
    let (dir, object) = photo_object("csv-flags", "urn:x:1", b"pixels");
    set_in(&object, "dcterms:title", "Harbour");
    let out = dir.join("out.csv");
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
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn json_rejects_a_flag_as_the_id_value() {
    let (dir, object) = photo_object("flag-as-value", "urn:x:1", b"pixels");
    let out = dir.join("out.json");
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
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn set_keeps_json_flag_names_as_literal_values() {
    let (dir, object) = photo_object("literal-flag", "urn:x:1", b"pixels");
    set_in(&object, "dcterms:title", "--file");
    assert!(metadata_md(&object).contains("--file"));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Big-endian TIFF with one ASCII `ImageDescription` ("Cat").
const TIFF_DESCRIPTION: &[u8] = &[
    b'M', b'M', 0, 42, 0, 0, 0, 8, 0, 1, 0x01, 0x0E, 0, 2, 0, 0, 0, 4, b'C', b'a', b't', 0, 0, 0,
    0, 0,
];

#[test]
fn extract_reads_a_plain_file_through_a_reader_into_metadata_md() {
    let (dir, object) = photo_object("extract", "urn:x:1", b"pixels");
    let photo = dir.join("cat.tif");
    std::fs::write(&photo, TIFF_DESCRIPTION).unwrap();
    let args = args(&[
        "extract",
        &photo.to_string_lossy(),
        "--object",
        &object.to_string_lossy(),
    ]);
    assert_eq!(run(&args).expect("extract"), 0);
    assert!(metadata_md(&object).contains("ImageDescription"));
    assert_eq!(versions(&object), ["v1", "v2"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn extract_reads_a_logical_file_through_filesquid_ocfl() {
    let (dir, object) = photo_object("extract-logical", "urn:x:1", TIFF_DESCRIPTION);
    let args = args(&[
        "extract",
        "--file",
        "photo.jpg",
        "--object",
        &object.to_string_lossy(),
    ]);
    assert_eq!(run(&args).expect("extract"), 0);
    let text = metadata_md(&object);
    assert!(
        text.contains("ImageDescription") && text.contains("Cat"),
        "{text}"
    );
    assert_eq!(versions(&object), ["v1", "v2"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn extract_reports_a_missing_file() {
    let (dir, object) = photo_object("missing", "urn:x:1", b"pixels");
    let args = args(&[
        "extract",
        &dir.join("absent.jpg").to_string_lossy(),
        "--object",
        &object.to_string_lossy(),
    ]);
    let err = run(&args).expect_err("missing file is reported");
    assert!(err.contains("absent.jpg"), "{err}");
    assert_eq!(versions(&object), ["v1"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Run `namespace register <prefix> <iri> [extra..] --object <dir>`.
fn register_in(object: &Path, prefix: &str, iri: &str, extra: &[&str]) -> Result<i32, String> {
    let object = object.to_string_lossy();
    let mut list = vec!["namespace", "register", prefix, iri];
    list.extend_from_slice(extra);
    list.extend_from_slice(&["--object", &object]);
    run(&args(&list))
}

/// Run `validate [curie] --object <dir>`.
fn validate_in(object: &Path, curie: Option<&str>) -> Result<i32, String> {
    let object = object.to_string_lossy();
    let mut list = vec!["validate"];
    list.extend(curie);
    list.extend_from_slice(&["--object", &object]);
    run(&args(&list))
}

/// The OCFL object holding a `rico` registry with the term `title`.
fn rico_object(name: &str) -> (PathBuf, PathBuf) {
    let (dir, object) = photo_object(name, "urn:x:1", b"pixels");
    let code = register_in(
        &object,
        "rico",
        "https://www.ica.org/standards/RiC/ontology#",
        &["--terms", "title,hasCreator"],
    )
    .expect("register");
    assert_eq!(code, 0);
    (dir, object)
}

#[test]
fn a_registered_extension_statement_is_set_in_its_own_map() {
    let (dir, object) = rico_object("ns-set");
    set_in(&object, "rico:title", "Fonds 12");
    assert!(metadata_md(&object).contains("rico:\n  title: Fonds 12\n"));
    assert_eq!(versions(&object), ["v1", "v2", "v3"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn set_rejects_an_unregistered_prefix_or_an_unlisted_term_by_name() {
    let (dir, object) = rico_object("ns-refuse");
    let object_arg = object.to_string_lossy().into_owned();
    let err = run(&args(&["set", "lido:title", "x", "--object", &object_arg])).expect_err("lido");
    assert!(err.contains("lido"), "{err}");
    let err = run(&args(&["set", "rico:colour", "x", "--object", &object_arg])).expect_err("term");
    assert!(err.contains("colour"), "{err}");
    assert_eq!(versions(&object), ["v1", "v2"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn namespace_register_refuses_a_built_in_prefix_and_another_iri() {
    let (dir, object) = rico_object("ns-register");
    let err = register_in(&object, "dcterms", "https://example.org/x#", &[]).expect_err("built-in");
    assert!(err.contains("dcterms"), "{err}");
    let err = register_in(&object, "rico", "https://example.org/x#", &[]).expect_err("another IRI");
    assert!(err.contains("rico"), "{err}");
    let code = register_in(
        &object,
        "mine",
        "urn:uuid:0b5c4d1e-8a6f-4b21-9a0e-1f2d3c4b5a69",
        &["--custom"],
    )
    .expect("custom");
    assert_eq!(code, 0);
    let listed = run(&args(&[
        "namespace",
        "list",
        "--object",
        &object.to_string_lossy(),
    ]));
    assert_eq!(listed, Ok(0));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn validate_reports_unregistered_prefixes() {
    let (dir, object) = rico_object("ns-validate");
    assert_eq!(validate_in(&object, Some("rico:title")), Ok(0));
    assert_eq!(validate_in(&object, Some("rico:colour")), Ok(1));
    assert_eq!(validate_in(&object, Some("lido:title")), Ok(1));
    assert_eq!(validate_in(&object, Some("schema:album")), Ok(0));
    set_in(&object, "rico:title", "Fonds 12");
    assert_eq!(validate_in(&object, None), Ok(0));
    let record = "---\nrico:\n  title: Fonds 12\nlido:\n  title: Stray\n---\n";
    let local = LocalObject::open(&object, None).unwrap();
    local
        .save(&filesquid_metadata::markdown::from_markdown(record))
        .unwrap();
    assert_eq!(validate_in(&object, None), Ok(1));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn extension_statements_round_trip_through_cli_csv_and_xmp() {
    let (dir, object) = rico_object("ns-interchange");
    set_in(&object, "rico:title", "Fonds 12");
    let object_arg = object.to_string_lossy().into_owned();
    for fmt in ["csv", "xmp"] {
        let path = dir.join(format!("out.{fmt}"));
        let path_arg = path.to_string_lossy().into_owned();
        let code =
            run(&args(&["export", fmt, &path_arg, "--object", &object_arg])).expect("export");
        assert_eq!(code, 0);
        assert!(
            std::fs::read_to_string(&path).unwrap().contains("Fonds 12"),
            "{fmt}"
        );
        let (_, other) = rico_object(&format!("ns-interchange-{fmt}"));
        let other_arg = other.to_string_lossy().into_owned();
        let code = run(&args(&["import", fmt, &path_arg, "--object", &other_arg])).expect("import");
        assert_eq!(code, 0);
        assert!(
            metadata_md(&other).contains("rico:\n  title: Fonds 12\n"),
            "{fmt}"
        );
        std::fs::remove_dir_all(other.parent().unwrap()).unwrap();
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_separate_registry_object_is_named_with_registry() {
    let (dir, registry) = rico_object("ns-registry");
    let (other_dir, object) = photo_object("ns-registry-user", "urn:x:2", b"pixels");
    let code = run(&args(&[
        "set",
        "rico:title",
        "Fonds 12",
        "--registry",
        &registry.to_string_lossy(),
        "--object",
        &object.to_string_lossy(),
    ]))
    .expect("set with --registry");
    assert_eq!(code, 0);
    assert!(metadata_md(&object).contains("rico:"));
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::remove_dir_all(&other_dir).unwrap();
}
