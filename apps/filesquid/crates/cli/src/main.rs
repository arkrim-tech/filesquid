//! The `fsquid` binary: the one FileSquid command-line tool.
//!
//! Before any command runs it opens the journal `<data dir>/filesquid.sqlite`
//! (`FILESQUID_DATA_DIR`, else the desktop app's data directory); a journal that
//! cannot be opened refuses the command. Help needs no journal.
//!
//! Exit codes: the module's own code, 2 for usage errors, 3 when the journal
//! cannot be opened, 4 when a command succeeded but its journal entry could
//! not be written.

use filesquid_cli::journal::{
    data_dir, needs_journal, open_journal, refuse_unavailable, run_journaled, DATA_DIR_VAR,
};
use std::env;
use std::process;

/// Run `fsquid` with the process arguments, journaled, and exit with its code.
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut stdout = std::io::stdout();
    if !needs_journal(&args) {
        process::exit(filesquid_cli::run(&args).unwrap_or_else(|err| {
            eprintln!("{err}");
            2
        }));
    }
    let Some(dir) = data_dir(env::var_os(DATA_DIR_VAR)) else {
        let reason = "the platform names no data directory";
        process::exit(refuse_unavailable(&args, reason, &mut stdout));
    };
    match open_journal(&dir) {
        Ok(journal) => process::exit(run_journaled(&args, &journal, &mut stdout)),
        Err(err) => process::exit(refuse_unavailable(&args, &err.to_string(), &mut stdout)),
    }
}
