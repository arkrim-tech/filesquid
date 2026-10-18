//! Journals a panic on any host thread (Task DE-0.9; DE-S-003; logging-enforce.md: every
//! error is journaled).
//!
//! Without a hook a panic is printed to stderr only. [`install_panic_hook`] adds a process-wide
//! hook that journals [`PANIC_OPERATION`] (error code `panic`, never the panic message, which
//! may hold content) through the host's tracing layer, then flushes the journal so the entry
//! survives a panic that ends the process, and then runs the previous hook.

use filesquid_api::journal_writer::JournalSender;
use std::sync::{Arc, Once, OnceLock, RwLock};

/// Journal operation of a host thread panic.
pub const PANIC_OPERATION: &str = "desktop.panic";

/// Name of the journal writer's own thread (`filesquid_api::journal_writer`). A panic there
/// is journaled but not flushed, because the flush would wait on that thread.
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

/// Journal one panic without its message, then flush the journal unless this is the journal
/// writer's thread. Writes to the journal.
fn journal_panic() {
    tracing::error!(
        operation = PANIC_OPERATION,
        outcome = "failed",
        error_code = "panic",
        "a FileSquid host thread panicked"
    );
    if std::thread::current().name() == Some(JOURNAL_THREAD) {
        return;
    }
    let sender = FLUSH
        .read()
        .ok()
        .and_then(|slot| slot.as_ref().and_then(|journal| journal.get().cloned()));
    if let Some(sender) = sender {
        sender.flush();
    }
}
