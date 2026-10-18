//! The server administrator token file. This is host storage I/O, so it lives in the host
//! and not in `filesquid-api` (API-S-006). The token authorises every API call until server
//! accounts, sessions and groups arrive (API-F1.9).
//!
//! The file fails closed: on Unix a stored token is used only when the file is a regular file
//! (not a symbolic link), owned by the user running the server, and readable by that user
//! only (no group or other permission bits). Anything else is an error that names the fix
//! (`chmod 600`), never a silent repair. A first start creates the file atomically, so two
//! servers starting together on one data directory agree on one token.

use filesquid_api::auth::generate_token;
use std::path::Path;
use std::time::Duration;

/// File name of the stored administrator token inside the data directory.
pub const TOKEN_FILE: &str = "admin-token";

/// Reads of a token file another process has just created, before it is declared empty.
const MAX_REREADS: u32 = 50;

/// Wait between those rereads (at most 50 x 20 ms = 1 s in total).
const REREAD_WAIT: Duration = Duration::from_millis(20);

/// Return the administrator token: `configured` if given and not blank, else the token stored
/// in `<data_dir>/admin-token`, else a new random 256-bit token written there with
/// owner-only permissions (Unix mode 0600). Returns the token and whether it was created.
/// Performs filesystem I/O.
pub fn load_or_create_token(
    data_dir: &Path,
    configured: Option<&str>,
) -> Result<(String, bool), String> {
    if let Some(token) = configured.map(str::trim).filter(|t| !t.is_empty()) {
        return Ok((token.to_string(), false));
    }
    let path = data_dir.join(TOKEN_FILE);
    if let Some(stored) = read_stored_token(&path)? {
        return Ok((stored, false));
    }
    std::fs::create_dir_all(data_dir)
        .map_err(|e| format!("create data dir {}: {e}", data_dir.display()))?;
    let token = generate_token()?;
    match create_private(&path, &token) {
        Ok(()) => Ok((token, true)),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => reread_winner(&path),
        Err(err) => Err(format!("write {}: {err}", path.display())),
    }
}

/// Another process created the file first: read its token, waiting a bounded time for it to
/// finish writing. Errors when the file stays empty.
fn reread_winner(path: &Path) -> Result<(String, bool), String> {
    for _ in 0..MAX_REREADS {
        if let Some(stored) = read_stored_token(path)? {
            return Ok((stored, false));
        }
        std::thread::sleep(REREAD_WAIT);
    }
    Err(format!(
        "{} exists but stays empty; remove it and restart",
        path.display()
    ))
}

/// Read a non-blank token from `path`; `None` when the file is absent or blank. A file that
/// fails [`check_private`] or cannot be read is an error, so a token file is never silently
/// replaced or used while others can read it.
fn read_stored_token(path: &Path) -> Result<Option<String>, String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("inspect {}: {err}", path.display())),
    };
    check_private(path, &metadata)?;
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(Some(text.trim().to_string()).filter(|t| !t.is_empty()))
}

/// Refuse a token file that is a symbolic link, not owned by this user, or open to group or
/// other users (Unix). The error names the fix and never contains the token.
#[cfg(unix)]
fn check_private(path: &Path, metadata: &std::fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let shown = path.display();
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "{shown} is a symbolic link; replace it with a regular file (chmod 600)"
        ));
    }
    if !metadata.is_file() {
        return Err(format!("{shown} is not a regular file"));
    }
    // SAFETY: geteuid has no preconditions and cannot fail.
    let me = unsafe { libc::geteuid() };
    if metadata.uid() != me {
        return Err(format!(
            "{shown} is owned by another user; chown it to the server user and chmod 600 it"
        ));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(format!(
            "{shown} is readable by other users; run chmod 600 on it and restart"
        ));
    }
    Ok(())
}

/// Windows and other platforms: only refuse a symbolic link; access is governed by the
/// data directory's ACLs.
#[cfg(not(unix))]
fn check_private(path: &Path, metadata: &std::fs::Metadata) -> Result<(), String> {
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "{} is a symbolic link; replace it with a regular file",
            path.display()
        ));
    }
    Ok(())
}

/// Create `path` (failing if it exists) and write `text`, readable and writable by the owner
/// only on Unix. The file handle is closed when this function returns, on every path.
fn create_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}
