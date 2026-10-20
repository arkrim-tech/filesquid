# Task MD-0.8 Walkthrough: Import record

Branch: `feat/md-0.8-import-record` (part of global Task FS-1.3).

## MD-S-013: Import record

Also MD-S-008: Canonical metadata record (one per-file record `<file-uuid>.metadata.md`, each edit one new OCFL version).

## What changed

- `raw::Raw`: exact bytes of a name or value. UTF-8 is written as a string; any other bytes as `{"encoding":"base64","value":...}` (RFC 4648, no new dependency).
- `fact::Fact`: a known value or `{"unavailable": <reason>}` (`not_supported_by_source`, `permission_denied`, `not_provided_by_upload`, `withheld_by_policy`). Every fact a source or host can fail to give is a `Fact`, so a missing value is always listed with a reason.
- `import`: `ImportEntry` (`Acquisition`, `SourceFile`, `SourceVolume`, `Attributes`, `Host`), strict serde (unknown or missing fields refused), `validate` (reported size equals verified size, cited size equals verified size, RFC 3339 UTC times, cited digest, logical path rules), `ImportEntry::for_upload` (upload profile: the rest unavailable with `not_provided_by_upload`).
- `import::attributes`: `AttributeCollector` and `Attributes::validate` enforce 256 names, 1 MiB of value bytes, 64 KiB stored values (larger: size and SHA-256 only) and the truncated and incomplete marks.
- `import::scrub`: `IdentityClass` (path, account, host, numeric ID, origin URL, quarantine agent, Finder comment) and `ImportEntry::scrubbed`.
- `file_record::FileRecord`: the whole `<file-uuid>.metadata.md` (schema `filesquid.original.v1`, original name and normalisation form, display name, extension, format, media type, statements, `import` list), `check_extends` (the append-only rule), `check_binding`, `facts()` (the read projection without absolute path, import root, host or account), and the naming helpers `extension_of`, `stored_extension`, `record_name`.
- `FileRef::with_record_path`: the caller names the record's logical path; without it the record stays `metadata.md` (CLI and stand-alone callers unchanged).
- `store::ocfl::OcflStore`: the record is read and written at the `FileRef`'s record path. New: `commit_import_with` (record committed over content staged through `change_model`, one version), `append_import_with`, `commit_file_record_with`, `read_file_record`; a statements-only `save` keeps the header and the import section; every save is refused when it edits or removes an import entry or the original name (`StoreError::Record`).

## Design notes for API-F1.4 and API-F1.13 (not implemented here)

- API-F1.4's upload: build `ImportEntry::for_upload`, `FileRecord::new`, stage the original with `change_model(..).stage_bytes`, then `commit_import_with` (one version, the caller's `CommitOptions`). The read-only route answers `FileRecord::facts()`.
- API-F1.13: the display name is `FileRecord::display_name()`; a rename is `set_display_name` plus `commit_file_record_with` (a new version; the original name and every import entry stay). The stored name builder stays in API-F1.13 and can use `stored_extension`; the `FileRef` is `FileRef::new(object, "original/<uuid>.<ext>")` with `with_record_path("original/<uuid>.metadata.md")`.
- Decision: the first import entry cites `object_version` as the caller supplies it (the version that holds the content); the crate checks the object and logical path, not the version, because the version is not known before the commit.
- Decision: the original file name is not an identity class of the scrub; API-S-027's sharing default (FileId plus extension) already covers it.

## Cubic round 1 changes

- The import commit now verifies the staged content: a new managed entry's content MUST be staged at its cited logical path with the entry's SHA-256 (`commit_import_with`), or held by the object's head with that digest (`append_import_with`); otherwise `RecordError::ContentNotStaged` or `ContentDigestMismatch`, and nothing is published. A record saved at its own content path is refused (`RecordAtContentPath`).
- The extension MUST be the original name's; `import:` parsing refuses any non-entry line instead of dropping it; Schema.org statements in a file record are written under `schemaorg:` because the record's `schema:` identifier takes that key (both keys are read); `facts()` shows the display name after the one safe transformation of API-S-005 (`safe_display_name`).
- Validation: timestamps accept any fraction length and allow `60` seconds only as `23:59:60`; whole attribute values may not exceed the bytes the entry says it read; a managed entry's logical path must be a valid logical path; an upload lists a missing UTI as unavailable with a reason and keeps the extension of a non-UTF-8 name; a duplicate field in a base64 envelope is refused.
- Tests strengthened: every time field, exact SHA-256 values, the projection's relative path and volume name, numeric-ID scrub, referenced import, re-import budget, store refusals.

## SRS traceability

| Requirement | Tests |
|---|---|
| MD-S-013 Import record: record, schema, header fields | `unit_import_record::file_record::*`, BDD "The record opens with the original name, display name, extension, format and media type" |
| MD-S-013 Import record: unavailable facts with a reason | `entry_validation::the_upload_profile_is_valid_and_lists_what_an_upload_cannot_know`, `an_unavailable_fact_serializes_with_its_reason`, BDD "Every missing fact is listed as unavailable with a reason" |
| MD-S-013 Import record: reported size must equal verified size | `differing_reported_and_verified_sizes_are_rejected`, BDD "A source size that differs from the verified size is rejected" |
| MD-S-013 Import record: RFC 3339 UTC times | `timestamps_must_be_rfc_3339_utc`, `a_file_time_must_be_rfc_3339_utc_too` |
| MD-S-013 Import record: base64 for non-UTF-8 | `raw_values::*`, `a_non_utf8_original_name_round_trips_exactly`, `a_non_utf8_name_survives_the_store`, BDD "Names and values that are not UTF-8 round-trip as base64" |
| MD-S-013 Import record: bounds: 256 names, 1 MiB, 64 KiB, truncated, incomplete | `attribute_bounds::*`, BDD "Attribute inspection stops at 256 names and 1 MiB and marks what it cut", "A value over 64 KiB is kept as size and SHA-256 only" |
| MD-S-013 Import record: append-only, refusal of a changed or removed entry or the original name | `a_save_that_changes_an_import_entry_is_refused`, `..._removes_an_import_entry_...`, `..._changes_the_original_name_...` (pure and store), BDD refusal scenarios |
| MD-S-013 Import record: re-import appends in a new OCFL version | `a_re_import_appends_an_entry_in_a_new_version_and_leaves_v1_untouched`, BDD "A re-import appends an entry in a new OCFL version" |
| MD-S-013 Import record: no filesystem reads, wasm32 | BDD "The import record builds for the browser without filesystem access", "Engine builds for the browser" (default features and `ocfl`) |
| MD-S-013 Import record, with ADR 038 Decision 6: read access and identity classes | `projection_and_scrub::*`, BDD "The read projection omits paths, host and account", "The outgoing policy scrubs the identity fields" |
| MD-S-008 Canonical metadata record (one `<file-uuid>.metadata.md` per file, one OCFL version per edit): per-file record path, one version per edit | `the_record_lives_beside_the_original_in_the_same_version_as_its_content`, `a_statements_only_save_keeps_the_header_and_every_import_entry`, `a_plain_metadata_md_is_unchanged_by_the_per_file_path`, BDD "A file's one record lives beside its stored original in the same version", "A statements edit keeps the header and the import section", "A rename changes only the display name" |

## TDD: failing run before implementation

```text
error[E0432]: unresolved import `filesquid_metadata::fact`
error[E0432]: unresolved import `filesquid_metadata::file_record`
error[E0432]: unresolved import `filesquid_metadata::import`
error[E0432]: unresolved import `filesquid_metadata::raw`
error[E0599]: no method named `commit_import_with` found for struct `OcflStore<...>`
error[E0599]: no method named `append_import_with` found for struct `OcflStore<...>`
error[E0599]: no method named `commit_file_record_with` found for struct `OcflStore<...>`
error[E0599]: no method named `read_file_record` found for struct `OcflStore<...>`
error[E0599]: no method named `with_record_path` found for struct `FileRef`
error: could not compile `filesquid-metadata` (test "unit_import_record") due to 15 previous errors
error: could not compile `filesquid-metadata` (test "unit_file_record_store") due to 35 previous errors
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-metadata --features ocfl --locked --test unit_import_record --test unit_file_record_store`:

```text
Compiling filesquid-metadata v0.1.0 (/home/user/filesquid/apps/filesquid/crates/metadata)
    Finished `test` profile [unoptimized] target(s) in 2.22s
     Running tests/unit/file_record_store.rs (<target>/debug/deps/unit_file_record_store-8a22a1ebe8b6b073)

running 17 tests
test a_non_utf8_name_survives_the_store ... ok
test a_plain_metadata_md_is_unchanged_by_the_per_file_path ... ok
test a_record_path_must_be_a_valid_logical_path ... ok
test a_re_import_uses_the_callers_commit_options ... ok
test a_re_import_appends_an_entry_in_a_new_version_and_leaves_v1_untouched ... ok
test a_save_that_changes_an_import_entry_is_refused_and_publishes_nothing ... ok
test a_save_that_changes_the_original_name_is_refused ... ok
test a_rename_is_a_new_version_that_changes_only_the_display_name ... ok
test an_entry_must_cite_the_object_and_the_path_of_the_file_it_is_committed_for ... ok
test a_statements_only_save_without_a_file_record_is_refused ... ok
test a_save_that_removes_an_import_entry_is_refused ... ok
test appending_to_a_missing_record_is_refused ... ok
test a_statements_only_save_keeps_the_header_and_every_import_entry ... ok
test an_unchanged_record_publishes_nothing ... ok
test the_record_lives_beside_the_original_in_the_same_version_as_its_content ... ok
test the_record_reads_back_exactly ... ok
test the_store_type_is_usable_with_the_default_commit_helper ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/unit/import_record.rs (<target>/debug/deps/unit_import_record-2b03bca8d17c10d8)

running 46 tests
test attribute_bounds::a_non_utf8_name_and_value_are_stored_as_base64 ... ok
test attribute_bounds::a_value_cut_short_has_an_incomplete_digest ... ok
test attribute_bounds::a_value_up_to_64_kib_is_stored_whole ... ok
test attribute_bounds::dropping_names_without_marking_truncation_is_rejected ... ok
test attribute_bounds::a_hand_built_entry_over_a_bound_is_rejected ... ok
test attribute_bounds::the_257th_name_is_refused_and_the_entry_is_marked_truncated ... ok
test attribute_bounds::a_larger_value_is_kept_as_size_and_digest_only ... ok
test entry_validation::a_managed_import_cites_its_logical_path_and_a_referenced_one_does_not ... ok
test entry_validation::a_file_time_must_be_rfc_3339_utc_too ... ok
test entry_validation::an_unavailable_fact_serializes_with_its_reason ... ok
test entry_validation::an_unavailable_reported_size_is_allowed ... ok
test entry_validation::differing_reported_and_verified_sizes_are_rejected ... ok
test entry_validation::the_cited_digest_is_64_lowercase_hex_digits ... ok
test entry_validation::the_cited_size_must_be_the_verified_size ... ok
test entry_validation::the_upload_profile_is_valid_and_lists_what_an_upload_cannot_know ... ok
test entry_validation::timestamps_must_be_rfc_3339_utc ... ok
test file_record::a_new_record_starts_with_the_original_name_as_its_display_name ... ok
test entry_validation::json_round_trips_and_every_field_must_be_present ... ok
test file_record::a_missing_header_field_or_a_broken_entry_is_refused ... ok
test file_record::a_re_import_appends_and_leaves_earlier_entries_unchanged ... ok
test file_record::a_non_utf8_original_name_round_trips_exactly ... ok
test file_record::a_record_without_an_import_entry_is_invalid ... ok
test file_record::a_rename_changes_only_the_display_name ... ok
test file_record::a_save_that_removes_an_import_entry_is_refused ... ok
test file_record::a_save_that_changes_an_import_entry_is_refused ... ok
test file_record::a_wrong_or_missing_schema_is_refused ... ok
test file_record::names_and_types_are_validated ... ok
test file_record::a_save_that_changes_the_original_name_is_refused ... ok
test file_record::statements_and_the_display_name_may_change ... ok
test file_record::with_statements_keeps_the_header_and_the_imports ... ok
test file_record::the_markdown_opens_with_the_schema_and_the_names ... ok
test names::the_extension_is_what_follows_the_last_dot ... ok
test file_record::markdown_round_trips_statements_body_and_imports ... ok
test names::the_record_name_is_the_file_uuid_plus_metadata_md ... ok
test names::the_stored_extension_is_one_to_sixteen_ascii_letters_or_digits ... ok
test projection_and_scrub::each_identity_class_reaches_its_own_fields ... ok
test projection_and_scrub::known_attributes_are_withheld_by_their_class_and_keep_name_and_size ... ok
test projection_and_scrub::scrubbing_a_class_withholds_exactly_its_fields ... ok
test raw_values::base64_follows_rfc_4648_vectors ... ok
test projection_and_scrub::the_projection_names_the_file_and_the_import_but_no_location_or_account ... ok
test projection_and_scrub::scrubbing_every_class_leaves_no_identity_value ... ok
test raw_values::invalid_base64_or_an_unknown_encoding_is_refused ... ok
test raw_values::other_bytes_are_base64_marked_with_their_encoding ... ok
test raw_values::every_byte_value_round_trips ... ok
test raw_values::utf8_is_a_plain_string ... ok
test attribute_bounds::the_read_budget_is_one_mebibyte_across_the_entry ... ok

test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

`cargo test -p filesquid-metadata --all-features --locked` (all binaries, including the BDD feature run with both `wasm32` builds): exit 0; per binary: lib 3, cli 22, csv 10, extract 13, file_record_store 23, file_ref 3, import_record 58, isolation 4, json 19, markdown 3, register 10, sidecar 8, store 28 passed, 0 failed. BDD: 43 scenarios (42 passed, 1 skipped), 129 steps (128 passed, 1 skipped); the skipped step is the existing UI-only "Metadata panel" scenario.

`cargo check -p filesquid-metadata --lib --no-default-features [--features ocfl] --target wasm32-unknown-unknown --locked`: both `Finished`.

`cargo clippy -p filesquid-metadata --all-targets [--all-features] --locked -- -D warnings` and `cargo doc -p filesquid-metadata --no-deps --all-features --locked`: clean.
