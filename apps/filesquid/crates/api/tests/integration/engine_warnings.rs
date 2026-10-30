//! Integration test of the journal coverage of a file-route commit (Task API-F2.12;
//! API-S-031, API-S-028): an OCFL engine warning raised while a route commits is journaled
//! with the commit's operation ID, through the router against a real OCFL storage root.
//!
//! It has a binary of its own because `tracing` caches whether a callsite is enabled for the
//! whole process: a test elsewhere in the same binary reaching the engine's callsites with
//! no subscriber on its thread races with this test's thread-local subscriber.

#[path = "../support/files_fixture.rs"]
mod files_fixture;

use files_fixture::{FilesRig, LIBRARY, OBJECT};
use filesquid_api::journal::JournalEntry;
use serde_json::json;

/// An engine warning raised while a file route commits is journaled with the commit's
/// operation ID (API-S-031, Task API-F2.12): a move that takes over a stalled writer's
/// reservation makes the OCFL engine warn about the takeover inside the commit's activity
/// span.
#[tokio::test]
async fn an_engine_warning_during_a_file_route_commit_carries_its_operation_id() {
    use filesquid_api::trace_layer::JournalLayer;
    use filesquid_ocfl::write::VersionReservations;
    use tracing_subscriber::layer::SubscriberExt;
    let rig = FilesRig::with_files(&[("a.txt", b"alpha")]).await;
    let layer = JournalLayer::new();
    layer.attach(rig.monitor.journal());
    let _subscriber = tracing::subscriber::set_default(tracing_subscriber::registry().with(layer));
    let next = filesquid_ocfl::inventory::VersionNum::parse("v2").expect("v2");
    rig.leases
        .reserve(OBJECT, &next, "stalled")
        .await
        .expect("a stalled writer holds v2");
    rig.clock.advance(5 * 60_000);
    let moving =
        json!({ "library_id": LIBRARY, "object_id": OBJECT, "from": "a.txt", "to": "c.txt" });
    let moved = rig.command("/api/v1/files/moves", moving).await;
    assert_eq!(
        moved.status,
        axum::http::StatusCode::OK,
        "{:?}",
        moved.json()
    );
    let id = moved.json()["operation_id"]
        .as_str()
        .expect("operation id")
        .to_string();
    let warnings: Vec<JournalEntry> = rig
        .journal()
        .into_iter()
        .filter(|entry| entry.operation.starts_with("ocfl.engine."))
        .collect();
    assert!(!warnings.is_empty(), "the engine warned about the takeover");
    assert!(
        warnings
            .iter()
            .all(|entry| entry.operation_id.as_deref() == Some(id.as_str())),
        "{warnings:?}"
    );
}
