# Task API-F1.4 Walkthrough: Metadata property routes

Branch `feat/api-f1.4-metadata-property-routes`, module `filesquid-api` ([plan row](../plan.md)), part of global Task FS-1.3. It follows the owner's record-format decision of 2026-10-20, "A+3a" ([Conflict Artifact](./Task-API-F1.4-Conflict.md), [ADR 038](../../../../../../resources/architecture/038-import-file-record.md) Decision 8):

- `filesquid-metadata`'s `FileRecord` is the only codec of `<file-uuid>.metadata.md`.
- API-F1.4 takes API-F1.13's stored-name half: an upload creates the file's own object.

Holistic UI check:

- The metadata panel (UI rows of F-052) reads statements with their source and sets, removes and places them.
- The add dialog asks `GET /files/metadata/terms` for the add order.
- The file-info panel reads `GET /files/facts`.
- The UI's `FileSquidClient` still uploads to a logical path, and moves to the new upload query with its UI row. This PR only regenerates `schema.d.ts`.

## SRS mapping

| SRS ID and title | Covered by |
|---|---|
| [API-S-034](../srs.md) Metadata property routes | `integration_metadata` (13 tests), `metadata_property_routes_API-S-034.feature` (8 scenarios), `unit_contract`, `unit_auth` |
| [API-S-026](../srs.md) The FileSquid OCFL application profile: one record codec, uploads create `files/<file-id>` | `unit_canonical` (32 tests, 12 on originals), `an_upload_creates_the_file_object_with_its_original_and_record_in_one_version`, `a_later_upload_of_the_same_file_is_a_new_version_with_one_more_import_entry` |
| [API-S-027](../srs.md) Display and download names, the facts projection | `content_download_and_listing_take_the_name_and_media_type_from_the_record`, `the_facts_projection_has_no_absolute_path_host_or_account` |
| [API-S-018](../srs.md) Content handles and ranged reads (the record's media type) | `content_download_and_listing_take_the_name_and_media_type_from_the_record`, the content scenario |
| [MD-S-002](../../../metadata/resources/srs.md) The `/terms/` register and Schema.org overflow | `unknown_dcterms_names_and_unregistered_prefixes_are_refused_without_a_version`, `a_term_name_is_offered_dcterms_first_then_schema_then_registered_namespaces` |
| [MD-S-012](../../../metadata/resources/srs.md) Namespace registry | `a_registered_namespace_accepts_its_terms_and_is_kept_in_the_snapshot_object` |
| [MD-S-013](../../../metadata/resources/srs.md) Import record | `an_upload_whose_declared_size_differs_fails_verification_and_commits_nothing`, `a_name_that_is_not_ascii_needs_its_normalisation_form`, the canonical tests |
| [MD-S-014](../../../metadata/resources/srs.md) Commit options | every edit commits through `OcflStore` with the caller as the version user |
| [API-S-008](../srs.md) File operation routes | `file_operation_routes_API-S-008.feature` (upload scenario rewritten), `integration_files` |
| [API-S-002](../srs.md) Versioned contract and generated types | contract 0.17.0, `unit_contract` drift and sample tests, regenerated `ui/src/api/schema.d.ts` |

## What changed

- **One codec** (`src/canonical/original.rs`, `store.rs`).
  - `OriginalRecord`, `encode_companion` and `decode_companion` are gone.
  - `commit_file_original` takes a full `FileRecord` and the record at the head. It refuses a record that does not extend that head record, or whose new import entries do not cite this object, the original's path, the bytes' SHA-256 and their length.
  - `read_file_original` decodes with `FileRecord::from_markdown`.
  - The stored name keeps the extension only when it is 1 to 16 ASCII letters or digits; the record keeps the full extension.
- **Upload records** (`src/canonical/upload.rs`).
  - An upload's record holds the client's facts and the receiving host's operating-system family, architecture and FileSquid version. Every other fact is listed unavailable; the host name and account are never read.
  - A declared size that differs from the bytes builds no record.
  - The client states `name_normalization` for a name that is not ASCII. No Unicode crate is added (AGENTS.md section 17).
- **Upload route** (`src/files/upload.rs`). `PUT /api/v1/files/upload` takes `library_id`, `name`, `size`, and optionally `file_id`, `media_type`, `last_modified`, `name_normalization` and `message`.
  - A new file gets `files/<file-id>` in v1.
  - An existing `file_id` gets a new version: new bytes at the same stored path and one more import entry.
  - The answer is `UploadCommit` (with `file_id`).
- **Record-based answers** (`src/files/records.rs`). Content, download and listing answers of a file object's original use the record's display name and media type; the record is not listed. Records are cached by content digest.
- **Metadata routes** (`src/files/statements.rs`, `metadata_routes.rs`).
  - Statements are read, set by CURIE and removed.
  - The add order of a bare name is answered.
  - The registry is `namespaces.md` in the library's collection-snapshot object; registration needs `files.write`.
  - The facts projection is read-only.
  - Every edit is one OCFL version through `OcflStore`, journaled with opaque IDs only.
- **SRS:** a new [API-S-034](../srs.md). API-S-026 was already amended by the owner's decision.

## Test output

```text
$ cargo test -p filesquid-api --locked --test integration_metadata --test unit_canonical --test features
     Running tests/integration/metadata.rs (/home/user/.cargo-target-shared/debug/deps/integration_metadata-4b057c93d4884fc2)
running 13 tests
test a_name_that_is_not_ascii_needs_its_normalisation_form ... ok
test a_file_the_desktop_committed_is_edited_through_the_same_record ... ok
test a_later_upload_with_another_media_type_is_answered_with_it ... ok
test a_later_upload_of_the_same_file_is_a_new_version_with_one_more_import_entry ... ok
test an_upload_whose_declared_size_differs_fails_verification_and_commits_nothing ... ok
test an_upload_creates_the_file_object_with_its_original_and_record_in_one_version ... ok
test content_download_and_listing_take_the_name_and_media_type_from_the_record ... ok
test details_name_the_original_by_its_record ... ok
test a_term_name_is_offered_dcterms_first_then_schema_then_registered_namespaces ... ok
test a_registered_namespace_accepts_its_terms_and_is_kept_in_the_snapshot_object ... ok
test the_facts_projection_has_no_absolute_path_host_or_account ... ok
test unknown_dcterms_names_and_unregistered_prefixes_are_refused_without_a_version ... ok
test statements_are_set_read_and_removed_one_version_each ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/unit/canonical.rs (/home/user/.cargo-target-shared/debug/deps/unit_canonical-fa601950e2b172f5)
running 32 tests
test a_commit_journal_a_replay_could_not_read_is_refused_before_publishing ... ok
test a_head_without_its_version_inventory_is_refused_and_nothing_is_written ... ok
test a_committed_snapshot_reads_back_entity_for_entity ... ok
test a_file_original_is_its_own_object_with_its_metadata_record ... ok
test a_provenance_only_message_names_no_receipt ... ok
test a_record_that_does_not_cite_the_bytes_commits_nothing ... ok
test a_recorded_kind_is_written_and_read_back_by_both_readers ... ok
test a_referenced_import_entry_cannot_describe_a_stored_original ... ok
test a_root_without_a_snapshot_has_no_commit_journal ... ok
test a_stale_draft_left_in_staging_is_never_published ... ok
test a_long_extension_stays_in_the_record_but_not_in_the_stored_name ... ok
test a_storage_root_is_created_only_in_an_empty_folder ... ok
test a_later_version_changes_only_the_entities_it_touches ... ok
test an_entity_that_does_not_match_its_path_is_refused_on_read ... ok
test an_entity_whose_identifier_cannot_name_a_path_is_a_decode_error ... ok
test an_original_name_or_media_type_that_is_not_valid_is_refused ... ok
test an_unreadable_operation_message_is_refused_on_read ... ok
test an_upload_whose_declared_size_differs_builds_no_record ... ok
test canonical_failures_keep_their_meaning_at_the_catalog_port ... ok
test catalog_operations_map_to_history_kinds ... ok
test each_change_touches_the_library_and_its_own_entity ... ok
test every_entity_has_one_logical_path_and_back ... ok
test an_owner_name_round_trips_through_its_metadata_record ... ok
test a_new_upload_of_the_same_file_appends_one_import_entry ... ok
test identifiers_that_cannot_name_a_path_segment_are_refused ... ok
test the_metadata_record_is_the_metadata_modules_file_record ... ok
test the_head_record_committed_again_must_cite_the_bytes ... ok
test the_old_four_line_record_is_refused_on_read ... ok
test the_operation_message_carries_its_receipt ... ok
test the_stored_name_is_the_file_identifier_and_its_extension_only ... ok
test history_files_ride_the_snapshot_version_and_are_not_entities ... ok
test an_entity_file_larger_than_the_bound_is_refused_before_it_is_read ... ok
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
```

```text
Feature: Metadata property routes
  Scenario: An upload creates the file's object with its original and metadata record
   ✔  When the owner uploads "Holiday 2023.JPG" as a new file
   ✔  Then the file's object should hold the original and its metadata record in version "v1"
   ✔  And the record should name "Holiday 2023.JPG" with the client's facts and the receiving host's system, architecture and FileSquid version
   ✔  And every source, volume and attribute fact the upload did not supply should be listed as unavailable
  Scenario: An upload whose declared size differs commits nothing
   ✔  When the owner uploads "a.txt" declaring 9 bytes but sending 5
   ✔  Then the metadata answer should be the error "invalid_request"
   ✔  And no file object should have been created
  Scenario: Statements are set, read and removed, one version each
   ✔  Given the owner uploaded "harbour.jpg"
   ✔  When the owner sets "dcterms:title" to "Harbour"
   ✔  And the owner sets "dc:creator" to "Ana"
   ✔  And the owner removes "dcterms:creator"
   ✔  Then the file should show only "dcterms:title" as "Harbour"
   ✔  And the file's object should be at version "v4"
  Scenario: Unknown Dublin Core names and unregistered prefixes are refused
   ✔  Given the owner uploaded "a.txt"
   ✔  When the owner sets "dcterms:colour" to "red"
   ✔  Then the metadata answer should be the error "invalid_request"
   ✔  When the owner sets "rico:title" to "Fonds"
   ✔  Then the metadata answer should be the error "invalid_request"
   ✔  And the file's object should be at version "v1"
  Scenario: A registered namespace accepts its own terms
   ✔  Given the owner uploaded "a.txt"
   ✔  When the owner registers the namespace "rico" with the term "title"
   ✔  And the owner sets "rico:title" to "Fonds"
   ✔  Then the file should show only "rico:title" as "Fonds"
  Scenario: A bare term name is offered Dublin Core first
   ✔  Given the owner registered the namespace "rico" with the term "title"
   ✔  When the owner asks where "title" can go
   ✔  Then the answer should offer "dcterms:title", "schema:title" and "rico:title" in that order
  Scenario: The import record's facts show no absolute path, host or account
   ✔  Given the owner uploaded "notes.txt"
   ✔  When the owner reads the file's facts
   ✔  Then the facts should name "notes.txt" with its size and method "api_upload"
   ✔  And the facts should hold no absolute path, import root, host or account name
  Scenario: Content answers use the record's display name and media type
   ✔  Given the owner uploaded "Café.md" as "text/markdown"
   ✔  When the owner reads the file's original
   ✔  Then the answer should carry the media type "text/markdown" and the name "Café.md"
[Summary]
22 features
338 scenarios (236 passed, 102 skipped)
1161 steps (1059 passed, 102 skipped)
```

The head-commit hook run (`FULL=1 bash scripts/pre-commit.sh`, `Test scope: full`) is recorded in the pull request.

## Mocked Environment Variables

None. The tests use real filesystem OCFL roots under the system temporary directory (`TMPDIR`) and remove them.
