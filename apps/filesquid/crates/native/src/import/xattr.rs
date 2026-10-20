//! The extended attributes of a source file (ADR 038 Decisions 3 and 4), read through the
//! operating system's `listxattr` and `getxattr` and bounded by the metadata module's
//! [`AttributeCollector`]: at most 256 names and 1 MiB of value bytes per file, every cut
//! recorded. Links are followed, because the bytes FileSquid copies are the target's.

use filesquid_metadata::fact::{Fact, Reason};
use filesquid_metadata::import::attributes::{AttributeCollector, Attributes};
use std::collections::BTreeMap;
use std::path::Path;

/// Longest attribute name list read, in bytes (the kernel's own limit is 64 KiB).
const MAX_NAME_LIST: usize = 64 * 1024;

/// The extended attributes of `path`, or why they could not be read. Reads the
/// filesystem.
pub fn read_attributes(path: &Path) -> Fact<Attributes> {
    let names = match sys::list(path) {
        Ok(names) => names,
        Err(reason) => return Fact::unavailable_because(reason.0, reason.1),
    };
    let mut collector = AttributeCollector::new();
    collector.note_names_enumerated(names.len() as u64);
    for name in names.iter().take(collector.names_remaining()) {
        add_one(path, name, &mut collector);
    }
    Fact::value(collector.finish())
}

/// Read one attribute within the collector's remaining budget and add it, decoding the
/// ones FileSquid knows. An attribute that vanished between the listing and the read is
/// recorded with no value read.
fn add_one(path: &Path, name: &[u8], collector: &mut AttributeCollector) {
    let budget = usize::try_from(collector.read_budget()).unwrap_or(usize::MAX);
    let (size, value) = sys::get(path, name, budget).unwrap_or((0, Vec::new()));
    if collector.add(name, size, &value).is_err() {
        return;
    }
    let decoded = decode(name, &value);
    if !decoded.is_empty() {
        collector.decode_last(decoded);
    }
}

/// The decoded fields of a known attribute (ADR 038 Decision 3); empty for any other.
/// Pure.
pub fn decode(name: &[u8], value: &[u8]) -> BTreeMap<String, Vec<String>> {
    match name {
        b"com.apple.quarantine" => decode_quarantine(value),
        _ => BTreeMap::new(),
    }
}

/// `com.apple.quarantine` = `flags;hex-time;agent;event-id`: the agent, the time (seconds
/// since 1970, as RFC 3339 UTC) and the event ID. Pure.
fn decode_quarantine(value: &[u8]) -> BTreeMap<String, Vec<String>> {
    let text = String::from_utf8_lossy(value);
    let parts: Vec<&str> = text.trim_end_matches('\0').split(';').collect();
    let mut decoded = BTreeMap::new();
    if let Some(time) = parts
        .get(1)
        .and_then(|hex| u64::from_str_radix(hex, 16).ok())
    {
        decoded.insert("time".into(), vec![super::time::at_nanos(time, 0)]);
    }
    for (key, index) in [("agent", 2), ("event_id", 3)] {
        if let Some(part) = parts.get(index).filter(|p| !p.is_empty()) {
            decoded.insert(key.into(), vec![(*part).to_string()]);
        }
    }
    decoded
}

/// Why a listing failed: the record's reason and a short detail with no path.
pub struct ListError(pub Reason, pub &'static str);

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod sys {
    //! `listxattr` and `getxattr` through libc, following links.

    use super::{ListError, MAX_NAME_LIST};
    use filesquid_metadata::fact::Reason;
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    /// The NUL-terminated path, or a refusal for a path with a NUL byte.
    fn c_path(path: &Path) -> Result<CString, ListError> {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|_| ListError(Reason::NotSupportedBySource, "path holds a NUL byte"))
    }

    /// The attribute names of `path`, each without its NUL. Reads the filesystem.
    pub fn list(path: &Path) -> Result<Vec<Vec<u8>>, ListError> {
        let path = c_path(path)?;
        let mut buffer = vec![0u8; MAX_NAME_LIST];
        // SAFETY: a NUL-terminated path and a buffer of the length passed.
        let read = unsafe { list_raw(&path, &mut buffer) };
        if read < 0 {
            return Err(list_error());
        }
        buffer.truncate(read as usize);
        Ok(buffer
            .split(|b| *b == 0)
            .filter(|name| !name.is_empty())
            .map(<[u8]>::to_vec)
            .collect())
    }

    /// The listing failure as the record states it.
    fn list_error() -> ListError {
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EACCES) | Some(libc::EPERM) => {
                ListError(Reason::PermissionDenied, "listing refused")
            }
            Some(libc::ERANGE) => ListError(Reason::NotSupportedBySource, "name list too long"),
            _ => ListError(
                Reason::NotSupportedBySource,
                "not supported by the filesystem",
            ),
        }
    }

    /// The size of the attribute `name` of `path` and at most `budget` of its value bytes
    /// (all of them when it fits). Reads the filesystem.
    pub fn get(path: &Path, name: &[u8], budget: usize) -> Option<(u64, Vec<u8>)> {
        let path = c_path(path).ok()?;
        let name = CString::new(name).ok()?;
        // SAFETY: NUL-terminated strings and a null buffer of length 0 (size query).
        let size = unsafe { get_raw(&path, &name, &mut []) };
        if size < 0 {
            return None;
        }
        let size = size as usize;
        if size > budget {
            return Some((size as u64, Vec::new()));
        }
        let mut value = vec![0u8; size];
        // SAFETY: as above, with a buffer of the length passed.
        let read = unsafe { get_raw(&path, &name, &mut value) };
        if read < 0 {
            return Some((size as u64, Vec::new()));
        }
        value.truncate(read as usize);
        Some((size as u64, value))
    }

    /// `listxattr(path, buffer, len)` (Linux).
    #[cfg(target_os = "linux")]
    unsafe fn list_raw(path: &CString, buffer: &mut [u8]) -> isize {
        libc::listxattr(path.as_ptr(), buffer.as_mut_ptr().cast(), buffer.len())
    }

    /// `listxattr(path, buffer, len, 0)` (macOS; follows links).
    #[cfg(target_os = "macos")]
    unsafe fn list_raw(path: &CString, buffer: &mut [u8]) -> isize {
        libc::listxattr(path.as_ptr(), buffer.as_mut_ptr().cast(), buffer.len(), 0)
    }

    /// `getxattr(path, name, buffer, len)` (Linux).
    #[cfg(target_os = "linux")]
    unsafe fn get_raw(path: &CString, name: &CString, buffer: &mut [u8]) -> isize {
        let target = if buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            buffer.as_mut_ptr().cast()
        };
        libc::getxattr(path.as_ptr(), name.as_ptr(), target, buffer.len())
    }

    /// `getxattr(path, name, buffer, len, 0, 0)` (macOS; follows links).
    #[cfg(target_os = "macos")]
    unsafe fn get_raw(path: &CString, name: &CString, buffer: &mut [u8]) -> isize {
        let target = if buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            buffer.as_mut_ptr().cast()
        };
        libc::getxattr(path.as_ptr(), name.as_ptr(), target, buffer.len(), 0, 0)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod sys {
    //! Platforms without an extended-attribute reader here: every read is unavailable.

    use super::ListError;
    use filesquid_metadata::fact::Reason;
    use std::path::Path;

    /// No listing on this platform.
    pub fn list(_path: &Path) -> Result<Vec<Vec<u8>>, ListError> {
        Err(ListError(
            Reason::NotSupportedBySource,
            "not read on this platform",
        ))
    }

    /// No value on this platform.
    pub fn get(_path: &Path, _name: &[u8], _budget: usize) -> Option<(u64, Vec<u8>)> {
        None
    }
}
