//! Journals a panic on any host thread (Task DE-0.9; DE-S-003; logging-enforce.md: every
//! error is journaled).
//!
//! Without a hook a panic is printed to stderr only. [`install_panic_hook`] adds a process-wide
//! hook that journals [`PANIC_OPERATION`] (error code `panic`, never the panic message, which
//! may hold content) through the host's tracing layer, then flushes the journal so the entry
//! survives a panic that ends the process, and then runs the previous hook.
//!
//! Two panics cannot reach the journal: one before the journal opens (the hook is installed
//! first, so it reaches stdout only), and one on the journal writer's own thread, which is
//! printed to stdout as a JSON line marked `journal_unavailable` ([`writer_panic_line`]).

use filesquid_api::journal_writer::JournalSender;
use std::sync::{Arc, Once, OnceLock, RwLock};

/// Journal operation of a host thread panic.
pub const PANIC_OPERATION: &str = "desktop.panic";

/// The stdout record of a panic on the journal writer's own thread: the panic entry's fields,
/// marked `journal_unavailable`. Pure.
pub fn writer_panic_line() -> String {
    serde_json::json!({
        "operation": PANIC_OPERATION,
        "outcome": "failed",
        "level": "error",
        "error_code": "panic",
        "journal_unavailable": true,
    })
    .to_string()
}

/// Name of the journal writer's own thread (`filesquid_api::journal_writer`).
const JOURNAL_THREAD: &str = "filesquid-journal";

/// The journal the hook flushes; replaced by each [`install_panic_hook`] call.
static FLUSH: RwLock<Option<Arc<OnceLock<JournalSender>>>> = RwLock::new(None);

/// Guards the one-time hook installation.
static INSTALL: Once = Once::new();

/// Install the panic hook once per process, and make it flush `journal` (set once the
/// journal opens). Later calls only replace the journal to flush.
pub fn install_panic_hook(journal: Arc<OnceLock<JournalSender>>) {
    match FLUSH.write() {
        Ok(mut slot) => *slot = Some(journal),
        Err(err) => {
            tracing::error!(%err, operation = PANIC_OPERATION, "panic journal slot poisoned")
        }
    }
    INSTALL.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            journal_panic();
            previous(info);
        }));
    });
}

/// Journal one panic without its message and flush the journal. On the journal writer's own
/// thread nothing is queued (the queue may be full and that thread is its only consumer), so
/// the record goes to stdout only. Writes to the journal or stdout.
fn journal_panic() {
    if std::thread::current().name() == Some(JOURNAL_THREAD) {
        println!("{}", writer_panic_line());
        return;
    }
    tracing::error!(
        operation = PANIC_OPERATION,
        outcome = "failed",
        error_code = "panic",
        "a FileSquid host thread panicked"
    );
    let sender = FLUSH
        .read()
        .ok()
        .and_then(|slot| slot.as_ref().and_then(|journal| journal.get().cloned()));
    if let Some(sender) = sender {
        sender.flush();
    }
}
