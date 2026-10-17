//! The `fsquid` binary: the one FileSquid command-line tool.
//!
//! Exit codes: the module's own code on success, 2 for usage errors.

use std::env;
use std::process;

/// Run `fsquid` with the process arguments and exit with its code.
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match filesquid_cli::run(&args) {
        Ok(code) => process::exit(code),
        Err(err) => {
            eprintln!("{err}");
            process::exit(2);
        }
    }
}
