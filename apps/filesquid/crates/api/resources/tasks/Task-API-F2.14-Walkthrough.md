# Task API-F2.14 Walkthrough: Home collections for uploads, copies and moves

Branch `feat/api-f2.14-home-collections`, modules `filesquid-api`, `filesquid-native` and `filesquid-desktop` (host wiring), part of global Task FS-2.5 ([plan row](../plan.md)). Contract 0.34.0 (main is 0.33.0); `ui/src/api/schema.d.ts` and `contract-version.ts` regenerated with `npm run contract:types`. The task ships in two pull requests (orchestrator decision of 2026-10-30): **part 1**, the port and the upload and copy destinations; **part 2** (branch `feat/api-f2.14-moves-folders-renames`, contract 0.35.0), moves, renames and folders, which ticks the plan and checklist boxes ([Part 2](#part-2-moves-renames-and-collection-folders)). Binding sources: [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md) Decisions 3, 4, 5 and 12.

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

## Part 2: moves, renames and collection folders

Branch `feat/api-f2.14-moves-folders-renames`, modules `filesquid-api` and `filesquid-native`. Contract 0.35.0 (main is 0.34.0); `ui/src/api/schema.d.ts` and `contract-version.ts` regenerated with `npm run contract:types`; `tsc` is clean and no UI caller changed (the in-object move and folder shapes are kept).

### SRS mapping (part 2)

| Requirement | What part 2 does | Tests |
|---|---|---|
| [API-S-011](../srs.md) Drive-style views, sorting, copy and move — **Destination** | `POST /api/v1/files/moves` takes `{library_id, file_id, collection_id, folder?}` beside the in-object `{object_id, from, to}` (untagged `MoveInput`, contract `anyOf`). A collection move keeps the file's name, publishes one snapshot version and rewrites no bytes; without `collection_id` it is `400 invalid_request` (`destination_required`), journaled once as `file.move` | `a_collection_move_gives_a_file_another_folder_keeping_its_name_and_its_bytes`; `moves_renames_and_folders_API-S-011.feature` (move scenarios) |
| [API-S-011](../srs.md) — **Same logical path** for move, rename and folder creation | A move, a rename and a new collection folder at a taken path are numbered beside it (`Archive/beach-1.jpg`, `Summer/Harbour-1.jpg`, `filepathX/directoryY-1` then `-2`), answered with the path used; after 9,999 numbers `409 conflict` (`path_suffix_exhausted`) with nothing changed; a numbered folder segment over 255 bytes `name_too_long`; a folder in a reserved root folder `invalid_path` | `a_rename_gives_…`, `a_rename_that_cannot_be_placed_is_refused_before_the_name_changes`, `a_new_folder_at_a_taken_path_is_numbered_beside_it_and_nothing_is_written`, `a_new_folder_at_a_file_path_is_numbered_beside_the_file`, `a_new_folder_names_its_collection_or_is_refused_with_the_rule_code`, `after_9999_numbers_a_new_folder_is_path_suffix_exhausted`, `a_numbered_folder_over_255_bytes_is_name_too_long`; the feature's edge and error scenarios |
| [API-S-014](../srs.md) Versions, restore, labels — reserved `.filesquid/` | A new collection folder inside `provenance` or `.filesquid` at the collection's root is refused (`invalid_path`) | `a_new_folder_names_its_collection_or_is_refused_with_the_rule_code`, "A new folder inside a reserved folder is refused" |
| [API-S-026](../srs.md) FileSquid OCFL application profile — collection snapshot | `HomeCollections::placement` and `relocate` (memory port and `SqliteCatalog`, scoped to the library): each relocation publishes one snapshot version journaled `library.files.place` under the operation ID; a file's own path never counts as taken; an unregistered file is `not_found` | native `a_registered_file_is_relocated_in_one_version_and_its_own_path_never_takes_it`, the integration tests above |
| [API-S-027](../srs.md) Display names — rename | `POST /api/v1/files/renames` checks the new path before the name is committed, then moves the registered file's collection path to its new safe name; the answer's `MetadataCommit.path` carries it | `a_rename_gives_…`, "A rename gives a file its new name in its folder" |
| [API-S-021](../srs.md) Navigation listing — `navigation.change` | A rename announces its folder; a collection move announces the folder it left and the one it entered; a new folder writes nothing and announces nothing | the rename and move tests (`topics`), "both folders should be announced as changed" |
| [API-S-005](../srs.md) / [API-S-036](../srs.md) No names in the journal | Move, rename and folder refusals journal opaque IDs and the rule code only | `assert_redacted` in every part-2 test, "the move should be journaled once without its name or folder" |
| [API-S-002](../srs.md) Versioned contract and generated types | 0.35.0: `CollectionMoveRequest`, `CollectionMove`, `FileMoveOutcome`; `CollectionFolderRequest`, `CollectionFolder`; `CreateFolderCommand.input` and `FolderAnswerOutcome.result` are `anyOf` the in-object and collection shapes; `MetadataCommit.path` | `unit_contract` (every schema has a Rust sample and matches it), the UI drift test |

TDD note: the move, rename and folder code was written by the earlier lane of this branch; the folder tests, contract samples and feature scenarios were added here. The contract samples failed first (`every_component_schema_has_a_rust_sample`, `every_component_schema_matches_its_serialized_rust_type`) until the two folder schemas were added; one folder test expectation (a nested `Summer/.filesquid` refused) was wrong, since the reserved folders are reserved at the collection's root only, as for uploads in part 1, and was replaced with `.filesquid/notes`.

### What changed (part 2)

- `filesquid_api::home`: `HomeCollections::placement` and `relocate`, `PLACE_OPERATION` (`library.files.place`) and `place_entry`; `MemoryHomeCollections` implements both.
- `files::homing`: `prepare_rename`/`settle_rename`, `move_to_destination` and `plan_folder`.
- `files::view`: `CollectionMoveRequest`, `MoveInput`, `CollectionMoveView`, `CollectionFolderRequest`, `FolderInput`, `CollectionFolderView`; `files::routes` dispatches the move and folder routes on the input's shape; the rename route (`metadata_routes`) settles the new path after the name commit.
- `filesquid-native`: `SqliteCatalog` implements `placement` and `relocate` over the catalog (`catalog/home.rs`).
- Contract 0.35.0 and the regenerated UI types; the feature file [`moves_renames_and_folders_API-S-011.feature`](../../tests/features/moves_renames_and_folders_API-S-011.feature) with its steps in `tests/support/home_steps.rs`.
- Plan row ticked with its Status, the checklist box ticked and the MVP progress redrawn.

### Decisions to confirm (part 2)

- **An empty folder is not stored.** ADR 041 Decision 3 makes a folder only a prefix of its files' logical paths, so `POST /api/v1/files/folders` with `collection_id` answers the free path (`directoryY-1`) with `pending: true` and persists nothing; the client keeps it until a file is uploaded, copied or moved into it. A pending folder that the client forgets is gone, and two clients can be answered the same free path until one of them places a file there.
- **A new collection folder is not journaled on success.** It changes nothing, which matches the owner-recorded exception for a command that succeeds without a change (and the in-object folder route, which journals none either); its refusals are journaled once as `files.create_folder`. If the owner counts a folder creation as activity, a success entry with opaque IDs is a small change.
- **The in-object shapes stay beside the collection shapes.** `/files/moves` and `/files/folders` accept both the API-S-008 in-object input (`object_id`, `from`/`to` or `path`) and the new collection input, told apart by their fields (untagged), because the default UI still calls the in-object shapes. Retiring them is a contract change for a later row.
- **A collection move compares no revision.** The envelope's `expected` revision is ignored for a collection move (and a collection folder): the move changes only the collection snapshot, not the file object whose head the revision names. Two changes to the same file's place therefore apply in the order the catalog receives them, without a stale-revision refusal.
- **A rename relocates the collection path.** Renaming a registered file now also moves its collection logical path to the new safe name (numbered when taken), so the Drive shows the new name in place; the rename commits the display name first and then publishes one snapshot version. A rename that cannot be placed (`path_suffix_exhausted`, `name_too_long`) is refused before the name commit, so nothing changes. When the relocation fails after the name commit (a store failure), the file is not left half renamed: any later rename to that name, the retry under the same operation ID included, finds the name already on the file's head, moves the collection path first, and is then answered as the name commit answers (`409 conflict`, since the name is already there); the client reads the file again (Cubic round 1).
- **A relocated file never takes its own target.** A move or rename leaves the file's own path, so that path does not count as taken at the target, below it or above it (a file `notes` moved into a folder `notes/` lands at `notes/notes`); a relocation checks its collection before answering a file already in place, so a file left in a removed collection is `collection_not_found` (Cubic round 1).
- **Reserved folders are reserved at the collection's root only** (`provenance/`, `.filesquid/`), as for uploads in part 1; `Summer/.filesquid` is an ordinary folder.

### API follow-ups (still open after part 2)

The part-1 follow-ups above are unchanged and still open: no collection ID on navigation nodes or view folder rows; no error sub-code on `ApiError`; and the API-S-011 "journal the path it used" versus API-S-036 (and API-S-011's own "opaque IDs only") journaling conflict, now also for moves, renames and folders, which journal opaque IDs and put the path in the answer and the snapshot version.

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

### Part 2

`cargo test -p filesquid-api --test integration_home_collections`, `--test unit_contract` and `--test features -- --input tests/features/moves_renames_and_folders_API-S-011.feature` (run on the shared target):

```text
     Running tests/integration/home_collections.rs (<target>/debug/deps/integration_home_collections-99a7bb71a02c95ab)
test a_new_folder_at_a_file_path_is_numbered_beside_the_file ... ok
test a_failed_upload_releases_its_reservation ... ok
test a_new_folder_at_a_taken_path_is_numbered_beside_it_and_nothing_is_written ... ok
test a_numbered_folder_over_255_bytes_is_name_too_long ... ok
test a_collection_move_gives_a_file_another_folder_keeping_its_name_and_its_bytes ... ok
test a_new_folder_names_its_collection_or_is_refused_with_the_rule_code ... ok
test a_copy_is_registered_in_its_destination_beside_its_source ... ok
test a_reserved_folder_is_invalid_path ... ok
test a_retry_after_a_crash_between_the_object_and_its_registration_completes_once ... ok
test a_rename_gives_a_registered_file_its_new_name_in_its_folder_numbered_when_taken ... ok
test a_taken_path_is_numbered_beside_it_and_a_deleted_file_keeps_its_place ... ok
test an_unknown_or_foreign_collection_is_collection_not_found ... ok
test an_upload_is_registered_at_its_folder_and_name_and_answered_with_it ... ok
test an_upload_without_a_collection_is_destination_required_and_stages_nothing ... ok
test new_content_of_a_file_answers_its_own_home ... ok
test the_same_operation_id_with_other_bytes_after_a_crash_is_refused ... ok
test after_9999_numbers_a_new_folder_is_path_suffix_exhausted ... ok
test a_rename_that_cannot_be_placed_is_refused_before_the_name_changes ... ok
test after_9999_numbers_the_upload_is_path_suffix_exhausted_with_no_change ... ok
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.90s
     Running tests/unit/contract.rs (<target>/debug/deps/unit_contract-9d2af10e8a3ee2c6)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.12s
     Running tests/features.rs (<target>/debug/deps/features-65175459859f17e0)
Feature: Moves, renames and folders in the home collections
  Scenario: A move gives a file another folder and keeps its name
  Scenario: A rename gives a file its new name in its folder
  Scenario: A new folder is answered with its path and writes nothing
  Scenario: A move into a taken path is numbered beside it
  Scenario: A rename to a taken name is numbered beside it
  Scenario Outline: A new folder at a taken path is numbered beside it
  Scenario Outline: A new folder at a taken path is numbered beside it
  Scenario: A second folder at the same path takes the next number
  Scenario: A move without a collection is refused
  Scenario: A rename that cannot be placed changes nothing
  Scenario: A new folder without a collection is refused
  Scenario: A new folder inside a reserved folder is refused
[Summary]
12 scenarios (12 passed)
59 steps (59 passed)
```

`cargo test -p filesquid-native --features desktop --test unit_folder_libraries` (before Cubic round 1; the round renamed the relocation fixtures, whose dotted names (`api-f2.14-…`) made `with_extension` give every fixture the same library folder, so tests run together failed with `Unavailable("storage unavailable")`; after it the target passes 27 of 27, under a `TMPDIR` in `/dev/shm` too):

```text
     Running tests/unit/folder_libraries.rs (<target>/debug/deps/unit_folder_libraries-9c58a0c9fd172ba7)
test a_create_library_command_is_refused_and_creates_nothing ... ok
test a_folder_that_cannot_be_a_library_leaves_nothing_in_the_catalog_directory ... ok
test a_file_object_without_a_snapshot_entry_is_counted_and_left_unregistered ... ok
test a_file_s_path_is_taken_below_it_and_no_file_sits_below_a_file ... ok
test a_library_folder_copied_elsewhere_reopens_with_its_collections_folders_and_files ... ok
test a_file_of_the_earlier_form_is_placed_when_it_is_registered ... ok
test a_refused_registration_is_journaled_as_failed_with_its_code ... ok
test a_registration_whose_numbers_are_all_taken_is_refused_and_changes_nothing ... ok
test a_registered_file_names_its_path_and_object_in_the_snapshot_and_the_index ... ok
test a_retired_in_app_root_is_neither_opened_nor_served ... ok
test a_registered_file_is_relocated_in_one_version_and_its_own_path_never_takes_it ... ok
test an_empty_folder_attached_with_an_index_of_another_folder_starts_afresh ... ok
test an_index_write_for_a_library_that_is_not_attached_is_refused ... ok
test a_version_2_catalog_is_migrated_to_the_current_version_and_rebuilt_from_its_folder ... ok
test an_index_built_from_another_copy_of_the_folder_is_rebuilt ... ok
test a_taken_path_is_numbered_beside_it_and_a_deleted_file_s_path_counts ... ok
test no_index_is_created_beyond_the_catalog_s_limit_of_files ... ok
test files_deleted_with_their_collection_are_deleted_not_unfiled ... ok
test no_library_root_or_original_is_ever_created_in_the_catalog_directory ... ok
test deleting_a_collection_deletes_its_files_in_place_in_one_version ... ok
test removing_a_collection_that_still_holds_files_is_refused_before_anything_is_published ... ok
test renaming_a_library_retitles_its_entity_in_one_journaled_version_and_a_same_name_rename_publishes_nothing ... ok
test the_catalog_is_the_home_collections_of_uploads_and_copies ... ok
test folders_and_scope_entries_come_from_the_index_of_ordinary_files ... ok
test the_library_s_own_retention_survives_a_rebuild_and_sets_purge_after ... ok
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s
```

The head-commit `FULL=1 bash scripts/pre-commit.sh` summary line is recorded in the pull request.

## Mocked Environment Variables

None. The tests use in-memory home collections, a real OCFL root in a temporary directory, and a clock the tests set.
