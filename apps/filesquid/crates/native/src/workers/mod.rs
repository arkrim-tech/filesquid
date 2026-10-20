//! Desktop job and media workers (Task DE-0.8; DE-S-015): the work a desktop host runs in
//! the background, here in `filesquid-native` behind the `desktop` feature so the Tauri
//! package stays a thin host and no other crate links it.
//!
//! - [`history_backfill`]: the durable job store of a launch, the launch-time job
//!   reconciliation and the history backfill as a durable job (DE-S-014). Media workers
//!   (DE-0.5) join this module.

pub mod history_backfill;
