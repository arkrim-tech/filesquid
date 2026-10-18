//! Journals a panic on any host thread (Task DE-0.9; DE-S-003; logging-enforce.md: every
//! error is journaled).
//!
//! Without a hook a panic is printed to stderr only. [`install_panic_hook`] adds a process-wide
//! hook that journals [`PANIC_OPERATION`] (error code `panic`, never the panic message, which
//! may hold content) through the host's tracing layer, then flushes the journal so the entry
//! survives a panic that ends the process, and then runs the previous hook.
//!
//! Two panics cannot reach the journal: one before the journal opens, and one on the journal
//! writer's own thread. Their record is written straight to stdout as a JSON line marked
//! `journal_unavailable` ([`unjournaled_line`]), whatever `RUST_LOG` says. In every case the
//! previous hook (Rust's default) still prints the panic message to stderr.

use filesquid_api::journal_writer::JournalSender;
use std::io::Write;
use std::sync::{Arc, Once, OnceLock, RwLock};

/// Journal operation of a host thread panic.
pub const PANIC_OPERATION: &str = "desktop.panic";

/// The stdout record of a host failure that cannot reach the journal: a journal entry's
/// fields (actor `system`, outcome `failed`, level `error`) for `operation` and `error_code`,
/// marked `journal_unavailable`. Pure.
pub fn unjournaled_line(operation: &str, error_code: &str) -> String {
    serde_json::json!({
        "operation": operation,
        "actor": "system",
        "outcome": "failed",
        "level": "error",
        "error_code": error_code,
        "journal_unavailable": true,
    })
    .to_string()
}

/// Write `line` to `stdout`; a failure is reported on stderr, since stdout itself failed.
pub fn write_line(stdout: &mut dyn Write, line: &str) {
    if let Err(err) = writeln!(stdout, "{line}") {
        eprintln!("unjournaled record not written: {err}");
    }
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
/// journal. When no journal is open yet, or on the journal writer's own thread (whose queue
/// may be full, with that thread its only consumer), the only record is one
/// [`unjournaled_line`] written to `stdout`, and nothing is queued or traced. The panic hook
/// calls it with the process's stdout. Writes to the journal or `stdout`.
pub fn record_panic(stdout: &mut dyn Write) {
    let on_writer = std::thread::current().name() == Some(JOURNAL_THREAD);
    let journal = if on_writer { None } else { open_journal() };
    let Some(sender) = journal else {
        write_line(stdout, &unjournaled_line(PANIC_OPERATION, "panic"));
        return;
    };
    tracing::error!(
        operation = PANIC_OPERATION,
        outcome = "failed",
        error_code = "panic",
        "a FileSquid host thread panicked"
    );
    sender.flush();
}

/// The journal the hook flushes, once it is open.
fn open_journal() -> Option<JournalSender> {
    FLUSH
        .read()
        .ok()
        .and_then(|slot| slot.as_ref().and_then(|journal| journal.get().cloned()))
}
