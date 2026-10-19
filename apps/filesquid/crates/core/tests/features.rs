//! Cucumber harness for portable-contract Gherkin under `tests/features/`.

use std::fs;
use std::path::PathBuf;

use cucumber::{given, then, when, writer, StatsWriter as _, World, WriterExt as _};
use filesquid_core::{
    admit_active_transfers, health, reserve_gib, retain_files_after_collection_removal,
    Argon2idParams, C2paAction, C2paImportRecord, C2paValidationStatus, CanonicalReceipt,
    CapabilityInventory, CapabilityRegister, CapabilityRow, CapabilityStatus, Collection,
    CollectionId, CollectionMembership, CollectionSort, CommandEnvelope, CommandOutcome,
    CommandOutcomeKind, ContentEnvelope, ExpectedRevision, File as DomainFile, FileId,
    FileResource, FileResourceId, FileVersion, FileVersionId, Folder, FolderId, HkdfInfoSet,
    InstallationId, ItemType, JobCheckpoint, JobLease, KeyId, KeyPurpose, Library, LibraryId,
    OperationId, OperationKind, OperationMessage, OperationMessageError, OperationScope, PageSize,
    PortError, ProvOSidecar, QueryEnvelope, RecipeOperation, Repository, RepositoryId, Rfc3339Utc,
    Sha256Digest, UiProgress, VirtualCollection, VirtualCollectionId, VirtualExhibit,
    VirtualExhibitId, VirtualMembership, WrapAssociatedData, WrapWireLayout,
    DEFAULT_ACTIVE_TRANSFERS, DEFAULT_CPU_HEAVY_WORKERS, DEFAULT_STAGING_CAP_GIB,
    HEALTH_SCHEMA_VERSION, HKDF_INFO_CONTENT, HKDF_INFO_DEVICE, HKDF_INFO_EVIDENCE, HKDF_INFO_WRAP,
    MAX_ATTEMPTS_PER_RESOURCE, MIGRATION_PROCESS_MEMORY_GIB, OPERATION_MESSAGE_PREFIX,
    PROTOCOL_SCHEMA_VERSION, PROV_O_CONTEXT, PROV_O_PROFILE, RESTART_RESUMABLE_WITHIN_SECS,
    RETRY_BACKOFF_CAP_SECS, WRAP_WIRE_VERSION,
};

/// World state for portable-contract scenarios.
#[derive(Debug, Default, World)]
struct CoreWorld {
    /// Text of the last operation message or installation URN a step made.
    message_text: String,
    /// How the last operation message parsed.
    message_parse: Option<ParsedMessage>,
    raw_id: String,
    library_id: Option<LibraryId>,
    file_id: Option<FileId>,
    repository_id: Option<RepositoryId>,
    collection_id: Option<CollectionId>,
    virtual_collection_id: Option<VirtualCollectionId>,
    virtual_exhibit_id: Option<VirtualExhibitId>,
    file_version_id: Option<FileVersionId>,
    id_failed: bool,
    repository: Option<Repository>,
    libraries: Vec<Library>,
    item_type_error: bool,
    file: Option<DomainFile>,
    original: Option<FileResource>,
    original_digest: Option<String>,
    version: Option<FileVersion>,
    virtual_membership: Option<VirtualMembership>,
    virtual_collection: Option<VirtualCollection>,
    virtual_exhibit: Option<VirtualExhibit>,
    parent_folder: Option<Folder>,
    child_folder: Option<Folder>,
    memberships: Vec<CollectionMembership>,
    collection: Option<Collection>,
    health: Option<filesquid_core::HealthStatus>,
    library_dep_names: Vec<String>,
    second_home_failed: bool,
    replace_failed: bool,
    observed_item_type: Option<String>,
    retained_files: Vec<DomainFile>,
    operation_raw: String,
    operation_failed: bool,
    command: Option<CommandEnvelope>,
    query: Option<QueryEnvelope>,
    page_size_failed: bool,
    outcome: Option<CommandOutcome>,
    port_error: Option<PortError>,
    ui_progress: Option<UiProgress>,
    checkpoint: Option<JobCheckpoint>,
    receipt: Option<CanonicalReceipt>,
    lease: Option<JobLease>,
    reserve_gib: Option<u64>,
    transfer_failed: bool,
    envelope: Option<ContentEnvelope>,
    hkdf: HkdfInfoSet,
    hkdf_failed: bool,
    argon: Option<Argon2idParams>,
    aad: Option<WrapAssociatedData>,
    historical_ciphertext: Option<Vec<u8>>,
    new_wrap_key: Option<String>,
    resource_id: Option<FileResourceId>,
    resource_digest: Option<Sha256Digest>,
    c2pa: Option<C2paImportRecord>,
    c2pa_action: Option<C2paAction>,
    provo: Option<ProvOSidecar>,
    signed_mutate_failed: bool,
    capability_rows: Vec<CapabilityRow>,
    register: Option<CapabilityRegister>,
    register_failed: bool,
    crate_root: String,
}

fn digest() -> Sha256Digest {
    Sha256Digest::parse("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        .expect("digest")
}

fn quoted_package_rename(line: &str) -> Option<String> {
    let idx = line.find("package")?;
    let after = line[idx + "package".len()..].trim_start();
    let after = after.strip_prefix('=')?.trim_start();
    let inner = after.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

fn library_dependency_package_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if let Some(rest) = trimmed.strip_prefix("[dependencies.") {
                if let Some(name) = rest.strip_suffix(']') {
                    names.push(name.to_string());
                }
                in_deps = false;
                continue;
            }
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if !in_deps || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let key = trimmed
            .split([' ', '='])
            .next()
            .filter(|name| !name.is_empty());
        let Some(key) = key else {
            continue;
        };
        names.push(quoted_package_rename(trimmed).unwrap_or_else(|| key.to_string()));
    }
    names.sort();
    names.dedup();
    names
}

#[given(expr = "an opaque identifier string {string}")]
fn opaque_identifier(world: &mut CoreWorld, value: String) {
    world.raw_id = value;
    world.id_failed = false;
}

#[given(expr = "a repository {string}")]
fn a_repository(world: &mut CoreWorld, value: String) {
    let id = RepositoryId::new(value).expect("repository id");
    world.repository = Some(Repository::new(id));
}

#[given(expr = "a file whose home collection is {string}")]
fn file_with_home(world: &mut CoreWorld, collection: String) {
    let file = DomainFile::new(
        FileId::new("file-1").expect("file"),
        LibraryId::new("lib-1").expect("lib"),
        CollectionId::new(collection).expect("col"),
    );
    world.file = Some(file);
}

#[given("an original file resource")]
fn original_resource(world: &mut CoreWorld) {
    let original = FileResource::original(
        FileResourceId::new("res-1").expect("res"),
        FileId::new("file-1").expect("file"),
        1024,
        digest(),
    );
    world.original_digest = Some(original.digest().as_str().to_string());
    world.original = Some(original);
}

#[given(expr = "a folder {string} in a library")]
fn a_folder(world: &mut CoreWorld, name: String) {
    world.parent_folder = Some(
        Folder::new(
            FolderId::new(&name).expect("id"),
            LibraryId::new("lib-1").expect("lib"),
            name,
        )
        .expect("folder"),
    );
}

#[given(expr = "a collection with files in order {string}")]
fn collection_order(world: &mut CoreWorld, order: String) {
    let collection = CollectionId::new("col-1").expect("col");
    let library = LibraryId::new("lib-1").expect("lib");
    let mut memberships: Vec<_> = order
        .split(',')
        .enumerate()
        .map(|(i, name)| {
            let file = DomainFile::new(
                FileId::new(name).expect("file"),
                library.clone(),
                collection.clone(),
            );
            CollectionMembership::for_file(&file, i as i64)
        })
        .collect();
    memberships.reverse();
    world.memberships = memberships;
}

#[given(expr = "a collection titled {string} with date descending sort and cover {string}")]
fn collection_sort_cover(world: &mut CoreWorld, title: String, cover: String) {
    world.collection = Some(
        Collection::new(
            CollectionId::new("col-1").expect("col"),
            LibraryId::new("lib-1").expect("lib"),
            title,
        )
        .expect("collection")
        .with_sort(CollectionSort::DateDescending)
        .with_cover(FileId::new(cover).expect("file")),
    );
}

#[when("I construct a library id")]
fn construct_library_id(world: &mut CoreWorld) {
    match LibraryId::new(&world.raw_id) {
        Ok(id) => {
            world.library_id = Some(id);
            world.id_failed = false;
        }
        Err(_) => world.id_failed = true,
    }
}

#[when("I construct a file id")]
fn construct_file_id(world: &mut CoreWorld) {
    world.file_id = Some(FileId::new(&world.raw_id).expect("file id"));
}

#[when(
    "I construct repository, collection, virtual collection, virtual exhibit, and file version ids"
)]
fn construct_other_ids(world: &mut CoreWorld) {
    world.repository_id = Some(RepositoryId::new(&world.raw_id).expect("repo"));
    world.collection_id = Some(CollectionId::new(&world.raw_id).expect("col"));
    world.virtual_collection_id = Some(VirtualCollectionId::new(&world.raw_id).expect("vc"));
    world.virtual_exhibit_id = Some(VirtualExhibitId::new(&world.raw_id).expect("ex"));
    world.file_version_id = Some(FileVersionId::new(&world.raw_id).expect("ver"));
}

#[when(expr = "I attach libraries {string} and {string} to that repository")]
fn attach_libraries(world: &mut CoreWorld, a: String, b: String) {
    let repo = world.repository.as_ref().expect("repository");
    world.libraries = vec![
        Library::new(LibraryId::new(a).expect("id"), repo.id().clone()),
        Library::new(LibraryId::new(b).expect("id"), repo.id().clone()),
    ];
}

#[when(expr = "I construct collection {string} in library {string}")]
fn construct_collection_in_library(world: &mut CoreWorld, collection: String, library: String) {
    world.collection = Some(
        Collection::new(
            CollectionId::new(collection).expect("col"),
            LibraryId::new(library).expect("lib"),
            "Holidays",
        )
        .expect("collection"),
    );
}

#[when(expr = "I construct virtual exhibit {string} on that repository")]
fn construct_virtual_exhibit(world: &mut CoreWorld, exhibit: String) {
    let repo = world.repository.as_ref().expect("repository");
    world.virtual_exhibit = Some(
        VirtualExhibit::new(
            VirtualExhibitId::new(exhibit).expect("ex"),
            repo.id().clone(),
            "Public feed",
        )
        .expect("exhibit"),
    );
}

#[when("I inspect the portable item type")]
fn inspect_item_type(world: &mut CoreWorld) {
    world.observed_item_type = Some(ItemType::File.as_str().to_string());
}

#[when(expr = "I deserialize {string} as an item type")]
fn deserialize_item_type(world: &mut CoreWorld, label: String) {
    let json = format!("\"{label}\"");
    world.item_type_error = serde_json::from_str::<ItemType>(&json).is_err();
}

#[when(expr = "I assign home collection {string}")]
fn assign_home(world: &mut CoreWorld, collection: String) {
    let file = world.file.as_ref().expect("file");
    match file.assign_home_collection(CollectionId::new(collection).expect("col")) {
        Ok(updated) => {
            world.file = Some(updated);
            world.second_home_failed = false;
        }
        Err(_) => world.second_home_failed = true,
    }
}

#[when(expr = "I add virtual membership in {string}")]
fn add_virtual_membership(world: &mut CoreWorld, vc: String) {
    let file = world.file.as_ref().expect("file");
    let id = VirtualCollectionId::new(&vc).expect("vc");
    world.virtual_membership = Some(VirtualMembership::new(id.clone(), file.id().clone(), 0));
    world.virtual_collection =
        Some(VirtualCollection::new(id, LibraryId::new("lib-1").expect("lib"), vc).expect("vc"));
}

#[when("I replace its stored bytes")]
fn replace_bytes(world: &mut CoreWorld) {
    let original = world.original.as_ref().expect("original");
    world.replace_failed = original.with_replaced_bytes(2048, digest()).is_err();
}

#[when("I create a file version with a recipe operation")]
fn create_version(world: &mut CoreWorld) {
    let original = world.original.as_ref().expect("original");
    let op = RecipeOperation::new("exposure", "engine-1").with_parameter("ev", "0.5");
    world.version = Some(FileVersion::new(
        FileVersionId::new("ver-1").expect("ver"),
        FileId::new("file-1").expect("file"),
        original.id().clone(),
        vec![op],
    ));
}

#[when(expr = "I remove collection {string} from the model")]
fn remove_collection(world: &mut CoreWorld, _collection: String) {
    let files: Vec<_> = world.file.clone().into_iter().collect();
    world.retained_files = retain_files_after_collection_removal(&files).to_vec();
    world.file = world.retained_files.first().cloned();
}

#[when(expr = "I nest folder {string} under {string}")]
fn nest_folder(world: &mut CoreWorld, child: String, parent: String) {
    let parent_id = world
        .parent_folder
        .as_ref()
        .map(|f| f.id().clone())
        .unwrap_or_else(|| FolderId::new(parent).expect("parent"));
    world.child_folder = Some(
        Folder::new(
            FolderId::new(&child).expect("id"),
            LibraryId::new("lib-1").expect("lib"),
            child,
        )
        .expect("folder")
        .nest_under(parent_id),
    );
}

#[when("I inspect membership order")]
fn inspect_membership(world: &mut CoreWorld) {
    world
        .memberships
        .sort_by_key(CollectionMembership::order_key);
}

#[when("I request portable health status")]
fn request_health(world: &mut CoreWorld) {
    world.health = Some(health());
}

#[when("I inspect the core crate library dependencies")]
fn inspect_deps(world: &mut CoreWorld) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(path).expect("Cargo.toml");
    world.library_dep_names = library_dependency_package_names(&manifest);
}

#[when("I inspect the core crate root attributes")]
fn inspect_crate_root(world: &mut CoreWorld) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    world.crate_root = fs::read_to_string(path).expect("lib.rs");
}

#[given(expr = "an operation id {string}")]
fn an_operation_id(world: &mut CoreWorld, value: String) {
    world.operation_raw = value;
    world.operation_failed = false;
}

#[given(expr = "a file resource {string} with digest {string}")]
fn a_file_resource(world: &mut CoreWorld, id: String, hex: String) {
    world.resource_id = Some(FileResourceId::new(id).expect("resource"));
    world.resource_digest = Some(Sha256Digest::parse(hex).expect("digest"));
}

#[when(expr = "I construct a command envelope with expected revision {string} and scope {string}")]
fn construct_command(world: &mut CoreWorld, revision: String, scope: String) {
    match OperationId::new(&world.operation_raw) {
        Ok(id) => {
            world.command = Some(CommandEnvelope::new(
                id,
                ExpectedRevision::new(revision).expect("rev"),
                OperationScope::new(scope).expect("scope"),
            ));
            world.operation_failed = false;
        }
        Err(_) => world.operation_failed = true,
    }
}

#[when(expr = "I construct a query envelope with page size {int} and scope {string}")]
fn construct_query(world: &mut CoreWorld, page: u32, scope: String) {
    match PageSize::new(page) {
        Ok(size) => {
            world.query = Some(QueryEnvelope::new(
                size,
                OperationScope::new(scope).expect("scope"),
            ));
            world.page_size_failed = false;
        }
        Err(_) => world.page_size_failed = true,
    }
}

#[when(expr = "I record a committed command outcome for operation {string}")]
fn record_committed(world: &mut CoreWorld, id: String) {
    world.outcome = Some(CommandOutcome::new(
        OperationId::new(id).expect("op"),
        CommandOutcomeKind::Committed,
    ));
}

#[when(expr = "I record a pending command outcome for operation {string}")]
fn record_pending(world: &mut CoreWorld, id: String) {
    world.outcome = Some(CommandOutcome::new(
        OperationId::new(id).expect("op"),
        CommandOutcomeKind::Pending,
    ));
}

#[when(expr = "I record a conflict command outcome for operation {string}")]
fn record_conflict(world: &mut CoreWorld, id: String) {
    world.outcome = Some(CommandOutcome::new(
        OperationId::new(id).expect("op"),
        CommandOutcomeKind::Conflict,
    ));
}

#[when(expr = "I record a retryable port error {string}")]
fn record_port_error(world: &mut CoreWorld, code: String) {
    world.port_error = Some(PortError::new(code, true).expect("error"));
}

#[when(
    expr = "I record an error command outcome for operation {string} with retryable code {string}"
)]
fn record_error_outcome(world: &mut CoreWorld, id: String, code: String) {
    let err = PortError::new(code, true).expect("error");
    world.port_error = Some(err.clone());
    world.outcome = Some(CommandOutcome::failed(
        OperationId::new(id).expect("op"),
        err,
    ));
}

#[when(expr = "I record UI progress for operation {string}")]
fn record_ui_progress(world: &mut CoreWorld, id: String) {
    world.ui_progress = Some(UiProgress::new(OperationId::new(id).expect("op")));
}

#[when("I construct a job checkpoint")]
fn construct_checkpoint(world: &mut CoreWorld) {
    let id = OperationId::new(&world.operation_raw).expect("op");
    world.checkpoint = Some(JobCheckpoint::new(id));
}

#[when("I construct a canonical receipt")]
fn construct_receipt(world: &mut CoreWorld) {
    let id = OperationId::new(&world.operation_raw).expect("op");
    world.receipt = Some(CanonicalReceipt::new(id));
}

#[when("I inspect job worker caps")]
fn inspect_worker_caps(_world: &mut CoreWorld) {}

#[when(expr = "I inspect staging bounds for a {int} GiB destination")]
fn inspect_staging(world: &mut CoreWorld, dest: u64) {
    world.reserve_gib = Some(reserve_gib(dest));
}

#[when("I inspect retry bounds")]
fn inspect_retry(_world: &mut CoreWorld) {}

#[when(expr = "a worker holds lease fencing token {int}")]
fn hold_lease(world: &mut CoreWorld, token: u64) {
    let id = OperationId::new(&world.operation_raw).expect("op");
    world.lease = Some(JobLease::grant(id, token));
}

#[when(expr = "a replacement takes lease fencing token {int}")]
fn take_lease(world: &mut CoreWorld, token: u64) {
    world
        .lease
        .as_mut()
        .expect("lease")
        .take_over(token)
        .expect("takeover");
}

#[when(expr = "I admit {int} active transfers")]
fn admit_transfers(world: &mut CoreWorld, count: u32) {
    world.transfer_failed = admit_active_transfers(count).is_err();
}

#[when(expr = "I construct a content envelope for purpose {string}")]
fn construct_envelope(world: &mut CoreWorld, purpose: String) {
    let purpose = parse_purpose(&purpose);
    world.envelope = Some(ContentEnvelope::new(purpose, [1u8; 12], vec![4, 5, 6]));
}

#[when("I inspect wrap wire parameters")]
fn inspect_wrap(_world: &mut CoreWorld) {}

#[when("I register HKDF info labels for content, evidence, wrap, and device")]
fn register_hkdf(world: &mut CoreWorld) {
    world.hkdf = HkdfInfoSet::default();
    world.hkdf.register(KeyPurpose::Content).expect("content");
    world.hkdf.register(KeyPurpose::Evidence).expect("evidence");
    world.hkdf.register(KeyPurpose::Wrap).expect("wrap");
    world.hkdf.register(KeyPurpose::Device).expect("device");
    world.hkdf_failed = false;
}

#[when("I reuse the content HKDF purpose")]
fn reuse_hkdf(world: &mut CoreWorld) {
    world.hkdf_failed = world.hkdf.register(KeyPurpose::Content).is_err();
}

#[when("I construct Argon2id wrap parameters")]
fn construct_argon(world: &mut CoreWorld) {
    world.argon = Some(Argon2idParams::recovery_default());
}

#[when(expr = "I construct wrap associated data for key {string} and purpose {string}")]
fn construct_aad(world: &mut CoreWorld, key: String, purpose: String) {
    world.aad = Some(WrapAssociatedData::new(
        KeyId::new(key).expect("key"),
        parse_purpose(&purpose),
    ));
}

#[when("I rewrap the envelope for a new device")]
fn rewrap_envelope(world: &mut CoreWorld) {
    let envelope = world.envelope.as_ref().expect("envelope");
    world.historical_ciphertext = Some(envelope.historical_ciphertext().to_vec());
    let result = envelope.plan_rewrap(KeyId::new("device-2").expect("key"));
    world.new_wrap_key = Some(result.target_key_id().as_str().to_string());
}

#[when("I inspect key purposes")]
fn inspect_purposes(_world: &mut CoreWorld) {}

#[when(expr = "I initialize a C2PA import record at {string}")]
fn init_c2pa(world: &mut CoreWorld, time: String) {
    world.c2pa = Some(C2paImportRecord::initialize(
        world.resource_id.clone().expect("resource"),
        world.resource_digest.clone().expect("digest"),
        Rfc3339Utc::parse(time).expect("time"),
    ));
}

#[when(expr = "I set validation status to {string}")]
fn set_status(world: &mut CoreWorld, status: String) {
    if world.c2pa.is_none() {
        world.c2pa = Some(C2paImportRecord::initialize(
            FileResourceId::new("res-1").expect("res"),
            Sha256Digest::parse("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
                .expect("digest"),
            Rfc3339Utc::parse("2026-09-23T12:00:00Z").expect("time"),
        ));
    }
    let parsed = match status.as_str() {
        "signed" => C2paValidationStatus::Signed,
        "invalid" => C2paValidationStatus::Invalid,
        "unavailable" => C2paValidationStatus::Unavailable,
        "unsigned" => C2paValidationStatus::Unsigned,
        other => panic!("unknown status {other}"),
    };
    world.c2pa.as_mut().expect("c2pa").set_status(parsed);
}

#[when("I append a catalog-only C2PA action")]
fn catalog_action(world: &mut CoreWorld) {
    world.c2pa_action = Some(C2paAction::catalog_only());
}

#[when(expr = "I record an ephemeral cache activity at {string}")]
fn ephemeral_activity(world: &mut CoreWorld, time: String) {
    world.provo = Some(ProvOSidecar::ephemeral_cache(
        Rfc3339Utc::parse(time).expect("time"),
    ));
}

#[when("I mutate the signed manifest in place")]
fn mutate_signed(world: &mut CoreWorld) {
    world.signed_mutate_failed = world
        .c2pa
        .as_ref()
        .expect("c2pa")
        .mutate_signed_in_place()
        .is_err();
}

#[when(expr = "I construct a PROV-O sidecar at {string}")]
fn construct_provo(world: &mut CoreWorld, time: String) {
    world.provo = Some(ProvOSidecar::retained(
        Rfc3339Utc::parse(time).expect("time"),
    ));
}

#[when(expr = "I add a supported capability {string} with status {string}")]
fn add_supported(world: &mut CoreWorld, id: String, status: String) {
    let status = parse_capability_status(&status);
    world
        .capability_rows
        .push(CapabilityRow::new(id, status, None, capability_inventory()).expect("row"));
    world.register = CapabilityRegister::new(world.capability_rows.clone()).ok();
    world.register_failed = world.register.is_none();
}

#[when(expr = "I add an unavailable child capability {string}")]
fn add_unavailable_child(world: &mut CoreWorld, id: String) {
    let parent = world
        .capability_rows
        .first()
        .map(|row| row.id().to_string());
    world.capability_rows.push(
        CapabilityRow::new(
            id,
            CapabilityStatus::Unavailable,
            parent,
            capability_inventory(),
        )
        .expect("row"),
    );
    world.register = CapabilityRegister::new(world.capability_rows.clone()).ok();
    world.register_failed = world.register.is_none();
}

#[when("I build a capability register with no rows")]
fn empty_register(world: &mut CoreWorld) {
    world.register_failed = CapabilityRegister::new(vec![]).is_err();
}

#[when("I add only unsupported rows for a mandatory capability")]
fn all_unsupported(world: &mut CoreWorld) {
    let row = CapabilityRow::new(
        "heic-edit",
        CapabilityStatus::Unavailable,
        None,
        capability_inventory(),
    )
    .expect("row");
    world.register_failed = CapabilityRegister::new(vec![row]).is_err();
}

fn capability_inventory() -> CapabilityInventory {
    CapabilityInventory::new("linux-desktop", "jpeg", "import", "F-040", "unit").expect("inv")
}

fn parse_purpose(label: &str) -> KeyPurpose {
    match label {
        "content" => KeyPurpose::Content,
        "evidence" => KeyPurpose::Evidence,
        "wrap" => KeyPurpose::Wrap,
        "device" => KeyPurpose::Device,
        other => panic!("unknown purpose {other}"),
    }
}

fn parse_capability_status(label: &str) -> CapabilityStatus {
    match label {
        "equivalent" => CapabilityStatus::Equivalent,
        "adapter-equivalent" => CapabilityStatus::AdapterEquivalent,
        "limited" => CapabilityStatus::Limited,
        "preservation-only" => CapabilityStatus::PreservationOnly,
        "unavailable" => CapabilityStatus::Unavailable,
        other => panic!("unknown capability status {other}"),
    }
}

#[then(expr = "the library id should equal {string}")]
fn library_id_equals(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.library_id.as_ref().expect("library id").as_str(),
        expected
    );
}

#[then(expr = "the file id should equal {string}")]
fn file_id_equals(world: &mut CoreWorld, expected: String) {
    assert_eq!(world.file_id.as_ref().expect("file id").as_str(), expected);
}

#[then(expr = "the item type should be {string}")]
fn item_type_is(world: &mut CoreWorld, expected: String) {
    assert_eq!(ItemType::File.as_str(), expected);
    if let Some(observed) = &world.observed_item_type {
        assert_eq!(observed, &expected);
    }
}

#[then("identifier construction should fail")]
fn id_failed(world: &mut CoreWorld) {
    assert!(world.id_failed);
}

#[then(expr = "each constructed identifier should equal {string}")]
fn each_id_equals(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.repository_id.as_ref().expect("repo").as_str(),
        expected
    );
    assert_eq!(
        world.collection_id.as_ref().expect("col").as_str(),
        expected
    );
    assert_eq!(
        world.virtual_collection_id.as_ref().expect("vc").as_str(),
        expected
    );
    assert_eq!(
        world.virtual_exhibit_id.as_ref().expect("ex").as_str(),
        expected
    );
    assert_eq!(
        world.file_version_id.as_ref().expect("ver").as_str(),
        expected
    );
}

#[then(expr = "both libraries should belong to {string}")]
fn both_libraries(world: &mut CoreWorld, repo: String) {
    assert_eq!(world.libraries.len(), 2);
    for lib in &world.libraries {
        assert_eq!(lib.repository_id().as_str(), repo);
    }
}

#[then("item type deserialization should fail")]
fn item_type_failed(world: &mut CoreWorld) {
    assert!(world.item_type_error);
}

#[then("assigning a second home collection should fail")]
fn second_home_failed(world: &mut CoreWorld) {
    assert!(world.second_home_failed);
}

#[then(expr = "the file home collection should be {string}")]
fn home_collection_is(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .file
            .as_ref()
            .expect("file")
            .home_collection_id()
            .as_str(),
        expected
    );
}

#[then(expr = "the file should have a virtual membership in {string}")]
fn has_virtual_membership(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .virtual_membership
            .as_ref()
            .expect("membership")
            .virtual_collection_id()
            .as_str(),
        expected
    );
}

#[then("replacing original bytes should fail")]
fn replace_failed(world: &mut CoreWorld) {
    assert!(world.replace_failed);
}

#[then("the original digest should be unchanged")]
fn digest_unchanged(world: &mut CoreWorld) {
    let expected = world.original_digest.as_ref().expect("digest");
    assert_eq!(
        world.original.as_ref().expect("original").digest().as_str(),
        expected
    );
}

#[then("the version should reference the recipe")]
fn version_has_recipe(world: &mut CoreWorld) {
    let version = world.version.as_ref().expect("version");
    assert_eq!(version.operations().len(), 1);
    assert_eq!(version.operations()[0].name(), "exposure");
}

#[then("the file identifier should still exist")]
fn file_still_exists(world: &mut CoreWorld) {
    assert_eq!(world.retained_files.len(), 1);
    assert_eq!(world.retained_files[0].id().as_str(), "file-1");
}

#[then(expr = "the child folder parent should be {string}")]
fn child_parent(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .child_folder
            .as_ref()
            .expect("child")
            .parent_id()
            .expect("parent")
            .as_str(),
        expected
    );
}

#[then(expr = "the membership order should be {string}")]
fn membership_order(world: &mut CoreWorld, expected: String) {
    let actual: Vec<_> = world
        .memberships
        .iter()
        .map(|m| m.file_id().as_str())
        .collect();
    let expected: Vec<_> = expected.split(',').collect();
    assert_eq!(actual, expected);
}

#[then("the virtual collection should not store bytes")]
fn vc_no_bytes(world: &mut CoreWorld) {
    assert!(!world
        .virtual_collection
        .as_ref()
        .expect("vc")
        .stores_bytes());
}

#[then(expr = "the virtual exhibit should belong to {string}")]
fn exhibit_belongs_to_repo(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .virtual_exhibit
            .as_ref()
            .expect("exhibit")
            .repository_id()
            .as_str(),
        expected
    );
}

#[then("the virtual exhibit should not store bytes")]
fn exhibit_no_bytes(world: &mut CoreWorld) {
    assert!(!world
        .virtual_exhibit
        .as_ref()
        .expect("exhibit")
        .stores_bytes());
}

#[then("virtual exhibit members must be public")]
fn exhibit_members_public(world: &mut CoreWorld) {
    assert!(world
        .virtual_exhibit
        .as_ref()
        .expect("exhibit")
        .members_are_public());
}

#[then(expr = "the collection should belong to library {string}")]
fn collection_belongs_to_library(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .collection
            .as_ref()
            .expect("collection")
            .library_id()
            .as_str(),
        expected
    );
}

#[then("the collection sort should be date descending")]
fn sort_date_desc(world: &mut CoreWorld) {
    assert_eq!(
        world.collection.as_ref().expect("collection").sort(),
        CollectionSort::DateDescending
    );
}

#[then(expr = "the collection cover should be {string}")]
fn cover_is(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .collection
            .as_ref()
            .expect("collection")
            .cover_file_id()
            .expect("cover")
            .as_str(),
        expected
    );
}

#[then(expr = "the health status should be {string}")]
fn health_status(world: &mut CoreWorld, expected: String) {
    assert_eq!(world.health.as_ref().expect("health").status, expected);
}

#[then(expr = "the product should be {string}")]
fn health_product(world: &mut CoreWorld, expected: String) {
    assert_eq!(world.health.as_ref().expect("health").product, expected);
}

#[then("the health schema version should be 1")]
fn health_version(world: &mut CoreWorld) {
    assert_eq!(
        world.health.as_ref().expect("health").schema_version,
        HEALTH_SCHEMA_VERSION
    );
}

#[then("Axum, Tauri, libvips, FFmpeg, and OCFL I/O should be absent")]
fn forbidden_absent(world: &mut CoreWorld) {
    assert_eq!(
        world.library_dep_names,
        vec!["serde".to_string()],
        "portable contracts [dependencies] must be serde only"
    );
}

#[then("axum, tauri, libvips, ffmpeg-sys, filesquid-ocfl, filesquid_ocfl, imagesquid-ocfl, and imagesquid_ocfl should be absent from library dependencies")]
fn named_forbidden_absent(world: &mut CoreWorld) {
    for forbidden in [
        "axum",
        "tauri",
        "libvips",
        "ffmpeg-sys",
        "filesquid-ocfl",
        "filesquid_ocfl",
        "imagesquid-ocfl",
        "imagesquid_ocfl",
    ] {
        assert!(
            !world.library_dep_names.iter().any(|name| name == forbidden),
            "{forbidden} must be absent"
        );
    }
}

#[then("missing rustdoc should be denied")]
fn missing_docs_denied(world: &mut CoreWorld) {
    assert!(world.crate_root.contains("#![deny(missing_docs)]"));
}

#[then("the command schema version should be 1")]
fn command_schema(world: &mut CoreWorld) {
    assert_eq!(
        world.command.as_ref().expect("command").schema_version(),
        PROTOCOL_SCHEMA_VERSION
    );
}

#[then(expr = "the command operation id should be {string}")]
fn command_op(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .command
            .as_ref()
            .expect("command")
            .operation_id()
            .as_str(),
        expected
    );
}

#[then(expr = "the expected revision should be {string}")]
fn expected_revision(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .command
            .as_ref()
            .expect("command")
            .expected_revision()
            .as_str(),
        expected
    );
}

#[then(expr = "the command scope should be {string}")]
fn command_scope(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.command.as_ref().expect("command").scope().as_str(),
        expected
    );
}

#[then(expr = "the query page size should be {int}")]
fn query_page(world: &mut CoreWorld, expected: u32) {
    assert_eq!(
        world.query.as_ref().expect("query").page_size().get(),
        expected
    );
}

#[then("the query schema version should be 1")]
fn query_schema(world: &mut CoreWorld) {
    assert_eq!(world.query.as_ref().expect("query").schema_version(), 1);
}

#[then("page size construction should fail")]
fn page_size_failed(world: &mut CoreWorld) {
    assert!(world.page_size_failed);
}

#[then("operation id construction should fail")]
fn operation_failed(world: &mut CoreWorld) {
    assert!(world.operation_failed);
}

#[then(expr = "the command outcome should be {string}")]
fn outcome_kind(world: &mut CoreWorld, expected: String) {
    let kind = world.outcome.as_ref().expect("outcome").kind();
    let actual = match kind {
        CommandOutcomeKind::Committed => "committed",
        CommandOutcomeKind::Pending => "pending",
        CommandOutcomeKind::Conflict => "conflict",
        CommandOutcomeKind::Error => "error",
    };
    assert_eq!(actual, expected);
}

#[then("the outcome should be durable completion evidence")]
fn outcome_durable(world: &mut CoreWorld) {
    assert!(world
        .outcome
        .as_ref()
        .expect("outcome")
        .is_durable_completion());
}

#[then("the outcome should not be durable completion evidence")]
fn outcome_not_durable(world: &mut CoreWorld) {
    assert!(!world
        .outcome
        .as_ref()
        .expect("outcome")
        .is_durable_completion());
}

#[then(expr = "the import digest should be {string}")]
fn import_digest_is(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.c2pa.as_ref().expect("c2pa").digest().as_str(),
        expected
    );
}

#[then(expr = "the port error code should be {string}")]
fn port_code(world: &mut CoreWorld, expected: String) {
    assert_eq!(world.port_error.as_ref().expect("error").code(), expected);
}

#[then("the port error should be retryable")]
fn port_retryable(world: &mut CoreWorld) {
    assert!(world.port_error.as_ref().expect("error").retryable());
}

#[then("UI progress should not be durable completion evidence")]
fn ui_not_durable(world: &mut CoreWorld) {
    assert!(!world
        .ui_progress
        .as_ref()
        .expect("ui")
        .is_durable_completion());
}

#[then(expr = "the checkpoint operation id should be {string}")]
fn checkpoint_op(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .checkpoint
            .as_ref()
            .expect("checkpoint")
            .operation_id()
            .as_str(),
        expected
    );
}

#[then("the checkpoint should not be an OCFL collection snapshot")]
fn checkpoint_not_snapshot(world: &mut CoreWorld) {
    assert!(!world
        .checkpoint
        .as_ref()
        .expect("checkpoint")
        .is_ocfl_collection_snapshot());
}

#[then("the receipt should be durable completion evidence")]
fn receipt_durable(world: &mut CoreWorld) {
    assert!(world
        .receipt
        .as_ref()
        .expect("receipt")
        .is_durable_completion());
}

#[then(expr = "the receipt operation id should be {string}")]
fn receipt_op(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .receipt
            .as_ref()
            .expect("receipt")
            .operation_id()
            .as_str(),
        expected
    );
}

#[then("the default active transfers should be 4")]
fn default_transfers(_world: &mut CoreWorld) {
    assert_eq!(DEFAULT_ACTIVE_TRANSFERS, 4);
}

#[then("the default CPU-heavy workers should be 2")]
fn default_cpu(_world: &mut CoreWorld) {
    assert_eq!(DEFAULT_CPU_HEAVY_WORKERS, 2);
}

#[then("the default staging cap should be 10 GiB")]
fn staging_cap(_world: &mut CoreWorld) {
    assert_eq!(DEFAULT_STAGING_CAP_GIB, 10);
}

#[then(expr = "the reserve should be {int} GiB")]
fn reserve_is(world: &mut CoreWorld, expected: u64) {
    assert_eq!(world.reserve_gib.expect("reserve"), expected);
}

#[then("the maximum attempts per resource should be 5")]
fn max_attempts(_world: &mut CoreWorld) {
    assert_eq!(MAX_ATTEMPTS_PER_RESOURCE, 5);
}

#[then("the retry backoff cap should be 60 seconds")]
fn backoff_cap(_world: &mut CoreWorld) {
    assert_eq!(RETRY_BACKOFF_CAP_SECS, 60);
}

#[then("the restart resumable bound should be 30 seconds")]
fn restart_bound(_world: &mut CoreWorld) {
    assert_eq!(RESTART_RESUMABLE_WITHIN_SECS, 30);
}

#[then("the migration process memory bound should be 2 GiB")]
fn migration_memory(_world: &mut CoreWorld) {
    assert_eq!(MIGRATION_PROCESS_MEMORY_GIB, 2);
}

#[then("the expired worker with token 1 should not be allowed to commit")]
fn expired_cannot_commit(world: &mut CoreWorld) {
    assert!(!world.lease.as_ref().expect("lease").allows_commit(1));
}

#[then("the replacement with token 2 should be allowed to commit")]
fn replacement_can_commit(world: &mut CoreWorld) {
    assert!(world.lease.as_ref().expect("lease").allows_commit(2));
}

#[then("admitting the transfer should fail")]
fn transfer_failed(world: &mut CoreWorld) {
    assert!(world.transfer_failed);
}

#[then(expr = "the envelope algorithm should be {string}")]
fn envelope_alg(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.envelope.as_ref().expect("envelope").algorithm(),
        expected
    );
}

#[then("the nonce length should be 12 bytes")]
fn nonce_len(world: &mut CoreWorld) {
    assert_eq!(world.envelope.as_ref().expect("envelope").nonce_len(), 12);
}

#[then("the tag length should be 16 bytes")]
fn tag_len(world: &mut CoreWorld) {
    assert_eq!(world.envelope.as_ref().expect("envelope").tag_len(), 16);
}

#[then("the wrap wire version should be 1")]
fn wrap_version(_world: &mut CoreWorld) {
    assert_eq!(WrapWireLayout.version(), 1);
    assert_eq!(WRAP_WIRE_VERSION, 1);
}

#[then("the wrap wire should start with version then a 12-byte nonce")]
fn wrap_layout(_world: &mut CoreWorld) {
    assert_eq!(WrapWireLayout.nonce_offset(), 1);
    assert_eq!(WrapWireLayout.nonce_len(), 12);
}

#[then(expr = "the content HKDF info should be {string}")]
fn content_hkdf(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.hkdf.info(KeyPurpose::Content),
        Some(HKDF_INFO_CONTENT)
    );
    assert_eq!(HKDF_INFO_CONTENT, expected);
}

#[then(expr = "the evidence HKDF info should be {string}")]
fn evidence_hkdf(_world: &mut CoreWorld, expected: String) {
    assert_eq!(HKDF_INFO_EVIDENCE, expected);
}

#[then(expr = "the wrap HKDF info should be {string}")]
fn wrap_hkdf(_world: &mut CoreWorld, expected: String) {
    assert_eq!(HKDF_INFO_WRAP, expected);
}

#[then(expr = "the device HKDF info should be {string}")]
fn device_hkdf(_world: &mut CoreWorld, expected: String) {
    assert_eq!(HKDF_INFO_DEVICE, expected);
}

#[then("HKDF purpose registration should fail")]
fn hkdf_failed(world: &mut CoreWorld) {
    assert!(world.hkdf_failed);
}

#[then("Argon2id memory should be 65536 KiB")]
fn argon_mem(world: &mut CoreWorld) {
    assert_eq!(world.argon.expect("argon").memory_kib(), 65_536);
}

#[then("Argon2id iterations should be 3")]
fn argon_iter(world: &mut CoreWorld) {
    assert_eq!(world.argon.expect("argon").iterations(), 3);
}

#[then("Argon2id parallelism should be 1")]
fn argon_par(world: &mut CoreWorld) {
    assert_eq!(world.argon.expect("argon").parallelism(), 1);
}

#[then("Argon2id output should be 32 bytes")]
fn argon_out(world: &mut CoreWorld) {
    assert_eq!(world.argon.expect("argon").output_len(), 32);
}

#[then("Argon2id salt should be 16 bytes")]
fn argon_salt(world: &mut CoreWorld) {
    assert_eq!(world.argon.expect("argon").salt_len(), 16);
}

#[then(expr = "associated data should bind version, key id {string}, and purpose {string}")]
fn aad_binds(world: &mut CoreWorld, key: String, purpose: String) {
    let key = KeyId::new(key).expect("key");
    assert!(world.aad.as_ref().expect("aad").binds(
        WRAP_WIRE_VERSION,
        &key,
        parse_purpose(&purpose)
    ));
}

#[then("the historical ciphertext should be unchanged")]
fn historical_unchanged(world: &mut CoreWorld) {
    let expected = world.historical_ciphertext.as_ref().expect("hist");
    assert_eq!(
        world
            .envelope
            .as_ref()
            .expect("envelope")
            .historical_ciphertext(),
        expected
    );
}

#[then("a new wrap record should exist")]
fn new_wrap_exists(world: &mut CoreWorld) {
    assert_eq!(world.new_wrap_key.as_deref(), Some("device-2"));
}

#[then("library content, evidence, wrap, and device purposes should be distinct")]
fn purposes_distinct(_world: &mut CoreWorld) {
    let mut labels = vec![
        KeyPurpose::Content.hkdf_info(),
        KeyPurpose::Evidence.hkdf_info(),
        KeyPurpose::Wrap.hkdf_info(),
        KeyPurpose::Device.hkdf_info(),
    ];
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels.len(), 4);
}

#[then(expr = "the import record resource should be {string}")]
fn import_resource(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.c2pa.as_ref().expect("c2pa").resource_id().as_str(),
        expected
    );
}

#[then(expr = "the import digest algorithm should be {string}")]
fn import_digest_alg(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world
            .c2pa
            .as_ref()
            .expect("c2pa")
            .digest_algorithm()
            .as_str(),
        expected
    );
}

#[then(expr = "the import time should be {string}")]
fn import_time(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.c2pa.as_ref().expect("c2pa").imported_at().as_str(),
        expected
    );
}

#[then(expr = "the validation status should be {string}")]
fn validation_status(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.c2pa.as_ref().expect("c2pa").status().as_str(),
        expected
    );
}

#[then("the unsigned record should not be a valid signed manifest")]
fn unsigned_not_signed(world: &mut CoreWorld) {
    assert!(!world
        .c2pa
        .as_ref()
        .expect("c2pa")
        .status()
        .is_valid_signed_manifest());
}

#[then("the record should be a valid signed manifest")]
fn is_signed(world: &mut CoreWorld) {
    assert!(world
        .c2pa
        .as_ref()
        .expect("c2pa")
        .status()
        .is_valid_signed_manifest());
}

#[then("the action should not claim a pixel change")]
fn no_pixel(world: &mut CoreWorld) {
    assert!(!world
        .c2pa_action
        .as_ref()
        .expect("action")
        .claims_pixel_change());
}

#[then("the activity should not be permanent PROV-O history")]
fn not_permanent(world: &mut CoreWorld) {
    assert!(!world.provo.as_ref().expect("provo").is_permanent_history());
}

#[then("the import should not invent an original capture event")]
fn no_invented_capture(world: &mut CoreWorld) {
    assert!(!world.c2pa.as_ref().expect("c2pa").invents_capture());
}

#[then("mutating the signed manifest should fail")]
fn signed_mutate_failed(world: &mut CoreWorld) {
    assert!(world.signed_mutate_failed);
}

#[then(expr = "the sidecar time should be {string}")]
fn sidecar_time(world: &mut CoreWorld, expected: String) {
    assert_eq!(
        world.provo.as_ref().expect("provo").time().as_str(),
        expected
    );
}

#[then("the sidecar profile should be JSON-LD")]
fn sidecar_jsonld(world: &mut CoreWorld) {
    assert!(world.provo.as_ref().expect("provo").is_json_ld());
}

#[then(expr = "the register should contain {string}")]
fn register_contains(world: &mut CoreWorld, id: String) {
    assert!(world.register.as_ref().expect("register").contains(&id));
}

#[then(expr = "the capability status should be {string}")]
fn capability_status(world: &mut CoreWorld, expected: String) {
    let row = world
        .register
        .as_ref()
        .expect("register")
        .row(world.capability_rows.last().expect("row").id())
        .expect("row");
    assert_eq!(row.status().as_str(), expected);
}

#[then("the supported set should be nonempty")]
fn supported_nonempty(world: &mut CoreWorld) {
    assert!(world
        .register
        .as_ref()
        .expect("register")
        .supported_set_nonempty());
}

#[then("preservation-only should not count as editable parity")]
fn preservation_not_parity(_world: &mut CoreWorld) {
    assert!(!CapabilityStatus::PreservationOnly.counts_as_editable_parity());
}

#[then(expr = "the aggregate should expose unavailable child {string}")]
fn expose_child(world: &mut CoreWorld, id: String) {
    assert!(world
        .register
        .as_ref()
        .expect("register")
        .exposes_unavailable_child(&id));
}

#[then("register construction should fail")]
fn register_failed(world: &mut CoreWorld) {
    assert!(world.register_failed);
}

fn write_junit_report(path: &PathBuf, passed: usize, failed: usize, skipped: usize) {
    let tests = passed + failed + skipped;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="filesquid_core.features" tests="{tests}" failures="{failed}" skipped="{skipped}" />
"#
    );
    fs::write(path, xml).expect("write junit");
}

/// Outcome of an operation-message parse (CORE-S-010).
type ParsedMessage = Result<Option<OperationMessage>, OperationMessageError>;

/// Remember `text` and how it parsed on the world.
fn remember_message(world: &mut CoreWorld, text: String) {
    world.message_parse = Some(OperationMessage::parse(&text));
    world.message_text = text;
}

/// The last parse remembered on the world.
fn last_parse(world: &CoreWorld) -> &ParsedMessage {
    world.message_parse.as_ref().expect("a message was parsed")
}

#[when(
    expr = "I format the operation message for operation {string} of kind {string} with label {string}"
)]
fn format_operation_message(world: &mut CoreWorld, op: String, kind: String, label: String) {
    let kind = OperationKind::parse(&kind).expect("kind");
    let message = OperationMessage::new(OperationId::new(op).expect("op"), kind)
        .with_label(label)
        .expect("label");
    remember_message(world, message.format());
}

#[then(expr = "the message should start with {string}")]
fn message_starts_with(world: &mut CoreWorld, prefix: String) {
    assert!(world.message_text.starts_with(&prefix));
    assert_eq!(prefix, OPERATION_MESSAGE_PREFIX);
}

#[then(expr = "parsing it should give operation {string}, kind {string} and label {string}")]
fn parsed_message_is(world: &mut CoreWorld, op: String, kind: String, label: String) {
    let message = last_parse(world)
        .clone()
        .expect("readable")
        .expect("FileSquid message");
    assert_eq!(message.operation_id().as_str(), op);
    assert_eq!(message.kind().as_str(), kind);
    assert_eq!(message.label(), Some(label.as_str()));
}

#[when(expr = "I parse the version message {string}")]
fn parse_version_message(world: &mut CoreWorld, text: String) {
    remember_message(world, text.replace("\\\"", "\""));
}

#[then("it should not be a FileSquid operation message")]
fn not_a_filesquid_message(world: &mut CoreWorld) {
    assert_eq!(last_parse(world), &Ok(None));
}

#[then("the message should be refused as unreadable")]
fn message_refused(world: &mut CoreWorld) {
    assert!(matches!(
        last_parse(world),
        Err(OperationMessageError::Unreadable { .. })
    ));
}

#[when(expr = "I parse the installation ID {string}")]
fn parse_installation_id(world: &mut CoreWorld, text: String) {
    let id = InstallationId::parse(&text).expect("installation id");
    world.message_text = id.urn();
}

#[then(expr = "its agent identifier should be {string}")]
fn installation_urn_is(world: &mut CoreWorld, urn: String) {
    assert_eq!(world.message_text, urn);
}

#[then(expr = "the PROV-O context should be {string} and the profile {string}")]
fn prov_context_is(_world: &mut CoreWorld, context: String, profile: String) {
    assert_eq!(PROV_O_CONTEXT, context);
    assert_eq!(PROV_O_PROFILE, profile);
}

#[tokio::main]
async fn main() {
    let results_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    let _ = fs::remove_dir_all(&results_dir);
    fs::create_dir_all(&results_dir).expect("allure-results");
    let features = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/features");
    let writer = CoreWorld::cucumber()
        .with_writer(writer::Basic::stdout().summarized())
        .run(features)
        .await;
    write_junit_report(
        &results_dir.join("cucumber-junit.xml"),
        writer.passed_steps(),
        writer.failed_steps(),
        writer.skipped_steps(),
    );
    if writer.execution_has_failed() {
        std::process::exit(1);
    }
}
