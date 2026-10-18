//! The server administrator token. The file is host storage I/O, so it lives in the host and
//! not in `filesquid-api` (API-S-006). The token authorises every API call until server
//! accounts, sessions and groups arrive (API-F1.9). Rules: API-S-016.
//!
//! - **`FILESQUID_ADMIN_TOKEN` wins** on every platform.
//! - **Unix only for the token file.** Without `FILESQUID_ADMIN_TOKEN`, Unix servers read or
//!   create `<data dir>/admin-token`. On other platforms the server never reads or writes a
//!   token file: it refuses to start until `FILESQUID_ADMIN_TOKEN` is set.
//! - **Fails closed (Unix).** The file is opened once with `O_NOFOLLOW | O_NONBLOCK`, so
//!   neither a symbolic link nor a FIFO can redirect or hang the open; the opened handle must
//!   be a regular file owned by the user running the server with no group or other permission
//!   bits. Anything else is an error that names the remedy and never contains the token.
//! - **Atomic first start.** The file is created with `create_new`, so two servers starting
//!   together agree on one token. A write or sync that fails removes the partly written file.
//! - **Crash recovery.** A blank, owner-only, regular file can only be left by a first start
//!   that crashed. After a bounded wait of [`MAX_REREADS`] x 20 ms (another process may still
//!   be writing it), it is replaced atomically: the new token goes to a temporary file with a
//!   random name, which is renamed over it. Stale temporary files of earlier crashes
//!   (regular files owned by this user) are removed at startup.

use std::path::Path;

/// File name of the stored administrator token inside the data directory.
pub const TOKEN_FILE: &str = "admin-token";

/// Return the administrator token: `configured` if given and not blank; else, on Unix, the
/// token stored in `<data_dir>/admin-token`, or a new random 256-bit token written there
/// with owner-only permissions (mode 0600). Returns the token and whether it was created by
/// this call. On other platforms an unconfigured token is an error. Performs filesystem I/O
/// on Unix.
pub fn load_or_create_token(
    data_dir: &Path,
    configured: Option<&str>,
) -> Result<(String, bool), String> {
    if let Some(token) = configured.map(str::trim).filter(|t| !t.is_empty()) {
        return Ok((token.to_string(), false));
    }
    stored_or_new_token(data_dir)
}

/// Platforms without the Unix file checks never read or write a token file.
#[cfg(not(unix))]
fn stored_or_new_token(_data_dir: &Path) -> Result<(String, bool), String> {
    Err(
        "set FILESQUID_ADMIN_TOKEN: on this platform filesquid-server does not store an \
         administrator token file"
            .to_string(),
    )
}

#[cfg(unix)]
pub use unix::MAX_REREADS;
#[cfg(unix)]
use unix::*;

/// The Unix token-file implementation.
#[cfg(unix)]
mod unix {
    use super::TOKEN_FILE;
    use filesquid_api::auth::generate_token;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    /// Reads of a blank token file, waiting for another process to finish writing it.
    pub const MAX_REREADS: u32 = 50;

    /// Wait between those rereads (at most 50 x 20 ms = 1 s in total).
    const REREAD_WAIT: Duration = Duration::from_millis(20);

    /// Attempts at a fresh random temporary name before giving up.
    const MAX_TEMPORARY_NAMES: u32 = 5;

    /// Suffix of temporary token files.
    const TEMPORARY_SUFFIX: &str = ".tmp";

    /// What the token file holds.
    pub(super) enum Stored {
        /// No file.
        Absent,
        /// A file with no token: being written, or left by a crashed first start.
        Blank,
        /// A token.
        Token(String),
    }

    /// Unix: remove stale temporaries, then use the stored token, create the first one, or
    /// recover a blank file. Performs filesystem I/O.
    pub(super) fn stored_or_new_token(data_dir: &Path) -> Result<(String, bool), String> {
        if data_dir.is_dir() {
            remove_stale_temporaries(data_dir)?;
        }
        let path = data_dir.join(TOKEN_FILE);
        match read_token_file(&path)? {
            Stored::Token(token) => Ok((token, false)),
            Stored::Absent => create_first(data_dir, &path),
            Stored::Blank => replace_blank(&path),
        }
    }

    /// Remove `admin-token.*.tmp` files left by crashed recoveries: only regular files (never
    /// links) owned by this user. Performs filesystem I/O.
    pub(super) fn remove_stale_temporaries(data_dir: &Path) -> Result<(), String> {
        let entries =
            std::fs::read_dir(data_dir).map_err(|e| format!("list {}: {e}", data_dir.display()))?;
        let prefix = format!("{TOKEN_FILE}.");
        for entry in entries {
            let entry = entry.map_err(|e| format!("list {}: {e}", data_dir.display()))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) && name.ends_with(TEMPORARY_SUFFIX) {
                remove_if_own_regular_file(&entry.path())?;
            }
        }
        Ok(())
    }

    /// Remove `path` when it is a regular file (not a link) owned by this user.
    fn remove_if_own_regular_file(path: &Path) -> Result<(), String> {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|e| format!("inspect {}: {e}", path.display()))?;
        if !metadata.is_file() || metadata.uid() != current_uid() {
            return Ok(());
        }
        std::fs::remove_file(path).map_err(|e| format!("remove {}: {e}", path.display()))
    }

    /// First start: create the token file atomically. If another process created it first,
    /// use its token (or recover its file if it stays blank). Performs filesystem I/O.
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
        let temporary = write_temporary(path, &token)?;
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

    /// Write `token` to a new temporary file with a random name next to `path`, retrying a
    /// bounded number of times if the name exists. Returns the temporary path.
    fn write_temporary(path: &Path, token: &str) -> Result<PathBuf, String> {
        for _ in 0..MAX_TEMPORARY_NAMES {
            let temporary = temporary_path(path)?;
            match create_with(&temporary, |file| file.write_all(token.as_bytes())) {
                Ok(()) => return Ok(temporary),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(format!("write {}: {err}", temporary.display())),
            }
        }
        Err(format!(
            "no free temporary name next to {} after {MAX_TEMPORARY_NAMES} attempts",
            path.display()
        ))
    }

    /// A random temporary file name next to `path`: `admin-token.<16 hex>.tmp`.
    pub(super) fn temporary_path(path: &Path) -> Result<PathBuf, String> {
        let random = generate_token()?;
        let name = format!("{TOKEN_FILE}.{}{TEMPORARY_SUFFIX}", &random[..16]);
        Ok(path.with_file_name(name))
    }

    /// Reread `path` until it holds a token, at most [`MAX_REREADS`] times. `None` when it
    /// stays blank or absent. Performs filesystem I/O and sleeps between reads.
    fn wait_for_token(path: &Path) -> Result<Option<String>, String> {
        for _ in 0..MAX_REREADS {
            if let Stored::Token(token) = read_token_file(path)? {
                return Ok(Some(token));
            }
            std::thread::sleep(REREAD_WAIT);
        }
        Ok(None)
    }

    /// Open the token file once with `O_NOFOLLOW | O_NONBLOCK`, check the opened handle with
    /// [`check_private`], and read the token from that same handle. Performs filesystem I/O.
    pub(super) fn read_token_file(path: &Path) -> Result<Stored, String> {
        let mut file = match open_checked(path) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Stored::Absent),
            Err(err) if err.raw_os_error() == Some(libc::ELOOP) => return Err(symlink_error(path)),
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

    /// Open `path` for reading without following a symbolic link (`ELOOP`) and without
    /// blocking on a FIFO or device; the type check on the handle then rejects those.
    fn open_checked(path: &Path) -> std::io::Result<File> {
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
    }

    /// The error for a token file that is a symbolic link, with its remedy.
    fn symlink_error(path: &Path) -> String {
        format!(
            "{} is a symbolic link; delete it and restart to generate a new token, or replace \
             it with a regular file holding the token (then chmod 600 it)",
            path.display()
        )
    }

    /// The effective user id of this process.
    fn current_uid() -> u32 {
        // SAFETY: geteuid has no preconditions and cannot fail.
        unsafe { libc::geteuid() }
    }

    /// Refuse a token file handle that is not a regular file, not owned by this user, or open
    /// to group or other users. The error names the remedy and never contains the token.
    fn check_private(path: &Path, metadata: &std::fs::Metadata) -> Result<(), String> {
        let shown = path.display();
        if !metadata.is_file() {
            return Err(format!(
                "{shown} is not a regular file; remove it (for example a directory or a FIFO) \
                 and restart to generate a new token"
            ));
        }
        if metadata.uid() != current_uid() {
            return Err(format!(
                "{shown} is owned by another user; chown it to the user that runs the server, \
                 or delete it and restart to generate a new token"
            ));
        }
        if metadata.mode() & 0o077 != 0 {
            return Err(format!(
                "{shown} is readable by other users; run chmod 600 on it and restart"
            ));
        }
        Ok(())
    }

    /// Create `path` (failing if it exists) with mode 0600, run `write` on it and sync it.
    /// When `write` or the sync fails, the partly written file is removed, so a failed first
    /// start leaves no blank file behind. The handle is closed on every path. Performs I/O.
    pub(super) fn create_with(
        path: &Path,
        write: impl FnOnce(&mut File) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
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
}

#[cfg(all(test, not(unix)))]
mod non_unix_tests {
    use super::*;

    #[test]
    fn without_a_configured_token_the_server_refuses_to_start() {
        let dir = std::env::temp_dir().join("fs-token-non-unix");
        let error = load_or_create_token(&dir, None).expect_err("refused");
        assert!(error.contains("FILESQUID_ADMIN_TOKEN"), "{error}");
        assert!(!dir.join(TOKEN_FILE).exists(), "no token file is written");
        let (token, created) = load_or_create_token(&dir, Some("t")).expect("configured");
        assert_eq!((token.as_str(), created), ("t", false));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

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
    fn a_fifo_at_the_token_path_is_refused_without_hanging() {
        let dir = dir("fifo");
        let path = dir.join(TOKEN_FILE);
        let c_path = std::ffi::CString::new(path.to_str().expect("utf8")).expect("c path");
        // SAFETY: c_path is a valid NUL-terminated path for the duration of the call.
        if unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) } != 0 {
            eprintln!("skipped: mkfifo is unavailable here");
            std::fs::remove_dir_all(dir).expect("cleanup");
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let probe = path.clone();
        std::thread::spawn(move || tx.send(read_token_file(&probe).err()).expect("send"));
        let refused = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("opening a FIFO must not hang");
        let error = refused.expect("a FIFO is refused");
        assert!(error.contains("not a regular file"), "{error}");
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn an_orphaned_temporary_file_does_not_block_recovery() {
        let dir = dir("orphan");
        let path = dir.join(TOKEN_FILE);
        std::fs::write(&path, "").expect("blank token file");
        let owner_only = std::os::unix::fs::PermissionsExt::from_mode(0o600);
        std::fs::set_permissions(&path, owner_only).expect("chmod");
        let orphan = dir.join(format!("{TOKEN_FILE}.{}.tmp", std::process::id()));
        std::fs::write(&orphan, "stale").expect("orphan");
        let (token, created) = load_or_create_token(&dir, None).expect("recovered");
        assert!(created);
        assert_eq!(
            std::fs::read_to_string(&path).expect("stored").trim(),
            token
        );
        assert!(
            !orphan.exists(),
            "a stale temporary file of ours is removed"
        );
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn stale_cleanup_never_touches_a_link_or_another_name() {
        let dir = dir("cleanup");
        let target = dir.join("keep");
        std::fs::write(&target, "x").expect("target");
        let link = dir.join(format!("{TOKEN_FILE}.link.tmp"));
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        let other = dir.join("notes.tmp");
        std::fs::write(&other, "x").expect("other");
        remove_stale_temporaries(&dir).expect("cleanup runs");
        assert!(link.symlink_metadata().is_ok(), "a link is never removed");
        assert!(other.exists() && target.exists());
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn temporary_names_are_random_not_the_process_id() {
        let a = temporary_path(Path::new("/data/admin-token")).expect("name");
        let b = temporary_path(Path::new("/data/admin-token")).expect("name");
        assert_ne!(a, b);
        let pid = std::process::id().to_string();
        assert!(!a.to_string_lossy().contains(&format!(".{pid}.")));
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
