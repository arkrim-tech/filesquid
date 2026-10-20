//! The facts the operating system provides about a source file, its volume and the
//! importing host (Task DE-1.2; DE-S-008, ADR 038 Decision 3), gathered before FileSquid
//! reads the file's bytes so its own read cannot change the accessed time. A fact the host
//! cannot read is recorded as unavailable with a reason, never invented.
//!
//! Linux and macOS read through the standard library and libc; other platforms record the
//! Unix-only facts as unavailable.

use super::time::rfc3339_nanos;
use super::xattr::read_attributes;
use filesquid_metadata::fact::{Fact, Reason};
use filesquid_metadata::import::attributes::Attributes;
use filesquid_metadata::import::{Link, SourceFile, SourceVolume};
use filesquid_metadata::raw::Raw;
use std::io;
use std::path::Path;

/// What the host gathered about one source file before reading it.
#[derive(Clone, Debug)]
pub struct GatheredSource {
    /// The source file's facts; `verified_size` is set after the copy.
    pub file: SourceFile,
    /// The volume it is on.
    pub volume: SourceVolume,
    /// Its extended attributes.
    pub attributes: Fact<Attributes>,
}

/// Why a source could not be imported at all (each with no path).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceError {
    /// The source is gone.
    Missing,
    /// The operating system refused access to it.
    PermissionDenied,
    /// It is not a regular file (a folder, device or socket).
    NotAFile,
    /// It has no file name.
    NoName,
}

impl SourceError {
    /// The journal error code.
    pub fn error_code(self) -> &'static str {
        match self {
            SourceError::Missing => "source_missing",
            SourceError::PermissionDenied => "permission_denied",
            SourceError::NotAFile => "not_a_file",
            SourceError::NoName => "invalid_name",
        }
    }
}

/// Map a metadata read failure to a [`SourceError`].
fn source_error(err: &io::Error) -> SourceError {
    match err.kind() {
        io::ErrorKind::NotFound => SourceError::Missing,
        io::ErrorKind::PermissionDenied => SourceError::PermissionDenied,
        _ => SourceError::Missing,
    }
}

/// Gather the facts of the file at `path`, imported from the folder `import_root` (the
/// file itself when a single file was chosen). Reads filesystem metadata and extended
/// attributes, never the content.
///
/// # Errors
/// [`SourceError`] when the file cannot be imported at all.
pub fn gather_source(path: &Path, import_root: &Path) -> Result<GatheredSource, SourceError> {
    let link = std::fs::symlink_metadata(path).map_err(|e| source_error(&e))?;
    let meta = std::fs::metadata(path).map_err(|e| source_error(&e))?;
    if !meta.is_file() {
        return Err(SourceError::NotAFile);
    }
    let name = path.file_name().ok_or(SourceError::NoName)?;
    let name = Raw::from_bytes(name.as_encoded_bytes());
    let file = SourceFile {
        name_normalization: super::names::name_form(name.as_bytes()),
        extension: filesquid_metadata::file_record::extension_of_bytes(name.as_bytes())
            .and_then(|ext| String::from_utf8(ext.to_vec()).ok()),
        original_name: name,
        absolute_path: Fact::value(raw_path(path)),
        import_root: Fact::value(raw_path(import_root)),
        relative_path: relative(path, import_root),
        reported_size: Fact::value(meta.len()),
        verified_size: 0,
        birth_time: time_fact(meta.created()),
        modified_time: time_fact(meta.modified()),
        status_changed_time: unix::changed_time(&meta),
        accessed_time: time_fact(meta.accessed()),
        accessed_after_read: false,
        date_added_time: Fact::unavailable_because(
            Reason::NotSupportedBySource,
            "not read on this platform",
        ),
        owner_name: unix::owner_name(&meta),
        owner_id: unix::owner_id(&meta),
        group_name: unix::group_name(&meta),
        group_id: unix::group_id(&meta),
        posix_mode: unix::mode(&meta),
        acl: Fact::unavailable_because(Reason::NotSupportedBySource, "not read on this platform"),
        flags: unix::flags(&meta),
        link: link_fact(path, &link),
        os_content_type: Fact::unavailable_because(
            Reason::NotSupportedBySource,
            "the operating system reports no content type here",
        ),
    };
    Ok(GatheredSource {
        volume: super::volume::gather_volume(path),
        attributes: read_attributes(path),
        file,
    })
}

/// `path` as raw bytes. Pure.
fn raw_path(path: &Path) -> Raw {
    Raw::from_bytes(path.as_os_str().as_encoded_bytes())
}

/// `path` relative to `root`, or unavailable for a single chosen file. Pure.
fn relative(path: &Path, root: &Path) -> Fact<Raw> {
    match path.strip_prefix(root) {
        Ok(rest) if !rest.as_os_str().is_empty() => Fact::value(raw_path(rest)),
        _ => Fact::unavailable_because(Reason::NotSupportedBySource, "a single file was chosen"),
    }
}

/// A file time as a fact: unavailable when the filesystem keeps none (or one before 1970).
fn time_fact(time: io::Result<std::time::SystemTime>) -> Fact<String> {
    match time.ok().and_then(rfc3339_nanos) {
        Some(text) => Fact::value(text),
        None => {
            Fact::unavailable_because(Reason::NotSupportedBySource, "not kept by the filesystem")
        }
    }
}

/// Whether the source was a symbolic link, and its target. Reads the link.
fn link_fact(path: &Path, link: &std::fs::Metadata) -> Fact<Link> {
    if !link.file_type().is_symlink() {
        return Fact::value(Link::None);
    }
    match std::fs::read_link(path) {
        Ok(target) => Fact::value(Link::Symlink {
            target: raw_path(&target),
        }),
        Err(_) => Fact::unavailable_because(Reason::PermissionDenied, "link target unreadable"),
    }
}

#[cfg(unix)]
mod unix {
    //! The Unix facts of `stat`: change time, owner, group, mode and (macOS) flags.

    use super::super::time::at_nanos;
    use super::{Fact, Raw, Reason};
    use std::ffi::CStr;
    use std::os::unix::fs::MetadataExt;

    /// The status-changed time.
    pub fn changed_time(meta: &std::fs::Metadata) -> Fact<String> {
        match (
            u64::try_from(meta.ctime()),
            u32::try_from(meta.ctime_nsec()),
        ) {
            (Ok(seconds), Ok(nanos)) => Fact::value(at_nanos(seconds, nanos)),
            _ => Fact::unavailable_because(Reason::NotSupportedBySource, "before 1970"),
        }
    }

    /// The owner's numeric ID.
    pub fn owner_id(meta: &std::fs::Metadata) -> Fact<u64> {
        Fact::value(u64::from(meta.uid()))
    }

    /// The group's numeric ID.
    pub fn group_id(meta: &std::fs::Metadata) -> Fact<u64> {
        Fact::value(u64::from(meta.gid()))
    }

    /// The POSIX mode bits.
    pub fn mode(meta: &std::fs::Metadata) -> Fact<u32> {
        Fact::value(meta.mode())
    }

    /// The owner's account name. Reads the account database.
    pub fn owner_name(meta: &std::fs::Metadata) -> Fact<Raw> {
        named(user_name(meta.uid()), "no account has this ID")
    }

    /// The group's name. Reads the group database.
    pub fn group_name(meta: &std::fs::Metadata) -> Fact<Raw> {
        named(group_name_of(meta.gid()), "no group has this ID")
    }

    /// A looked-up name as a fact.
    fn named(name: Option<Vec<u8>>, missing: &str) -> Fact<Raw> {
        match name {
            Some(name) => Fact::value(Raw::from_bytes(&name)),
            None => Fact::unavailable_because(Reason::NotSupportedBySource, missing),
        }
    }

    /// The account name of `uid` (`getpwuid_r`). Reads the account database.
    pub fn user_name(uid: u32) -> Option<Vec<u8>> {
        let mut buffer = vec![0u8; 16 * 1024];
        // SAFETY: zeroed passwd storage, a buffer of the length passed, and an out pointer.
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                &mut entry,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut found,
            )
        };
        if status != 0 || found.is_null() || entry.pw_name.is_null() {
            return None;
        }
        // SAFETY: getpwuid_r succeeded, so pw_name points to a NUL-terminated string inside
        // `buffer`, which is still alive.
        Some(unsafe { CStr::from_ptr(entry.pw_name) }.to_bytes().to_vec())
    }

    /// The group name of `gid` (`getgrgid_r`). Reads the group database.
    fn group_name_of(gid: u32) -> Option<Vec<u8>> {
        let mut buffer = vec![0u8; 16 * 1024];
        // SAFETY: zeroed group storage, a buffer of the length passed, and an out pointer.
        let mut entry: libc::group = unsafe { std::mem::zeroed() };
        let mut found: *mut libc::group = std::ptr::null_mut();
        let status = unsafe {
            libc::getgrgid_r(
                gid,
                &mut entry,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut found,
            )
        };
        if status != 0 || found.is_null() || entry.gr_name.is_null() {
            return None;
        }
        // SAFETY: as in `user_name`.
        Some(unsafe { CStr::from_ptr(entry.gr_name) }.to_bytes().to_vec())
    }

    /// The BSD file flags (macOS `st_flags`) by name.
    #[cfg(target_os = "macos")]
    pub fn flags(meta: &std::fs::Metadata) -> Fact<Vec<String>> {
        use std::os::macos::fs::MetadataExt as MacExt;
        let bits = meta.st_flags();
        let names = [
            (libc::UF_NODUMP, "nodump"),
            (libc::UF_IMMUTABLE, "uchg"),
            (libc::UF_APPEND, "uappnd"),
            (libc::UF_HIDDEN, "hidden"),
            (libc::SF_ARCHIVED, "arch"),
            (libc::SF_IMMUTABLE, "schg"),
            (libc::SF_APPEND, "sappnd"),
        ];
        Fact::value(
            names
                .iter()
                .filter(|(bit, _)| bits & *bit != 0)
                .map(|(_, name)| (*name).to_string())
                .collect(),
        )
    }

    /// Linux keeps inode flags behind an ioctl this host does not issue.
    #[cfg(not(target_os = "macos"))]
    pub fn flags(_meta: &std::fs::Metadata) -> Fact<Vec<String>> {
        Fact::unavailable_because(Reason::NotSupportedBySource, "not read on this platform")
    }
}

#[cfg(not(unix))]
mod unix {
    //! Platforms without POSIX ownership: every such fact is unavailable.

    use super::{Fact, Raw, Reason};

    /// Not provided here.
    fn none<T>() -> Fact<T> {
        Fact::unavailable_because(Reason::NotSupportedBySource, "not read on this platform")
    }

    /// Not provided here.
    pub fn changed_time(_meta: &std::fs::Metadata) -> Fact<String> {
        none()
    }

    /// Not provided here.
    pub fn owner_id(_meta: &std::fs::Metadata) -> Fact<u64> {
        none()
    }

    /// Not provided here.
    pub fn group_id(_meta: &std::fs::Metadata) -> Fact<u64> {
        none()
    }

    /// Not provided here.
    pub fn mode(_meta: &std::fs::Metadata) -> Fact<u32> {
        none()
    }

    /// Not provided here.
    pub fn owner_name(_meta: &std::fs::Metadata) -> Fact<Raw> {
        none()
    }

    /// Not provided here.
    pub fn group_name(_meta: &std::fs::Metadata) -> Fact<Raw> {
        none()
    }

    /// Not provided here.
    pub fn flags(_meta: &std::fs::Metadata) -> Fact<Vec<String>> {
        none()
    }
}

#[cfg(unix)]
pub(crate) use unix::user_name;
