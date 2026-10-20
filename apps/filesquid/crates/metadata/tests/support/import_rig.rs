//! Fixtures for the import record tests (MD-0.8): a complete upload-profile
//! entry, so each test changes only the field it is about.

#![allow(dead_code)]

use filesquid_metadata::fact::Fact;
use filesquid_metadata::import::attributes::AttributeCollector;
use filesquid_metadata::import::{
    Acquisition, ByteStorage, Host, ImportEntry, ImportMethod, Link, NameForm, UploadFacts,
    VolumeKind,
};
use filesquid_metadata::raw::Raw;

/// SHA-256 of the six bytes `pixels` the tests stage as the stored original.
pub const DIGEST: &str = "6ec9c2b0eb14010746c8bce8939303b382344b296206612eb8a907a37b2b2f37";

/// The OCFL object the sample entries cite.
pub const OBJECT: &str = "urn:uuid:5a1f1b2e-0000-4000-8000-000000000001";

/// The stored original the sample entries cite.
pub const STORED: &str = "original/5a1f1b2e-0000-4000-8000-000000000001.JPG";

/// The record that describes [`STORED`].
pub const RECORD: &str = "original/5a1f1b2e-0000-4000-8000-000000000001.metadata.md";

/// A host whose every fact is known.
pub fn host() -> Host {
    Host {
        os_name: Fact::value("macOS".to_string()),
        os_version: Fact::value("15.1".to_string()),
        os_build: Fact::value("24B83".to_string()),
        architecture: Fact::value("aarch64".to_string()),
        host_name: Fact::value("studio.local".to_string()),
        account: Fact::value("pieter".to_string()),
        filesquid_version: "0.1.0".to_string(),
        filesquid_build: "abc1234".to_string(),
        distribution: "filesquid".to_string(),
    }
}

/// A managed-import acquisition of `size` bytes.
pub fn acquisition(size: u64) -> Acquisition {
    Acquisition {
        recorded_at: "2026-10-20T10:00:02.250Z".to_string(),
        started_at: "2026-10-20T10:00:00Z".to_string(),
        finished_at: "2026-10-20T10:00:02Z".to_string(),
        method: ImportMethod::ApiUpload,
        storage: ByteStorage::Copied,
        job_id: Fact::value("job-1".to_string()),
        operation_id: Fact::value("op-1".to_string()),
        object_id: OBJECT.to_string(),
        object_version: "v1".to_string(),
        logical_path: Some(STORED.to_string()),
        sha256: DIGEST.to_string(),
        size,
    }
}

/// An upload-profile entry for `name` of 6 bytes.
pub fn entry_named(name: &[u8]) -> ImportEntry {
    ImportEntry::for_upload(UploadFacts {
        acquisition: acquisition(6),
        name: Raw::from_bytes(name),
        name_normalization: if std::str::from_utf8(name).is_ok() {
            NameForm::Nfc
        } else {
            NameForm::Unnormalised
        },
        reported_size: 6,
        last_modified: Some("2023-08-01T12:30:00Z".to_string()),
        media_type: Some("image/jpeg".to_string()),
        host: host(),
    })
}

/// The standard entry: `Holiday 2023.JPG`.
pub fn entry() -> ImportEntry {
    entry_named(b"Holiday 2023.JPG")
}

/// An entry whose every identity field is known and recognisable.
pub fn rich_entry() -> ImportEntry {
    let mut entry = entry();
    let s = &mut entry.source;
    s.absolute_path = Fact::value(Raw::from_bytes(b"/Users/pieter/Pictures/Holiday 2023.JPG"));
    s.import_root = Fact::value(Raw::from_bytes(b"/Users/pieter/Pictures"));
    s.relative_path = Fact::value(Raw::from_bytes(b"Holiday 2023.JPG"));
    s.owner_name = Fact::value(Raw::from_bytes(b"pieter"));
    s.owner_id = Fact::value(501);
    s.group_name = Fact::value(Raw::from_bytes(b"staff"));
    s.group_id = Fact::value(20);
    s.acl = Fact::value(vec![Raw::from_bytes(b"user:pieter allow read")]);
    s.link = Fact::value(Link::Symlink {
        target: Raw::from_bytes(b"/Volumes/Photos/original.jpg"),
    });
    entry.volume.name = Fact::value(Raw::from_bytes(b"Macintosh HD"));
    entry.volume.mount_point = Fact::value(Raw::from_bytes(b"/"));
    entry.volume.kind = Fact::value(VolumeKind::Local);
    let mut collector = AttributeCollector::new();
    collector.add(b"com.apple.quarantine", 5, b"0081;").unwrap();
    collector
        .add(b"com.apple.metadata:kMDItemWhereFroms", 4, b"http")
        .unwrap();
    collector
        .add(b"com.apple.metadata:kMDItemFinderComment", 4, b"memo")
        .unwrap();
    collector.add(b"user.keep", 3, b"abc").unwrap();
    entry.attributes = Fact::value(collector.finish());
    entry
}
