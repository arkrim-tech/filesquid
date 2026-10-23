//! Contract fixtures for the library and repair routes (Task API-F1.12): library commands
//! over in-memory OCFL roots registered as locations, the sample request of every library
//! route, and serialized Rust samples of every library and repair schema.

#![allow(dead_code)]

use axum::http::Method;
use filesquid_api::imports::{
    ImportAnswer, ImportOutcome, ImportRequest, ImportSource, ImportStartError, ImportStarter,
};
use filesquid_api::local_library::choice::{
    ChoiceOutcome, ChoicePurpose, ChooseError, LocationChoice, LocationChoiceRequest,
    LocationChooser,
};
use filesquid_api::local_library::{
    FreeBytesReason, FreeSpace, LibraryFacts, MemoryLibraryRegistry, RecentLibraries,
    RecentLibrary, RenameLibraryRequest,
};
use filesquid_api::local_library::{
    JournalRebuild, LibraryCommands, LibraryLocations, LibraryRequest, LibraryRoot, LibraryView,
    LocalLibraryService, LocationRequest, MemoryRepairLedger, ObjectFindings, ObjectRepair,
    RebuildReport, RebuiltObject, RepairAnswer, RepairCandidate, RepairCandidates, RepairConsent,
    RepairRequest, RepairResult, SnapshotCopyRequest, UnreadableObject, CONSENT_STATEMENT_ID,
};
use filesquid_api::monitor::Monitor;
use filesquid_api::ApiState;
use filesquid_core::LibraryId;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::storage::{MemoryBackend, StorageBackend, StorageKey};
use filesquid_ocfl::write::{rfc3339_utc, LeaseConfig, LeaseStore, SystemClock};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// The library the samples name.
pub const LIBRARY: &str = "0d6e1f2a-3b4c-4d5e-8f60-718293a4b5c6";

/// The W010 object of the sample library.
const OBJECT: &str = "urn:uuid:7c1d2e3f-4a5b-4c6d-8e7f-90a1b2c3d4e5";

/// The location of a new library, of the sample library and of the copy.
const NEW: &str = "loc-new";
const SAMPLE: &str = "loc-sample";
const COPY: &str = "loc-copy";

/// Lease TTL 60 s with a 10 s margin.
const LEASES: LeaseConfig = LeaseConfig {
    ttl_ms: 60_000,
    margin_ms: 10_000,
};

/// The lease store of a sample location.
type Leases = LeaseStore<MemoryBackend, SystemClock>;

/// In-memory locations, as a host would register the folders an owner chose.
struct MemoryLocations {
    roots: HashMap<&'static str, LibraryRoot<MemoryBackend, Leases>>,
}

impl LibraryLocations for MemoryLocations {
    type Backend = MemoryBackend;
    type Reservations = Leases;

    fn location(&self, location_id: &str) -> Option<LibraryRoot<MemoryBackend, Leases>> {
        self.roots.get(location_id).cloned()
    }

    fn overlaps(&self, a: &str, b: &str) -> bool {
        a == b
    }
}

/// An empty in-memory location with its own leases.
fn location() -> LibraryRoot<MemoryBackend, Leases> {
    let leases = LeaseStore::new(MemoryBackend::new(), "leases", SystemClock, LEASES);
    LibraryRoot {
        backend: Arc::new(MemoryBackend::new()),
        reservations: Arc::new(leases.expect("leases")),
    }
}

/// Install library commands on `state` whose sample library is open and holds one W010
/// object, with an empty location for a new library and one for the copy.
pub async fn install(state: &ApiState, monitor: &Monitor) {
    let roots: HashMap<&'static str, _> = [NEW, SAMPLE, COPY]
        .into_iter()
        .map(|id| (id, location()))
        .collect();
    let sample = roots[SAMPLE].clone();
    let locations = Arc::new(MemoryLocations { roots });
    let service = LocalLibraryService::new(monitor.journal(), Arc::new(MemoryRepairLedger::new()));
    let registry = MemoryLibraryRegistry::new(|_: &str| FreeSpace::Bytes(1_000_000));
    let commands = LibraryCommands::new(service, locations).with_registry(Arc::new(registry));
    let request = LocationRequest {
        location_id: SAMPLE.to_string(),
        library_id: Some(library()),
        name: Some("Sample library".to_string()),
    };
    commands.create("owner", request).await.expect("create");
    commit_w010(&sample).await;
    assert!(state.install_libraries(Arc::new(commands)));
    assert!(state.install_location_chooser(Arc::new(PickingChooser)));
    assert!(state.install_import_starter(Arc::new(StartingImports)));
}

/// An import dialog in which the owner always picks something, starting job `job-1`.
struct StartingImports;

impl ImportStarter for StartingImports {
    fn start(
        &self,
        _actor: &str,
        request: &ImportRequest,
    ) -> Result<ImportAnswer, ImportStartError> {
        Ok(ImportAnswer::started(
            request.envelope.operation_id(),
            "job-1".to_string(),
        ))
    }
}

/// The sample import request into the sample library.
fn import_request() -> ImportRequest {
    ImportRequest {
        envelope: filesquid_core::CommandEnvelope::new(
            filesquid_core::OperationId::new("op-import").expect("op"),
            filesquid_core::ExpectedRevision::new("r1").expect("revision"),
            filesquid_core::OperationScope::new(format!("library:{LIBRARY}")).expect("scope"),
        ),
        library_id: LIBRARY.to_string(),
        collection_id: None,
        source: ImportSource::Folder,
    }
}

/// A folder dialog in which the owner always picks the empty folder of a new library.
struct PickingChooser;

impl LocationChooser for PickingChooser {
    fn choose(&self, _purpose: ChoicePurpose) -> Result<Option<String>, ChooseError> {
        Ok(Some(NEW.to_string()))
    }
}

/// Commit the sample object's v1 and v2, then remove its head version inventory and sidecar.
async fn commit_w010(root: &LibraryRoot<MemoryBackend, Leases>) {
    let staging = MemoryBackend::new();
    for path in ["a.txt", "b.txt"] {
        let area = StagingArea::new(&staging, "fixture").expect("staging");
        let backend = root.backend.as_ref();
        let leases = root.reservations.as_ref();
        let model = ChangeModel::new(backend, leases, "fixture", "objects/sample", OBJECT, area);
        model.stage_bytes(path, b"x").await.expect("stage");
        model.commit(VersionMeta::new(now())).await.expect("commit");
    }
    for name in ["inventory.json", "inventory.json.sha256"] {
        let key = StorageKey::new(&format!("objects/sample/v2/{name}")).expect("key");
        root.backend.delete(&key).await.expect("remove");
    }
}

/// Now, RFC 3339 UTC.
fn now() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    rfc3339_utc(millis)
}

/// The sample library ID.
fn library() -> LibraryId {
    LibraryId::new(LIBRARY).expect("library id")
}

/// A sample request body: its content type and bytes.
type SampleBody = Option<(&'static str, Vec<u8>)>;

/// A JSON body.
fn json_body(body: Value) -> SampleBody {
    Some(("application/json", body.to_string().into_bytes()))
}

/// The request URI and body of the sample call of `method path`, or `None` when `path` is
/// not a library route.
pub fn sample(method: &Method, path: &str) -> Option<(String, SampleBody)> {
    let body = match (method.as_str(), path) {
        ("POST", "/api/v1/libraries") => json!({ "location_id": NEW }),
        ("POST", "/api/v1/libraries/openings") => {
            json!({ "location_id": SAMPLE, "library_id": LIBRARY })
        }
        ("POST", "/api/v1/libraries/snapshot-copies") => {
            json!({ "library_id": LIBRARY, "destination_location_id": COPY })
        }
        ("POST", "/api/v1/libraries/rebuilds") => json!({ "library_id": LIBRARY }),
        ("POST", "/api/v1/libraries/location-choices") => json!({ "purpose": "create" }),
        ("POST", "/api/v1/imports") => serde_json::to_value(import_request()).expect("json"),
        ("GET", "/api/v1/libraries/repair-candidates") => {
            return Some((format!("{path}?library_id={LIBRARY}"), None));
        }
        ("POST", "/api/v1/libraries/repairs") => {
            serde_json::to_value(repair_request()).expect("json")
        }
        ("POST", "/api/v1/libraries/renames") => {
            json!({ "library_id": LIBRARY, "name": "Renamed sample library" })
        }
        ("GET", "/api/v1/libraries/{library}/facts") => {
            return Some((format!("/api/v1/libraries/{LIBRARY}/facts"), None));
        }
        ("GET", "/api/v1/libraries/recent") => return Some((path.to_string(), None)),
        _ => return None,
    };
    Some((path.to_string(), json_body(body)))
}

/// The consented repair of the sample object.
fn repair_request() -> RepairRequest {
    RepairRequest {
        library_id: library(),
        object_ids: vec![OBJECT.to_string()],
        consent: Some(RepairConsent {
            library_id: library(),
            object_ids: vec![OBJECT.to_string()],
            statement_id: CONSENT_STATEMENT_ID.to_string(),
            confirmed: true,
        }),
    }
}

/// `value` as JSON.
fn to_value<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("serializable")
}

/// Two repaired object outcomes, with and without a provenance record, and a refused one:
/// between them every member, each in a shape the service answers.
fn object_repairs() -> [ObjectRepair; 3] {
    let repaired = ObjectRepair {
        object_id: OBJECT.to_string(),
        version: Some("v2".to_string()),
        result: RepairResult::Repaired,
        error_code: None,
        retryable: false,
        provenance_version: Some("v3".to_string()),
        provenance_error_code: None,
    };
    let unrecorded = ObjectRepair {
        provenance_version: None,
        provenance_error_code: Some("provenance_not_configured".to_string()),
        ..repaired.clone()
    };
    let refused = ObjectRepair {
        result: RepairResult::Refused,
        error_code: Some("nothing_to_repair".to_string()),
        provenance_version: None,
        ..repaired.clone()
    };
    [repaired, unrecorded, refused]
}

/// The rebuilt sample object.
fn rebuilt() -> RebuiltObject {
    RebuiltObject {
        object_id: OBJECT.to_string(),
        head: "v2".to_string(),
        files: 2,
        digest_algorithm: "sha256".to_string(),
        error_codes: vec!["E023".to_string()],
        missing_version_inventory: true,
    }
}

/// The sample candidate.
fn candidate() -> RepairCandidate {
    RepairCandidate {
        object_id: OBJECT.to_string(),
        head: "v2".to_string(),
        digest_algorithm: "sha256".to_string(),
        interrupted: false,
        committable_after_repair: true,
    }
}

/// Serialized samples of every library and repair schema.
pub fn samples() -> Vec<(&'static str, Vec<Value>)> {
    let findings = ObjectFindings {
        object_id: OBJECT.to_string(),
        error_codes: vec!["E092".to_string()],
    };
    let copy = filesquid_api::local_library::CopyReport {
        objects: 1,
        files: 9,
        bytes: 2_048,
        invalid_objects: vec![findings.clone()],
    };
    let unreadable = UnreadableObject {
        error_code: "unreadable".to_string(),
    };
    let mut unconsented = repair_request();
    unconsented.consent = None;
    let mut all = facts_samples();
    all.extend(vec![
        (
            "ImportSource",
            [ImportSource::Files, ImportSource::Folder]
                .map(to_value)
                .to_vec(),
        ),
        (
            "ImportOutcome",
            [ImportOutcome::Started, ImportOutcome::Cancelled]
                .map(to_value)
                .to_vec(),
        ),
        (
            "ImportRequest",
            vec![
                to_value(import_request()),
                to_value(ImportRequest {
                    collection_id: Some("collection-1".into()),
                    source: ImportSource::Files,
                    ..import_request()
                }),
            ],
        ),
        (
            "ImportAnswer",
            vec![
                to_value(ImportAnswer::started(
                    import_request().envelope.operation_id(),
                    "job-1".into(),
                )),
                to_value(ImportAnswer::cancelled(
                    import_request().envelope.operation_id(),
                )),
            ],
        ),
        (
            "ChoicePurpose",
            [
                ChoicePurpose::Create,
                ChoicePurpose::Open,
                ChoicePurpose::CopyDestination,
            ]
            .map(to_value)
            .to_vec(),
        ),
        (
            "ChoiceOutcome",
            [ChoiceOutcome::Chosen, ChoiceOutcome::Cancelled]
                .map(to_value)
                .to_vec(),
        ),
        (
            "LocationChoiceRequest",
            vec![to_value(LocationChoiceRequest {
                purpose: ChoicePurpose::Open,
            })],
        ),
        (
            "LocationChoice",
            vec![
                to_value(LocationChoice {
                    outcome: ChoiceOutcome::Chosen,
                    location_id: Some(NEW.to_string()),
                }),
                to_value(LocationChoice {
                    outcome: ChoiceOutcome::Cancelled,
                    location_id: None,
                }),
            ],
        ),
        (
            "LibraryLocationRequest",
            vec![to_value(LocationRequest {
                location_id: SAMPLE.to_string(),
                library_id: Some(library()),
                name: Some("Family archive".to_string()),
            })],
        ),
        (
            "LibraryView",
            vec![to_value(LibraryView {
                library_id: library(),
                spec_version: "1.1".to_string(),
                objects: 1,
            })],
        ),
        (
            "SnapshotCopyRequest",
            vec![to_value(SnapshotCopyRequest {
                library_id: library(),
                destination_location_id: COPY.to_string(),
            })],
        ),
        (
            "LibraryRequest",
            vec![to_value(LibraryRequest {
                library_id: library(),
            })],
        ),
        ("ObjectFindings", vec![to_value(findings)]),
        ("SnapshotCopyReport", vec![to_value(copy)]),
        ("RebuiltObject", vec![to_value(rebuilt())]),
        ("UnreadableObject", vec![to_value(unreadable.clone())]),
        (
            "RebuildReport",
            vec![
                to_value(RebuildReport {
                    objects: vec![rebuilt()],
                    unreadable: vec![unreadable.clone()],
                    journal: None,
                }),
                to_value(RebuildReport {
                    objects: vec![rebuilt()],
                    unreadable: vec![unreadable],
                    journal: Some(journal_rebuild()),
                }),
            ],
        ),
        (
            "JournalRebuild",
            vec![
                to_value(journal_rebuild()),
                to_value(JournalRebuild::default()),
            ],
        ),
        ("RepairCandidate", vec![to_value(candidate())]),
        (
            "RepairCandidates",
            vec![to_value(RepairCandidates {
                candidates: vec![candidate()],
                truncated: false,
            })],
        ),
        (
            "RepairConsent",
            vec![to_value(repair_request().consent.expect("consent"))],
        ),
        (
            "RepairRequest",
            vec![to_value(repair_request()), to_value(unconsented)],
        ),
        (
            "RepairResult",
            vec![
                to_value(RepairResult::Repaired),
                to_value(RepairResult::Refused),
            ],
        ),
        ("ObjectRepair", object_repairs().map(to_value).to_vec()),
        (
            "RepairAnswer",
            vec![to_value(RepairAnswer {
                library_id: library(),
                run_id: "5f0c1d2e-3a4b-4c5d-8e6f-708192a3b4c5".to_string(),
                objects: object_repairs().to_vec(),
            })],
        ),
    ]);
    all
}

/// A journal rebuild that restored entries and found one conflict.
fn journal_rebuild() -> JournalRebuild {
    JournalRebuild {
        restored: 42,
        already_present: 3,
        conflicting: 1,
        unreadable: 0,
        last_sealed_at: Some("2026-10-20T10:00:00Z".into()),
    }
}

/// Serialized samples of the library facts schemas (Task API-F2.18; API-S-038): a known and
/// an unknown free space, an open and a remembered library, every reason.
fn facts_samples() -> Vec<(&'static str, Vec<Value>)> {
    let known = LibraryFacts {
        library_id: library(),
        name: "Family archive".to_string(),
        free_bytes: Some(1_000_000),
        free_bytes_reason: None,
    };
    let unknown = LibraryFacts {
        free_bytes: None,
        free_bytes_reason: Some(FreeBytesReason::NotLocal),
        ..known.clone()
    };
    let recent = RecentLibrary {
        library_id: library(),
        name: "Family archive".to_string(),
        last_opened: "2026-10-23T10:00:00.000Z".to_string(),
        open: true,
        free_bytes: Some(1_000_000),
        free_bytes_reason: None,
    };
    let remembered = RecentLibrary {
        open: false,
        free_bytes: None,
        free_bytes_reason: Some(FreeBytesReason::VolumeUnreadable),
        ..recent.clone()
    };
    let rename = RenameLibraryRequest {
        library_id: library(),
        name: "Family archive 1990s".to_string(),
    };
    vec![
        ("LibraryFacts", vec![to_value(&known), to_value(&unknown)]),
        (
            "RecentLibrary",
            vec![to_value(&recent), to_value(&remembered)],
        ),
        (
            "RecentLibraries",
            vec![to_value(RecentLibraries {
                libraries: vec![recent, remembered],
            })],
        ),
        ("RenameLibraryRequest", vec![to_value(rename)]),
        (
            "FreeBytesReason",
            [FreeBytesReason::VolumeUnreadable, FreeBytesReason::NotLocal]
                .map(to_value)
                .to_vec(),
        ),
    ]
}
