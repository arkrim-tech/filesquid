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
//! - [`DataDirChecks`]: the data-directory checks (write probe, free space);
//! - [`HostMonitor`]: opens both in a data directory and runs the host's monitor;
//! - [`HostFiles`]: the file service over local OCFL roots, with each library's leases and
//!   staging area in the data directory ([`install_file_service`], [`open_fs_library`]).
//!
//! # Features
//!
//! - default (always built): the items above.
//! - `desktop`: desktop-only native processing: the per-library SQLite catalog
//!   ([`SqliteCatalog`], Task DE-0.2), and later the OCFL coordinator and the durable job and
//!   media workers that DE-0.8 adds. Only `filesquid-desktop` enables it.
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
pub mod host_checks;
pub mod host_monitor;
pub mod journal_sqlite;
mod owner_only;

#[cfg(feature = "desktop")]
pub use catalog::{
    library_ocfl_root, CommitProbe, NoProbe, PendingIntent, SqliteCatalog, CATALOG_DIR,
    CATALOG_FILE_EXTENSION, CATALOG_SCHEMA_VERSION, MAX_LIBRARIES, OCFL_ROOT_SUFFIX,
    RECONCILE_OPERATION, WORK_DIR_SUFFIX,
};
pub use files::{install_file_service, library_dir_name, open_fs_library, HostFiles, HostLeases};
pub use host_checks::{probe_name, DataDirChecks, MAX_PROBE_ATTEMPTS};
pub use host_monitor::{HostMonitor, ALREADY_INSTALLED, HOST_HEARTBEAT_TIMEOUT_MS, JOURNAL_FILE};
pub use journal_sqlite::SqliteJournal;
