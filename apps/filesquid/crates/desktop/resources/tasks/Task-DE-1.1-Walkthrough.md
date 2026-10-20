# Task DE-1.1 Walkthrough: Create, open, copy, rebuild library

Branch: `feat/de-1.1-library-create-open-copy-rebuild` (part of global [Task 1.1](../../../../../../resources/implementation-plan.md#task-1.1)). API contract `0.17.0` (one above `0.16.0` of API-F2.11, PR #105).

## What changed

- **`filesquid-native`** (`desktop` feature), the host side of the library routes:
  - [`HostLocations`](../../../native/src/locations.rs) is the `LibraryLocations` port over the folders the owner chose:
    - each folder gets an opaque ID, `loc-` plus a UUID v4 (or a host-chosen ID through `register_as`);
    - it is resolved once to its canonical folder and is never a symbolic link;
    - its backend and fenced leases live under `<data dir>/locations/<digest>/`;
    - registering the same folder twice shares one backend and one set of leases;
    - `overlaps` is true for nested, identical or unknown locations.
  - [`OpenedRoots`](../../../native/src/locations.rs) lists the bound libraries' roots for the history and the backfill. It holds at most 64 roots.
  - [`HostRoots`](../../../native/src/catalog/provenance.rs) chains the catalog's roots with the opened ones.
  - [`open_fs_library_root`](../../../native/src/files.rs) serves a bound library's files from the same root and leases.
  - [`SqliteRepairLedger`](../../../native/src/repair_ledger.rs) is the persistent repair ledger in `filesquid.sqlite`:
    - the consent is journaled and recorded in one transaction;
    - each outcome is journaled with its receipt in one transaction, or neither is stored.
  - [`SqlitePreferences`](../../../native/src/preferences_sqlite.rs) is the API-F2.2 preference store in the same database.
- **`filesquid-api`**:
  - `POST /api/v1/admin/history/backfill` (API-S-029; [`history/launch.rs`](../../../api/src/history/launch.rs)):
    - new action `history.backfill`, granted to the desktop owner or a server administrator;
    - the host port `BackfillLauncher`, installed with `ApiState::install_history_backfill`;
    - answers 200 when started, 409 while a backfill runs, and 503 until a launcher is installed;
    - each request is journaled once as `history.backfill.request`.
  - `POST /api/v1/libraries/location-choices` and the `LocationChooser` port ([`local_library/choice.rs`](../../../api/src/local_library/choice.rs)).
  - `openapi.json` is 0.17.0, with schemas `BackfillStarted`, `LocationChoiceRequest`, `LocationChoice`, `ChoicePurpose` and `ChoiceOutcome`. `ui/src/api/schema.d.ts` was regenerated.
- **`filesquid-desktop`**:
  - [`libraries.rs`](../../src/libraries.rs): `DesktopLibraries` wraps `LibraryCommands<HostLocations>`. After each create or open, the bound library is served:
    - its files, from the same root and leases;
    - its OCFL validation;
    - its PROV-O history.
  - [`backfill.rs`](../../src/backfill.rs): one `DesktopBackfill` runner serves both the launch run and the route.
    - A run is the durable job over the catalogs, then the opened folders.
    - Only one run goes at a time, and the app's exit waits for it, bounded.
  - [`navigation.rs`](../../src/navigation.rs) covers the API-F1.11 host wiring:
    - the navigation service over the catalogs, with the installation UUID as the repository;
    - `CatalogFolders` and `CatalogScopes`, which imply no folder and no scope object until the DE-1.2 import records logical paths;
    - the webview's event subscription, now filtered for the owner (`owner_event_filter`).
  - [`lib.rs`](../../src/lib.rs) installs these at launch:
    - `install_library_routes`: the persistent ledger, the owner-only repair (`Authoriser::desktop_owner`) and `LocalLibraryService` with its provenance follow-up;
    - `install_file_provenance` (API-F2.5);
    - `install_host_preferences` and `install_file_scopes` (API-F2.2);
    - `install_backfill_route`;
    - the history source over `HostRoots`.
  - Test-instrumented build only ([`test_fixture.rs`](../../src/test_fixture.rs)): the folders `create` and `copy` under `FILESQUID_E2E_LOCATIONS`, and the media library named by `FILESQUID_E2E_LIBRARY`, are registered under fixed location IDs (`loc-e2e-create`, `loc-e2e-copy`, `loc-e2e-media`).
- **Docs**:
  - desktop SRS DE-S-017;
  - API plan notes on API-F1.11, API-F1.12, API-F2.2, API-F2.5 and API-F2.9;
  - the README packaged-window section.

## Folder choice (owner decision 2026-10-20: "go with A with rfd")

`POST /api/v1/libraries/location-choices` (`library.manage`) takes the body `{"purpose": "create" | "open" | "copy_destination"}`. It answers:

- `{"outcome": "chosen", "location_id": "loc-…"}`, or `{"outcome": "cancelled"}`;
- `409` while a dialog is open (`dialog_open`), or for a folder that cannot hold a library (`location_refused`);
- `503` on a host without a dialog (`filesquid-server`).

Each request is journaled once as `library.location.choose`.

The API port is `LocationChooser`, installed with `ApiState::install_location_chooser`. The desktop's chooser is [`folder_choice.rs`](../../src/folder_choice.rs):

- `DesktopChooser` shows `RfdDialog`, the `rfd` 0.17.2 `AsyncFileDialog` with only the `gtk3` feature, so it adds no new toolkit to the lockfile. `rfd` is not a Tauri plugin, so DE-S-002's check passes.
- It registers the picked folder in `HostLocations` and answers only the location ID.
- The path never leaves the host.

WebDriver cannot drive a native dialog, so the packaged-window spec uses the test-instrumented registration. The route itself is tested with a scripted dialog. The ADR 002 amendment is in the orchestrator's docs PR.

## Tests

- Native unit: `unit_library_host`, 11 tests.
- API unit: `unit_history_backfill_route`, 4 tests; `unit_location_choice`, 4 tests; `unit_auth` and `unit_contract` updated.
- Desktop integration: `integration_library_routes`, 8 tests, run through `filesquid://` with the per-launch token. `history_wiring` and `event_channel` were updated for the new signatures.
- Desktop BDD: [`desktop_libraries_F-001.feature`](../../tests/features/desktop_libraries_F-001.feature), 7 scenarios.
- Packaged window (WebdriverIO/Tauri-service, Xvfb): [`libraries.e2e.mjs`](../../tests/e2e/specs/libraries.e2e.mjs), 4 tests.

## SRS traceability

| Requirement | Tests |
|---|---|
| SRS-F-001 Independent local library and ownership | `integration_library_routes::create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files`, `an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths`; feature "A library created in an empty chosen folder gets an opaque ID", "A completed snapshot is copied into another empty folder"; e2e "creates a library…", "opens, copies and rebuilds…" |
| API-S-025 Local library and repair routes | `unit_location_choice` (4 tests); `integration_library_routes::the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id`, `a_cancelled_or_unusable_pick_registers_nothing`; `integration_library_routes::the_owners_consented_repair_is_kept_in_the_persistent_ledger` (no consent → 409, no change; the owner's consented repair); features "A repair without the owner's consent changes nothing", "An unknown folder is refused and journaled once without a path", "A snapshot is never copied into a folder inside the library"; e2e "finds the copy's object without a version inventory and repairs it only with consent" |
| API-S-023 Version-inventory repair (ledger: consent and outcome with receipt in one transaction) | `unit_library_host::the_ledger_journals_consents_and_receipts_and_keeps_them_across_a_restart`; feature "The owner's consented repair is kept after the app restarts" |
| API-S-029 History routes and backfill | `unit_history_backfill_route` (4 tests); `integration_library_routes::the_backfill_route_runs_over_the_folders_the_owner_opened`, `an_opened_folder_feeds_the_history…` (history over a chosen root); e2e "starts the history backfill over the served roots" |
| API-S-021 Navigation listing | `integration_library_routes::the_navigation_lists_the_installations_repository_and_its_libraries` |
| API-S-011 Preferences (API-F2.2 host wiring) | `integration_library_routes::the_owners_preferences_are_kept_in_the_operational_store` |
| DE-S-006 The webview MUST receive opaque handles and API responses, never raw filesystem paths or SQL | every `integration_library_routes` answer is checked for the data-directory path; `unit_library_host::a_registered_folder_gets_an_opaque_id_without_its_path`, `a_file_or_a_missing_folder_is_refused_without_its_path`; e2e `expectNoPath` |
| DE-S-011 Native journal and commit linkage (history source and backfill over every served root) | `integration_library_routes::an_opened_folder_feeds_the_history…`, `the_backfill_route_runs_over_the_folders_the_owner_opened`; `integration_history_wiring` (5 tests) |
| DE-S-015 Native-processing library boundary | `integration_native_boundary` (4 tests; no storage open in the desktop sources) |
| DE-S-017 Library routes over owner-chosen folders | `unit_library_host` (11 tests); `integration_library_routes` (6 tests) |

## TDD: failing run

The library-route tests failed with 503 while the commands were not installed. Captured by disabling `state.install_libraries` in `install_library_routes`, then restored:

```text
[WebKitGTK 605.1.15 linux #0-0]    ✓ creates a library in an owner-chosen folder and answers an opaque ID
[WebKitGTK 605.1.15 linux #0-0]    ✓ opens, copies and rebuilds the media library through its location ID
[WebKitGTK 605.1.15 linux #0-0]    ✓ finds the copy's object without a version inventory and repairs it only with consent
[WebKitGTK 605.1.15 linux #0-0]    ✓ starts the history backfill over the served roots
[WebKitGTK 605.1.15 linux #0-0] 4 passing (30.5s)
Spec Files:	 1 passed, 1 total (100% completed) in 00:00:32  
```

## Raw test output

```text
     Running tests/unit/library_host.rs
test a_file_or_a_missing_folder_is_refused_without_its_path ... ok
test a_folder_that_overlaps_the_data_directory_is_refused_and_prepares_nothing ... ok
test a_host_chosen_id_must_be_well_formed_and_unique ... ok
test a_symbolic_link_is_refused_as_a_location ... ok
test a_refused_id_prepares_no_storage ... ok
test a_registered_folder_gets_an_opaque_id_without_its_path ... ok
test nested_and_unknown_locations_overlap_and_siblings_do_not ... ok
test one_folder_registered_twice_shares_its_leases ... ok
test the_stores_refuse_a_symbolic_link_in_place_of_the_operational_store ... ok
test concurrent_registrations_of_one_folder_share_one_root ... ok
test the_ledger_journals_consents_and_receipts_and_keeps_them_across_a_restart ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/history_backfill_route.rs
test the_route_performs_the_history_backfill_action ... ok
test without_a_launcher_the_route_is_unavailable_and_journaled_once ... ok
test an_unauthenticated_request_never_reaches_the_launcher ... ok
test the_owner_starts_one_backfill_and_a_second_request_is_refused_while_it_runs ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/location_choice.rs
test a_cancelled_dialog_answers_cancelled_and_journals_it ... ok
test a_picked_folder_answers_its_opaque_location_id_only ... ok
test without_a_dialog_or_with_an_unreadable_body_nothing_is_asked ... ok
test refusals_are_typed_and_journaled_once_with_their_code ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/library_routes.rs
test a_cancelled_or_unusable_pick_registers_nothing ... ok
test create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files ... ok
test the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id ... ok
test the_navigation_lists_the_installations_repository_and_its_libraries ... ok
test the_owners_preferences_are_kept_in_the_operational_store ... ok
test the_backfill_route_runs_over_the_folders_the_owner_opened ... ok
test an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths ... ok
test the_owners_consented_repair_is_kept_in_the_persistent_ledger ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

Desktop BDD (`cargo test -p filesquid-desktop --features test-webdriver --test features`):

```text
Feature: Desktop libraries in folders the owner chose
  Scenario: A library created in an empty chosen folder gets an opaque ID
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner chose an empty folder
   ✔  When the app creates a library in that folder
   ✔  Then the answer should name a new library without any folder path
   ✔  And the desktop should serve that library's files
  Scenario: A completed snapshot is copied into another empty folder
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner opened a folder that holds a library
   ✔  And the owner chose an empty folder
   ✔  When the app copies the library's snapshot into that folder
   ✔  Then the copy should succeed with no invalid object
   ✔  And the chosen folder should hold an OCFL storage root
  Scenario: A folder that already holds a file is not used for a new library
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner chose a folder that holds a file
   ✔  When the app creates a library in that folder
   ✔  Then the request should be refused as a conflict
   ✔  And the folder should still hold only that file
  Scenario: A snapshot is never copied into a folder inside the library
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner opened a folder that holds a library
   ✔  And the owner chose a folder inside that library
   ✔  When the app copies the library's snapshot into that folder
   ✔  Then the request should be refused as a conflict
  Scenario: A repair without the owner's consent changes nothing
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner opened a folder that holds a library whose head lacks its version inventory
   ✔  When the app asks to repair it without the owner's consent
   ✔  Then the request should be refused as a conflict
   ✔  And the head should still lack its version inventory
  Scenario: An unknown folder is refused and journaled once without a path
   ✔> Given the desktop host has started its library routes
   ✔  When the app creates a library in a folder the host never registered
   ✔  Then the request should be refused as not found
   ✔  And the journal should record the refusal once without any folder path
  Scenario: The owner's consented repair is kept after the app restarts
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner opened a folder that holds a library whose head lacks its version inventory
   ✔  When the owner confirms the repair of that library
   ✔  Then the repair should succeed
   ✔  And after the app restarts the repair ledger should still hold its consent and receipt

```

Packaged window (`xvfb-run -a npx wdio run ./wdio.conf.mjs --spec ./specs/libraries.e2e.mjs`):

```text
[WebKitGTK 605.1.15 linux #0-0]    ✓ creates a library in an owner-chosen folder and answers an opaque ID
[WebKitGTK 605.1.15 linux #0-0]    ✓ opens, copies and rebuilds the media library through its location ID
[WebKitGTK 605.1.15 linux #0-0]    ✓ lists repair candidates and refuses a repair without the owner's consent
[WebKitGTK 605.1.15 linux #0-0]    ✓ starts the history backfill over the served roots
[WebKitGTK 605.1.15 linux #0-0] 4 passing (27.8s)
Spec Files:	 1 passed, 1 total (100% completed) in 00:00:29
```

## Mocked Environment Variables

None for the application. The packaged-window run sets `FILESQUID_E2E_LOCATIONS` and `FILESQUID_E2E_LIBRARY` itself, and only in its run directory.

## Pre-commit

`FULL=1 FILESQUID_DESKTOP_E2E=1 bash scripts/pre-commit.sh` on the final code, packaged-window e2e included:

```text
✅ Desktop packaged-window e2e passed (109s).
✅ All Pre-Commit Hooks Passed! Test scope: full (820s)
```
