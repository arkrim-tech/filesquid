# Task DE-1.1 Plan: Create, open, copy, rebuild library

Branch: `feat/de-1.1-library-create-open-copy-rebuild`. Part of global Task 1.1 (CL-1.1, API-F1.12, DE-1.1, UI-1.1, IO-1.1).

## Traceability

- PRD [F-001](../../../../../../resources/prd.md#f-001-independent-local-library-and-ownership); root SRS [SRS-F-001](../../../../../../resources/srs.md#srs-f-001-independent-local-library-and-ownership).
- API SRS [API-S-025](../../../api/resources/srs.md) (local library and repair routes), API-S-021 (navigation), API-S-028/029 (PROV-O history, backfill), API-S-023 (repair ledger).
- Desktop SRS [DE-S-002, DE-S-006, DE-S-011, DE-S-015](../srs.md).
- Feature: [`independent_local_library_and_ownership_F-001.feature`](../../../../ui/tests/features/independent_local_library_and_ownership_F-001.feature).

## Constraints copied from the requirements

1. API-S-025: create on an owner-chosen OCFL storage root, open, copy a completed snapshot, rebuild the catalog from files, the repair-candidate query and the repair command are reachable as the router's versioned routes, mounted unchanged by the desktop host.
2. API-S-025: the routes call the application layer's one implementation; hosts inject the storage root and the repair ledger (API-S-006). The host MUST NOT re-implement the operations.
3. API-S-025: answers carry opaque library and object IDs, never a filesystem path, storage key or OCFL content path (API-S-005, DE-S-006). A folder never crosses the API as a path: the host registers it under an opaque location ID (`MAX_LOCATION_ID_BYTES` = 128).
4. API-S-025: repair is refused for every caller other than the desktop owner (the holder of the per-launch token). The repair must name the library and the explicit objects and carry the owner's consent; without consent, refused with no change.
5. API-S-023 / ledger port: the consent is recorded durably before any object is repaired; each object's outcome entry and its receipt are stored in one transaction or neither. Desktop: in the SQLite operational store (`<data dir>/filesquid.sqlite`) next to the activity journal. Entries are already redacted (opaque IDs and codes only).
6. API-S-025: every refused call is journaled exactly once, with no path, title or content.
7. `LibraryLocations`: both calls are pure in-memory lookups; `overlaps` answers `true` for an unknown ID or when the host cannot tell (a snapshot copy never lands inside the library it copies, OCFL-S-019).
8. Every writer of one root shares one set of fenced version reservations: the file routes (`HostFiles`) and the library commands use the same `LeaseStore` for a location.
9. `LibraryCommands`: at most 64 open libraries (`MAX_OPEN_LIBRARIES`).
10. DE-S-011: the history source covers the OCFL roots of every library the host serves, the owner-chosen roots included (FS-2.8 gap); the repair service carries the API-S-028 provenance follow-up.
11. API-F2.5: restore and label commits carry their PROV-O record: the host calls `FileService::install_provenance(provenance_context(installation), announcing_store)`.
12. API-F1.11: install `NavigationService` with `ApiState::install_navigation` over the catalog, a `FolderIndex`, `HostAccess` and the repository; open the webview's event subscription with `EventBus::subscription_filtered(service.event_filter(Principal::Owner))`.
13. API-S-029: `POST /api/v1/admin/history/backfill` (owner or administrator) registers in the process registry, checkpoints per object, stops at its cap or on cancel, skips versions with a record, and commits one provenance-only version per object.
14. DE-S-015: no direct storage opens (`rusqlite`, `FsBackend::new`, `LeaseStore::new`, ...) or `std::fs` in the desktop sources; they live in `filesquid-native`.
15. DE-S-002: no dialog plugin and no permission for the webview; the folder choice (pending orchestrator decision) is Rust-side only.
16. Packaged-window path proven with WebdriverIO/Tauri-service (ADR 017); the test registers its folders through a test-instrumented-only variable.
17. AGENTS.md §22–26: bounded loops, every resource closed, functions under 60 lines, named side effects, no swallowed errors.

## Approach

- `filesquid-native`:
  - `locations.rs` `HostLocations`: `LibraryLocations` over canonical folders. Each location gets an `FsBackend` and a `HostLeases` under `<data dir>/locations/<digest>/`. IDs are `loc-<uuid v4>`, bounded to 256 locations. `overlaps` compares canonical paths component-wise.
  - `files.rs` `open_fs_library_root`: serve a library through `HostFiles` with a given root and reservations.
  - `repair_ledger.rs` `SqliteRepairLedger` in `filesquid.sqlite`: the consent row and the outcome row plus receipt, each with its journal row in one transaction (`SqliteJournal::record_in_transaction`).
  - `HostHistoryRoots`: the catalog roots plus the bound owner-chosen roots.
- `filesquid-desktop`: `DesktopLibraries` decorates `LibraryCommands` and, after each create or open, serves the bound library's files, validation and history. It also installs the file provenance, navigation and the filtered event subscription.
- `filesquid-api`: the admin backfill route and, per the decision, the location-choice route. Each is a contract change, with a minor version bump.

## Verification

- Native unit tests: locations and the ledger.
- Desktop integration test `integration_library_routes`, driven through `api_protocol::handle` with the token:
  - create, open, copy, rebuild and repair;
  - refusals;
  - no path in any answer;
  - the persistent ledger after restart;
  - history over the chosen roots.
- Desktop cucumber feature, and a WebdriverIO spec over the packaged window.
- `FULL=1 bash scripts/pre-commit.sh`.
