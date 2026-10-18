//! Step definitions for `local_library_and_version_inventory_repair_API-S-023.feature`
//! (Task CL-1.1): create, open, copy, rebuild, the repair candidate query and the consented
//! version-inventory repair, driven through `LocalLibraryService` over an in-memory OCFL
//! storage root.

use crate::ocfl_fixture::{library, ready, CountingBackend, Rig};
use crate::MonitorWorld;
use cucumber::{given, then, when};
use filesquid_api::auth::Principal;
use filesquid_api::journal::Outcome;
use filesquid_api::local_library::{
    LocalLibraryError, RepairAnswer, RepairRequest, RepairResult, REPAIR_OPERATION,
};
use filesquid_ocfl::DigestAlgorithm;

/// The complete object's root and identifier.
const COMPLETE: (&str, &str) = ("obj-a", "urn:uuid:a");

/// The W010 object's root and identifier.
const W010: (&str, &str) = ("obj-b", "urn:uuid:b");

/// Per-scenario state of the local library steps.
#[derive(Default)]
pub struct LocalWorld {
    rig: Option<Rig>,
    writes: u64,
    error: Option<LocalLibraryError>,
    answer: Option<RepairAnswer>,
    copy: Option<CountingBackend>,
    rebuilt: Vec<(String, bool)>,
    candidates: Vec<String>,
}

impl std::fmt::Debug for LocalWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalWorld")
            .field("writes", &self.writes)
            .finish_non_exhaustive()
    }
}

impl LocalWorld {
    fn rig(&self) -> &Rig {
        self.rig.as_ref().expect("rig")
    }

    /// Install `rig` and remember its write count.
    fn start(&mut self, rig: Rig) {
        self.writes = rig.root.writes();
        self.rig = Some(rig);
    }

    /// Run the repair of the W010 object (or the complete one when there is none) as
    /// `caller`, keeping the answer or the error.
    fn repair(&mut self, caller: Principal, request: RepairRequest) {
        let rig = self.rig();
        let result =
            ready(
                rig.service
                    .repair_version_inventory(&caller, request, &rig.root, &rig.leases),
            );
        match result {
            Ok(answer) => self.answer = Some(answer),
            Err(err) => self.error = Some(err),
        }
    }

    /// The object the repair steps name.
    fn target(&self) -> &'static str {
        if self.rig().stored("obj-b/0=ocfl_object_1.1") {
            W010.1
        } else {
            COMPLETE.1
        }
    }
}

/// A v3 of the W010 object, as the next writer would commit it.
fn commit_v3(rig: &Rig) -> bool {
    let mut draft = rig.draft(W010.1, "bdd-v3", &[("c.txt", b"charlie")]);
    draft.keep("a.txt", DigestAlgorithm::Sha256.digest_hex(b"alpha"));
    draft.keep("b.txt", DigestAlgorithm::Sha256.digest_hex(b"bravo"));
    rig.commit(W010.0, Some("v2"), &draft).is_ok()
}

#[given("an empty folder chosen by the owner")]
fn empty_folder(w: &mut MonitorWorld) {
    w.local.start(Rig::new());
}

#[given("a folder chosen by the owner that already holds a file")]
fn folder_with_file(w: &mut MonitorWorld) {
    let rig = Rig::new();
    let key = crate::ocfl_fixture::key("holiday.jpg");
    ready(filesquid_ocfl::storage::StorageBackend::put(
        &rig.root, &key, b"jpeg",
    ))
    .expect("put");
    w.local.start(rig);
}

#[given("a library with a complete object and an object whose head has no version inventory")]
fn library_with_both(w: &mut MonitorWorld) {
    let rig = Rig::with_root();
    rig.object(COMPLETE.0, COMPLETE.1);
    rig.w010_object(W010.0, W010.1);
    w.local.start(rig);
}

#[given("a library with an object whose head has no version inventory")]
fn library_with_w010(w: &mut MonitorWorld) {
    let rig = Rig::with_root();
    rig.w010_object(W010.0, W010.1);
    w.local.start(rig);
}

#[given("a library with a complete object")]
fn library_with_complete(w: &mut MonitorWorld) {
    let rig = Rig::with_root();
    rig.object(COMPLETE.0, COMPLETE.1);
    w.local.start(rig);
}

#[given("a new version of that object is refused until it is repaired")]
fn commit_refused(w: &mut MonitorWorld) {
    assert!(
        !commit_v3(w.local.rig()),
        "W010 objects stay closed to commits"
    );
    w.local.writes = w.local.rig().root.writes();
}

#[when("the owner creates a library there")]
fn create(w: &mut MonitorWorld) {
    let rig = w.local.rig();
    if let Err(err) = ready(rig.service.create_root("owner", &library(), &rig.root)) {
        w.local.error = Some(err);
    }
}

#[when("the owner copies a completed snapshot to an empty folder")]
fn copy(w: &mut MonitorWorld) {
    let dest = CountingBackend::new();
    let rig = w.local.rig();
    let report = ready(
        rig.service
            .copy_snapshot("owner", &library(), &rig.root, &dest),
    )
    .expect("copy");
    assert!(report.invalid_objects.is_empty(), "{report:?}");
    w.local.copy = Some(dest);
}

#[when("the owner rebuilds the library from its files")]
fn rebuild(w: &mut MonitorWorld) {
    let rig = w.local.rig();
    let report = ready(rig.service.rebuild("owner", &library(), &rig.root)).expect("rebuild");
    w.local.rebuilt = report
        .objects
        .into_iter()
        .map(|o| (o.object_id, o.missing_version_inventory))
        .collect();
}

#[when("the owner asks which objects need a version-inventory repair")]
fn ask_candidates(w: &mut MonitorWorld) {
    let rig = w.local.rig();
    let found = ready(
        rig.service
            .repair_candidates("owner", &library(), &rig.root),
    )
    .expect("query");
    w.local.candidates = found.candidates.into_iter().map(|c| c.object_id).collect();
}

#[when("the owner consents and repairs that object")]
fn consented_repair(w: &mut MonitorWorld) {
    let request = w.local.rig().consented(&[w.local.target()]);
    w.local.repair(Principal::Owner, request);
}

#[when("the repair is requested without the owner's consent")]
fn repair_without_consent(w: &mut MonitorWorld) {
    let mut request = w.local.rig().consented(&[W010.1]);
    request.consent = None;
    w.local.repair(Principal::Owner, request);
}

#[when("the server administrator requests the repair with consent")]
fn repair_as_administrator(w: &mut MonitorWorld) {
    let request = w.local.rig().consented(&[W010.1]);
    w.local.repair(Principal::Administrator, request);
}

#[then("the folder should hold only the OCFL 1.1 storage-root declaration")]
fn only_declaration(w: &mut MonitorWorld) {
    let keys: Vec<String> = w
        .local
        .rig()
        .root
        .snapshot()
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(keys, ["0=ocfl_1.1"]);
}

#[then("the creation should be journaled as succeeded")]
fn creation_journaled(w: &mut MonitorWorld) {
    let entries = w.local.rig().journaled("library.root.create");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Succeeded);
}

#[then(expr = "the request should be refused with {string}")]
fn refused_with(w: &mut MonitorWorld, code: String) {
    let err = w.local.error.as_ref().expect("refused");
    assert_eq!(err.error_code(), code);
}

#[then(regex = r"^nothing should be written to the (?:folder|library)$")]
fn nothing_written(w: &mut MonitorWorld) {
    assert_eq!(w.local.rig().root.writes(), w.local.writes, "no write");
}

#[then(expr = "the copy should validate without errors and open as a library of {int} objects")]
fn copy_opens(w: &mut MonitorWorld, objects: usize) {
    let dest = w.local.copy.as_ref().expect("copy");
    let summary =
        ready(w.local.rig().service.open_root("owner", &library(), dest)).expect("open copy");
    assert_eq!(summary.objects, objects);
}

#[then("the object without a version inventory should still lack it")]
fn still_w010(w: &mut MonitorWorld) {
    assert!(!w.local.rig().stored("obj-b/v2/inventory.json"));
}

#[then(expr = "the rebuild should list {int} objects")]
fn rebuild_lists(w: &mut MonitorWorld, objects: usize) {
    assert_eq!(w.local.rebuilt.len(), objects);
}

#[then("the object without a version inventory should be flagged as a repair candidate")]
fn rebuild_flags(w: &mut MonitorWorld) {
    let flagged: Vec<&str> = w
        .local
        .rebuilt
        .iter()
        .filter(|(_, missing)| *missing)
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(flagged, [W010.1]);
}

#[then("only the object without a version inventory should be listed")]
fn only_w010_listed(w: &mut MonitorWorld) {
    assert_eq!(w.local.candidates, [W010.1]);
}

#[then("the object should be repaired")]
fn repaired(w: &mut MonitorWorld) {
    let answer = w.local.answer.as_ref().expect("answer");
    assert_eq!(answer.objects[0].result, RepairResult::Repaired);
}

#[then("a new version of that object should be accepted")]
fn commit_accepted(w: &mut MonitorWorld) {
    let rig = w.local.rig();
    rig.clock.advance(120_000);
    assert!(commit_v3(rig), "the repaired object is committable");
}

#[then("the consent and one outcome per object should be journaled without paths")]
fn journaled_without_paths(w: &mut MonitorWorld) {
    let rig = w.local.rig();
    assert_eq!(rig.ledger.consents().len(), 1);
    let outcomes = rig.ledger.outcomes();
    assert_eq!(outcomes.len(), 1);
    let text = serde_json::to_string(&outcomes[0].journal).expect("json");
    assert!(
        !text.contains("obj-") && !text.contains("inventory.json"),
        "{text}"
    );
}

#[then("the refusal should be journaled")]
fn refusal_journaled(w: &mut MonitorWorld) {
    let entries = w.local.rig().journaled(REPAIR_OPERATION);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, Outcome::Failed);
}

#[then(expr = "the object should be refused with {string}")]
fn object_refused(w: &mut MonitorWorld, code: String) {
    let answer = w.local.answer.as_ref().expect("answer");
    assert_eq!(answer.objects[0].error_code.as_deref(), Some(code.as_str()));
}
