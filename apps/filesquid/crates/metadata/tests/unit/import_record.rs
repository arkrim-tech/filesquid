//! The import record's pure types (MD-0.8; MD-S-013, MD-S-008): exact names,
//! unavailable facts, size verification, attribute bounds, the append-only
//! rule, the read projection and the identity-field classes. No store, no
//! filesystem.

use filesquid_metadata::fact::{Fact, Reason};
use filesquid_metadata::file_record::{
    extension_of, record_name, stored_extension, FileRecord, RecordError,
};
use filesquid_metadata::import::attributes::{
    AttrValue, Attribute, AttributeCollector, Attributes, MAX_ATTRIBUTE_NAMES, MAX_INSPECTED_BYTES,
    MAX_STORED_VALUE_BYTES,
};
use filesquid_metadata::import::scrub::IdentityClass;
use filesquid_metadata::import::{ImportError, NameForm, SCHEMA};
use filesquid_metadata::raw::Raw;
use filesquid_metadata::record::Record;
use filesquid_metadata::MetadataNamespace;

#[path = "../support/import_rig.rs"]
mod rig;

use rig::{entry, entry_named};

/// A file record of the standard entry.
fn record() -> FileRecord {
    FileRecord::new(
        Raw::from_bytes(b"Holiday 2023.JPG"),
        NameForm::Nfc,
        Some("JPG".to_string()),
        Some("JPEG".to_string()),
        Some("image/jpeg".to_string()),
        entry(),
    )
    .expect("record")
}

/// An attribute entry of the standard host's collector.
fn attribute(name: &str, value: AttrValue, size: u64) -> Attribute {
    Attribute {
        name: Raw::from_bytes(name.as_bytes()),
        size,
        value,
        decoded: Default::default(),
    }
}

mod raw_values {
    use super::*;

    #[test]
    fn utf8_is_a_plain_string() {
        let raw = Raw::from_bytes("Café.txt".as_bytes());
        assert_eq!(serde_json::to_string(&raw).unwrap(), "\"Café.txt\"");
    }

    #[test]
    fn other_bytes_are_base64_marked_with_their_encoding() {
        let raw = Raw::from_bytes(&[0xFF, 0xFE, b'a']);
        let json = serde_json::to_string(&raw).unwrap();
        assert_eq!(json, r#"{"encoding":"base64","value":"//5h"}"#);
        let back: Raw = serde_json::from_str(&json).unwrap();
        assert_eq!(back.as_bytes(), &[0xFF, 0xFE, b'a']);
    }

    #[test]
    fn base64_follows_rfc_4648_vectors() {
        let vectors = [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
        ];
        for (bytes, encoded) in vectors {
            let wrapped = format!(r#"{{"encoding":"base64","value":"{encoded}"}}"#);
            let raw: Raw = serde_json::from_str(&wrapped).unwrap();
            assert_eq!(raw.as_bytes(), bytes, "{encoded}");
        }
    }

    #[test]
    fn invalid_base64_or_an_unknown_encoding_is_refused() {
        for bad in [
            r#"{"encoding":"base64","value":"Zg="}"#,
            r#"{"encoding":"base64","value":"Z$=="}"#,
            r#"{"encoding":"hex","value":"00"}"#,
            r#"{"value":"Zg=="}"#,
        ] {
            assert!(serde_json::from_str::<Raw>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn every_byte_value_round_trips() {
        let bytes: Vec<u8> = (0..=255).collect();
        let raw = Raw::from_bytes(&bytes);
        let back: Raw = serde_json::from_str(&serde_json::to_string(&raw).unwrap()).unwrap();
        assert_eq!(back.as_bytes(), bytes.as_slice());
    }
}

mod names {
    use super::*;

    #[test]
    fn the_extension_is_what_follows_the_last_dot() {
        assert_eq!(extension_of("Holiday 2023.JPG"), Some("JPG"));
        assert_eq!(extension_of("a.tar.gz"), Some("gz"));
        assert_eq!(extension_of(".bashrc"), None);
        assert_eq!(extension_of("README"), None);
        assert_eq!(extension_of("trailing."), None);
    }

    #[test]
    fn the_stored_extension_is_one_to_sixteen_ascii_letters_or_digits() {
        assert_eq!(stored_extension("Holiday 2023.JPG"), Some("JPG"));
        assert_eq!(
            stored_extension("a.0123456789abcdef"),
            Some("0123456789abcdef")
        );
        assert_eq!(stored_extension("a.0123456789abcdefg"), None);
        assert_eq!(stored_extension("a.j p"), None);
        assert_eq!(stored_extension("a.é"), None);
        assert_eq!(stored_extension(".hidden"), None);
    }

    #[test]
    fn the_record_name_is_the_file_uuid_plus_metadata_md() {
        assert_eq!(record_name("5a1f"), "5a1f.metadata.md");
    }
}

mod entry_validation {
    use super::*;

    #[test]
    fn the_upload_profile_is_valid_and_lists_what_an_upload_cannot_know() {
        let entry = entry();
        assert_eq!(entry.validate(), Ok(()));
        assert!(matches!(
            entry.source.absolute_path,
            Fact::Unavailable(ref u) if u.unavailable == Reason::NotProvidedByUpload
        ));
        assert!(matches!(entry.volume.uuid, Fact::Unavailable(_)));
        assert!(matches!(entry.attributes, Fact::Unavailable(_)));
        assert_eq!(entry.source.reported_size, Fact::value(6));
        assert_eq!(entry.source.verified_size, 6);
        assert_eq!(entry.source.extension.as_deref(), Some("JPG"));
        assert_eq!(
            entry.source.modified_time,
            Fact::value("2023-08-01T12:30:00Z".to_string())
        );
    }

    #[test]
    fn differing_reported_and_verified_sizes_are_rejected() {
        let mut entry = entry();
        entry.source.reported_size = Fact::value(7);
        assert_eq!(
            entry.validate(),
            Err(ImportError::SizeMismatch {
                reported: 7,
                verified: 6
            })
        );
    }

    #[test]
    fn an_unavailable_reported_size_is_allowed() {
        let mut entry = entry();
        entry.source.reported_size = Fact::unavailable(Reason::NotSupportedBySource);
        assert_eq!(entry.validate(), Ok(()));
    }

    #[test]
    fn the_cited_size_must_be_the_verified_size() {
        let mut entry = entry();
        entry.acquisition.size = 9;
        assert!(matches!(
            entry.validate(),
            Err(ImportError::CitedSizeMismatch { .. })
        ));
    }

    /// Setters for every time field of an entry.
    type Setter = fn(&mut filesquid_metadata::import::ImportEntry, String);

    /// Every time field an entry carries, by name.
    fn time_fields() -> Vec<(&'static str, Setter)> {
        vec![
            ("recorded_at", |e, t| e.acquisition.recorded_at = t),
            ("started_at", |e, t| e.acquisition.started_at = t),
            ("finished_at", |e, t| e.acquisition.finished_at = t),
            ("birth_time", |e, t| e.source.birth_time = Fact::value(t)),
            ("modified_time", |e, t| {
                e.source.modified_time = Fact::value(t)
            }),
            ("status_changed_time", |e, t| {
                e.source.status_changed_time = Fact::value(t)
            }),
            ("accessed_time", |e, t| {
                e.source.accessed_time = Fact::value(t)
            }),
            ("date_added_time", |e, t| {
                e.source.date_added_time = Fact::value(t)
            }),
        ]
    }

    #[test]
    fn every_time_field_must_be_rfc_3339_utc() {
        let bad = [
            "2026-10-20 10:00:00Z",
            "2026-10-20T10:00:00+02:00",
            "2026-10-20T10:00:00",
            "2026-13-20T10:00:00Z",
            "2026-02-30T10:00:00Z",
            "2026-10-20T24:00:00Z",
            "2026-10-20T10:00:60Z",
            "2026-10-20T10:00:00.Z",
            "yesterday",
        ];
        for (field, set) in time_fields() {
            for text in bad {
                let mut entry = entry();
                set(&mut entry, text.to_string());
                assert_eq!(
                    entry.validate(),
                    Err(ImportError::InvalidTimestamp { field }),
                    "{field} {text}"
                );
            }
        }
    }

    #[test]
    fn every_time_field_accepts_valid_timestamps() {
        let good = [
            "2024-02-29T00:00:00Z",
            "2026-10-20T10:00:00.123456789Z",
            "2026-10-20T10:00:00.1234567890123Z",
            "2016-12-31T23:59:60Z",
        ];
        for (field, set) in time_fields() {
            for text in good {
                let mut entry = entry();
                set(&mut entry, text.to_string());
                assert_eq!(entry.validate(), Ok(()), "{field} {text}");
            }
        }
    }

    #[test]
    fn the_cited_digest_is_64_lowercase_hex_digits() {
        for bad in ["", "abc", &"E".repeat(64), &"g".repeat(64)] {
            let mut entry = entry();
            entry.acquisition.sha256 = bad.to_string();
            assert_eq!(entry.validate(), Err(ImportError::InvalidDigest), "{bad}");
        }
    }

    #[test]
    fn a_managed_import_cites_its_logical_path_and_a_referenced_one_does_not() {
        let mut managed = entry();
        managed.acquisition.logical_path = None;
        assert_eq!(managed.validate(), Err(ImportError::MissingLogicalPath));
        let mut referenced = entry();
        referenced.acquisition.storage = filesquid_metadata::import::ByteStorage::Referenced;
        assert_eq!(
            referenced.validate(),
            Err(ImportError::UnexpectedLogicalPath)
        );
        referenced.acquisition.logical_path = None;
        assert_eq!(referenced.validate(), Ok(()));
    }

    #[test]
    fn json_round_trips_and_every_field_must_be_present() {
        let entry = entry();
        let json = serde_json::to_string(&entry).unwrap();
        let back: filesquid_metadata::import::ImportEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back, entry);
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value["host"].as_object_mut().unwrap().remove("account");
        assert!(serde_json::from_value::<filesquid_metadata::import::ImportEntry>(value).is_err());
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value["source"]["invented"] = serde_json::json!(1);
        assert!(serde_json::from_value::<filesquid_metadata::import::ImportEntry>(value).is_err());
    }

    #[test]
    fn an_unavailable_fact_serializes_with_its_reason() {
        let fact: Fact<u64> = Fact::unavailable_because(Reason::PermissionDenied, "EACCES");
        let json = serde_json::to_string(&fact).unwrap();
        assert_eq!(
            json,
            r#"{"unavailable":"permission_denied","detail":"EACCES"}"#
        );
        let back: Fact<u64> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, fact);
    }
}

mod attribute_bounds {
    use super::*;

    #[test]
    fn a_value_up_to_64_kib_is_stored_whole() {
        let mut collector = AttributeCollector::new();
        let value = vec![b'x'; MAX_STORED_VALUE_BYTES as usize];
        collector
            .add(b"user.small", value.len() as u64, &value)
            .unwrap();
        let attrs = collector.finish();
        assert!(matches!(attrs.entries[0].value, AttrValue::Full { .. }));
        assert_eq!(attrs.bytes_inspected, MAX_STORED_VALUE_BYTES);
        assert!(!attrs.truncated);
    }

    #[test]
    fn a_larger_value_is_kept_as_size_and_digest_only() {
        let mut collector = AttributeCollector::new();
        let value = vec![0u8; MAX_STORED_VALUE_BYTES as usize + 1];
        collector
            .add(b"user.big", value.len() as u64, &value)
            .unwrap();
        let attrs = collector.finish();
        let entry = &attrs.entries[0];
        assert_eq!(entry.size, MAX_STORED_VALUE_BYTES + 1);
        match &entry.value {
            AttrValue::Digest { sha256 } => assert_eq!(
                sha256,
                "3266304f31be278d06c3bd3eb9aa3e00c59bedec0a890de466568b0b90b0e01f"
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_value_cut_short_has_an_incomplete_digest() {
        let mut collector = AttributeCollector::new();
        collector.add(b"user.cut", 500, &[1, 2, 3]).unwrap();
        collector.add(b"user.unread", 9, &[]).unwrap();
        let attrs = collector.finish();
        assert!(attrs
            .entries
            .iter()
            .all(|a| matches!(a.value, AttrValue::Incomplete)));
        assert_eq!(attrs.entries[0].size, 500);
    }

    #[test]
    fn the_read_budget_is_one_mebibyte_across_the_entry() {
        let mut collector = AttributeCollector::new();
        let chunk = vec![7u8; 100 * 1024];
        for i in 0..10 {
            let name = format!("user.v{i}");
            collector
                .add(name.as_bytes(), chunk.len() as u64, &chunk)
                .unwrap();
        }
        assert_eq!(
            collector.read_budget(),
            MAX_INSPECTED_BYTES - 10 * 100 * 1024
        );
        let over = vec![0u8; collector.read_budget() as usize + 1];
        let err = collector.add(b"user.over", over.len() as u64, &over);
        assert_eq!(err, Err(ImportError::AttributeReadOverBudget));
        collector.add(b"user.last", 1_000_000, &[]).unwrap();
        assert_eq!(collector.finish().bytes_inspected, 10 * 100 * 1024);
    }

    #[test]
    fn the_257th_name_is_refused_and_the_entry_is_marked_truncated() {
        let mut collector = AttributeCollector::new();
        for i in 0..MAX_ATTRIBUTE_NAMES {
            collector
                .add(format!("user.n{i}").as_bytes(), 0, &[])
                .unwrap();
        }
        assert_eq!(collector.names_remaining(), 0);
        assert_eq!(
            collector.add(b"user.extra", 0, &[]),
            Err(ImportError::TooManyAttributeNames)
        );
        collector.note_names_enumerated(300);
        let attrs = collector.finish();
        assert_eq!(attrs.entries.len(), MAX_ATTRIBUTE_NAMES);
        assert_eq!(attrs.names_enumerated, 300);
        assert!(attrs.truncated);
    }

    #[test]
    fn a_non_utf8_name_and_value_are_stored_as_base64() {
        let mut collector = AttributeCollector::new();
        collector.add(&[0xC3, 0x28], 2, &[0xFF, 0x00]).unwrap();
        let attrs = collector.finish();
        let json = serde_json::to_string(&attrs).unwrap();
        assert!(json.contains(r#""encoding":"base64""#), "{json}");
        let back: Attributes = serde_json::from_str(&json).unwrap();
        assert_eq!(back.entries[0].name.as_bytes(), &[0xC3, 0x28]);
        match &back.entries[0].value {
            AttrValue::Full { value } => assert_eq!(value.as_bytes(), &[0xFF, 0x00]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_hand_built_entry_over_a_bound_is_rejected() {
        let mut entry = entry();
        let many: Vec<Attribute> = (0..=MAX_ATTRIBUTE_NAMES)
            .map(|i| attribute(&format!("n{i}"), AttrValue::Incomplete, 0))
            .collect();
        entry.attributes = Fact::value(Attributes {
            entries: many,
            names_enumerated: 257,
            truncated: true,
            bytes_inspected: 0,
        });
        assert!(matches!(
            entry.validate(),
            Err(ImportError::TooManyAttributeNames)
        ));

        let mut entry = super::entry();
        entry.attributes = Fact::value(Attributes {
            entries: vec![],
            names_enumerated: 0,
            truncated: false,
            bytes_inspected: MAX_INSPECTED_BYTES + 1,
        });
        assert_eq!(entry.validate(), Err(ImportError::AttributeReadOverBudget));

        let mut entry = super::entry();
        let big = Raw::from_bytes(&vec![0u8; MAX_STORED_VALUE_BYTES as usize + 1]);
        entry.attributes = Fact::value(Attributes {
            entries: vec![attribute(
                "big",
                AttrValue::Full { value: big },
                MAX_STORED_VALUE_BYTES + 1,
            )],
            names_enumerated: 1,
            truncated: false,
            bytes_inspected: MAX_STORED_VALUE_BYTES + 1,
        });
        assert_eq!(entry.validate(), Err(ImportError::AttributeValueTooLarge));
    }

    #[test]
    fn dropping_names_without_marking_truncation_is_rejected() {
        let mut entry = entry();
        entry.attributes = Fact::value(Attributes {
            entries: vec![attribute("a", AttrValue::Incomplete, 1)],
            names_enumerated: 5,
            truncated: false,
            bytes_inspected: 0,
        });
        assert_eq!(entry.validate(), Err(ImportError::TruncationNotMarked));
    }
}

mod file_record {
    use super::*;

    #[test]
    fn a_new_record_starts_with_the_original_name_as_its_display_name() {
        let record = record();
        assert_eq!(record.original_name().as_bytes(), b"Holiday 2023.JPG");
        assert_eq!(record.display_name(), "Holiday 2023.JPG");
        assert_eq!(record.extension(), Some("JPG"));
        assert_eq!(record.format(), Some("JPEG"));
        assert_eq!(record.media_type(), Some("image/jpeg"));
        assert_eq!(record.imports().len(), 1);
    }

    #[test]
    fn the_markdown_opens_with_the_schema_and_the_names() {
        let text = record().to_markdown();
        let head: Vec<&str> = text.lines().take(8).collect();
        assert_eq!(head[0], "---");
        assert_eq!(head[1], format!("schema: {SCHEMA}"));
        assert_eq!(head[2], r#"original_name: "Holiday 2023.JPG""#);
        assert_eq!(head[3], r#"name_normalization: "nfc""#);
        assert_eq!(head[4], r#"display_name: "Holiday 2023.JPG""#);
        assert_eq!(head[5], r#"extension: "JPG""#);
        assert_eq!(head[6], r#"format: "JPEG""#);
        assert_eq!(head[7], r#"media_type: "image/jpeg""#);
        assert!(text.contains("\nimport:\n  - {"));
    }

    #[test]
    fn markdown_round_trips_statements_body_and_imports() {
        let mut original = record();
        original
            .statements_mut()
            .set(MetadataNamespace::Dcterms, "title", "Harbour: dawn")
            .unwrap();
        original.statements_mut().body = "Long notes.".to_string();
        let text = original.to_markdown();
        let back = FileRecord::from_markdown(&text).unwrap();
        assert_eq!(back, original);
        assert_eq!(back.to_markdown(), text);
    }

    #[test]
    fn a_non_utf8_original_name_round_trips_exactly() {
        let name = [b'a', 0xFF, b'.', b'j', b'p', b'g'];
        let record = FileRecord::new(
            Raw::from_bytes(&name),
            NameForm::Unnormalised,
            Some("jpg".to_string()),
            None,
            None,
            entry_named(&name),
        )
        .unwrap();
        let text = record.to_markdown();
        assert!(
            text.contains(r#"original_name: {"encoding":"base64""#),
            "{text}"
        );
        let back = FileRecord::from_markdown(&text).unwrap();
        assert_eq!(back.original_name().as_bytes(), &name);
        assert_eq!(back.format(), None);
        assert_eq!(back.media_type(), None);
    }

    #[test]
    fn a_wrong_or_missing_schema_is_refused() {
        let text = record().to_markdown();
        let wrong = text.replacen(SCHEMA, "filesquid.original.v9", 1);
        assert!(matches!(
            FileRecord::from_markdown(&wrong),
            Err(RecordError::UnsupportedSchema)
        ));
        assert!(matches!(
            FileRecord::from_markdown("---\ndcterms:\n  title: x\n---\n"),
            Err(RecordError::UnsupportedSchema)
        ));
        assert!(matches!(
            FileRecord::from_markdown("no front matter"),
            Err(RecordError::UnsupportedSchema)
        ));
    }

    #[test]
    fn a_missing_header_field_or_a_broken_entry_is_refused() {
        let text = record().to_markdown();
        let no_name = text.replace("original_name:", "orig_name:");
        assert!(matches!(
            FileRecord::from_markdown(&no_name),
            Err(RecordError::MissingField("original_name"))
        ));
        let broken = text.replace("\n  - {", "\n  - {broken");
        assert!(matches!(
            FileRecord::from_markdown(&broken),
            Err(RecordError::Malformed(_))
        ));
    }

    #[test]
    fn names_and_types_are_validated() {
        let new = |name: &[u8], ext: Option<&str>, mime: Option<&str>| {
            FileRecord::new(
                Raw::from_bytes(name),
                NameForm::Nfc,
                ext.map(str::to_string),
                None,
                mime.map(str::to_string),
                entry(),
            )
        };
        assert!(matches!(
            new(b"", None, None),
            Err(RecordError::InvalidName)
        ));
        assert!(matches!(
            new(b"a/b", None, None),
            Err(RecordError::InvalidName)
        ));
        assert!(matches!(
            new(b"a\0b", None, None),
            Err(RecordError::InvalidName)
        ));
        let long = vec![b'a'; 1025];
        assert!(matches!(
            new(&long, None, None),
            Err(RecordError::InvalidName)
        ));
        let longest = vec![b'a'; 1024];
        assert!(new(&longest, None, None).is_ok());
        assert!(matches!(
            new(b"a", Some("j/g"), None),
            Err(RecordError::InvalidExtension)
        ));
        assert!(matches!(
            new(b"a", Some(""), None),
            Err(RecordError::InvalidExtension)
        ));
        for bad in ["image", "image/", "/jpeg", "image/jp eg", "a/b/c", "-x/y"] {
            assert!(
                matches!(
                    new(b"a", None, Some(bad)),
                    Err(RecordError::InvalidMediaType)
                ),
                "{bad}"
            );
        }
        assert!(new(b"a", None, Some("application/vnd.api+json")).is_ok());
    }

    #[test]
    fn a_record_without_an_import_entry_is_invalid() {
        let mut text = record().to_markdown();
        let at = text.find("import:").unwrap();
        text.replace_range(at..text.rfind("---").unwrap(), "");
        assert!(matches!(
            FileRecord::from_markdown(&text),
            Err(RecordError::NoImport)
        ));
    }

    #[test]
    fn a_rename_changes_only_the_display_name() {
        let mut record = record();
        record.set_display_name("Harbour at dawn.jpg").unwrap();
        assert_eq!(record.display_name(), "Harbour at dawn.jpg");
        assert_eq!(record.original_name().as_bytes(), b"Holiday 2023.JPG");
        assert!(matches!(
            record.set_display_name("a/b"),
            Err(RecordError::InvalidDisplayName)
        ));
        assert!(matches!(
            record.set_display_name(""),
            Err(RecordError::InvalidDisplayName)
        ));
    }

    #[test]
    fn a_re_import_appends_and_leaves_earlier_entries_unchanged() {
        let base = record();
        let mut next = base.clone();
        let mut again = entry();
        again.acquisition.job_id = Fact::value("job-2".to_string());
        next.append_import(again).unwrap();
        assert_eq!(next.imports().len(), 2);
        assert_eq!(next.imports()[0], base.imports()[0]);
        assert_eq!(next.check_extends(&base), Ok(()));
    }

    #[test]
    fn a_save_that_changes_an_import_entry_is_refused() {
        let base = record();
        let mut text = base.to_markdown();
        text = text.replace("studio.local", "other.local");
        let changed = FileRecord::from_markdown(&text).unwrap();
        assert_eq!(
            changed.check_extends(&base),
            Err(RecordError::ImportEntryChanged { index: 0 })
        );
    }

    #[test]
    fn a_save_that_removes_an_import_entry_is_refused() {
        let mut base = record();
        base.append_import(entry()).unwrap();
        let shorter = record();
        assert_eq!(
            shorter.check_extends(&base),
            Err(RecordError::ImportEntryRemoved)
        );
    }

    #[test]
    fn a_save_that_changes_the_original_name_is_refused() {
        let base = record();
        let mut text = base.to_markdown();
        text = text.replacen(
            r#"original_name: "Holiday 2023.JPG""#,
            r#"original_name: "Other.JPG""#,
            1,
        );
        let changed = FileRecord::from_markdown(&text).unwrap();
        assert_eq!(
            changed.check_extends(&base),
            Err(RecordError::OriginalNameChanged)
        );
    }

    #[test]
    fn statements_and_the_display_name_may_change() {
        let base = record();
        let mut next = base.clone();
        next.set_display_name("Renamed.jpg").unwrap();
        next.statements_mut()
            .set(MetadataNamespace::Dcterms, "title", "Harbour")
            .unwrap();
        assert_eq!(next.check_extends(&base), Ok(()));
    }

    #[test]
    fn with_statements_keeps_the_header_and_the_imports() {
        let base = record();
        let mut replacement = Record::default();
        replacement
            .set(MetadataNamespace::Dcterms, "title", "Dawn")
            .unwrap();
        let next = base.clone().with_statements(replacement);
        assert_eq!(next.imports(), base.imports());
        assert_eq!(next.original_name(), base.original_name());
        assert_eq!(
            next.statements().get("dcterms:title").unwrap().value,
            "Dawn"
        );
    }
}

mod projection_and_scrub {
    use super::*;
    /// An entry whose every identity field is known and recognisable.
    fn rich() -> filesquid_metadata::import::ImportEntry {
        rig::rich_entry()
    }

    #[test]
    fn the_projection_names_the_file_and_the_import_but_no_location_or_account() {
        let mut record = record();
        record.append_import(rich()).unwrap();
        let facts = record.facts();
        let json = serde_json::to_string(&facts).unwrap();
        for secret in [
            "/Users/pieter",
            "/Volumes/Photos",
            "studio.local",
            "pieter",
            "501",
            "staff",
            "com.apple.quarantine",
            "import_root",
            "absolute_path",
        ] {
            assert!(!json.contains(secret), "{secret} leaked: {json}");
        }
        for wanted in [
            "Holiday 2023.JPG",
            "JPG",
            "JPEG",
            "image/jpeg",
            "Macintosh HD",
            rig::OBJECT,
            "v1",
            rig::DIGEST,
            "2026-10-20T10:00:02Z",
            "api_upload",
        ] {
            assert!(json.contains(wanted), "{wanted} missing: {json}");
        }
        assert_eq!(facts.imports.len(), 2);
        assert_eq!(facts.size, 6);
        assert_eq!(facts.imports[0].relative_path, None);
        assert_eq!(facts.imports[0].volume_name, None);
        let rich = &facts.imports[1];
        assert_eq!(
            rich.relative_path.as_ref().map(Raw::as_bytes),
            Some(&b"Holiday 2023.JPG"[..])
        );
        assert_eq!(
            rich.volume_name.as_ref().map(Raw::as_bytes),
            Some(&b"Macintosh HD"[..])
        );
    }

    #[test]
    fn scrubbing_a_class_withholds_exactly_its_fields() {
        let scrubbed = rich().scrubbed(&[IdentityClass::Path]);
        for fact in [
            &scrubbed.source.absolute_path,
            &scrubbed.source.import_root,
            &scrubbed.source.relative_path,
            &scrubbed.volume.mount_point,
        ] {
            assert!(
                matches!(fact, Fact::Unavailable(u) if u.unavailable == Reason::WithheldByPolicy)
            );
        }
        assert!(matches!(scrubbed.source.link, Fact::Unavailable(_)));
        assert_eq!(scrubbed.source.owner_name, rich().source.owner_name);
        assert_eq!(scrubbed.host, rich().host);
        assert_eq!(scrubbed.validate(), Ok(()));
    }

    #[test]
    fn each_identity_class_reaches_its_own_fields() {
        let all = rich();
        let account = all.scrubbed(&[IdentityClass::Account]);
        assert!(matches!(account.source.owner_name, Fact::Unavailable(_)));
        assert!(matches!(account.source.group_name, Fact::Unavailable(_)));
        assert!(matches!(account.source.acl, Fact::Unavailable(_)));
        assert!(matches!(account.host.account, Fact::Unavailable(_)));
        assert_eq!(account.source.owner_id, all.source.owner_id);

        let host = all.scrubbed(&[IdentityClass::Host]);
        assert!(matches!(host.host.host_name, Fact::Unavailable(_)));
        assert_eq!(host.host.account, all.host.account);

        let ids = all.scrubbed(&[IdentityClass::NumericId]);
        assert!(matches!(ids.source.owner_id, Fact::Unavailable(_)));
        assert!(matches!(ids.source.group_id, Fact::Unavailable(_)));
    }

    #[test]
    fn known_attributes_are_withheld_by_their_class_and_keep_name_and_size() {
        let all = rich();
        let scrubbed = all.scrubbed(&[
            IdentityClass::OriginUrl,
            IdentityClass::QuarantineAgent,
            IdentityClass::FinderComment,
        ]);
        let Fact::Value(attrs) = &scrubbed.attributes else {
            panic!("attributes");
        };
        let withheld = attrs
            .entries
            .iter()
            .filter(|a| matches!(a.value, AttrValue::Withheld))
            .count();
        assert_eq!(withheld, 3);
        let keep = attrs
            .entries
            .iter()
            .find(|a| a.name.as_bytes() == b"user.keep")
            .unwrap();
        assert!(matches!(keep.value, AttrValue::Full { .. }));
        assert_eq!(attrs.entries[0].size, 5);
        assert_eq!(scrubbed.validate(), Ok(()));
    }

    #[test]
    fn scrubbing_every_class_leaves_no_identity_value() {
        let scrubbed = rich().scrubbed(&IdentityClass::ALL);
        let json = serde_json::to_string(&scrubbed).unwrap();
        for secret in [
            "/Users/pieter",
            "pieter",
            "studio.local",
            "staff",
            "0081;",
            "http",
            "memo",
        ] {
            assert!(!json.contains(secret), "{secret} left: {json}");
        }
        assert!(json.contains("withheld_by_policy"));
    }
}

mod review_fixes {
    use super::*;
    use filesquid_metadata::file_record::safe_display_name;
    use filesquid_metadata::import::ContentType;

    #[test]
    fn an_extension_unrelated_to_the_original_name_is_refused() {
        let new = |name: &[u8], ext: Option<&str>| {
            FileRecord::new(
                Raw::from_bytes(name),
                NameForm::Nfc,
                ext.map(str::to_string),
                None,
                None,
                entry(),
            )
        };
        assert!(matches!(
            new(b"photo.jpg", Some("png")),
            Err(RecordError::InvalidExtension)
        ));
        assert!(matches!(
            new(b"photo.jpg", None),
            Err(RecordError::InvalidExtension)
        ));
        assert!(matches!(
            new(b"photo", Some("jpg")),
            Err(RecordError::InvalidExtension)
        ));
        assert!(new(b"photo.jpg", Some("jpg")).is_ok());
        assert!(new(b".bashrc", None).is_ok());
    }

    #[test]
    fn a_line_after_import_that_is_not_an_entry_is_refused_not_dropped() {
        let mut text = record().to_markdown();
        let close = text.rfind("---").unwrap();
        text.insert_str(close, "   - {\"misindented\": true}\n");
        assert!(matches!(
            FileRecord::from_markdown(&text),
            Err(RecordError::Malformed(_))
        ));
    }

    #[test]
    fn schema_org_statements_do_not_collide_with_the_record_schema_key() {
        let mut record = record();
        record
            .statements_mut()
            .set(MetadataNamespace::SchemaOrg, "certification", "ISO")
            .unwrap();
        let text = record.to_markdown();
        assert!(!text.lines().any(|l| l == "schema:"), "{text}");
        assert_eq!(
            text.lines().filter(|l| l.starts_with("schema: ")).count(),
            1
        );
        assert!(text.contains("\nschemaorg:\n  certification: ISO"));
        let back = FileRecord::from_markdown(&text).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn a_raw_envelope_with_a_duplicate_field_is_refused() {
        for bad in [
            r#"{"encoding":"base64","value":"Zg==","value":"Zm8="}"#,
            r#"{"encoding":"base64","encoding":"base64","value":"Zg=="}"#,
        ] {
            assert!(serde_json::from_str::<Raw>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn stored_whole_values_cannot_exceed_the_bytes_the_entry_says_it_read() {
        let mut entry = entry();
        entry.attributes = Fact::value(Attributes {
            entries: vec![attribute(
                "a",
                AttrValue::Full {
                    value: Raw::from_bytes(&[1, 2, 3]),
                },
                3,
            )],
            names_enumerated: 1,
            truncated: false,
            bytes_inspected: 2,
        });
        assert_eq!(
            entry.validate(),
            Err(ImportError::AttributeBytesUnderReported)
        );
    }

    #[test]
    fn each_acquisition_gets_its_own_attribute_budget() {
        let mut first = AttributeCollector::new();
        let big = vec![0u8; first.read_budget() as usize];
        first.add(b"user.a", big.len() as u64, &big).unwrap();
        assert_eq!(first.read_budget(), 0);
        let second = AttributeCollector::new();
        assert_eq!(second.read_budget(), MAX_INSPECTED_BYTES);
        assert_eq!(second.names_remaining(), MAX_ATTRIBUTE_NAMES);
    }

    #[test]
    fn a_managed_entry_cites_a_valid_logical_path() {
        for bad in ["/abs/x.jpg", "../x.jpg", "a//b", "a\\b", ""] {
            let mut entry = entry();
            entry.acquisition.logical_path = Some(bad.to_string());
            assert_eq!(
                entry.validate(),
                Err(ImportError::InvalidLogicalPath),
                "{bad}"
            );
        }
    }

    #[test]
    fn an_upload_keeps_a_utf8_extension_of_a_non_utf8_name() {
        let entry = entry_named(&[b'a', 0xFF, b'.', b'j', b'p', b'g']);
        assert_eq!(entry.source.extension.as_deref(), Some("jpg"));
        let entry = entry_named(&[b'a', b'.', 0xFF]);
        assert_eq!(entry.source.extension, None);
    }

    #[test]
    fn an_upload_lists_the_missing_uti_as_unavailable_with_a_reason() {
        let entry = entry();
        let Fact::Value(ContentType { uti, mime }) = &entry.source.os_content_type else {
            panic!("content type");
        };
        assert!(
            matches!(uti, Fact::Unavailable(u) if u.unavailable == Reason::NotProvidedByUpload)
        );
        assert_eq!(mime, &Fact::value("image/jpeg".to_string()));
    }

    #[test]
    fn the_display_name_is_transformed_once_for_the_projection() {
        assert_eq!(safe_display_name("a\u{0007}b/c\\d"), "ab_c_d");
        assert_eq!(safe_display_name("  spaced  "), "spaced");
        assert_eq!(safe_display_name("   "), "file");
        assert_eq!(safe_display_name("\n\t"), "file");
        let long = "é".repeat(600);
        assert_eq!(safe_display_name(&long).len(), 1024);
        let mut record = record();
        record.set_display_name("a\u{0001}b\\c.jpg").unwrap();
        assert_eq!(record.facts().display_name, "ab_c.jpg");
    }
}

mod review_fixes_two {
    use super::*;

    #[test]
    fn a_whole_value_must_be_its_declared_size_and_a_digest_must_be_sha256_hex() {
        let mut entry = entry();
        entry.attributes = Fact::value(Attributes {
            entries: vec![attribute(
                "a",
                AttrValue::Full {
                    value: Raw::from_bytes(&[1, 2, 3]),
                },
                5,
            )],
            names_enumerated: 1,
            truncated: false,
            bytes_inspected: 3,
        });
        assert_eq!(
            entry.validate(),
            Err(ImportError::AttributeValueInconsistent)
        );
        for bad in ["", "xyz", &"A".repeat(64)] {
            let mut entry = super::entry();
            entry.attributes = Fact::value(Attributes {
                entries: vec![attribute(
                    "a",
                    AttrValue::Digest {
                        sha256: bad.to_string(),
                    },
                    70_000,
                )],
                names_enumerated: 1,
                truncated: false,
                bytes_inspected: 0,
            });
            assert_eq!(
                entry.validate(),
                Err(ImportError::AttributeValueInconsistent),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_refused_257th_name_still_marks_the_entry_truncated() {
        let mut collector = AttributeCollector::new();
        for i in 0..MAX_ATTRIBUTE_NAMES {
            collector
                .add(format!("user.n{i}").as_bytes(), 0, &[])
                .unwrap();
        }
        assert!(collector.add(b"user.extra", 0, &[]).is_err());
        let attrs = collector.finish();
        assert!(attrs.truncated);
        assert_eq!(attrs.names_enumerated, 257);
        assert_eq!(attrs.validate(), Ok(()));
    }
}

mod review_fixes_three {
    use super::*;

    #[test]
    fn a_digest_is_only_kept_for_a_value_over_64_kib() {
        let mut entry = entry();
        entry.attributes = Fact::value(Attributes {
            entries: vec![attribute(
                "small",
                AttrValue::Digest {
                    sha256: "a".repeat(64),
                },
                MAX_STORED_VALUE_BYTES,
            )],
            names_enumerated: 1,
            truncated: false,
            bytes_inspected: 0,
        });
        assert_eq!(
            entry.validate(),
            Err(ImportError::AttributeValueInconsistent)
        );
    }
}

/// MD-0.10: the `OriginalName` identity class (MD-S-013, ADR 038 Decision 6).
mod original_name_class {
    use super::*;

    /// The unavailable fact a withheld name is written as.
    const WITHHELD: &str = r#"{"unavailable":"withheld_by_policy"}"#;

    /// The standard record with a second entry from a full set of facts.
    fn two_entry_record() -> FileRecord {
        let mut record = record();
        let mut again = rig::rich_entry();
        again.acquisition.object_version = "v2".to_string();
        record.append_import(again).unwrap();
        record
    }

    /// Whether `fact` is withheld by policy.
    fn is_withheld<T>(fact: &Fact<T>) -> bool {
        matches!(fact, Fact::Unavailable(u) if u.unavailable == Reason::WithheldByPolicy)
    }

    #[test]
    fn every_class_includes_the_original_name() {
        assert!(IdentityClass::ALL.contains(&IdentityClass::OriginalName));
        assert_eq!(IdentityClass::ALL.len(), 8);
    }

    #[test]
    fn an_entry_scrub_withholds_the_source_name() {
        let scrubbed = entry().scrubbed(&[IdentityClass::OriginalName]);
        assert!(scrubbed.source.name_withheld());
        assert_eq!(scrubbed.source.absolute_path, entry().source.absolute_path);
        assert_eq!(scrubbed.validate(), Ok(()));
        let json = serde_json::to_string(&scrubbed).unwrap();
        assert!(!json.contains("Holiday 2023"), "{json}");
    }

    #[test]
    fn a_record_scrub_withholds_the_header_names_and_every_source_name() {
        let scrubbed = two_entry_record().scrubbed(&[IdentityClass::OriginalName]);
        assert!(scrubbed.names_withheld());
        for entry in scrubbed.imports() {
            assert!(entry.source.name_withheld());
            let json = serde_json::to_string(&entry.source).unwrap();
            assert!(
                json.contains(&format!(r#""original_name":{WITHHELD}"#)),
                "{json}"
            );
        }
        let text = scrubbed.to_markdown();
        let head: Vec<&str> = text.lines().take(6).collect();
        assert_eq!(head[2], format!("original_name: {WITHHELD}"));
        assert_eq!(head[4], format!("display_name: {WITHHELD}"));
        assert_eq!(head[5], r#"extension: "JPG""#);
        // The paths still name the file: they are the `Path` class.
        assert!(
            !text.contains(r#""original_name":"Holiday 2023.JPG""#),
            "{text}"
        );
        assert!(text.contains(r#""relative_path":"Holiday 2023.JPG""#));
    }

    #[test]
    fn a_renamed_display_name_is_withheld_too() {
        let mut record = record();
        record.set_display_name("Harbour at dawn.JPG").unwrap();
        let text = record
            .scrubbed(&[IdentityClass::OriginalName])
            .to_markdown();
        assert!(!text.contains("Harbour at dawn"), "{text}");
        assert!(!text.contains("Holiday 2023"), "{text}");
    }

    #[test]
    fn a_record_scrub_applies_the_other_classes_to_every_entry() {
        let scrubbed = two_entry_record().scrubbed(&IdentityClass::ALL);
        let rich = &scrubbed.imports()[1];
        assert!(is_withheld(&rich.source.absolute_path));
        assert!(is_withheld(&rich.host.host_name));
        let text = scrubbed.to_markdown();
        for secret in ["/Users/pieter", "studio.local", "Holiday 2023", "pieter"] {
            assert!(!text.contains(secret), "{secret} left: {text}");
        }
        assert_eq!(scrubbed.statements(), two_entry_record().statements());
    }

    #[test]
    fn a_scrubbed_record_reads_back_as_scrubbed() {
        let scrubbed = two_entry_record().scrubbed(&IdentityClass::ALL);
        let back = FileRecord::from_markdown(&scrubbed.to_markdown()).expect("read back");
        assert_eq!(back, scrubbed);
        assert!(back.names_withheld());
    }

    #[test]
    fn a_scrub_without_the_original_name_keeps_every_name() {
        let record = two_entry_record();
        let others: Vec<IdentityClass> = IdentityClass::ALL
            .into_iter()
            .filter(|c| *c != IdentityClass::OriginalName)
            .collect();
        let scrubbed = record.scrubbed(&others);
        assert!(!scrubbed.names_withheld());
        assert_eq!(scrubbed.original_name(), record.original_name());
        assert_eq!(scrubbed.display_name(), record.display_name());
        for (kept, base) in scrubbed.imports().iter().zip(record.imports()) {
            assert_eq!(kept.source.original_name, base.source.original_name);
        }
        let text = scrubbed.to_markdown();
        assert!(text.contains(r#"original_name: "Holiday 2023.JPG""#));
        assert!(text.contains(r#"display_name: "Holiday 2023.JPG""#));
    }

    #[test]
    fn a_source_name_is_known_and_never_empty_or_unavailable_for_another_reason() {
        let json = serde_json::to_string(&entry()).unwrap();
        let named = r#""original_name":"Holiday 2023.JPG""#;
        assert!(json.contains(named));
        let empty = json.replace(named, r#""original_name":"""#);
        assert!(serde_json::from_str::<filesquid_metadata::import::ImportEntry>(&empty).is_err());
        let other = json.replace(
            named,
            r#""original_name":{"unavailable":"not_supported_by_source"}"#,
        );
        assert!(serde_json::from_str::<filesquid_metadata::import::ImportEntry>(&other).is_err());
        let withheld = json.replace(named, &format!(r#""original_name":{WITHHELD}"#));
        let back: filesquid_metadata::import::ImportEntry =
            serde_json::from_str(&withheld).unwrap();
        assert!(back.source.name_withheld());
        assert!(!entry().source.name_withheld());
    }

    #[test]
    fn a_scrubbed_record_cannot_be_renamed() {
        let mut scrubbed = record().scrubbed(&[IdentityClass::OriginalName]);
        assert_eq!(
            scrubbed.set_display_name("Other.JPG"),
            Err(RecordError::NamesWithheld)
        );
    }

    #[test]
    fn a_record_with_only_one_header_name_withheld_is_malformed() {
        let text = record().to_markdown().replacen(
            r#"original_name: "Holiday 2023.JPG""#,
            &format!("original_name: {WITHHELD}"),
            1,
        );
        assert!(matches!(
            FileRecord::from_markdown(&text),
            Err(RecordError::Malformed(_))
        ));
    }

    #[test]
    fn a_header_name_unavailable_for_another_reason_is_malformed() {
        let other = r#"{"unavailable":"not_supported_by_source"}"#;
        let text = record()
            .to_markdown()
            .replacen(
                r#"original_name: "Holiday 2023.JPG""#,
                &format!("original_name: {other}"),
                1,
            )
            .replacen(
                r#"display_name: "Holiday 2023.JPG""#,
                &format!("display_name: {other}"),
                1,
            );
        assert!(matches!(
            FileRecord::from_markdown(&text),
            Err(RecordError::Malformed(_))
        ));
    }
}
