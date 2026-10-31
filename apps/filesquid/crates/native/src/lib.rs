//! FileSquid native processing (`filesquid-native`).
//!
//! The host-side I/O that `filesquid-api` must not do itself
//! ([API-S-006](../../api/resources/srs.md), [ADR 001](../../../../../resources/architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md),
//! [ADR 025](../../../../../resources/architecture/025-client-side-filesquid-engines.md)).
//! The API owns the meaning (the [`filesquid_api::journal::JournalStore`] and
//! [`filesquid_api::monitor::HostChecks`] traits, the monitor, the health rules and the admin
//! routes); this crate implements the native backends that the hosts inject:
//!
//! - [`SqliteJournal`]: the activity journal in the host's SQLite operational store
//!   (WAL mode, append-only, owner-only file mode on Unix);
//! - [`SqliteJobs`]: the durable jobs of `filesquid-api` (API-S-033) in the same
//!   operational store (Task DE-0.4);
//! - [`SqliteFindings`]: the OCFL validation findings, cursors and last runs in the same
//!   operational store (API-S-022);
//! - [`DataDirChecks`]: the data-directory checks (write probe, free space);
//! - [`HostMonitor`]: opens both in a data directory and runs the host's monitor;
//! - [`HostFiles`]: the file service over local OCFL roots, with each library's leases and
//!   staging area in the data directory ([`install_file_service`], [`open_fs_library`]).
//!
//! # Features
//!
//! - default (always built): the items above.
//! - `desktop`: desktop-only native processing: the per-library SQLite catalog
//!   ([`SqliteCatalog`], Task DE-0.2), its libraries' storage roots for the PROV-O history
//!   routes ([`CatalogRoots`]) and the history backfill over them
//!   ([`SqliteCatalog::backfill_history`], Task FS-2.8 host wiring), and the job workers
//!   ([`workers`]: the durable history backfill job, Tasks DE-0.4 and DE-0.8), which the
//!   media workers of DE-0.5 join; and the library routes' host side (Task DE-1.1): the folders
//!   the owner chose ([`HostLocations`], served by [`open_fs_library_root`]) and the
//!   persistent repair ledger ([`SqliteRepairLedger`]), and each user's UI preferences
//!   ([`SqlitePreferences`]); and the journal objects the sealing service seals into, with the
//!   refill of a lost operational store ([`journal_objects`], Task DE-0.11). Only
//!   `filesquid-desktop` enables it.
//!
//! `filesquid-desktop` links this crate with `desktop`. `filesquid-server` MAY link it with its
//! default features only (journal store, data-directory checks, `HostMonitor`), keeping its
//! journal in SQLite until its PostgreSQL store exists; the server's `unit_native_features`
//! test fails if it enables `desktop`
//! ([ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md),
//! amendment of 2026-10-18). WebAssembly builds and the ciphertext-only hosted service MUST
//! NOT link it.

#[cfg(feature = "desktop")]
pub mod catalog;
pub mod files;
pub mod findings_sqlite;
pub mod host_checks;
pub mod host_monitor;
#[cfg(feature = "desktop")]
pub mod import;
pub mod installation;
pub mod jobs_sqlite;
#[cfg(feature = "desktop")]
pub mod journal_objects;
pub mod journal_sqlite;
#[cfg(feature = "desktop")]
pub mod locations;
mod owner_only;
#[cfg(feature = "desktop")]
pub mod preferences_sqlite;
#[cfg(feature = "desktop")]
pub mod registry_sqlite;
#[cfg(feature = "desktop")]
pub mod repair_ledger;
#[cfg(feature = "desktop")]
pub mod workers;

#[cfg(feature = "desktop")]
pub use catalog::{
    intent_entry, AttachReport, CatalogRoots, CommitProbe, HostRoots, LibraryBackfill,
    LibraryFolder, NoProbe, PendingIntent, RegisterFile, Registration, SqliteCatalog, CATALOG_DIR,
    CATALOG_FILE_EXTENSION, CATALOG_SCHEMA_VERSION, INTENT_RECORDED_OPERATION, MAX_BACKFILL_RUNS,
    MAX_LIBRARIES, RECONCILE_OPERATION, WORK_DIR_SUFFIX,
};
#[cfg(feature = "desktop")]
pub use files::open_fs_library_root;
pub use files::{install_file_service, library_dir_name, open_fs_library, HostFiles, HostLeases};
pub use findings_sqlite::SqliteFindings;
pub use host_checks::{probe_name, DataDirChecks, MAX_PROBE_ATTEMPTS};
pub use host_monitor::{
    HostMonitor, ALREADY_INSTALLED, HOST_HEARTBEAT_TIMEOUT_MS, JOURNAL_FILE,
    VALIDATION_ALREADY_STARTED, VALIDATION_STOP_WAIT,
};
pub use jobs_sqlite::SqliteJobs;
pub use journal_sqlite::SqliteJournal;
#[cfg(feature = "desktop")]
pub use locations::{HostLocations, HostRoot, OpenedBackfill, OpenedRoots};
#[cfg(feature = "desktop")]
pub use preferences_sqlite::{SqlitePreferences, MAX_PREFERENCE_USERS};
#[cfg(feature = "desktop")]
pub use repair_ledger::SqliteRepairLedger;
