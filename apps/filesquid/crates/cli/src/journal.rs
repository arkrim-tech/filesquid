//! Journaled `fsquid` commands (Task CLI-0.2; API-S-031, ADR 036 Decision 8).
//!
//! Before any command runs, the binary opens `<data dir>/filesquid.sqlite` through
//! `filesquid-native`'s SQLite journal store in WAL mode, so it writes beside a running
//! desktop host ([`data_dir`], [`open_journal`]). A journal that cannot be opened refuses the
//! command before it reads or writes anything else ([`refuse_unavailable`]).
//!
//! [`run_journaled`] then routes the command as [`crate::run`] does and writes one redacted
//! entry for each command that changes state or fails: operation `cli.<module>.<command>`,
//! its outcome and error code, and opaque IDs only (the OCFL object identifier and version a
//! commit made; never a path, file name, statement value or credential). A command line that
//! names no module command is one failed `cli.route` entry. A command that succeeds without
//! changing anything writes none (the no-op rule). Help reads and writes nothing, so it needs
//! no journal.
//!
//! Whether a command changed state is read from the OCFL objects it names: their committed
//! heads before and after it runs ([`object_heads`]). A module command that writes a file
//! outside its objects (listed in [`Module::file_writers`]) changes state whenever it
//! succeeds.

use crate::{resolve, usage, Module, Route};
use filesquid_api::journal::{JournalEntry, JournalError, JournalStore, Outcome};
use filesquid_native::{SqliteJournal, JOURNAL_FILE};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// The environment variable that names the data directory.
pub const DATA_DIR_VAR: &str = "FILESQUID_DATA_DIR";

/// The identifier of `filesquid-desktop`: its data directory's name under the platform's.
pub const DESKTOP_IDENTIFIER: &str = "tech.filesquid.desktop";

/// The host name the CLI's journal entries carry.
pub const JOURNAL_HOST: &str = "fsquid";

/// Exit code of a command line that names no module command.
pub const EXIT_USAGE: i32 = 2;

/// Exit code of a command refused because the journal cannot be opened.
pub const EXIT_JOURNAL_UNAVAILABLE: i32 = 3;

/// Exit code of a command that succeeded but whose journal entry could not be written.
pub const EXIT_JOURNAL_WRITE_FAILED: i32 = 4;

/// Operation of a command line that names no module command.
pub const ROUTE_OPERATION: &str = "cli.route";

/// The actor of every CLI entry: the person at the command line, who has no principal.
const ACTOR: &str = "cli";

/// Largest inventory read to learn an object's head (an OCFL inventory larger than this is
/// not read, and the object counts as unchanged).
const MAX_INVENTORY_BYTES: u64 = 64 << 20;

/// The data directory: `FILESQUID_DATA_DIR` when `var` (its value) is set and not empty,
/// otherwise the desktop app's data directory for the platform, or `None` when the platform
/// names none. Reads the platform's directory settings.
pub fn data_dir(var: Option<OsString>) -> Option<PathBuf> {
    match var.filter(|value| !value.is_empty()) {
        Some(value) => Some(PathBuf::from(value)),
        None => dirs::data_dir().map(|dir| dir.join(DESKTOP_IDENTIFIER)),
    }
}

/// Open (creating if needed) the journal `<dir>/filesquid.sqlite` in WAL mode. Filesystem
/// and SQLite I/O.
///
/// # Errors
/// [`JournalError`] when the directory cannot be created or the database opened.
pub fn open_journal(dir: &Path) -> Result<SqliteJournal, JournalError> {
    std::fs::create_dir_all(dir)
        .map_err(|err| JournalError(format!("{}: {err}", dir.display())))?;
    SqliteJournal::open(&dir.join(JOURNAL_FILE), JOURNAL_HOST)
}

/// Whether `args` run a command that needs the journal: every command line but help.
/// Pure.
pub fn needs_journal(args: &[String]) -> bool {
    match resolve(args) {
        Route::Help => false,
        Route::Run { args, .. } => !is_module_help(args),
        Route::NeedsModule { .. } | Route::Unknown(_) => true,
    }
}

/// Refuse the command `args` because its journal cannot be opened: print its redacted
/// refusal to `out` as one JSON line marked `"journal_unavailable": true`, explain on stderr
/// how to name a usable data directory, and answer the exit code. Writes to `out` and stderr.
pub fn refuse_unavailable(args: &[String], reason: &str, out: &mut dyn Write) -> i32 {
    eprintln!(
        "fsquid: the journal cannot be opened ({reason}); set {DATA_DIR_VAR} to a writable \
         data directory. Nothing was run."
    );
    let entry = failed(operation_of(args), "journal_unavailable");
    write_marked(out, &entry, "journal_unavailable");
    EXIT_JOURNAL_UNAVAILABLE
}

/// Run `args` and journal it in `journal`; answer the process exit code. Writes the entry
/// to `journal` (or, when that fails, to `out`), and runs the module's I/O.
pub fn run_journaled(args: &[String], journal: &dyn JournalStore, out: &mut dyn Write) -> i32 {
    match resolve(args) {
        Route::Help => {
            print!("{}", usage());
            0
        }
        Route::Run { module, args } => run_module(module, args, journal, out),
        Route::NeedsModule { command, modules } => {
            eprintln!("{}", crate::needs_module_message(&command, &modules));
            route_failure("module_required", journal, out)
        }
        Route::Unknown(command) => {
            eprintln!("unknown command: {command}\n\n{}", usage());
            route_failure("unknown_command", journal, out)
        }
    }
}

/// Run one module command, read which of its objects it changed, and journal it.
fn run_module(
    module: &'static Module,
    args: &[String],
    journal: &dyn JournalStore,
    out: &mut dyn Write,
) -> i32 {
    if is_module_help(args) {
        return report(&(module.run)(args));
    }
    let command = args[0].as_str();
    if !module.commands.contains(&command) {
        eprintln!("unknown command: {command}\n{}", module.usage);
        return route_failure("unknown_command", journal, out);
    }
    let objects = (module.objects)(args);
    let before = object_heads(&objects);
    let result = (module.run)(args);
    let changed = changed_heads(&before, &object_heads(&objects));
    let code = report(&result);
    let writes_file = module.file_writers.contains(&command);
    let failure = failure_code(&result);
    let Some(entry) = command_entry(module.name, command, failure, &changed, writes_file) else {
        return code;
    };
    record(journal, entry, out, code)
}

/// The error code of a module command's result: `command_error` for an error the module
/// reports, `command_failed` for a non-zero exit code, `None` for success. Pure.
fn failure_code(result: &Result<i32, String>) -> Option<&'static str> {
    match result {
        Ok(0) => None,
        Ok(_) => Some("command_failed"),
        Err(_) => Some("command_error"),
    }
}

/// Print a module's error message and answer the command's exit code: its own code, or
/// [`EXIT_USAGE`] for an error it reports. Writes to stderr.
fn report(result: &Result<i32, String>) -> i32 {
    match result {
        Ok(code) => *code,
        Err(message) => {
            eprintln!("{message}");
            EXIT_USAGE
        }
    }
}

/// Journal a command line that names no module command, with `error_code`.
fn route_failure(error_code: &str, journal: &dyn JournalStore, out: &mut dyn Write) -> i32 {
    record(
        journal,
        failed(ROUTE_OPERATION.into(), error_code),
        out,
        EXIT_USAGE,
    )
}

/// Write `entry` to `journal` and answer `code`; when the write fails, print the redacted
/// entry to `out` marked `"journal_write_failed": true` and answer a non-zero code. Writes
/// to the journal, `out` and stderr.
fn record(journal: &dyn JournalStore, entry: JournalEntry, out: &mut dyn Write, code: i32) -> i32 {
    let Err(err) = journal.append(std::slice::from_ref(&entry)) else {
        return code;
    };
    eprintln!("fsquid: the journal entry could not be written: {err}");
    write_marked(out, &entry, "journal_write_failed");
    match code {
        0 => EXIT_JOURNAL_WRITE_FAILED,
        other => other,
    }
}

/// The entry of a module command that failed with `failure` (or succeeded, `None`) after
/// changing the objects in `changed`, or `None` for a success that changed nothing. Pure.
fn command_entry(
    module: &str,
    command: &str,
    failure: Option<&str>,
    changed: &[ObjectHead],
    writes_file: bool,
) -> Option<JournalEntry> {
    let operation = format!("cli.{module}.{command}");
    let mut entry = match failure {
        Some(code) => failed(operation, code),
        None if changed.is_empty() && !writes_file => return None,
        None => JournalEntry::new(operation, Outcome::Succeeded).actor(ACTOR),
    };
    if let Some(first) = changed.first() {
        entry = entry.object_ref(format!("{}@{}", first.id, first.head));
    }
    for other in changed.iter().skip(1) {
        entry = entry.target(other.id.clone());
    }
    Some(entry)
}

/// A failed entry of `operation` with `error_code`. Pure.
fn failed(operation: String, error_code: &str) -> JournalEntry {
    JournalEntry::new(operation, Outcome::Failed)
        .actor(ACTOR)
        .error_code(error_code)
}

/// The operation `args` would journal: `cli.<module>.<command>`, or [`ROUTE_OPERATION`] for
/// a command line that names no module command. Pure.
fn operation_of(args: &[String]) -> String {
    let Route::Run { module, args } = resolve(args) else {
        return ROUTE_OPERATION.into();
    };
    match args.first() {
        Some(command) if module.commands.contains(&command.as_str()) => {
            format!("cli.{}.{command}", module.name)
        }
        _ => ROUTE_OPERATION.into(),
    }
}

/// Whether module arguments `args` ask for the module's help. Pure.
fn is_module_help(args: &[String]) -> bool {
    args.first()
        .is_none_or(|first| matches!(first.as_str(), "help" | "-h" | "--help"))
}

/// Print `entry`, redacted and with the CLI's host name, to `out` as one JSON line with
/// `marker` set to `true`; a failure to print is reported on stderr, since `out` itself
/// failed. Writes to `out`.
fn write_marked(out: &mut dyn Write, entry: &JournalEntry, marker: &str) {
    let mut stored = entry.clone().redacted();
    if stored.host.is_empty() {
        stored.host = JOURNAL_HOST.to_string();
    }
    let mut value = match serde_json::to_value(stored) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("fsquid: journal entry not printable: {err}");
            return;
        }
    };
    value[marker] = serde_json::Value::Bool(true);
    if let Err(err) = writeln!(out, "{value}").and_then(|()| out.flush()) {
        eprintln!("fsquid: journal record not written to stdout: {err}");
    }
}

/// The committed head of one OCFL object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectHead {
    /// The object directory.
    pub dir: PathBuf,
    /// The object identifier.
    pub id: String,
    /// The head version (`v1`, `v2`, ...).
    pub head: String,
}

/// The committed head of each OCFL object directory in `dirs` that holds one, read from its
/// root inventory. Filesystem I/O.
pub fn object_heads(dirs: &[PathBuf]) -> Vec<ObjectHead> {
    dirs.iter().filter_map(|dir| object_head(dir)).collect()
}

/// The committed head of the object in `dir`, or `None` when it holds no readable one.
/// Filesystem I/O.
fn object_head(dir: &Path) -> Option<ObjectHead> {
    let file = std::fs::File::open(dir.join("inventory.json")).ok()?;
    let mut text = String::new();
    file.take(MAX_INVENTORY_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    let inventory: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(ObjectHead {
        dir: dir.to_path_buf(),
        id: inventory["id"].as_str()?.to_string(),
        head: inventory["head"].as_str()?.to_string(),
    })
}

/// The heads in `after` that are not in `before`: the objects a command committed. Pure.
pub fn changed_heads(before: &[ObjectHead], after: &[ObjectHead]) -> Vec<ObjectHead> {
    after
        .iter()
        .filter(|head| !before.contains(head))
        .cloned()
        .collect()
}
