//! The server administrator token file. This is host storage I/O, so it lives in the host
//! and not in `filesquid-api` (API-S-006). The token authorises every API call until server
//! accounts, sessions and groups arrive (API-F1.9).

use filesquid_api::auth::generate_token;
use std::path::Path;

/// File name of the stored administrator token inside the data directory.
pub const TOKEN_FILE: &str = "admin-token";

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
    write_private(&path, &token)?;
    Ok((token, true))
}

/// Read a non-blank token from `path`; `None` when the file is absent or blank. Any other
/// read error is returned, so an unreadable token file is never silently replaced.
fn read_stored_token(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text.trim().to_string()).filter(|t| !t.is_empty())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("read {}: {err}", path.display())),
    }
}

/// Write `text` to `path`, readable and writable by the owner only on Unix. The file handle
/// is closed when this function returns, on every path.
fn write_private(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))
}
