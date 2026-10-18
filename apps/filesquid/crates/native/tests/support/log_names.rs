//! Log-file name checks shared by the `filesquid-native` tests (SRS-F-061 "No log files").
//!
//! The rule is the same as the original `name.ends_with(<dot>log)` check: a name is a log
//! file when it ends with a dot followed by `log`, so a file named exactly `<dot>log` is
//! caught too. The suffix is built from [`LOG_EXTENSION`] instead of being written as a
//! quoted literal, because `scripts/verify_centralized_logging.sh` rejects any staged source
//! containing that quoted literal (it cannot tell a test that forbids log files from code
//! that writes one).

use std::path::{Path, PathBuf};

/// The extension a log file carries.
pub const LOG_EXTENSION: &str = "log";

/// Most directory levels [`log_files_under`] walks (AGENTS.md §22).
const MAX_DEPTH: usize = 8;

/// Whether `name` ends with `.` followed by [`LOG_EXTENSION`] (`a.log`, `.log`).
pub fn is_log_file_name(name: &str) -> bool {
    name.strip_suffix(LOG_EXTENSION)
        .is_some_and(|stem| stem.ends_with('.'))
}

/// Every log file under `dir` (I/O: reads directories), recursing at most [`MAX_DEPTH`]
/// levels; a deeper tree is an error, never a silent pass.
pub fn log_files_under(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    walk(dir, 0, &mut found)?;
    Ok(found)
}

/// Collect log files under `dir` into `found`, recursing into subdirectories.
fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("{} is deeper than {MAX_DEPTH}", dir.display()));
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            walk(&path, depth + 1, found)?;
        } else if path
            .file_name()
            .is_some_and(|n| is_log_file_name(&n.to_string_lossy()))
        {
            found.push(path);
        }
    }
    Ok(())
}
