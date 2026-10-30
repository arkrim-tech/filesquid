//! The metadata property routes' reads and writes (Task API-F1.4; MD-S-002, MD-S-012,
//! MD-S-013, MD-S-014, API-S-027): a file's populated `exif:`, `dcterms:`, `schema:` and
//! registered extension statements, the add order of a bare term name, the library's
//! namespace registry, and the read-only projection of the file's import record.
//!
//! Statements live in the file's metadata record. Every edit goes through the `ocfl` store of
//! `filesquid-metadata` (`OcflStore`), which commits the whole record in exactly one new OCFL
//! version with the API user and an operation message (MD-S-014) and refuses a save that
//! touches an import entry or the original name. A `dcterms:` term passes the ISO 15836-2
//! `/terms/` register (`dc:` maps onto it), `schema:` is accepted as the Schema.org overflow,
//! and any other prefix must be registered (MD-S-012). The registry is the library's
//! `namespaces.md` in its collection-snapshot object.

use super::error::FileError;
use super::provenance::{Caller, Committed, Prepared, Write};
use super::records::file_id_of;
use super::service::{resolve, FileService, Library};
use super::HOLDER;
use crate::canonical::layout::{
    file_object_id, file_object_root, snapshot_object_id, SNAPSHOT_ROOT,
};
use crate::canonical::original::companion_path;
use crate::canonical::read_file_original;
use crate::canonical::OwnerReadable;
use filesquid_core::{FileId, LibraryId, OperationKind};
use filesquid_metadata::file_record::{FileFacts, FileRecord};
use filesquid_metadata::file_ref::FileRef;
use filesquid_metadata::namespace::{
    valid_term, NamespaceKind, NamespaceRegistry, Registration, NAMESPACES_MD,
};
use filesquid_metadata::record::parse_curie_with;
use filesquid_metadata::store::ocfl::OcflStore;
use filesquid_metadata::store::StoreError;
use filesquid_metadata::{MetadataError, MetadataNamespace};
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::StorageBackend;
use filesquid_ocfl::write::{Clock, CommitReceipt, VersionReservations};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::Instrument;

/// Most bytes of one statement value.
pub const MAX_VALUE_BYTES: usize = 64 * 1024;

/// Most candidates one term lookup answers.
const MAX_CANDIDATES: usize = 64;

/// Query naming one file, at a version or its head.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRecordQuery {
    /// The library.
    pub library_id: String,
    /// The file (UUID v4).
    pub file_id: String,
    /// The version; the head when absent.
    pub version: Option<String>,
}

/// Where a statement's value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatementSource {
    /// Extracted from the file (`exif:`).
    Extracted,
    /// Asserted by the owner (`dcterms:`, `schema:` and extension namespaces).
    Asserted,
}

/// One populated statement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StatementView {
    /// `prefix:term`.
    pub curie: String,
    /// The namespace prefix (`dcterms`, `exif`, `schema` or a registered prefix).
    pub prefix: String,
    /// The local name.
    pub term: String,
    /// The literal value.
    pub value: String,
    /// Extracted or owner-asserted.
    pub source: StatementSource,
}

/// The statements of one file at one version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StatementsView {
    /// The library.
    pub library_id: String,
    /// The file.
    pub file_id: String,
    /// The version read.
    pub version: String,
    /// Every populated statement, in record order.
    pub statements: Vec<StatementView>,
}

/// Body of `PUT /api/v1/files/metadata/statements`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetStatementRequest {
    /// The library.
    pub library_id: String,
    /// The file.
    pub file_id: String,
    /// `prefix:term`; `dc:` maps onto `dcterms:`.
    pub curie: String,
    /// The value, 1 to [`MAX_VALUE_BYTES`] bytes.
    pub value: String,
}

/// Body of `POST /api/v1/files/metadata/removals`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveStatementRequest {
    /// The library.
    pub library_id: String,
    /// The file.
    pub file_id: String,
    /// `prefix:term` of the statement to remove.
    pub curie: String,
}

/// Body of `POST /api/v1/files/renames`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenameRequest {
    /// The library.
    pub library_id: String,
    /// The file.
    pub file_id: String,
    /// The new display name, 1 to 1,024 bytes without `/` or NUL.
    pub display_name: String,
}

/// A version a metadata edit published.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MetadataCommitView {
    /// The library.
    pub library_id: String,
    /// The OCFL object it changed: the file's, or the library's snapshot for the registry.
    pub object_id: String,
    /// The new head version.
    pub version: String,
    /// When it was created (RFC 3339 UTC).
    pub created: String,
}

/// Query of `GET /api/v1/files/metadata/terms`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TermQuery {
    /// The library.
    pub library_id: String,
    /// The bare term name to place (`title`, `creator`).
    pub name: String,
}

/// Which kind of namespace a candidate comes from, in the add order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    /// An ISO 15836-2 `/terms/` property, offered first.
    Dcterms,
    /// The Schema.org suggestion, offered next.
    Schema,
    /// A registered extension namespace.
    Extension,
    /// An owner-declared custom namespace.
    Custom,
}

/// One place a term name can go.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TermCandidate {
    /// `prefix:name`.
    pub curie: String,
    /// The namespace kind.
    pub kind: CandidateKind,
}

/// The add order of one term name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TermsView {
    /// The name looked up.
    pub name: String,
    /// The candidates, `dcterms:` first, then Schema.org, then registered namespaces.
    pub candidates: Vec<TermCandidate>,
}

/// The library's namespace registry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NamespacesView {
    /// The library.
    pub library_id: String,
    /// Every registration, in registration order.
    pub namespaces: Vec<Registration>,
}

/// Body of `POST /api/v1/files/metadata/namespaces`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterNamespaceRequest {
    /// The library.
    pub library_id: String,
    /// The prefix statements will carry.
    pub prefix: String,
    /// The namespace IRI.
    pub iri: String,
    /// An extension namespace (optional term list) or a custom one (free keys).
    pub kind: NamespaceKind,
    /// The allowed terms of an extension namespace; any valid name when absent.
    pub terms: Option<Vec<String>>,
}

/// The read-only projection of a file's import record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FactsView {
    /// The library.
    pub library_id: String,
    /// The file.
    pub file_id: String,
    /// The version read.
    pub version: String,
    /// The projection, without absolute paths, import roots, host or account names.
    pub facts: FileFacts,
}

/// The view of one statement. Pure.
fn statement_view(statement: &filesquid_metadata::record::Statement) -> StatementView {
    let source = match statement.namespace {
        MetadataNamespace::Exif => StatementSource::Extracted,
        _ => StatementSource::Asserted,
    };
    StatementView {
        curie: statement.curie(),
        prefix: statement.prefix().to_string(),
        term: statement.term.clone(),
        value: statement.value.clone(),
        source,
    }
}

/// The add order of `name`: `dcterms:` when the register knows it, then the `schema:`
/// suggestion, then each registered namespace that accepts it. Pure.
pub fn term_candidates(name: &str, registry: &NamespaceRegistry) -> Vec<TermCandidate> {
    let mut candidates = Vec::new();
    let known = |curie: &str| parse_curie_with(curie, registry).is_ok();
    if known(&format!("dcterms:{name}")) {
        candidates.push(candidate("dcterms", name, CandidateKind::Dcterms));
    }
    if known(&format!("schema:{name}")) {
        candidates.push(candidate("schema", name, CandidateKind::Schema));
    }
    for registration in registry.registrations() {
        if candidates.len() >= MAX_CANDIDATES {
            break;
        }
        if registry.check(registration.prefix(), name).is_ok() {
            let kind = match registration.kind() {
                NamespaceKind::Extension => CandidateKind::Extension,
                NamespaceKind::Custom => CandidateKind::Custom,
            };
            candidates.push(candidate(registration.prefix(), name, kind));
        }
    }
    candidates
}

/// One candidate. Pure.
fn candidate(prefix: &str, name: &str, kind: CandidateKind) -> TermCandidate {
    TermCandidate {
        curie: format!("{prefix}:{name}"),
        kind,
    }
}

/// The file identifier a request names.
fn parse_file_id(id: &str) -> Result<FileId, FileError> {
    match crate::library::is_uuid_v4(id) {
        true => FileId::new(id).map_err(|_| FileError::invalid("A file_id is a UUID v4.")),
        false => Err(FileError::invalid("A file_id is a UUID v4.")),
    }
}

/// The library identifier a request names.
fn parse_library_id(id: &str) -> Result<LibraryId, FileError> {
    LibraryId::new(id).map_err(|_| FileError::not_found())
}

/// The file error of a metadata-module refusal.
pub(super) fn metadata_error(err: MetadataError) -> FileError {
    FileError::invalid(format!("The statement was refused: {err}."))
}

/// The file error of a store refusal or failure.
fn store_error(err: StoreError) -> FileError {
    match err {
        StoreError::Conflict { .. } => {
            FileError::conflict("Another change reached the record first; read it again.")
        }
        StoreError::Record(err) => FileError::invalid(format!("The record was refused: {err}.")),
        StoreError::Registry(err) => {
            FileError::invalid(format!("The registration was refused: {err}."))
        }
        _ => FileError::unavailable("the metadata store failed"),
    }
}

/// The library's clock as the `ocfl` store takes it.
struct StoreClock(super::FileClock);

impl Clock for StoreClock {
    fn now_millis(&self) -> Option<u64> {
        u64::try_from((self.0)()).ok()
    }
}

impl<B: StorageBackend, R, S> FileService<B, R, S> {
    /// The statements of one file at a version or its head.
    pub async fn statements(&self, query: FileRecordQuery) -> Result<StatementsView, FileError> {
        let (version, record) = self.record_at(&query).await?;
        Ok(StatementsView {
            library_id: query.library_id,
            file_id: query.file_id,
            version,
            statements: record
                .statements()
                .statements
                .iter()
                .map(statement_view)
                .collect(),
        })
    }

    /// The read-only projection of a file's import record.
    pub async fn facts(&self, query: FileRecordQuery) -> Result<FactsView, FileError> {
        let (version, record) = self.record_at(&query).await?;
        Ok(FactsView {
            library_id: query.library_id,
            file_id: query.file_id,
            version,
            facts: record.facts(),
        })
    }

    /// The version and record of the file a query names.
    async fn record_at(&self, query: &FileRecordQuery) -> Result<(String, FileRecord), FileError> {
        let file_id = parse_file_id(&query.file_id)?;
        let library = self.library(&query.library_id)?;
        let root =
            crate::canonical::file_object_root(&file_id).map_err(|_| FileError::not_found())?;
        let object = Object::open(library.files.backend.as_ref(), &root)
            .await
            .map_err(|_| FileError::not_found())?;
        let version = resolve(&object, query.version.as_deref())?;
        let described = library
            .described(&object, &version)
            .await?
            .ok_or_else(FileError::not_found)?;
        Ok((version.to_string(), (*described.record).clone()))
    }
}

impl<B, R, S> FileService<B, R, S>
where
    B: StorageBackend,
    R: VersionReservations,
    S: StorageBackend,
{
    /// The add order of a bare term name in a library.
    pub async fn terms(&self, query: TermQuery) -> Result<TermsView, FileError> {
        let valid = !query.name.is_empty() && !query.name.contains(':') && query.name.len() <= 128;
        if !valid {
            return Err(FileError::invalid(
                "A term name is 1 to 128 bytes without `:`.",
            ));
        }
        let registry = self.registry(&query.library_id).await?;
        Ok(TermsView {
            candidates: term_candidates(&query.name, &registry),
            name: query.name,
        })
    }

    /// The library's namespace registry.
    pub async fn namespaces(&self, library_id: &str) -> Result<NamespacesView, FileError> {
        let registry = self.registry(library_id).await?;
        Ok(NamespacesView {
            library_id: library_id.to_string(),
            namespaces: registry.registrations().to_vec(),
        })
    }

    /// The registry of `library_id`, empty when none was saved.
    pub(super) async fn registry(&self, library_id: &str) -> Result<NamespaceRegistry, FileError> {
        let library = self.library(library_id)?;
        let reference = registry_ref(library_id)?;
        let store = store_of(&library, library_id, self.clock())?;
        store.read_registry(&reference).await.map_err(store_error)
    }
    /// Set one statement of a file by `caller`, in one new version of its object.
    pub async fn set_statement(
        &self,
        request: SetStatementRequest,
        caller: impl Into<Caller>,
    ) -> Result<Committed<MetadataCommitView>, FileError> {
        let caller = caller.into();
        if request.value.is_empty() || request.value.len() > MAX_VALUE_BYTES {
            return Err(FileError::invalid(format!(
                "A statement value is 1 to {MAX_VALUE_BYTES} bytes."
            )));
        }
        let registry = self.registry(&request.library_id).await?;
        let (namespace, term) =
            parse_curie_with(&request.curie, &registry).map_err(metadata_error)?;
        if !valid_term(term) {
            return Err(FileError::invalid(
                "A term name is letters, digits, `_`, `-` and `.`, starting with a letter or `_`.",
            ));
        }
        let edit = |record: &mut FileRecord| {
            record
                .statements_mut()
                .set_registered(&registry, namespace.clone(), term, request.value.clone())
                .map_err(metadata_error)
        };
        let ids = (&request.library_id, &request.file_id);
        let write = (
            super::METADATA_SET_OPERATION,
            OperationKind::MetadataEdit,
            None,
        );
        self.edit_record(ids, &caller, write, edit).await
    }

    /// Rename a file by `caller`: set its display name in one new version of its metadata
    /// record. The stored name, the logical path and the original name stay (API-S-027).
    ///
    /// `expected` is the file object's head the caller read; any other is a `conflict`.
    pub async fn rename(
        &self,
        request: RenameRequest,
        caller: impl Into<Caller>,
        expected: &str,
    ) -> Result<Committed<MetadataCommitView>, FileError> {
        let caller = caller.into();
        let name = request.display_name.clone();
        let edit = |record: &mut FileRecord| {
            record
                .set_display_name(&name)
                .map_err(|e| FileError::invalid(format!("The name was refused: {e}.")))
        };
        let ids = (&request.library_id, &request.file_id);
        let write = (
            super::RENAME_OPERATION,
            OperationKind::Rename,
            Some(expected),
        );
        self.edit_record(ids, &caller, write, edit).await
    }

    /// Remove one statement of a file by `caller`, in one new version of its object.
    pub async fn remove_statement(
        &self,
        request: RemoveStatementRequest,
        caller: impl Into<Caller>,
    ) -> Result<Committed<MetadataCommitView>, FileError> {
        let caller = caller.into();
        let curie = normalised_curie(&request.curie);
        let edit = |record: &mut FileRecord| {
            let statements = &mut record.statements_mut().statements;
            let before = statements.len();
            statements.retain(|s| s.curie() != curie);
            match statements.len() < before {
                true => Ok(()),
                false => Err(FileError::not_found()),
            }
        };
        let ids = (&request.library_id, &request.file_id);
        let write = (
            super::METADATA_REMOVE_OPERATION,
            OperationKind::MetadataEdit,
            None,
        );
        self.edit_record(ids, &caller, write, edit).await
    }

    /// Register a namespace in a library by `caller` (the owner), in one new version of the
    /// library's snapshot object.
    pub async fn register_namespace(
        &self,
        request: RegisterNamespaceRequest,
        caller: impl Into<Caller>,
    ) -> Result<Committed<MetadataCommitView>, FileError> {
        let caller = caller.into();
        let registration = match request.kind {
            NamespaceKind::Extension => Registration::extension(
                &request.prefix,
                &request.iri,
                request.terms.clone(),
                filesquid_metadata::namespace::Registrant::Owner,
            ),
            NamespaceKind::Custom => Registration::custom(&request.prefix, &request.iri),
        };
        if request.kind == NamespaceKind::Custom && request.terms.is_some() {
            return Err(FileError::invalid("A custom namespace has no term list."));
        }
        let library = self.library(&request.library_id)?;
        let _writing = library.writes.lock().await;
        let mut registry = self.registry(&request.library_id).await?;
        let added = registry
            .register(registration)
            .map_err(|e| FileError::invalid(format!("The registration was refused: {e}.")))?;
        if !added {
            return Err(FileError::conflict("That namespace is already registered."));
        }
        let reference = registry_ref(&request.library_id)?;
        let store = store_of(&library, &request.library_id, self.clock())?;
        let prepared = self.prepare(&metadata_write(
            &caller,
            (super::NAMESPACE_OPERATION, OperationKind::MetadataEdit),
            &request.library_id,
            reference.object_id(),
        ))?;
        let receipt = store
            .commit_registry_with(&reference, &registry, &prepared.options())
            .instrument(prepared.span())
            .await
            .map_err(store_error)?;
        committed(&request.library_id, receipt, prepared)
    }

    /// Apply `edit` to the head record of a file and commit it by `caller` as `operation` of
    /// `kind`; with an `expected` revision (a command's), only when the file object's head is
    /// it. The commit is linked to the journal ([`super::provenance`]).
    async fn edit_record(
        &self,
        (library_id, file_id): (&String, &String),
        caller: &Caller,
        (operation, kind, expected): (&'static str, OperationKind, Option<&str>),
        edit: impl FnOnce(&mut FileRecord) -> Result<(), FileError>,
    ) -> Result<Committed<MetadataCommitView>, FileError> {
        let id = parse_file_id(file_id)?;
        let library = self.library(library_id)?;
        let _writing = library.writes.lock().await;
        let backend = library.files.backend.as_ref();
        let original = read_file_original(backend, &OwnerReadable, &id)
            .await
            .map_err(|_| FileError::not_found())?
            .ok_or_else(FileError::not_found)?;
        if let Some(expected) = expected {
            let root = file_object_root(&id).map_err(|_| FileError::not_found())?;
            super::service::check_head(backend, &root, expected).await?;
        }
        let mut record = original.record.clone();
        edit(&mut record)?;
        if record == original.record {
            return Err(FileError::conflict(
                "Nothing would change, so no version was made.",
            ));
        }
        let record_path = companion_path(&id).map_err(|_| FileError::not_found())?;
        let reference = FileRef::new(&file_object_id(&id), &original.path)
            .and_then(|r| r.with_record_path(&record_path))
            .map_err(|_| FileError::internal("a file reference was refused"))?;
        let store = store_of(&library, library_id, self.clock())?;
        let object_id = file_object_id(&id);
        let write = metadata_write(caller, (operation, kind), library_id, &object_id);
        let prepared = self.prepare(&write)?;
        let receipt = store
            .commit_file_record_with(&reference, &record, &prepared.options())
            .instrument(prepared.span())
            .await
            .map_err(store_error)?;
        committed(library_id, receipt, prepared)
    }
}

/// The write of a metadata record of `object_id` in `library_id` by `caller` as the route's
/// `operation` of `kind`: journaled as `metadata.edit` too, with no label or moves. Pure.
fn metadata_write<'a>(
    caller: &'a Caller,
    (operation, kind): (&'static str, OperationKind),
    library_id: &'a str,
    object_id: &'a str,
) -> Write<'a> {
    Write {
        caller,
        kind,
        label: None,
        operation,
        library_id,
        object_id,
        metadata: true,
        moves: BTreeMap::new(),
    }
}

/// The answer to a published edit with its commit's journal entries, or `conflict` when the
/// store published nothing.
fn committed(
    library_id: &str,
    receipt: Option<CommitReceipt>,
    prepared: Prepared,
) -> Result<Committed<MetadataCommitView>, FileError> {
    let receipt = receipt
        .ok_or_else(|| FileError::conflict("Nothing would change, so no version was made."))?;
    let view = MetadataCommitView {
        library_id: library_id.to_string(),
        object_id: receipt.object_id.clone(),
        version: receipt.version.to_string(),
        created: receipt.created.clone(),
    };
    Ok(prepared.finish(view, &receipt))
}

/// `dc:` written as `dcterms:`, so a removal names the statement as it is stored. Pure.
fn normalised_curie(curie: &str) -> String {
    match curie.strip_prefix("dc:") {
        Some(term) => format!("dcterms:{term}"),
        None => curie.to_string(),
    }
}

/// The reference of the registry record of `library_id`: `namespaces.md` in the library's
/// collection-snapshot object.
fn registry_ref(library_id: &str) -> Result<FileRef, FileError> {
    let id = parse_library_id(library_id)?;
    FileRef::new(&snapshot_object_id(&id), NAMESPACES_MD)
        .and_then(|reference| reference.with_record_path(NAMESPACES_MD))
        .map_err(|_| FileError::internal("the registry reference was refused"))
}

/// Maps one of a library's object identifiers to its object root.
type RootOf = Box<dyn Fn(&str) -> String + Send + Sync>;

/// The `ocfl` metadata store of one library.
type LibraryStore<'a, B, R, S> = OcflStore<'a, B, R, S, StoreClock, RootOf>;

/// The `ocfl` metadata store over the storage of `library` (`library_id`), staging under its
/// own prefix.
fn store_of<'a, B, R, S>(
    library: &'a Library<B, R, S>,
    library_id: &str,
    clock: super::FileClock,
) -> Result<LibraryStore<'a, B, R, S>, FileError> {
    let snapshot = snapshot_object_id(&parse_library_id(library_id)?);
    let files = &library.files;
    Ok(OcflStore {
        storage: files.backend.as_ref(),
        reservations: files.reservations.as_ref(),
        staging: files.staging.as_ref(),
        staging_prefix: format!("{}/metadata", files.staging_prefix.trim_end_matches('/')),
        holder: HOLDER.to_string(),
        clock: StoreClock(clock),
        object_root: Box::new(move |object_id: &str| object_root(&snapshot, object_id)),
    })
}

/// The object root of one of a library's object identifiers: the collection snapshot for
/// `snapshot`, else the file object `files/<file-id>`. Library and file identifiers are both
/// UUIDs, so the snapshot is matched first. Pure.
fn object_root(snapshot: &str, object_id: &str) -> String {
    if object_id == snapshot {
        return SNAPSHOT_ROOT.to_string();
    }
    match file_id_of(object_id) {
        Some(file_id) => format!("files/{}", file_id.as_str()),
        None => SNAPSHOT_ROOT.to_string(),
    }
}
