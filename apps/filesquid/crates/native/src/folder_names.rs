//! The names of a library folder and of its volume that the desktop answers to its owner
//! (Task API-F2.31; API-S-034 **Folder and volume names**, DE-S-017; owner decision
//! 2026-11-02, verbatim: "RE: 5 - the desktop user can see folder name and volume so they can
//! better manage their own private desktop").
//!
//! The folder name is the folder's own last path component, never a parent folder or the
//! path. The volume name is read from the folder's mount point, found with the standard
//! library only, so the same code runs on macOS and Linux: the topmost ancestor of the folder
//! on the same device. A mount point where the operating system mounts external and removable
//! volumes (`/Volumes/<name>` on macOS; `/media/<name>`, `/media/<user>/<name>`,
//! `/run/media/<user>/<name>` and `/mnt/<name>` on Linux) is named after the volume, so its
//! last component is the volume's name. Any other mount, the system disk included, answers
//! `None`, as does every platform without Unix device numbers (Windows): the API answers
//! `null` for a name the host cannot read. Both names pass API-S-005's safe transformation.
//!
//! No name read here is logged, journaled or sent as an event; the API answers it to the
//! caller only.

use filesquid_api::local_library::FolderNames;
use std::path::{Path, PathBuf};

/// Most ancestors walked to find a folder's mount point (AGENTS.md section 22); a deeper
/// folder answers the mount found so far.
const MAX_ANCESTORS: usize = 256;

/// Parents whose children are external or removable volumes, named after the volume.
const VOLUME_PARENTS: [&str; 3] = ["/Volumes", "/media", "/mnt"];

/// Grandparents of per-user volume mounts (`/media/<user>/<name>`, `/run/media/<user>/<name>`).
const USER_VOLUME_ROOTS: [&str; 2] = ["/media", "/run/media"];

/// The names of `folder` and of its volume, or no names when the folder cannot be reached
/// now (missing, moved, unplugged or not a folder). Reads filesystem metadata.
pub fn read_folder_names(folder: &Path) -> FolderNames {
    let Ok(canonical) = std::fs::canonicalize(folder) else {
        return FolderNames::default();
    };
    if !canonical.is_dir() {
        return FolderNames::default();
    }
    let folder_name = canonical
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let volume_name = mount_point_of(&canonical).and_then(|point| volume_name_of_mount(&point));
    FolderNames::safe(folder_name.as_deref(), volume_name.as_deref())
}

/// The volume name that the mount point `point` carries: its last component when the
/// operating system mounts external or removable volumes there, else `None`. Pure.
pub fn volume_name_of_mount(point: &Path) -> Option<String> {
    let name = point.file_name()?.to_string_lossy().into_owned();
    let parent = point.parent()?;
    let named_parent = VOLUME_PARENTS.iter().any(|root| parent == Path::new(root));
    let user_root = parent.parent().is_some_and(|grand| {
        USER_VOLUME_ROOTS
            .iter()
            .any(|root| grand == Path::new(root))
    });
    if !(named_parent || user_root) {
        return None;
    }
    FolderNames::safe(None, Some(&name)).volume_name
}

/// The mount point of the volume that holds `folder`: its topmost ancestor on the same
/// device, at most [`MAX_ANCESTORS`] levels up. Reads filesystem metadata.
#[cfg(unix)]
pub fn mount_point_of(folder: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let canonical = std::fs::canonicalize(folder).ok()?;
    let device = std::fs::metadata(&canonical).ok()?.dev();
    let mut point = canonical.clone();
    for ancestor in canonical.ancestors().skip(1).take(MAX_ANCESTORS) {
        let same = std::fs::metadata(ancestor).is_ok_and(|meta| meta.dev() == device);
        if !same {
            break;
        }
        point = ancestor.to_path_buf();
    }
    Some(point)
}

/// No device numbers to compare on this platform, so no mount point is found.
#[cfg(not(unix))]
pub fn mount_point_of(_folder: &Path) -> Option<PathBuf> {
    None
}
