//! The server administrator token file. This is host storage I/O, so it lives in the host
//! and not in `filesquid-api` (API-S-006). The token authorises every API call until server
//! accounts, sessions and groups arrive (API-F1.9). Rules: API-S-016.
//!
//! - **Fails closed.** On Unix the file is opened once without following a symbolic link, and
//!   the opened handle must be a regular file owned by the user running the server with no
//!   group or other permission bits. Anything else is an error that names the remedy, never a
//!   silent repair, and never contains the token.
//! - **Atomic first start.** The file is created with `create_new`, so two servers starting
//!   together on one data directory agree on one token. A write or sync that fails removes the
//!   partly written file.
//! - **Crash recovery.** A blank, owner-only, regular file can only be left by a first start
//!   that crashed. After a bounded wait (another process may still be writing it), it is
//!   replaced atomically: the new token goes to a temporary file that is renamed over it.

use filesquid_api::auth::generate_token;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// File name of the stored administrator token inside the data directory.
pub const TOKEN_FILE: &str = "admin-token";

/// Reads of a blank token file, waiting for another process to finish writing it.
const MAX_REREADS: u32 = 50;

/// Wait between those rereads (at most 50 x 20 ms = 1 s in total).
const REREAD_WAIT: Duration = Duration::from_millis(20);

/// What the token file holds.
enum Stored {
    /// No file.
    Absent,
    /// A file with no token: being written, or left by a crashed first start.
    Blank,
    /// A token.
    Token(String),
}

/// Return the administrator token: `configured` if given and not blank, else the token stored
/// in `<data_dir>/admin-token`, else a new random 256-bit token written there with
/// owner-only permissions (Unix mode 0600). Returns the token and whether it was created
/// (issued) by this call. Performs filesystem I/O.
pub fn load_or_create_token(
    data_dir: &Path,
    configured: Option<&str>,
) -> Result<(String, bool), String> {
    if let Some(token) = configured.map(str::trim).filter(|t| !t.is_empty()) {
        return Ok((token.to_string(), false));
    }
    let path = data_dir.join(TOKEN_FILE);
    match read_token_file(&path)? {
        Stored::Token(token) => Ok((token, false)),
        Stored::Absent => create_first(data_dir, &path),
        Stored::Blank => replace_blank(&path),
    }
}

/// First start: create the token file atomically. If another process created it first, use
/// its token (or recover its file if it stays blank). Performs filesystem I/O.
fn create_first(data_dir: &Path, path: &Path) -> Result<(String, bool), String> {
    std::fs::create_dir_all(data_dir)
        .map_err(|e| format!("create data dir {}: {e}", data_dir.display()))?;
    let token = generate_token()?;
    match create_with(path, |file| file.write_all(token.as_bytes())) {
        Ok(()) => Ok((token, true)),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => replace_blank(path),
        Err(err) => Err(format!("write {}: {err}", path.display())),
    }
}

/// The file exists but holds no token yet. Wait a bounded time for its writer; if it stays
/// blank it was left by a crashed first start, so replace it atomically with a new token.
/// Performs filesystem I/O.
fn replace_blank(path: &Path) -> Result<(String, bool), String> {
    if let Some(token) = wait_for_token(path)? {
        return Ok((token, false));
    }
    let token = generate_token()?;
    let temporary = temporary_path(path);
    create_with(&temporary, |file| file.write_all(token.as_bytes()))
        .map_err(|e| format!("write {}: {e}", temporary.display()))?;
    if let Err(err) = std::fs::rename(&temporary, path) {
        let cleanup = std::fs::remove_file(&temporary);
        return Err(format!(
            "replace {}: {err} (temporary file removed: {})",
            path.display(),
            cleanup.is_ok()
        ));
    }
    // Two recovering processes may race; whichever rename landed last is the stored token.
    match read_token_file(path)? {
        Stored::Token(stored) => Ok((stored.clone(), stored == token)),
        _ => Err(format!("{} is blank after replacing it", path.display())),
    }
}

/// Reread `path` until it holds a token, at most [`MAX_REREADS`] times. `None` when it stays
/// blank or absent. Performs filesystem I/O and sleeps between reads.
fn wait_for_token(path: &Path) -> Result<Option<String>, String> {
    for _ in 0..MAX_REREADS {
        if let Stored::Token(token) = read_token_file(path)? {
            return Ok(Some(token));
        }
        std::thread::sleep(REREAD_WAIT);
    }
    Ok(None)
}

/// A temporary file name next to `path`, unique to this process.
fn temporary_path(path: &Path) -> PathBuf {
    path.with_file_name(format!("{TOKEN_FILE}.{}.tmp", std::process::id()))
}

/// Open the token file once, without following a symbolic link, check the opened handle with
/// [`check_private`], and read the token from that same handle. Performs filesystem I/O.
fn read_token_file(path: &Path) -> Result<Stored, String> {
    let mut file = match open_no_follow(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Stored::Absent),
        Err(err) if is_symlink_refusal(&err) => return Err(symlink_error(path)),
        Err(err) => return Err(format!("open {}: {err}", path.display())),
    };
    let metadata = file
        .metadata()
        .map_err(|e| format!("inspect {}: {e}", path.display()))?;
    check_private(path, &metadata)?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let token = text.trim();
    Ok(if token.is_empty() {
        Stored::Blank
    } else {
        Stored::Token(token.to_string())
    })
}

/// Open `path` for reading; on Unix with `O_NOFOLLOW`, so a symbolic link fails to open.
fn open_no_follow(path: &Path) -> std::io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(not(unix))]
    {
        if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(std::io::Error::other("symbolic link"));
        }
    }
    options.open(path)
}

/// True when opening failed because the path is a symbolic link (`ELOOP` under `O_NOFOLLOW`).
fn is_symlink_refusal(err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        err.raw_os_error() == Some(libc::ELOOP)
    }
    #[cfg(not(unix))]
    {
        err.kind() == std::io::ErrorKind::Other
    }
}

/// The error for a token file that is a symbolic link, with its remedy.
fn symlink_error(path: &Path) -> String {
    format!(
        "{} is a symbolic link; delete it and restart to generate a new token, or replace it \
         with a regular file holding the token (then chmod 600 it)",
        path.display()
    )
}

/// Refuse a token file handle that is not a regular file, not owned by this user, or open to
/// group or other users (Unix). The error names the remedy and never contains the token.
#[cfg(unix)]
fn check_private(path: &Path, metadata: &std::fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let shown = path.display();
    if !metadata.is_file() {
        return Err(format!(
            "{shown} is not a regular file; delete it and restart"
        ));
    }
    // SAFETY: geteuid has no preconditions and cannot fail.
    let me = unsafe { libc::geteuid() };
    if metadata.uid() != me {
        return Err(format!(
            "{shown} is owned by another user; chown it to the user that runs the server, or \
             delete it and restart to generate a new token"
        ));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(format!(
            "{shown} is readable by other users; run chmod 600 on it and restart"
        ));
    }
    Ok(())
}

/// Windows and other platforms: access is governed by the data directory's ACLs.
#[cfg(not(unix))]
fn check_private(path: &Path, metadata: &std::fs::Metadata) -> Result<(), String> {
    if !metadata.is_file() {
        return Err(format!(
            "{} is not a regular file; delete it and restart",
            path.display()
        ));
    }
    Ok(())
}

/// Create `path` (failing if it exists), owner-only on Unix, run `write` on it and sync it.
/// When `write` or the sync fails, the partly written file is removed, so a failed first
/// start leaves no blank file behind. The handle is closed on every path. Performs I/O.
fn create_with(
    path: &Path,
    write: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let written = write(&mut file).and_then(|()| file.sync_all());
    drop(file);
    if let Err(err) = written {
        return match std::fs::remove_file(path) {
            Ok(()) => Err(err),
            Err(cleanup) => Err(std::io::Error::other(format!(
                "{err}; removing the partly written file also failed: {cleanup}"
            ))),
        };
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// A fresh, empty directory for one test.
    fn dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("fs-token-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn a_failed_write_removes_the_partly_written_file() {
        let dir = dir("failed-write");
        let path = dir.join(TOKEN_FILE);
        let failing = |_: &mut std::fs::File| Err(std::io::Error::other("disk full"));
        assert!(create_with(&path, failing).is_err());
        assert!(!path.exists(), "a failed first start leaves no blank file");
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn the_token_file_is_opened_once_without_following_a_link() {
        let dir = dir("no-follow");
        let target = dir.join("elsewhere");
        std::fs::write(&target, "known-token").expect("target");
        let link = dir.join(TOKEN_FILE);
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        let error = match read_token_file(&link) {
            Err(error) => error,
            Ok(_) => panic!("a symbolic link was followed"),
        };
        assert!(error.contains("symbolic link"), "{error}");
        assert!(!error.contains("known-token"), "{error}");
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}
