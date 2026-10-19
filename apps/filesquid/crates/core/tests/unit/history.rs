//! PROV-O history contract types (CO-0.11; CORE-S-010, CORE-S-008; PRD
//! F-067; ADR 040 Decision 10).

use filesquid_core::{
    HistoryEntry, InstallationId, JournalLink, OperationId, OperationKind, OperationMessage,
    OperationMessageError, ProvOSidecar, Rfc3339Utc, VersionRef, OPERATION_MESSAGE_PREFIX,
    PROV_O_CONTEXT, PROV_O_PROFILE, PROV_O_PROFILE_VERSION,
};

/// A canonical UUID v4.
const UUID: &str = "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b";

fn op(id: &str) -> OperationId {
    OperationId::new(id).expect("operation id")
}

fn time(text: &str) -> Rfc3339Utc {
    Rfc3339Utc::parse(text).expect("time")
}

#[test]
fn prov_context_is_the_prov_namespace_iri() {
    assert_eq!(PROV_O_CONTEXT, "http://www.w3.org/ns/prov#");
    assert_eq!(PROV_O_PROFILE_VERSION, 1);
    assert_eq!(PROV_O_PROFILE, "filesquid-prov/1");
}

#[test]
fn every_operation_kind_has_a_stable_token() {
    let expected = [
        (OperationKind::Import, "import"),
        (OperationKind::Upload, "upload"),
        (OperationKind::Copy, "copy"),
        (OperationKind::Rename, "rename"),
        (OperationKind::Move, "move"),
        (OperationKind::Hide, "hide"),
        (OperationKind::Unhide, "unhide"),
        (OperationKind::Delete, "delete"),
        (OperationKind::Restore, "restore"),
        (OperationKind::Purge, "purge"),
        (OperationKind::MetadataEdit, "metadata.edit"),
        (OperationKind::Label, "label"),
        (OperationKind::Revert, "revert"),
        (OperationKind::Reinstate, "reinstate"),
        (OperationKind::Snapshot, "snapshot"),
        (OperationKind::CatalogOrganisation, "catalog"),
        (OperationKind::Repair, "repair"),
        (OperationKind::Export, "export"),
        (OperationKind::Scrub, "scrub"),
        (OperationKind::Backfill, "backfill"),
        (OperationKind::Unknown, "unknown"),
    ];
    assert_eq!(OperationKind::ALL.len(), expected.len());
    for (kind, token) in expected {
        assert_eq!(kind.as_str(), token);
        assert_eq!(OperationKind::parse(token), Some(kind));
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{token}\""));
    }
    assert_eq!(OperationKind::parse("mods"), None);
}

#[test]
fn an_operation_message_formats_and_parses_back() {
    let message = OperationMessage::new(op("op-1"), OperationKind::Rename)
        .with_revision(7)
        .with_label("Before \"cleanup\" \\ n\u{e9}");
    let text = message.format();
    assert!(text.starts_with(OPERATION_MESSAGE_PREFIX));
    assert_eq!(
        text,
        "filesquid.operation.v1 {\"operation_id\":\"op-1\",\"kind\":\"rename\",\
         \"revision\":\"7\",\"label\":\"Before \\\"cleanup\\\" \\\\ n\u{e9}\"}"
    );
    assert_eq!(OperationMessage::parse(&text), Ok(Some(message)));
}

#[test]
fn optional_members_are_left_out() {
    let message = OperationMessage::new(op("op-2"), OperationKind::Import);
    let text = message.format();
    assert_eq!(
        text,
        "filesquid.operation.v1 {\"operation_id\":\"op-2\",\"kind\":\"import\"}"
    );
    assert_eq!(OperationMessage::parse(&text), Ok(Some(message)));
}

#[test]
fn a_message_without_the_prefix_is_not_a_filesquid_message() {
    assert_eq!(OperationMessage::parse("Edit metadata.md"), Ok(None));
    assert_eq!(OperationMessage::parse(""), Ok(None));
}

#[test]
fn an_unreadable_prefixed_message_is_refused_not_dropped() {
    let cases = [
        "filesquid.operation.v1 ",
        "filesquid.operation.v1 {",
        "filesquid.operation.v1 {\"kind\":\"import\"}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"teleport\"}",
        "filesquid.operation.v1 {\"operation_id\":\"\",\"kind\":\"import\"}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"import\",\"kind\":\"move\"}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"import\",\"extra\":1}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"import\"} trailing",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"import\",\"revision\":-1}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"import\",\"revision\":\"01\"}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"import\",\"revision\":\"x\"}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"deletion\":{\"deleted_at\":\"t\"}}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"deletion\":{\"a\":{\"b\":\"c\"}}}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"created_id\":7}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"deletion\":{\"deleted_at\":\"yesterday\",\"purge_after\":\"2026-11-18T00:00:00Z\"}}",
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"import\",\"label\":\"\\q\"}",
        "filesquid.operation.v1 [1]",
    ];
    for text in cases {
        assert!(
            matches!(
                OperationMessage::parse(text),
                Err(OperationMessageError::Unreadable { .. })
            ),
            "{text}"
        );
    }
}

#[test]
fn an_oversized_message_is_refused() {
    let label = "x".repeat(5000);
    let text = format!(
        "filesquid.operation.v1 {{\"operation_id\":\"op\",\"kind\":\"label\",\"label\":\"{label}\"}}"
    );
    assert_eq!(
        OperationMessage::parse(&text),
        Err(OperationMessageError::TooLarge)
    );
}

#[test]
fn unicode_escapes_and_surrogate_pairs_are_decoded() {
    let text = "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"label\",\
                \"label\":\"\\u00e9\\ud83d\\ude00\\n\"}";
    let message = OperationMessage::parse(text).unwrap().unwrap();
    assert_eq!(message.label(), Some("\u{e9}\u{1f600}\n"));
    assert_eq!(
        OperationMessage::parse(&message.format()),
        Ok(Some(message))
    );
}

#[test]
fn refusal_errors_name_no_stored_value() {
    let text = "filesquid.operation.v1 {\"operation_id\":\"secret-op\",\"kind\":\"secret-kind\"}";
    let err = OperationMessage::parse(text).unwrap_err();
    assert!(!err.to_string().contains("secret"), "{err}");
}

#[test]
fn installation_id_is_a_uuid_v4_rendered_as_a_urn() {
    let id = InstallationId::parse(UUID).expect("uuid");
    assert_eq!(id.as_str(), UUID);
    assert_eq!(id.urn(), format!("urn:uuid:{UUID}"));
    let upper = InstallationId::parse(&UUID.to_uppercase()).expect("upper");
    assert_eq!(upper, id, "stored lower-case");
    assert_eq!(InstallationId::parse(&id.urn()), Ok(id.clone()));
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, format!("\"{UUID}\""));
    assert_eq!(serde_json::from_str::<InstallationId>(&json).unwrap(), id);
}

#[test]
fn installation_ids_that_are_not_uuid_v4_are_refused() {
    for bad in [
        "",
        "not-a-uuid",
        "6f1d2c3b-4a5e-3f60-8a7b-9c0d1e2f3a4b",
        "6f1d2c3b-4a5e-4f60-cа7b-9c0d1e2f3a4b",
        "6f1d2c3b-4a5e-4f60-7a7b-9c0d1e2f3a4b",
        "6f1d2c3b4a5e4f608a7b9c0d1e2f3a4b",
        "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b0",
        "urn:uuid:",
    ] {
        assert!(InstallationId::parse(bad).is_err(), "{bad}");
        let json = format!("\"{bad}\"");
        assert!(
            serde_json::from_str::<InstallationId>(&json).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_version_ref_renders_the_object_reference() {
    let reference = VersionRef::new(format!("urn:uuid:{UUID}"), "v3").expect("ref");
    assert_eq!(reference.object_id(), format!("urn:uuid:{UUID}"));
    assert_eq!(reference.version(), "v3");
    assert_eq!(reference.object_ref(), format!("urn:uuid:{UUID}@v3"));
    assert_eq!(
        VersionRef::parse_object_ref(&reference.object_ref()),
        Ok(reference)
    );
    assert!(VersionRef::new("", "v1").is_err());
    assert!(VersionRef::new("obj", "3").is_err());
    assert!(VersionRef::new("obj", "v0").is_err());
    assert!(VersionRef::new("obj", "vx").is_err());
    assert!(VersionRef::new("obj@x", "v1").is_err());
    assert!(VersionRef::parse_object_ref("no-version").is_err());
}

#[test]
fn a_journal_link_names_an_operation_and_opaque_entry_ids() {
    let link = JournalLink::new(op("op-1"), vec!["17".into(), "18".into()]).expect("link");
    assert_eq!(link.operation_id(), &op("op-1"));
    assert_eq!(link.journal_entry_ids(), ["17", "18"]);
    assert!(JournalLink::new(op("op-1"), vec![String::new()]).is_err());
    let many = (0..=JournalLink::MAX_ENTRIES)
        .map(|n| n.to_string())
        .collect();
    assert!(JournalLink::new(op("op-1"), many).is_err());
}

#[test]
fn a_history_entry_serializes_one_activity_for_the_api() {
    let entry = HistoryEntry {
        kind: OperationKind::MetadataEdit,
        operation_id: op("op-1"),
        version: VersionRef::new("urn:uuid:obj", "v2").unwrap(),
        agents: vec![format!("urn:uuid:{UUID}")],
        started_at: Some(time("2026-10-19T10:00:00Z")),
        ended_at: time("2026-10-19T10:00:01Z"),
        journal: JournalLink::new(op("op-1"), vec!["17".into()]).unwrap(),
        label: None,
        reconstructed: false,
    };
    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["kind"], "metadata.edit");
    assert_eq!(json["version"]["object_id"], "urn:uuid:obj");
    assert_eq!(json["version"]["version"], "v2");
    assert_eq!(json["journal"]["journal_entry_ids"][0], "17");
    let back: HistoryEntry = serde_json::from_value(json).unwrap();
    assert_eq!(back, entry);
}

#[test]
fn the_sidecar_keeps_its_gate_and_links_version_operation_and_journal() {
    let link = JournalLink::new(op("op-1"), vec!["17".into()]).unwrap();
    let version = VersionRef::new("urn:uuid:obj", "v2").unwrap();
    let sidecar = ProvOSidecar::retained(time("2026-10-19T10:00:01Z"))
        .with_version(version.clone())
        .with_journal(link.clone());
    assert!(sidecar.is_json_ld());
    assert!(sidecar.is_permanent_history());
    assert_eq!(sidecar.version(), Some(&version));
    assert_eq!(sidecar.operation(), Some(&op("op-1")));
    assert_eq!(sidecar.journal(), Some(&link));
    let cache = ProvOSidecar::ephemeral_cache(time("2026-10-19T10:00:01Z")).with_version(version);
    assert!(!cache.is_permanent_history());
    let json = serde_json::to_string(&sidecar).unwrap();
    assert_eq!(
        serde_json::from_str::<ProvOSidecar>(&json).unwrap(),
        sidecar
    );
    let old = r#"{"time":"2026-09-23T12:00:00Z","context":"http://www.w3.org/ns/prov#","profile_version":1,"activity":"prov:Activity","ephemeral":false,"retained":true}"#;
    let parsed: ProvOSidecar = serde_json::from_str(old).expect("records without links still read");
    assert_eq!(parsed.version(), None);
}

#[test]
fn the_catalog_receipt_message_is_read() {
    let text = "filesquid.operation.v1 {\"operation_id\":\"op-9\",\"revision\":\"4\",\
                \"created_id\":\"c-1\",\"deletion\":{\"deleted_at\":\"2026-10-19T00:00:00.000Z\",\
                \"purge_after\":\"2026-11-18T00:00:00Z\"}}";
    let message = OperationMessage::parse(text).unwrap().unwrap();
    assert_eq!(message.operation_id().as_str(), "op-9");
    assert_eq!(
        message.kind(),
        OperationKind::Unknown,
        "written before kinds"
    );
    assert_eq!(message.revision(), Some(4));
    assert_eq!(message.created_id(), Some("c-1"));
    let kinded = text.replacen("\"revision\"", "\"kind\":\"delete\",\"revision\"", 1);
    let message = OperationMessage::parse(&kinded).unwrap().unwrap();
    assert_eq!(message.kind(), OperationKind::Delete);
}

#[test]
fn the_revision_is_written_as_a_decimal_string_and_read_back() {
    let message = OperationMessage::new(op("op-3"), OperationKind::CatalogOrganisation)
        .with_revision(12)
        .with_created_id("c-2");
    let text = message.format();
    assert!(text.contains("\"revision\":\"12\""), "{text}");
    assert_eq!(OperationMessage::parse(&text), Ok(Some(message)));
    let numeric =
        "filesquid.operation.v1 {\"operation_id\":\"op\",\"kind\":\"import\",\"revision\":12}";
    assert_eq!(
        OperationMessage::parse(numeric)
            .unwrap()
            .unwrap()
            .revision(),
        Some(12)
    );
}

#[test]
fn version_names_beyond_eighteen_digits_are_valid_object_versions() {
    let long = format!("v{}", "9".repeat(25));
    let reference = VersionRef::new(
        "urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b",
        long.clone(),
    )
    .expect("OCFL version numbers are unbounded");
    assert_eq!(reference.version(), long);
    let absurd = format!("v{}", "9".repeat(300));
    assert!(VersionRef::new("urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b", absurd).is_err());
}

#[test]
fn the_sidecar_operation_is_the_journal_links_only() {
    let sidecar = ProvOSidecar::retained(time("2026-10-19T10:00:01Z"));
    assert_eq!(sidecar.operation(), None);
    let json = serde_json::json!({
        "time": "2026-10-19T10:00:01Z",
        "context": "http://www.w3.org/ns/prov#",
        "profile_version": 1,
        "activity": "prov:Activity",
        "ephemeral": false,
        "retained": true,
        "operation": "op-elsewhere"
    });
    let parsed: Result<ProvOSidecar, _> = serde_json::from_value(json);
    assert!(
        parsed.map(|s| s.operation().is_none()).unwrap_or(true),
        "a stray operation member never names the operation"
    );
}
