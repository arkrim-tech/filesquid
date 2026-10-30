# Task API-F2.14 Walkthrough: Home collections for uploads, copies and moves

Branch `feat/api-f2.14-home-collections`, modules `filesquid-api`, `filesquid-native` and `filesquid-desktop` (host wiring), part of global Task FS-2.5 ([plan row](../plan.md)). Contract 0.34.0 (main is 0.33.0); `ui/src/api/schema.d.ts` and `contract-version.ts` regenerated with `npm run contract:types`. The task ships in two pull requests (orchestrator decision of 2026-10-30): **part 1** (this one), the port and the upload and copy destinations; **part 2**, moves, renames and folders, which ticks the plan and checklist boxes. Binding sources: [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md) Decisions 3, 4, 5 and 12.

## SRS mapping

| Requirement | What part 1 does | Tests |
|---|---|---|
| [API-S-011](../srs.md) Drive-style views, sorting, copy and move — **Destination** | Upload (`PUT /api/v1/files/upload`) and copy (`POST /api/v1/files/copies`) take `collection_id` and optional `folder`; a request without a collection is `400 invalid_request` (`destination_required`), an unknown or foreign collection `404 not_found` (`collection_not_found`), a reserved or malformed folder `invalid_path`, each before anything is staged and journaled once | `integration_home_collections`, `features` (`home_collections_API-S-011.feature`) |
| [API-S-011](../srs.md) — **Same logical path** | The new file takes `<folder>/<safe name>`, numbered beside a taken path (a recoverably deleted file's included): `photo-1.jpg`, `archive-1.tar.gz`, `report-1`, `.env-1`, `.config-1.json`; after 9,999 numbers `409 conflict` (`path_suffix_exhausted`) with nothing staged, reserved or registered; a numbered name over 255 bytes `name_too_long` | `integration_home_collections`, `features`, native `the_catalog_is_the_home_collections_of_uploads_and_copies` |
| [API-S-026](../srs.md) FileSquid OCFL application profile — collection snapshot | After the object is committed the file is registered in the snapshot (home collection, logical path, object) under the request's operation ID as one snapshot version journaled `library.files.register`; the answer carries `collection_id` and `path` | `integration_home_collections`, native test |
| [API-S-019](../srs.md) File write commands — one outcome per operation ID | Each operation reserves its new file's ID (`reserved_files`, catalog schema v4); a retry after a crash between the object and its registration completes the same file once, at v1 | `a_retry_after_a_crash_between_the_object_and_its_registration_completes_once`, the crash scenario |
| [API-S-021](../srs.md) Navigation listing — `navigation.change` | Each success announces the folder (or collection) that holds the file | `an_upload_is_registered_at_its_folder_and_name_and_answered_with_it`, the first scenario |
| [API-S-005](../srs.md) No paths across the API | The journal names opaque IDs only; a test asserts no file name or folder is journaled | the redaction assertions |
| [API-S-002](../srs.md) Versioned contract and generated types | 0.34.0: `UploadInput` and `CopyRequest` gain `collection_id` and `folder`; `UploadCommit` and `FileCopy` gain `collection_id` and `path`; the upload route documents 404 and 409 | `unit_contract`, the UI drift test |

## What changed

- `filesquid_api::home`: the `HomeCollections` port (collection lookup, first free path, file-ID reservation, registration), `MemoryHomeCollections`, `register_entry`, and `home::destination`, the one destination check of the import, upload and copy routes (moved out of `imports/routes.rs`; on a host without navigation it looks the collection up through the port).
- `files::homing`: the route steps (check, preview the path, reserve, register, announce); `FileError::coded` carries the rule's own journal code.
- Upload and copy resume a reserved file that is already committed instead of writing it again (`committed_before`); `FileOperations::copy_name` reads the source's display name so the copy's path is checked before anything is written.
- Journal names follow the spec: `files.upload`, `files.copy`, `files.move` became `file.upload`, `file.copy`, `file.move`. Consumers changed: the api tests only; no UI file references them (searched `apps/filesquid/ui/src` and `tests`).
- `filesquid-native`: `SqliteCatalog` implements the port over `register_file`; catalog schema v4 adds `reserved_files`.
- `filesquid-desktop`: `install_home_collections` installs the catalog next to the file scopes (one line in `install_library_services`).
- Closes the API-F2.2 remainder "registering a copy's home collection in the catalog".

## Decisions to confirm

- `collection_id` is required in the upload and copy schemas (0.34.0), now that the default UI's upload caller sends it (UI lane commit `778cca7`); the route refuses a request without it either way.
- **Spec conflict, owner to decide:** API-S-011 says the route "MUST answer and journal the path it used", while the journal rule for these routes (API-S-011 itself: "journaled once ... with opaque IDs only"; API-S-036: no file or folder name in a journal entry) keeps names out of the journal, and a collection logical path is made of names. This part journals opaque IDs only (the file, the collection, the operation) and puts the path in the answer and in the snapshot version the registration publishes under the same operation ID, so the path used can be recovered from the journal entry's operation. If the owner wants the path itself in the journal, the redaction tests change with it.
- An upload of new content for an existing file (`file_id`) keeps its home: it names a destination like every upload, and the answer names the file's own collection and path, not the destination it named.
- A reservation is bound to its command's input (the SHA-256 of the input's JSON, the upload's bytes included): a retry of the same command resumes its file even when a copy's source changed since; the same operation ID with another input is refused (`invalid_request`), and a command that fails before writing releases its reservation.

## API follow-ups (outside this row's scope)

- **No collection ID for a navigation node.** The navigation listing (`NavigationNode`: `node`, `kind`, `name`, `has_children`) and the view's folder rows (`ViewFolderRow`: `kind`, `node`, `node_kind`, `name`) name collections by opaque node handles only, so the default UI cannot fill an upload's `collection_id` from the tree, and the app refuses every upload as `destination_required` until it can. Neither this row's Description nor ADR 041 adds the mapping; it needs either a `collection_id` on collection and folder nodes (API-S-021) or a destination that names the node handle. Owner or orchestrator to place it (UI-F2.17 depends on it).
- **No sub-code on the wire.** `ApiError` carries `code` (`invalid_request`, `not_found`, `conflict`), `message`, `retryable` and the authorisation `reason` only; `destination_required`, `collection_not_found`, `path_suffix_exhausted`, `name_too_long` and `invalid_path` reach the journal as error codes and the answer only inside `message`. Telling them apart in the UI (mockup `48-write-refusals.svg`) needs a contract change to `ApiError` for every route (API-S-002), which is wider than this row.

## Test output

```text
     Running tests/integration/home_collections.rs (<target>/debug/deps/integration_home_collections-99a7bb71a02c95ab)
test a_reserved_folder_is_invalid_path ... ok
test a_retry_after_a_crash_between_the_object_and_its_registration_completes_once ... ok
test an_unknown_or_foreign_collection_is_collection_not_found ... ok
test an_upload_is_registered_at_its_folder_and_name_and_answered_with_it ... ok
test an_upload_without_a_collection_is_destination_required_and_stages_nothing ... ok
test a_copy_is_registered_in_its_destination_beside_its_source ... ok
test a_taken_path_is_numbered_beside_it_and_a_deleted_file_keeps_its_place ... ok
test after_9999_numbers_the_upload_is_path_suffix_exhausted_with_no_change ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.45s
     Running tests/unit/folder_libraries.rs (<target>/debug/deps/unit_folder_libraries-9c58a0c9fd172ba7)
test the_catalog_is_the_home_collections_of_uploads_and_copies ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.03s
Feature: Home collections for uploads and copies
  Scenario: An upload lands in the chosen folder of the collection
  Scenario: A copy lands beside its source with a numbered name
  Scenario Outline: A taken path is numbered beside the existing item
  Scenario Outline: A taken path is numbered beside the existing item
  Scenario Outline: A taken path is numbered beside the existing item
  Scenario Outline: A taken path is numbered beside the existing item
  Scenario Outline: A taken path is numbered beside the existing item
  Scenario: A recoverably deleted file keeps its place
  Scenario: A retry after a crash completes the same file once
  Scenario: An upload without a collection is refused before anything is staged
  Scenario: An upload into a collection of no library here is refused
  Scenario: After 9,999 numbers the upload is refused and nothing changes
  Scenario: The desktop host describes itself to the owner's webview
  Scenario: A server describes itself with its operator's name to any signed-in caller
  Scenario: Backup and sync are reported separately once enrolled
  Scenario: The host type never follows the request
  Scenario: The operator name is trimmed
  Scenario: A descriptor change reaches every subscriber
  Scenario: A client that missed events reads the descriptor again
  Scenario: A caller without a credential is refused
  Scenario: The descriptor and its event carry nothing private
426 scenarios (324 passed, 102 skipped)
1566 steps (1464 passed, 102 skipped)
```

`cargo test -p filesquid-api -p filesquid-desktop --locked` and `cargo test -p filesquid-native --features desktop --locked`: every target passes (run on the shared target).

## Mocked Environment Variables

None. The tests use in-memory home collections, a real OCFL root in a temporary directory, and a clock the tests set.
