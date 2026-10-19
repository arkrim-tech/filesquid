//! Portable FileSquid domain contracts.
//!
//! This crate is the shared **portable-contracts** layer from
//! [ADR 001](../../../resources/architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md).
//! It MUST NOT depend on Axum, Tauri, libvips, FFmpeg, or OCFL disk I/O.
//! Hosted mailbox code may depend on this crate. Private-client and native-processing
//! crates stay out of the Axum graph.
//!
//! Internal nouns are repository, library, collection, virtual collection,
//! virtual exhibit, and file. The item type is **file**.

#![deny(missing_docs)]

mod capability;
mod collection;
mod command;
mod crypto;
mod error;
mod file;
mod health;
mod history;
mod ids;
mod item;
mod job;
mod library;
mod provenance;

pub use capability::{CapabilityInventory, CapabilityRegister, CapabilityRow, CapabilityStatus};
pub use collection::{
    retain_files_after_collection_removal, Collection, CollectionMembership, CollectionSort,
    Folder, VirtualCollection, VirtualExhibit, VirtualExhibitMembership, VirtualMembership,
};
pub use command::{
    CommandEnvelope, CommandOutcome, CommandOutcomeKind, ExpectedRevision, OperationScope,
    PageSize, PortError, QueryEnvelope, UiProgress, MAX_PAGE_SIZE, PROTOCOL_SCHEMA_VERSION,
};
pub use crypto::{
    Argon2idParams, ContentEnvelope, Ed25519PublicKey, HkdfInfoSet, KeyPurpose, RewrapPlan,
    WrapAssociatedData, WrapWireLayout, X25519PublicKey, AES_256_GCM, ARGON2ID_ITERATIONS,
    ARGON2ID_MEMORY_KIB, ARGON2ID_OUTPUT_LEN, ARGON2ID_PARALLELISM, ARGON2ID_SALT_LEN,
    ED25519_KEY_LEN, HKDF_INFO_CONTENT, HKDF_INFO_DEVICE, HKDF_INFO_EVIDENCE, HKDF_INFO_WRAP,
    NONCE_LEN_BYTES, TAG_LEN_BYTES, WRAP_WIRE_VERSION, X25519_KEY_LEN,
};
pub use error::{DomainError, InvalidOpaqueId};
pub use file::{
    Derivative, File, FileResource, FileStatus, FileVersion, IntegrityState, RecipeOperation,
    ResourceRole, Sha256Digest,
};
pub use health::{health, HealthStatus, HEALTH_SCHEMA_VERSION};
pub use history::{
    HistoryEntry, InstallationId, InvalidHistoryRef, InvalidInstallationId, JournalLink,
    OperationKind, OperationMessage, OperationMessageError, VersionRef,
    MAX_OPERATION_MESSAGE_BYTES, OPERATION_MESSAGE_PREFIX,
};
pub use ids::{
    CollectionId, FileId, FileResourceId, FileVersionId, FolderId, KeyId, LibraryId, OpaqueId,
    OperationId, RepositoryId, VirtualCollectionId, VirtualExhibitId,
};
pub use item::ItemType;
pub use job::{
    admit_active_transfers, reserve_gib, CanonicalReceipt, JobCheckpoint, JobLease,
    DEFAULT_ACTIVE_TRANSFERS, DEFAULT_CPU_HEAVY_WORKERS, DEFAULT_STAGING_CAP_GIB,
    MAX_ATTEMPTS_PER_RESOURCE, MIGRATION_PROCESS_MEMORY_GIB, RESERVE_MIN_GIB, RESERVE_PERCENT,
    RESTART_RESUMABLE_WITHIN_SECS, RETRY_BACKOFF_CAP_SECS,
};
pub use library::{Library, Repository};
pub use provenance::{
    C2paAction, C2paImportRecord, C2paValidationStatus, DigestAlgorithm, ProvOSidecar, Rfc3339Utc,
    PROV_O_CONTEXT, PROV_O_PROFILE, PROV_O_PROFILE_VERSION,
};
