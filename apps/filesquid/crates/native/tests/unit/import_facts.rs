//! Unit tests for the import facts of Task DE-1.2 (DE-S-008, ADR 038 Decisions 3 and 4): a
//! fixture with known timestamps and extended attributes, read before its content; every
//! fact the platform cannot give recorded as unavailable with a reason; the attribute bounds;
//! known attributes decoded; the mount table; the name form; and the importing host.

use filesquid_metadata::fact::{Fact, Reason};
#[cfg(target_os = "linux")]
use filesquid_metadata::import::attributes::AttrValue;
use filesquid_metadata::import::{Link, NameForm};
use filesquid_native::import::facts::{gather_source, SourceError};
use filesquid_native::import::host::{gather_host, Build};
use filesquid_native::import::names::name_form;
use filesquid_native::import::time::at_nanos;
use filesquid_native::import::volume::{deepest_mount, is_network_type, parse_mountinfo, Mount};
use filesquid_native::import::xattr::decode;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A fresh directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "filesquid-native-import-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.0) {
            eprintln!("could not remove the test directory: {err}");
        }
    }
}

/// 2026-10-20T10:00:00.123456789Z.
fn known_time() -> SystemTime {
    UNIX_EPOCH + Duration::new(1_792_490_400, 123_456_789)
}

/// A file `name` in `dir` holding `bytes`, modified and accessed at [`known_time`].
fn fixture(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write");
    let file = std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("open");
    let times = std::fs::FileTimes::new()
        .set_modified(known_time())
        .set_accessed(known_time());
    file.set_times(times).expect("times");
    path
}

/// Set the extended attribute `name` of `path` to `value` (Linux `user.` namespace).
#[cfg(target_os = "linux")]
fn set_xattr(path: &Path, name: &str, value: &[u8]) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).expect("path");
    let name = std::ffi::CString::new(name).expect("name");
    // SAFETY: NUL-terminated strings and a value buffer of the length passed.
    let done = unsafe {
        libc::setxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_ptr().cast(),
            value.len(),
            0,
        )
    };
    done == 0
}

#[test]
fn a_fixture_reports_its_known_times_size_owner_and_mode() {
    let dir = TempDir::new("times");
    let path = fixture(&dir.0, "Holiday photo.JPG", b"0123456789");
    let gathered = gather_source(&path, &dir.0).expect("gathered");
    let file = gathered.file;
    let known = Fact::value("2026-10-20T10:00:00.123456789Z".to_string());
    assert_eq!(file.modified_time, known);
    assert_eq!(file.accessed_time, known, "read before the content");
    assert!(!file.accessed_after_read);
    assert_eq!(file.reported_size, Fact::value(10));
    assert_eq!(file.original_name.as_bytes(), b"Holiday photo.JPG");
    assert_eq!(file.extension.as_deref(), Some("JPG"));
    assert_eq!(file.name_normalization, NameForm::Nfc);
    assert_eq!(file.link, Fact::value(Link::None));
    assert_eq!(
        file.relative_path,
        Fact::value(filesquid_metadata::raw::Raw::from_bytes(
            b"Holiday photo.JPG"
        ))
    );
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions.
        let uid = u64::from(unsafe { libc::geteuid() });
        assert_eq!(file.owner_id, Fact::value(uid));
        assert!(matches!(file.owner_name, Fact::Value(_)));
        assert!(matches!(file.posix_mode, Fact::Value(mode) if mode & 0o777 != 0));
        assert!(matches!(file.status_changed_time, Fact::Value(_)));
    }
}

#[test]
fn facts_the_platform_cannot_give_are_listed_with_a_reason() {
    let dir = TempDir::new("unavailable");
    let path = fixture(&dir.0, "a.txt", b"x");
    let file = gather_source(&path, &path).expect("gathered").file;
    let unavailable = |fact: bool| assert!(fact);
    unavailable(
        matches!(file.date_added_time, Fact::Unavailable(ref u) if u.unavailable == Reason::NotSupportedBySource && u.detail.is_some()),
    );
    unavailable(matches!(file.os_content_type, Fact::Unavailable(_)));
    unavailable(matches!(file.acl, Fact::Unavailable(_)));
    assert!(
        matches!(file.relative_path, Fact::Unavailable(_)),
        "a single chosen file has no relative path"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn extended_attributes_are_read_with_their_values() {
    let dir = TempDir::new("xattrs");
    let path = fixture(&dir.0, "tagged.txt", b"x");
    if !set_xattr(&path, "user.filesquid.colour", b"red") {
        eprintln!("this filesystem keeps no user attributes; skipping the value check");
        return;
    }
    set_xattr(&path, "user.binary", &[0, 159, 146, 150]);
    let attributes = match gather_source(&path, &dir.0).expect("gathered").attributes {
        Fact::Value(attributes) => attributes,
        Fact::Unavailable(u) => panic!("attributes unavailable: {u:?}"),
    };
    assert!(!attributes.truncated);
    let colour = attributes
        .entries
        .iter()
        .find(|a| a.name.as_bytes() == b"user.filesquid.colour")
        .expect("colour");
    assert_eq!(colour.size, 3);
    assert!(matches!(&colour.value, AttrValue::Full { value } if value.as_bytes() == b"red"));
    let binary = attributes
        .entries
        .iter()
        .find(|a| a.name.as_bytes() == b"user.binary")
        .expect("binary");
    assert!(
        matches!(&binary.value, AttrValue::Full { value } if value.as_bytes() == [0, 159, 146, 150])
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_large_attribute_is_kept_by_size_and_digest() {
    let dir = TempDir::new("large-xattr");
    let path = fixture(&dir.0, "big.txt", b"x");
    let value = vec![7u8; 70 * 1024];
    if !set_xattr(&path, "user.big", &value) {
        eprintln!("this filesystem keeps no large user attributes; skipping");
        return;
    }
    let Fact::Value(attributes) = gather_source(&path, &dir.0).expect("gathered").attributes else {
        panic!("attributes unavailable");
    };
    let big = attributes
        .entries
        .iter()
        .find(|a| a.name.as_bytes() == b"user.big");
    let big = big.expect("big");
    assert_eq!(big.size, 70 * 1024);
    use sha2::Digest as _;
    let expected: String = sha2::Sha256::digest(vec![7u8; 70 * 1024])
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(big.value, AttrValue::Digest { sha256: expected });
}

#[test]
fn a_folder_or_a_missing_file_is_refused() {
    let dir = TempDir::new("refused");
    assert_eq!(
        gather_source(&dir.0, &dir.0).unwrap_err(),
        SourceError::NotAFile
    );
    let missing = dir.0.join("gone.txt");
    assert_eq!(
        gather_source(&missing, &dir.0).unwrap_err(),
        SourceError::Missing
    );
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_records_its_target_and_the_targets_size() {
    let dir = TempDir::new("link");
    let target = fixture(&dir.0, "target.bin", b"twelve bytes");
    let link = dir.0.join("link.bin");
    std::os::unix::fs::symlink(&target, &link).expect("link");
    let file = gather_source(&link, &dir.0).expect("gathered").file;
    assert_eq!(file.reported_size, Fact::value(12));
    let Fact::Value(Link::Symlink { target: recorded }) = file.link else {
        panic!("a link: {:?}", file.link);
    };
    assert_eq!(recorded.as_bytes(), target.as_os_str().as_encoded_bytes());
}

#[test]
fn the_quarantine_attribute_is_decoded() {
    let decoded = decode(
        b"com.apple.quarantine",
        b"0083;6712f3a0;Safari;9F3E2A7D-8C9E-4F11-0B6F-6A585C1D4B5E",
    );
    assert_eq!(decoded["agent"], ["Safari"]);
    assert_eq!(
        decoded["event_id"],
        ["9F3E2A7D-8C9E-4F11-0B6F-6A585C1D4B5E"]
    );
    assert_eq!(
        decoded["time"],
        [at_nanos(0x6712_f3a0, 0).expect("in range")]
    );
    assert!(decode(b"user.other", b"x").is_empty());
}

/// A property list of `body` in its XML form (the attributes are binary on disk; the
/// reader accepts both).
fn plist(body: &str) -> Vec<u8> {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0">{body}</plist>"#)
        .into_bytes()
}

#[test]
fn where_from_tags_comment_and_colour_label_are_decoded() {
    let urls = plist("<array><string>https://example.org/a.jpg</string><string>https://example.org/</string></array>");
    let decoded = decode(b"com.apple.metadata:kMDItemWhereFroms", &urls);
    assert_eq!(
        decoded["origin_urls"],
        ["https://example.org/a.jpg", "https://example.org/"]
    );
    let tags = plist("<array><string>Holiday\n6</string><string>Family</string></array>");
    let decoded = decode(b"com.apple.metadata:_kMDItemUserTags", &tags);
    assert_eq!(decoded["tags"], ["Holiday", "Family"]);
    assert_eq!(decoded["tag_colors"], ["6", "0"]);
    let comment = plist("<string>From the beach</string>");
    let decoded = decode(b"com.apple.metadata:kMDItemFinderComment", &comment);
    assert_eq!(decoded["comment"], ["From the beach"]);
    let mut info = [0u8; 32];
    info[9] = 2 << 1;
    let decoded = decode(b"com.apple.FinderInfo", &info);
    assert_eq!(decoded["color_label"], ["2"]);
    assert!(decode(b"com.apple.metadata:kMDItemWhereFroms", b"not a plist").is_empty());
}

#[test]
fn times_outside_what_rfc_3339_states_are_refused() {
    assert_eq!(
        at_nanos(0, 5).as_deref(),
        Some("1970-01-01T00:00:00.000000005Z")
    );
    assert_eq!(at_nanos(0, 1_000_000_000), None, "a whole second of nanos");
    assert_eq!(at_nanos(253_402_300_800, 0), None, "after the year 9999");
}

#[test]
fn the_mount_table_gives_device_point_and_type() {
    let line = r"36 35 98:0 /mnt1 /mnt/My\040Drive rw,noatime master:1 - ext4 /dev/root rw,errors=continue";
    let (device, mount) = parse_mountinfo(line).expect("parsed");
    assert_eq!(device, "98:0");
    assert_eq!(mount.point, b"/mnt/My Drive");
    assert_eq!(mount.fs_type, "ext4");
}

#[test]
fn the_deepest_mount_above_the_file_is_chosen_and_network_types_are_known() {
    let mount = |point: &str| Mount {
        point: point.as_bytes().to_vec(),
        fs_type: "ext4".into(),
    };
    let mounts = [
        mount("/"),
        mount("/srv/data"),
        mount("/srv/database"),
        mount("/home"),
    ];
    let found = deepest_mount(mounts.iter().cloned(), b"/srv/data/photos/a.jpg");
    assert_eq!(found.map(|m| m.point), Some(b"/srv/data".to_vec()));
    let found = deepest_mount(mounts.iter().cloned(), b"/srv/datapile/a.jpg");
    assert_eq!(
        found.map(|m| m.point),
        Some(b"/".to_vec()),
        "a whole segment only"
    );
    for network in ["nfs4", "cifs", "ceph", "fuse.sshfs", "davfs", "glusterfs"] {
        assert!(is_network_type(network), "{network}");
    }
    assert!(!is_network_type("ext4"));
}

#[test]
fn name_forms_are_told_apart() {
    assert_eq!(name_form(b"plain.txt"), NameForm::Nfc);
    assert_eq!(name_form("café.txt".as_bytes()), NameForm::Nfc);
    assert_eq!(name_form("cafe\u{301}.txt".as_bytes()), NameForm::Nfd);
    assert_eq!(
        name_form("\u{e9}e\u{301}".as_bytes()),
        NameForm::Unnormalised,
        "composed and decomposed mixed"
    );
    assert_eq!(name_form(&[0xff, 0xfe]), NameForm::Unnormalised);
}

#[test]
fn the_host_names_this_build_and_its_system() {
    let build = Build {
        version: "0.1.0".into(),
        build: "test".into(),
        distribution: "filesquid".into(),
    };
    let host = gather_host(&build);
    assert_eq!(host.filesquid_version, "0.1.0");
    assert_eq!(host.filesquid_build, "test");
    assert_eq!(host.distribution, "filesquid");
    assert_eq!(
        host.architecture,
        Fact::value(std::env::consts::ARCH.to_string())
    );
    #[cfg(target_os = "linux")]
    assert_eq!(host.os_name, Fact::value("Linux".to_string()));
    #[cfg(unix)]
    assert!(matches!(host.account, Fact::Value(_)));
}
