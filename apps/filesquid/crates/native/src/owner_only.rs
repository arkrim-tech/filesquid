//! Owner-only file creation shared by the journal store and the library catalogs.

use std::io;
use std::path::Path;

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
