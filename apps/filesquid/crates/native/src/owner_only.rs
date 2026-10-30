//! Owner-only file creation shared by the journal store and the library catalogs, and the
//! path the SQLite stores open.

use std::io;
use std::path::{Path, PathBuf};

/// `path` with the folders above it resolved (`std::fs::canonicalize` of its parent) and its
/// last component kept as given, for an `SQLITE_OPEN_NOFOLLOW` open. SQLite refuses a symbolic
/// link anywhere in the path it is given, not only at the file, so a store under a linked
/// folder (macOS's `/var` is one for `/private/var`, a home folder may be one) would never
/// open. A link at the file itself is still refused, by [`create_owner_only`] and by SQLite,
/// and SQLite still refuses a folder swapped for a link after this resolution. Filesystem I/O.
///
/// # Errors
/// `path` names no file, or the folder above it cannot be resolved (for example it is
/// missing).
pub(crate) fn resolve_folders(path: &Path) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path names no file"))?;
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    Ok(std::fs::canonicalize(parent)?.join(name))
}

/// Create the file at `path` if missing and set it to mode 0600 on Unix, so other local users
/// cannot read it; elsewhere the data directory's access control applies. A symbolic link at
/// `path` is refused, never followed: it is checked first, and on Unix the file is opened with
/// `O_NOFOLLOW`, so a link created after the check fails the open instead of redirecting it.
/// Filesystem I/O.
///
/// # Errors
/// `path` is a symbolic link, or the file cannot be created or its mode set.
pub(crate) fn create_owner_only(path: &Path) -> io::Result<()> {
    open_owner_only(path).map(drop)
}

/// [`create_owner_only`], answering the open, writable handle so the caller writes through
/// the file that was opened, never a second open by path. Creates the file on every
/// platform. On Unix the open refuses a symbolic link (`O_NOFOLLOW`); on Windows it opens a
/// reparse point itself instead of its target and refuses it, so a link swapped in after the
/// first check is never followed there either. Other targets rely on the first check alone.
/// Filesystem I/O.
///
/// # Errors
/// As [`create_owner_only`].
pub(crate) fn open_owner_only(path: &Path) -> io::Result<std::fs::File> {
    refuse_symlink(path)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        let file = options.open(path)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        Ok(file)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        /// `FILE_FLAG_OPEN_REPARSE_POINT`: open a reparse point, not what it points to.
        const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(OPEN_REPARSE_POINT);
        let file = options.open(path)?;
        match file.metadata()?.file_type().is_symlink() {
            true => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing a symbolic link",
            )),
            false => Ok(file),
        }
    }
    #[cfg(not(any(unix, windows)))]
    options.open(path)
}

/// Fail when `path` is a symbolic link; a missing path is fine. Filesystem I/O.
fn refuse_symlink(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing a symbolic link",
        )),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

/// Open the operational SQLite store at `path` as every operational store does: its folders
/// resolved, the file created owner-only, opened never through a symbolic link
/// (`SQLITE_OPEN_NOFOLLOW`) with a 5 s busy timeout. `what` names the store in an error, which
/// never names the path. Filesystem and database I/O.
///
/// # Errors
/// The file cannot be created or opened.
#[cfg(feature = "desktop")]
pub(crate) fn open_operational_db(path: &Path, what: &str) -> Result<rusqlite::Connection, String> {
    use rusqlite::OpenFlags;
    let unopened = |err: io::Error| format!("{what} could not be opened ({})", err.kind());
    let path = resolve_folders(path).map_err(unopened)?;
    create_owner_only(&path).map_err(unopened)?;
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    let conn = rusqlite::Connection::open_with_flags(&path, flags)
        .map_err(|err| format!("{what} could not be opened ({err})"))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|err| format!("{what} could not be opened ({err})"))?;
    Ok(conn)
}
