//! Step definitions for `enumerate_committed_state_OCFL-S-004.feature` and
//! the enumerator scenario of `reusable_ocfl_engine_F-051.feature` (IO-0.9).

use cucumber::{given, then, when};
use filesquid_ocfl::read::{CommittedEvent, StorageRoot, Versions};
use filesquid_ocfl::storage::MemoryBackend;

use crate::builder::{build, delete, put, write_inventory, ObjectSpec};
use crate::enumerate::{check_content, declare_storage_root, enumerate_all, Item};
use crate::OcflWorld;

/// What the enumerator scenarios carry between steps.
#[derive(Debug, Default)]
pub struct EnumerateState {
    backend: Option<MemoryBackend>,
    items: Vec<Item>,
}

/// v2 renames b.txt to c.txt and adds d.txt; v3 changes a.txt and removes
/// d.txt.
fn three_versions() -> ObjectSpec<'static> {
    ObjectSpec::new(
        "urn:example:one",
        vec![
            vec![("a.txt", b"alpha" as &[u8]), ("b.txt", b"bravo")],
            vec![
                ("a.txt", b"alpha"),
                ("c.txt", b"bravo"),
                ("d.txt", b"delta"),
            ],
            vec![("a.txt", b"alpha 2"), ("c.txt", b"bravo")],
        ],
    )
}

/// The scenario's backend.
fn backend(world: &OcflWorld) -> &MemoryBackend {
    world
        .enumerate
        .backend
        .as_ref()
        .expect("Given an OCFL storage root")
}

/// A comma-separated list as strings.
fn list(text: &str) -> Vec<String> {
    text.split(", ").map(str::to_string).collect()
}

#[given(
    expr = "an OCFL storage root in memory holding the objects {string} with three versions and {string} with one"
)]
fn storage_root(world: &mut OcflWorld, first: String, second: String) {
    let backend = MemoryBackend::new();
    declare_storage_root(&backend);
    build(&backend, &first, &three_versions());
    let one = ObjectSpec::new(
        "urn:example:two",
        vec![vec![("only.txt", b"only" as &[u8])]],
    );
    build(&backend, &second, &one);
    world.enumerate.backend = Some(backend);
}

#[when("I enumerate the committed logical state at each object's head")]
fn enumerate_head(world: &mut OcflWorld) {
    world.enumerate.items = enumerate_all(backend(world), Versions::Head);
}

#[when("I enumerate the committed logical state of every version")]
fn enumerate_every_version(world: &mut OcflWorld) {
    world.enumerate.items = enumerate_all(backend(world), Versions::All);
}

/// One event as a table row: event, object id, version, logical path and
/// content locator (the object root for an object event).
fn row(item: &Item) -> Vec<String> {
    let cells: [&str; 5] = match item {
        Ok(CommittedEvent::Object(o)) => ["object", &o.id, "", "", &o.root],
        Ok(CommittedEvent::Version(v)) => ["version", &v.object_id, "", "", ""],
        Ok(CommittedEvent::File(f)) => ["file", &f.object_id, "", &f.path, f.content_key.as_str()],
        Err(err) => panic!("unexpected error: {err}"),
    };
    let mut cells: Vec<String> = cells.iter().map(|c| c.to_string()).collect();
    cells[2] = match item {
        Ok(CommittedEvent::Object(o)) => o.head.to_string(),
        Ok(CommittedEvent::Version(v)) => v.version.to_string(),
        Ok(CommittedEvent::File(f)) => f.version.to_string(),
        Err(_) => String::new(),
    };
    cells
}

#[then("the enumeration should be:")]
fn enumeration_is(world: &mut OcflWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("an enumeration table");
    let expected: Vec<Vec<String>> = table.rows.iter().skip(1).cloned().collect();
    let actual: Vec<Vec<String>> = world.enumerate.items.iter().map(row).collect();
    assert_eq!(actual, expected);
}

#[then("every enumerated locator should hold bytes that match the file's digest")]
fn locators_match(world: &mut OcflWorld) {
    let checked = check_content(backend(world), &world.enumerate.items);
    assert!(checked > 0, "no file was enumerated");
}

#[then(expr = "the versions enumerated for {string} should be {string}")]
fn versions_are(world: &mut OcflWorld, object: String, expected: String) {
    let versions: Vec<String> = world
        .enumerate
        .items
        .iter()
        .filter_map(|item| match item {
            Ok(CommittedEvent::Version(v)) if v.object_id == object => Some(v.version.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(versions, list(&expected));
}

#[then(expr = "version {string} of {string} should list {string}")]
fn version_lists(world: &mut OcflWorld, version: String, object: String, expected: String) {
    let paths: Vec<String> = world
        .enumerate
        .items
        .iter()
        .filter_map(|item| match item {
            Ok(CommittedEvent::File(f))
                if f.object_id == object && f.version.to_string() == version =>
            {
                Some(f.path.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(paths, list(&expected));
}

#[given(expr = "a stray file {string} in the object {string}")]
fn stray_file(world: &mut OcflWorld, rel: String, root: String) {
    put(backend(world), &root, &rel, b"not in the inventory");
}

#[then(expr = "no enumerated file should be {string}")]
fn no_file_named(world: &mut OcflWorld, name: String) {
    for item in &world.enumerate.items {
        if let Ok(CommittedEvent::File(f)) = item {
            assert!(!f.path.ends_with(&name), "{} was enumerated", f.path);
            assert!(
                !f.content_key.as_str().ends_with(&name),
                "{}",
                f.content_key
            );
        }
    }
}

#[given(
    expr = "version {string} content and a version inventory exist in {string} while its root head is {string}"
)]
fn unpublished_version(world: &mut OcflWorld, version: String, root: String, _head: String) {
    let mut spec = three_versions();
    spec.versions.push(vec![("e.txt", b"echo" as &[u8])]);
    let next = build(&MemoryBackend::new(), "x", &spec);
    let backend = backend(world);
    put(backend, &root, &format!("{version}/content/e.txt"), b"echo");
    write_inventory(backend, &root, &version, &next);
}

#[given(expr = "the object {string} in the storage root has no root inventory")]
fn broken_object(world: &mut OcflWorld, root: String) {
    let spec = ObjectSpec::new("urn:example:broken", vec![vec![("x.txt", b"x" as &[u8])]]);
    build(backend(world), &root, &spec);
    delete(backend(world), &root, "inventory.json");
}

#[then(expr = "the enumeration should report {string} with OCFL code {string}")]
fn reports(world: &mut OcflWorld, root: String, code: String) {
    let errors: Vec<(String, Option<&str>)> = world
        .enumerate
        .items
        .iter()
        .filter_map(|item| item.as_ref().err())
        .map(|err| (err.root.clone(), err.code()))
        .collect();
    assert_eq!(errors, vec![(root, Some(code.as_str()))]);
}

#[then(expr = "the objects enumerated should be {string}")]
fn objects_are(world: &mut OcflWorld, expected: String) {
    let roots: Vec<String> = world
        .enumerate
        .items
        .iter()
        .filter_map(|item| match item {
            Ok(CommittedEvent::Object(o)) => Some(o.root.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(roots, list(&expected));
}

#[then("a committed logical-state enumerator should be available")]
fn enumerator_available(world: &mut OcflWorld) {
    let _enumerate = StorageRoot::<MemoryBackend>::enumerate;
    assert!(world.lib.contains("CommittedEvent"), "not re-exported");
}

/// Database crates the engine must not depend on (T-039, ADR 005).
const DATABASE_CRATES: [&str; 6] = [
    "rusqlite", "sqlx", "sqlite", "postgres", "diesel", "sea-orm",
];

#[then("it should need no SQLite, PostgreSQL or FileSquid catalog types")]
fn no_database(world: &mut OcflWorld) {
    let manifest: String = world
        .cargo
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    for name in DATABASE_CRATES {
        assert!(!manifest.contains(name), "{name} must not be a dependency");
    }
    let sources = crate::source_scan::read_sources().expect("scan src");
    assert!(
        sources.iter().any(|(file, _)| file == "read/enumerate.rs"),
        "the enumerator source was not scanned"
    );
    let findings: Vec<_> = sources
        .iter()
        .flat_map(|(file, text)| crate::source_scan::scan_text(file, text, &DATABASE_CODE))
        .collect();
    assert!(
        findings.is_empty(),
        "database or catalog code: {findings:?}"
    );
}

/// Database and catalog names no engine source may use in code (comments
/// and strings are ignored).
const DATABASE_CODE: [&str; 7] = [
    "rusqlite",
    "sqlx",
    "Sqlite",
    "Postgres",
    "Catalog",
    "filesquid_core",
    "filesquid_api",
];
