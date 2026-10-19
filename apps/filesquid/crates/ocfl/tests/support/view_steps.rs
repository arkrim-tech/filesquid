//! Step definitions for `view_api_OCFL-S-012.feature` (IO-1.2). Every
//! scenario runs on an in-memory storage root behind the fault-injecting
//! backend, writing through the change model of `crate::change`.

use std::fmt;

use cucumber::{given, then, when};
use filesquid_ocfl::change::ChangeError;
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend};
use filesquid_ocfl::view::{self, Entry, FolderPage, ViewError, MAX_RANGE_BYTES};
use filesquid_ocfl::write::{CommitError, CommitReceipt};

use crate::change::{commit_files, meta, model, staged_keys, state_of};
use crate::rig::{key, ready, sha256, v, Rig, ROOT};
use crate::OcflWorld;

/// What the view scenarios carry between steps.
#[derive(Default)]
pub struct ViewState {
    rig: Option<Rig<MemoryBackend>>,
    pages: Vec<FolderPage>,
    write: Option<Result<CommitReceipt, ViewError>>,
    read_error: Option<ViewError>,
}

impl fmt::Debug for ViewState {
    /// Show the outcome; the rig holds closures that are not `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ViewState")
            .field("write", &self.write)
            .finish_non_exhaustive()
    }
}

/// The scenario's rig.
fn rig(world: &OcflWorld) -> &Rig<MemoryBackend> {
    world.view.rig.as_ref().expect("Given an object")
}

/// The names of a page's entries, folders with a trailing `/`.
fn names(page: &FolderPage) -> Vec<String> {
    page.entries
        .iter()
        .map(|entry| match entry {
            Entry::Folder(folder) => format!("{}/", folder.name),
            Entry::File(file) => file.name.clone(),
        })
        .collect()
}

/// List `prefix` at `version`, `limit` entries per page.
fn list(
    world: &OcflWorld,
    version: &str,
    prefix: &str,
    cursor: Option<&str>,
    limit: usize,
) -> FolderPage {
    let object = rig(world).open().expect("open");
    ready(view::list_folder(
        &object,
        &v(version),
        prefix,
        cursor,
        limit,
    ))
    .expect("list")
}

#[given(
    expr = "an object whose {string} holds {string} and {string} and whose {string} changes {string}"
)]
fn object(world: &mut OcflWorld, v1: String, a: String, b: String, v2: String, changed: String) {
    let rig = Rig::new(MemoryBackend::new());
    let m = model(&rig, "writer-a");
    let first: [(&str, &[u8]); 2] = [(&a, b"first a"), (&b, b"first b")];
    assert_eq!(
        commit_files(&m, &rig, &first, "v1").expect("v1").version,
        v(&v1)
    );
    let second: [(&str, &[u8]); 1] = [(&changed, b"changed")];
    assert_eq!(
        commit_files(&m, &rig, &second, "v2").expect("v2").version,
        v(&v2)
    );
    world.view.rig = Some(rig);
}

#[given("the head version inventory was removed")]
fn remove_head_inventory(world: &mut OcflWorld) {
    let rig = rig(world);
    let head = rig.open().expect("open").head().to_string();
    for name in ["inventory.json", "inventory.json.sha256"] {
        let raw = format!("{ROOT}/{head}/{name}");
        ready(rig.storage.delete(&key(&raw))).expect("remove");
    }
}

#[when(expr = "the root folder of {string} is listed")]
fn list_root(world: &mut OcflWorld, version: String) {
    let page = list(world, &version, "", None, 100);
    world.view.pages = vec![page];
}

#[when(expr = "the root folder of {string} is listed one entry per page")]
fn list_paged(world: &mut OcflWorld, version: String) {
    let first = list(world, &version, "", None, 1);
    let cursor = first.next.clone().expect("a cursor");
    let second = list(world, &version, "", Some(&cursor), 1);
    world.view.pages = vec![first, second];
}

#[when(expr = "a file {string} is added through the view API")]
fn add(world: &mut OcflWorld, path: String) {
    let rig = rig(world);
    let result = ready(view::add_file(
        &model(rig, "writer-a"),
        &path,
        b"new",
        meta(rig, "add"),
    ));
    world.view.write = Some(result);
}

#[when(expr = "the folder {string} is moved to {string} through the view API")]
fn move_folder(world: &mut OcflWorld, from: String, to: String) {
    let rig = rig(world);
    let result = ready(view::move_path(
        &model(rig, "writer-a"),
        &from,
        &to,
        meta(rig, "mv"),
    ));
    world.view.write = Some(result);
}

#[when("a range larger than the read bound is requested")]
fn big_range(world: &mut OcflWorld) {
    let object = rig(world).open().expect("open");
    let result = ready(view::read_range(
        &object,
        &v("v2"),
        "readme.md",
        0,
        MAX_RANGE_BYTES + 1,
    ));
    world.view.read_error = result.err();
}

#[then(expr = "the listing should show {string} then {string}")]
fn shows_two(world: &mut OcflWorld, a: String, b: String) {
    assert_eq!(names(&world.view.pages[0]), [a, b]);
}

#[then(expr = "{string} should show its size, digest and the version that last changed it")]
fn file_facts(world: &mut OcflWorld, path: String) {
    let (folder, name) = path.rsplit_once('/').expect("a nested path");
    let page = list(world, "v2", folder, None, 100);
    let file = page
        .entries
        .iter()
        .find_map(|entry| match entry {
            Entry::File(file) if file.name == name => Some(file),
            _ => None,
        })
        .expect("listed");
    assert_eq!(file.size, 7);
    assert_eq!(file.digest, sha256(b"changed"));
    assert_eq!(file.modified_version, v("v2"));
}

#[then(expr = "the object head should be {string} through the view API")]
fn head_is(world: &mut OcflWorld, version: String) {
    let result = world.view.write.as_ref().expect("a write ran");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(rig(world).open().expect("open").head(), &v(&version));
}

#[then(expr = "the root folder of {string} should show {string}")]
fn root_shows(world: &mut OcflWorld, version: String, name: String) {
    assert!(names(&list(world, &version, "", None, 100)).contains(&name));
}

#[then("the view write should have written nothing")]
fn wrote_nothing(world: &mut OcflWorld) {
    let receipt = world
        .view
        .write
        .as_ref()
        .expect("a write ran")
        .as_ref()
        .expect("ok");
    assert!(receipt.written.is_empty());
}

#[then(expr = "{string} should exist at {string}")]
fn exists_at(world: &mut OcflWorld, path: String, version: String) {
    assert!(state_of(rig(world), &version).contains_key(&path));
}

#[then(expr = "the first page should show {string} and give a cursor")]
fn first_page(world: &mut OcflWorld, name: String) {
    assert_eq!(names(&world.view.pages[0]), [name]);
    assert!(world.view.pages[0].next.is_some());
}

#[then(expr = "the next page should show {string} and give no cursor")]
fn next_page(world: &mut OcflWorld, name: String) {
    assert_eq!(names(&world.view.pages[1]), [name]);
    assert!(world.view.pages[1].next.is_none());
}

#[then("the view write should be refused for the missing version inventory")]
fn refused_w010(world: &mut OcflWorld) {
    let result = world.view.write.as_ref().expect("a write ran");
    assert!(
        matches!(
            result,
            Err(ViewError::Change(ChangeError::Commit(
                CommitError::MissingVersionInventory { .. }
            )))
        ),
        "{result:?}"
    );
}

#[then("the staging area should be empty")]
fn staging_empty(world: &mut OcflWorld) {
    assert!(staged_keys(rig(world), "changes/writer-a").is_empty());
    let staging = &rig(world).staging;
    let page = ready(staging.list("changes/", None, 10)).expect("list");
    assert!(page.keys.is_empty());
}

#[then("the read should be refused as an invalid request")]
fn invalid_read(world: &mut OcflWorld) {
    let err = world.view.read_error.as_ref().expect("refused");
    assert!(matches!(err, ViewError::InvalidRequest { .. }), "{err:?}");
}
