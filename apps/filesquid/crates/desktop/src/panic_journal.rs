//! Journals a panic on any host thread (Task DE-0.9; DE-S-003; logging-enforce.md: every
//! error is journaled).
//!
//! Without a hook a panic is printed to stderr only. [`install_panic_hook`] adds a process-wide
//! hook that journals [`PANIC_OPERATION`] (error code `panic`, never the panic message, which
//! may hold content) through the host's tracing layer, then flushes the journal so the entry
//! survives a panic that ends the process, and then runs the previous hook.
//!
//! Two panics cannot reach the journal: one before the journal opens (its record reaches
//! stdout only), and one on the journal writer's own thread, whose record is printed to stdout
//! as a JSON line marked `journal_unavailable` ([`writer_panic_line`]). In every case the
//! previous hook (Rust's default) still prints the panic message to stderr.

use filesquid_api::journal_writer::JournalSender;
use std::io::Write;
use std::sync::{Arc, Once, OnceLock, RwLock};

/// Journal operation of a host thread panic.
pub const PANIC_OPERATION: &str = "desktop.panic";

/// The stdout record of a panic on the journal writer's own thread: the panic entry's fields,
/// marked `journal_unavailable`. Pure.
pub fn writer_panic_line() -> String {
    serde_json::json!({
        "operation": PANIC_OPERATION,
        "actor": "system",
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
            record_panic(&mut std::io::stdout());
            previous(info);
        }));
    });
}

/// Record one panic of the current thread without its message: journal it and flush the
/// journal; on the journal writer's own thread nothing is queued (the queue may be full and
/// that thread is its only consumer), so [`writer_panic_line`] is written to `stdout` instead.
/// The panic hook calls it with the process's stdout. Writes to the journal or `stdout`.
pub fn record_panic(stdout: &mut dyn Write) {
    if std::thread::current().name() == Some(JOURNAL_THREAD) {
        if let Err(err) = writeln!(stdout, "{}", writer_panic_line()) {
            eprintln!("panic record not written: {err}");
        }
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
