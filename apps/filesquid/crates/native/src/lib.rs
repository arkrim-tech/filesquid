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
//! - [`HostMonitor`]: opens both in a data directory and runs the host's monitor.
//!
//! `filesquid-desktop` links this crate. `filesquid-server` MAY link it for its SQLite
//! journal store until its PostgreSQL store exists
//! ([ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md),
//! amendment of 2026-10-18). WebAssembly builds and the ciphertext-only hosted service MUST
//! NOT link it.

pub mod host_checks;
pub mod host_monitor;
pub mod journal_sqlite;

pub use host_checks::DataDirChecks;
pub use host_monitor::{HostMonitor, HOST_HEARTBEAT_TIMEOUT_MS, JOURNAL_FILE};
pub use journal_sqlite::SqliteJournal;
