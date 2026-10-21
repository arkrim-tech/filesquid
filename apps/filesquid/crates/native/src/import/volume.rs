//! The volume a source file is on (ADR 038 Decision 3): its filesystem type, mount point and
//! whether it is network storage. On Linux the mount is found in `/proc/self/mountinfo` by
//! the file's device; the volume name and UUID need privileged probes and are recorded as
//! unavailable, as is local versus removable storage.

use filesquid_metadata::fact::{Fact, Reason};
use filesquid_metadata::import::{SourceVolume, VolumeKind};
use filesquid_metadata::raw::Raw;
use std::path::Path;

/// Filesystem types of network storage (Linux `mountinfo` names, FUSE ones with their
/// `fuse.` prefix).
const NETWORK_TYPES: [&str; 18] = [
    "nfs",
    "nfs4",
    "cifs",
    "smb3",
    "smbfs",
    "afpfs",
    "9p",
    "ceph",
    "fuse.ceph",
    "glusterfs",
    "fuse.glusterfs",
    "fuse.sshfs",
    "fuse.davfs",
    "davfs",
    "afs",
    "lustre",
    "gpfs",
    "ncpfs",
];

/// Whether a filesystem type is network storage. Pure.
pub fn is_network_type(fs_type: &str) -> bool {
    NETWORK_TYPES.contains(&fs_type)
}

/// Most mount table lines read (AGENTS.md section 22).
const MAX_MOUNTS: usize = 4_096;

/// The facts of the volume holding `path`. Reads the mount table.
pub fn gather_volume(path: &Path) -> SourceVolume {
    let mount = find_mount(path);
    let kind = match &mount {
        Some(found) if is_network_type(&found.fs_type) => Fact::value(VolumeKind::Network),
        _ => unavailable("local and removable storage are not told apart here"),
    };
    SourceVolume {
        name: unavailable("needs a privileged probe"),
        uuid: unavailable("needs a privileged probe"),
        filesystem_type: mount.as_ref().map_or_else(
            || unavailable("mount not found"),
            |m| Fact::value(m.fs_type.clone()),
        ),
        mount_point: mount.map_or_else(
            || unavailable("mount not found"),
            |m| Fact::value(Raw::from_bytes(&m.point)),
        ),
        kind,
    }
}

/// An unavailable fact the source does not provide here.
fn unavailable<T>(detail: &str) -> Fact<T> {
    Fact::unavailable_because(Reason::NotSupportedBySource, detail)
}

/// One mount: its point and filesystem type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    /// The mount point, as the mount table spells it (octal escapes decoded).
    pub point: Vec<u8>,
    /// The filesystem type.
    pub fs_type: String,
}

/// The mount holding `path`: of the mount table entries with `path`'s device, the one with
/// the deepest mount point above `path`'s canonical form (one device can be mounted at
/// several points). Reads `/proc/self/mountinfo` (Linux).
#[cfg(target_os = "linux")]
fn find_mount(path: &Path) -> Option<Mount> {
    use std::os::unix::fs::MetadataExt;
    let device = std::fs::metadata(path).ok()?.dev();
    let (major, minor) = (libc::major(device), libc::minor(device));
    let canonical = std::fs::canonicalize(path).ok()?;
    let table = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    let wanted = format!("{major}:{minor}");
    let mounts = table
        .lines()
        .take(MAX_MOUNTS)
        .filter_map(parse_mountinfo)
        .filter(|(dev, _)| *dev == wanted)
        .map(|(_, mount)| mount);
    deepest_mount(mounts, canonical.as_os_str().as_encoded_bytes())
}

/// Of `mounts`, the one whose point is the longest whole-segment prefix of `path`. Pure.
pub fn deepest_mount(mounts: impl Iterator<Item = Mount>, path: &[u8]) -> Option<Mount> {
    mounts
        .filter(|mount| holds(&mount.point, path))
        .max_by_key(|mount| mount.point.len())
}

/// Whether the mount point `point` is `path` or a folder above it. Pure.
fn holds(point: &[u8], path: &[u8]) -> bool {
    let point = match point {
        b"/" => &b""[..],
        other => other,
    };
    path.starts_with(point) && matches!(path.get(point.len()), None | Some(b'/'))
}

/// No mount table is read on this platform.
#[cfg(not(target_os = "linux"))]
fn find_mount(_path: &Path) -> Option<Mount> {
    None
}

/// The device (`major:minor`) and mount of one `/proc/self/mountinfo` line. Pure.
pub fn parse_mountinfo(line: &str) -> Option<(String, Mount)> {
    let (left, right) = line.split_once(" - ")?;
    let fields: Vec<&str> = left.split(' ').collect();
    let device = fields.get(2)?.to_string();
    let point = unescape(fields.get(4)?);
    let fs_type = right.split(' ').next()?.to_string();
    Some((device, Mount { point, fs_type }))
}

/// A mount table field with its `\NNN` octal escapes decoded. Pure.
fn unescape(field: &str) -> Vec<u8> {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes.get(i + 1..i + 4).and_then(|digits| {
            std::str::from_utf8(digits)
                .ok()
                .and_then(|d| u8::from_str_radix(d, 8).ok())
        });
        match (bytes[i], octal) {
            (b'\\', Some(byte)) => {
                out.push(byte);
                i += 4;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    out
}
