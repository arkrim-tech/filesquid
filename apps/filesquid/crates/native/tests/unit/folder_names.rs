//! Task API-F2.31 (API-S-034 **Folder and volume names**, DE-S-017; owner decision 2026-11-02):
//! the desktop reads the name of a library folder and of the volume that holds it. The folder
//! name is the folder's own last path component; the volume name is the name of the mount
//! point of an external or removable volume (`/Volumes/<name>` on macOS, `/media/<name>`,
//! `/media/<user>/<name>`, `/run/media/<user>/<name>` or `/mnt/<name>` on Linux) and `None`
//! for any other mount, the system disk included; a folder that cannot be reached answers no
//! name at all.

use filesquid_api::local_library::{FolderNames, LibraryRegistry};
use filesquid_native::folder_names::{mount_point_of, read_folder_names, volume_name_of_mount};
use filesquid_native::HostLocations;
use std::path::{Path, PathBuf};

/// A fresh, empty directory for one test (filesystem I/O).
fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("f231-names-{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("stale test directory");
    }
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

#[test]
fn the_volume_name_is_the_mount_point_name_of_an_external_volume_only() {
    let cases: [(&str, Option<&str>); 10] = [
        ("/Volumes/Photos SSD", Some("Photos SSD")),
        ("/media/Backup", Some("Backup")),
        ("/media/ana/Photos SSD", Some("Photos SSD")),
        ("/run/media/ana/Backup 2", Some("Backup 2")),
        ("/mnt/archive", Some("archive")),
        ("/", None),
        ("/home", None),
        ("/Users", None),
        ("/System/Volumes/Data", None),
        ("/Volumes", None),
    ];
    for (point, name) in cases {
        assert_eq!(
            volume_name_of_mount(Path::new(point)).as_deref(),
            name,
            "{point}"
        );
    }
    assert_eq!(
        volume_name_of_mount(Path::new("/Volumes/a\u{7}b")).as_deref(),
        Some("ab"),
        "the safe transformation of API-S-005"
    );
}

#[test]
fn a_folder_answers_its_own_name_and_the_volume_of_its_mount_point() {
    let dir = fresh_dir("read");
    let folder = dir.join("Family archive");
    std::fs::create_dir_all(&folder).expect("folder");
    let names = read_folder_names(&folder);
    assert_eq!(names.folder_name.as_deref(), Some("Family archive"));
    #[cfg(unix)]
    {
        let point = mount_point_of(&folder).expect("a mount point");
        let canonical = folder.canonicalize().expect("canonical");
        assert!(canonical.starts_with(&point));
        assert_eq!(names.volume_name, volume_name_of_mount(&point));
    }
    #[cfg(not(unix))]
    {
        assert_eq!(mount_point_of(&folder), None, "no device numbers here");
        assert_eq!(names.volume_name, None);
    }
    let text = format!("{names:?}");
    let parent = dir.file_name().expect("name").to_string_lossy().to_string();
    assert!(!text.contains(&parent), "no parent folder: {text}");
    std::fs::remove_dir_all(&folder).expect("gone");
    assert_eq!(read_folder_names(&folder), FolderNames::default());
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn the_registry_answers_a_registered_folder_s_names_and_none_once_it_is_gone() {
    let dir = fresh_dir("registry");
    let folder = dir.join("Picked library");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::create_dir_all(dir.join("data")).expect("data dir");
    let locations = HostLocations::new(&dir.join("data"));
    let location = locations.register(&folder).expect("registered");
    let names = locations.folder_names(&location);
    assert_eq!(names.folder_name.as_deref(), Some("Picked library"));
    assert_eq!(
        locations.folder_names("loc-unknown"),
        FolderNames::default()
    );
    std::fs::rename(&folder, dir.join("Moved away")).expect("moved");
    assert_eq!(
        locations.folder_names(&location),
        FolderNames::default(),
        "a folder that cannot be reached now has no names"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// A mount point that cannot be verified within the ancestor cap is no mount point, so an
/// ordinary folder is never reported as a volume.
#[test]
fn a_folder_deeper_than_the_ancestor_cap_answers_no_mount_point_and_no_volume() {
    let dir = fresh_dir("deep");
    let mut folder = dir.clone();
    for _ in 0..300 {
        folder.push("d");
    }
    std::fs::create_dir_all(&folder).expect("deep folder");
    assert_eq!(
        mount_point_of(&folder),
        None,
        "the cap is reached before the boundary"
    );
    let names = read_folder_names(&folder);
    assert_eq!(names.folder_name.as_deref(), Some("d"));
    assert_eq!(names.volume_name, None);
    std::fs::remove_dir_all(&dir).expect("cleanup");
}
