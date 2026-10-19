# Task MD-0.2 Walkthrough: Commit statements through filesquid-ocfl

Branch: `feat/md-0.2-commit-statements-through-ocfl`. Module: `filesquid-metadata` ([plan](../plan.md), [SRS](../srs.md)).

## MD-S-008: Canonical metadata is `metadata.md`; with the `ocfl` store each edit MUST be a new OCFL version committed through `filesquid-ocfl` and MUST NOT rewrite an earlier version

Also MD-S-001 (default public API is extract plus namespaced metadata CRUD), MD-S-003 (persist through a pluggable `MetadataStore`; the crate MUST NOT reimplement OCFL), MD-S-006 (CSV and XMP sidecar read/write; no MODS or SQL), MD-S-007 (extract MUST map EXIF, ID3, XMP and other supported packets onto `dcterms:` or `schema:` while keeping native observations) and MD-S-010 (JSON sidecar import/export), under the owner decisions of 2026-10-18 recorded in [ADR 027](../../../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md) Decision 2.

## What changed

- **`ocfl` store over the IO-0.10 change model** (`src/store/ocfl.rs`). `OcflStore` is wired by the host with its storage root, shared `VersionReservations`, a staging backend outside the root, a clock, a holder name and an object-identifier → object-root mapping. `commit_record` (and `MetadataStore::save`) discards this writer's stale change set, stages `metadata.md` with `ChangeModel::stage_bytes` and publishes it with `ChangeModel::commit`, so every edit is exactly one new version through the fenced `vN` reservation and head compare-and-swap. The crate builds no inventory and publishes no `vN` itself. An unchanged record publishes nothing (`NothingToCommit` → `None`). A `FileRef` that pins a version which is not the base the change set was staged against is a `StoreError::Conflict` (checked after staging, so a head that moves in between is caught), as is a change-model conflict, so a stale edit never overwrites a newer one. Every read checks that the object root holds the named object (`StoreError::WrongObject`). A head without its version inventory (W010) stays refused with `ChangeError::Commit(CommitError::MissingVersionInventory)` and nothing is repaired (OCFL-S-020). `read_record` (and `load`) reads `metadata.md` of the pinned version or the head through `Object::stream_file`, bounded to `MAX_RECORD_BYTES` (64 MiB) and checked against the inventory digest; an object with no version, or no `metadata.md`, is an empty record. `read_file` and `file_sha256` read or hash a logical file through `filesquid-ocfl`.
- **Removed** the raw-key helpers `metadata_key`, `load_from`, `save_to` and the path-based `load`/`save`, which wrote `metadata.md` straight into the object directory outside any version.
- **`LocalObject`** (`ocfl-fs`, native only): one object directory on the local disk. Leases and change staging live in `filesquid-metadata` under the system temporary directory, scoped by the SHA-256 of the resolved directory, never inside the object. A directory without an object needs an identifier; an existing object's identifier is checked. The Git guard still runs first.
- **CLI** (`src/cli.rs`): every command reads and commits through `LocalObject`, saving each edit pinned to the head it loaded (`load_head`, `save_over`), so a concurrent writer's newer record is never overwritten; `--id` creates or checks the object; `--file` names a logical file of the head (JSON binding digest, and the new `extract --file <logical-path>`, which reads bytes through `filesquid-ocfl`); a flag given without a value is refused; each command reports the committed version or "unchanged".
- **XMP packets** (`src/extract.rs`, `src/xmp.rs`): a third extraction pass finds the first `<x:xmpmeta>` packet in the first 16 MiB and maps its `dc:` and `dcterms:` elements (including `rdf:Alt`/`Bag`/`Seq` items, joined by `"; "`) onto `dcterms:`. Terms the register refuses are counted in `Extracted::rejected`, never silently dropped. `Extracted` gained `xmp`.
- **`StoreError`** gained `Conflict`, `WrongObject` and `MissingObjectId`.
- `tracing` is an optional dependency enabled only by the `ocfl` feature; `filesquid-ocfl` stays optional and outside the default features; no dependency on `filesquid-native`; no `filesquid-ocfl` source changed.
- The `sidecar` store already writes one `<filename>.metadata.md` beside each plain file (unchanged; covered by `tests/unit/sidecar.rs` and the F-052 sidecar scenario).

## SRS traceability

| Requirement | Tests |
|---|---|
| MD-S-008 per-edit OCFL versions | `unit_store`: `each_edit_commits_exactly_one_new_version`, `an_edit_never_modifies_an_earlier_version`, `metadata_md_is_a_logical_file_inside_a_version`, `an_unchanged_record_publishes_nothing`, `the_first_edit_of_a_new_object_creates_v1`, `a_pinned_version_that_is_not_the_head_is_a_conflict`, `local_object::an_edit_from_a_head_another_writer_moved_is_a_conflict`, `per_edit_versions_on_a_filesystem_backend`, `local_object::each_save_is_a_new_version_of_a_valid_object`; `unit_cli`: `set_and_get_round_trip_with_one_version_per_edit`, `import_csv_and_xmp_commit_new_versions`; F-052 scenarios "Each metadata edit is a new OCFL version", "An unchanged record publishes no version", "Canonical store is metadata.md" |
| OCFL-S-018 and OCFL-S-020: a W010 head stays refused as a base and is not repaired | `unit_store`: `a_head_without_its_version_inventory_stays_refused`; F-052 scenario "A head without its version inventory stays refused" |
| MD-S-003 no OCFL reimplementation; objects validate | `metadata_md_is_a_logical_file_inside_a_version` and the filesystem tests run the engine validator; F-052 "the object should pass the OCFL validator"; "Persistence is pluggable and filesquid-ocfl is optional" |
| MD-S-006 CSV and XMP through OCFL | `unit_cli`: `export_csv_contains_header`, `import_csv_and_xmp_commit_new_versions`, `csv_export_still_ignores_json_flags` |
| MD-S-007 EXIF, ID3, XMP extraction | `unit_extract`: `an_embedded_xmp_packet_maps_onto_dcterms`, `bytes_without_an_xmp_packet_report_none`, `an_unterminated_xmp_packet_is_not_an_error`; `unit_cli`: `extract_reads_a_logical_file_through_filesquid_ocfl`; F-052 "Extraction reads bytes from filesquid-ocfl", "Extraction maps an embedded XMP packet onto dcterms" |
| MD-S-010 JSON sidecar bound to the object and digest | `unit_cli`: `json_export_then_import_round_trips_through_metadata_md`, `json_import_rejects_digest_mismatch_and_publishes_nothing`, `json_import_rejects_other_object_id`, `json_rejects_a_file_the_object_does_not_hold`; `unit_store`: `file_sha256_hashes_the_logical_file_through_filesquid_ocfl`, `reading_a_root_that_holds_another_object_is_refused`; F-052 JSON sidecar scenarios |
| MD-S-001, MD-S-009 | F-052 "Core API is namespaced metadata", wasm32 scenarios (default features off, and `ocfl`) |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. The public items added in `src/store/ocfl.rs`, `src/extract.rs` and `src/xmp.rs` and the CLI's `extract`, `get`, `set`, `export` (CSV, XMP, JSON) and `import` (CSV, XMP, JSON) paths are exercised by the tests above; this is a list of covered scenarios, not a measured percentage.

## TDD: failing run before implementation

The tests above were written first. Run against the previous `src/` (the new tests stashed over the old sources), the build fails before any test runs, because the API they specify does not exist yet:

```text
      3 error[E0609]: no field `xmp` on type `Extracted`
      1 error: could not compile `filesquid-metadata` (test "unit_extract") due to 3 previous errors
```

## Gates

The pre-commit hook result for the head commit is recorded in the pull request (`FULL=1 bash scripts/pre-commit.sh`, `Test scope: full`).

- `cargo fmt --all`: clean.
- `cargo clippy -p filesquid-metadata --all-targets --all-features --locked -- -D warnings`, and with default features, `--no-default-features`, and `--no-default-features --features ocfl`: clean.
- `cargo doc -p filesquid-metadata --all-features --no-deps`: no warnings.
- `cargo check -p filesquid-metadata --target wasm32-unknown-unknown --no-default-features --features ocfl --locked`: builds.
- Documented commands run against a scratch object: `set ... --id`, `set`, `get`, `extract --file`, `export json --file`, `import json --file`, `metadata --help`.

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-metadata --locked` (default features; the `ocfl`/`cli` tests are skipped by `required-features`):

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_metadata-6617ebc00498accf)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/csv.rs (<target>/debug/deps/unit_csv-5817f75c47a65cab)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/extract.rs (<target>/debug/deps/unit_extract-53be4755fd49f9c9)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/file_ref.rs (<target>/debug/deps/unit_file_ref-a4b4f17aba2547cb)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-588910f5b0b3f82b)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/json.rs (<target>/debug/deps/unit_json-5bf7e87a332f21d4)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/markdown.rs (<target>/debug/deps/unit_markdown-1bc1aeb571b8deff)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/register.rs (<target>/debug/deps/unit_register-cf90b0f81a15bf07)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/sidecar.rs (<target>/debug/deps/unit_sidecar-72d4be7269e61762)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test -p filesquid-metadata --all-features --locked --no-fail-fast`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.20s
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_metadata-f851fec434ee5da3)

running 3 tests
test tests::accrual_is_rejected_on_file ... ok
test tests::title_is_allowed ... ok
test tests::name_is_unknown ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/features.rs (<target>/debug/deps/features-08e86a57382d7750)
Feature: Reusable filesquid-metadata engine
  Scenario: Core API is namespaced metadata
fsquid metadata — metadata CRUD (via filesquid-ocfl)

Usage:
  fsquid <command> [args] [--object <dir>] [--id <object-id>]
  fsquid metadata <command> [args] [--object <dir>] [--id <object-id>]

Commands:
  extract <file>          Extract EXIF/ID3/XMP from a media file into metadata.md
  extract --file <logical-path>
                          Extract from a logical file of the object
  get [prefix:term]       Print populated statements (or one CURIE)
  set <prefix:term> <val> Set a statement and commit metadata.md
  export csv|xmp|json <path>  Write CSV, XMP or JSON sidecar interchange
  import csv|xmp|json <path>  Read CSV, XMP or JSON sidecar into metadata.md
  validate <prefix:term>  Check a dcterms: term against the /terms/ register
                          (always run as `fsquid metadata validate`)
  help                    Show this help

--object is an OCFL object directory (default `.`). --id names the object:
required to create one in a directory without an object, and checked
against an existing object's identifier.

JSON sidecars also take --file <logical-path>: the logical file of the
object's head whose SHA-256 digest binds the sidecar with the object
identifier. Import rejects a sidecar whose identifier or digest differs.

Canonical store: metadata.md (YAML front matter + Markdown body), a logical
file of the OCFL object. Each edit commits a new OCFL version; earlier
versions are never modified.
   ✔  When I inspect the filesquid-metadata public API
exported json to <tmp>/filesquid-metadata-features-9860-4.json
exported json to <tmp>/filesquid-metadata-features-9860-5.json
exported json to <tmp>/filesquid-metadata-features-9860-6.json
exported json to <tmp>/filesquid-metadata-features-9860-7.json
imported json from <tmp>/filesquid-metadata-features-9860-5.json: committed v2
   ✔  Then dcterms register and statement operations should be available
   ✔  And FileSquid catalog, recipe, C2PA, and collection-snapshot types should be absent
  Scenario: Unknown dcterms local names are rejected
   ✔  When I author dcterms:name on a file
   ✔  Then the write should be rejected
  Scenario: Accrual terms are rejected on a file
   ✔  When I author dcterms:accrualMethod on a file
   ✔  Then the write should be rejected
  Scenario: Canonical store is metadata.md
   ✔  When I set dcterms:title Harbour on an object
   ✔  Then metadata.md YAML front matter should contain dcterms title Harbour
   ✔  And catalogs should be rebuildable from that file
  Scenario: Each metadata edit is a new OCFL version
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I set dcterms:title Dawn through the ocfl store
   ✔  Then the object should have a new version whose metadata.md holds dcterms title Dawn
   ✔  And the earlier versions should be unchanged
   ✔  And the object should pass the OCFL validator
  Scenario: An unchanged record publishes no version
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I set dcterms:title Harbour through the ocfl store
   ✔  Then no version should be published
  Scenario: A head without its version inventory stays refused
   ✔  Given an OCFL object whose head has no version inventory
   ✔  When I set dcterms:title Dawn through the ocfl store
   ✔  Then the edit should be refused for the missing version inventory
   ✔  And no version should be published
   ✔  And the missing version inventory should not be repaired
  Scenario: Persistence is pluggable and filesquid-ocfl is optional
   ✔  When I inspect filesquid-metadata library dependencies
   ✔  Then filesquid-ocfl should be an optional dependency outside the default features
   ✔  And axum, tauri, libvips, and filesquid-core should be absent
  Scenario: CLI is the stand-alone UI
   ✔  When I ask the metadata command runner for help
   ✔  Then I should see extract, get, set, export, import, and validate
  Scenario: CSV headers are prefix:term
   ✔  When I import a CSV with headers id,dcterms:title,dcterms:name
   ✔  Then dcterms:name should be rejected
  Scenario: Interchange excludes MODS and SQL
   ✔  When I inspect filesquid-metadata interchange
   ✔  Then CSV, JSON sidecar, XMP sidecar, and embedded extract should be available
   ✔  And MODS and SQL should be absent
  Scenario: JSON sidecar binds statements to the object and file digest
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I export the object's JSON sidecar
   ✔  Then the sidecar should be one UTF-8 JSON document for that object
   ✔  And it should carry the object identifier and the SHA-256 digest of the file
   ✔  And each statement should have a namespace, term, and value
  Scenario: JSON sidecar export then import reproduces the statements
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I export the object's JSON sidecar
   ✔  And I import that sidecar into an empty object holding the same file
   ✔  Then the imported metadata.md should hold the same statements
  Scenario: JSON sidecar import rejects a digest mismatch
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I export the object's JSON sidecar
   ✔  And I import that sidecar into an object whose file has different bytes
   ✔  Then the import should be rejected for a digest mismatch
   ✔  And that object's metadata.md should be unchanged
  Scenario: JSON sidecar import applies the /terms/ register
   ✔  When I import a JSON sidecar holding dcterms:name
   ✔  Then the JSON import should be rejected
  Scenario: JSON sidecar import rejects an unregistered namespace
   ✔  When I import a JSON sidecar holding rico:title
   ✔  Then the JSON import should be rejected naming the prefix rico
  Scenario: JSON sidecar does not replace metadata.md
   ✔  Given an OCFL object with a logical file and dcterms:title Harbour
   ✔  When I export the object's JSON sidecar
   ✔  Then metadata.md should remain the canonical store in the object
  Scenario: Stand-alone FOSS product
   ✔  When I inspect filesquid-metadata product identity
   ✔  Then it should remain installable without the FileSquid app
   ✔  And it should build and publish without FileSquid application crates
  Scenario: Engine builds for the browser
   ✔  When I build filesquid-metadata for wasm32-unknown-unknown with default features off
   ✔  Then the build should succeed
  Scenario: Engine builds for the browser with the OCFL store
   ✔  When I build filesquid-metadata for wasm32-unknown-unknown with the ocfl feature
   ✔  Then the build should succeed
  Scenario: Extraction reads bytes from filesquid-ocfl
   ✔  When I extract metadata from a logical file in an OCFL object
   ✔  Then the extractor should read bytes supplied by filesquid-ocfl, not a filesystem path
  Scenario: Extraction maps an embedded XMP packet onto dcterms
   ✔  When I extract metadata from bytes holding an XMP packet with dc:title Harbour
   ✔  Then the extracted dcterms:title should be Harbour
  Scenario: Extraction takes a reader and a file reference, independent of any store
   ✔  When I inspect the filesquid-metadata extraction API
   ✔  Then extraction should take bytes or a reader plus a FileRef and no path
  Scenario: Sidecar store keeps a record beside a plain file
   ✔  Given a plain file in a directory
   ✔  When I save a record through the sidecar store
   ✔  Then the record should be loadable from the sidecar beside the file
  Scenario: CLI and path helpers are native-only
   ✔  When I inspect the filesquid-metadata native-only modules
   ✔  Then the CLI, the sidecar store and the OCFL path helpers should be compiled out for wasm32
  Scenario: Metadata panel plugs into a file browser sidebar
   ?  Given the filesquid-ocfl file browser with a selected file
      Step skipped: tests/features/reusable_metadata_engine_F-052.feature:136:5
[Summary]
1 feature
26 scenarios (25 passed, 1 skipped)
73 steps (72 passed, 1 skipped)
     Running tests/unit/cli.rs (<target>/debug/deps/unit_cli-fca5a15484d2aaf5)

running 20 tests
test export_csv_contains_header ... ok
test extract_reads_a_logical_file_through_filesquid_ocfl ... ok
test csv_export_still_ignores_json_flags ... ok
test help_lists_json_interchange ... ok
test help_prints_commands ... ok
test extract_reports_a_missing_file ... ok
test extract_reads_a_plain_file_through_a_reader_into_metadata_md ... ok
test json_import_rejects_other_object_id ... ok
test json_rejects_a_file_the_object_does_not_hold ... ok
test json_rejects_a_flag_as_the_id_value ... ok
test import_csv_and_xmp_commit_new_versions ... ok
test json_requires_file ... ok
test json_export_then_import_round_trips_through_metadata_md ... ok
test json_import_rejects_digest_mismatch_and_publishes_nothing ... ok
test unknown_command_errors ... ok
test validate_name_fails ... ok
test validate_title_ok ... ok
test set_on_a_new_object_needs_an_identifier ... ok
test set_keeps_json_flag_names_as_literal_values ... ok
test set_and_get_round_trip_with_one_version_per_edit ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

     Running tests/unit/csv.rs (<target>/debug/deps/unit_csv-5bd695e62e5a615c)

running 10 tests
test dcterms_title_header ... ok
test physically_blank_lines_are_skipped ... ok
test line_skips_id_column ... ok
test quoted_cell_maps_to_its_own_column ... ok
test quoted_whitespace_value_is_kept ... ok
test round_trip_keeps_commas ... ok
test unknown_dcterms_header_rejected ... ok
test round_trip_keeps_quotes ... ok
test round_trip_keeps_newlines ... ok
test unterminated_quote_is_rejected ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/extract.rs (<target>/debug/deps/unit_extract-cb8ec81a5ddb769e)

running 8 tests
test a_reader_can_be_extracted_twice ... ok
test an_unterminated_xmp_packet_is_not_an_error ... ok
test bytes_without_an_xmp_packet_report_none ... ok
test bytes_without_tags_extract_nothing_and_are_not_an_error ... ok
test exif_from_bytes_sets_native_and_dcterms_statements ... ok
test id3_from_a_reader_maps_title_and_artist ... ok
test truncated_tag_blocks_are_not_an_error ... ok
test an_embedded_xmp_packet_maps_onto_dcterms ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/file_ref.rs (<target>/debug/deps/unit_file_ref-a62b5dba5ec281fe)

running 3 tests
test accessors_return_what_the_caller_supplied ... ok
test escaping_or_empty_logical_paths_are_rejected ... ok
test version_is_optional_and_object_id_may_be_empty ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-2ad164b104aef085)

running 4 tests
test default_features_exclude_filesquid_ocfl ... ok
test excludes_hosts_codecs_and_filesquid_core ... ok
test the_ocfl_feature_enables_the_optional_dependency ... ok
test filesquid_ocfl_is_an_optional_dependency ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/json.rs (<target>/debug/deps/unit_json-5f5532124da20da9)

running 19 tests
test digest_algorithm_is_sha256 ... ok
test array_of_objects_is_rejected ... ok
test export_binds_object_id_and_digest ... ok
test export_then_import_reproduces_statements ... ok
test export_writes_namespace_term_value_statements ... ok
test export_is_utf8_without_escaping_unicode ... ok
test import_accepts_uppercase_digest_hex ... ok
test import_maps_dc_onto_dcterms ... ok
test import_merges_into_existing_record ... ok
test import_rejects_accrual_term_on_file ... ok
test import_rejects_other_digest_algorithm ... ok
test import_rejects_digest_mismatch ... ok
test import_rejects_other_object_id ... ok
test import_rejects_unknown_dcterms_term ... ok
test import_rejects_unregistered_namespace_by_name ... ok
test malformed_json_is_rejected ... ok
test missing_digest_is_rejected ... ok
test rejected_import_leaves_record_unchanged ... ok
test sha256_matches_fips_vector ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/markdown.rs (<target>/debug/deps/unit_markdown-d013e68ff32a4bec)

running 3 tests
test line_breaks_survive_metadata_md ... ok
test yaml_front_matter_round_trip ... ok
test quotes_and_backslashes_survive_metadata_md ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/register.rs (<target>/debug/deps/unit_register-293af9f1979c216e)

running 10 tests
test close_and_unmatched_terms_do_not_alias ... ok
test creator_is_accepted ... ok
test dc_title_maps_to_dcterms_title ... ok
test one_to_one_map_has_22_rows_of_registered_terms ... ok
test one_to_one_map_is_injective_both_ways ... ok
test one_to_one_map_matches_expected_table ... ok
test renamed_equivalents_map ... ok
test title_maps_to_schema_name_both_ways ... ok
test unknown_dc_name_does_not_map ... ok
test unknown_term_is_rejected ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/sidecar.rs (<target>/debug/deps/unit_sidecar-5462564e5b35473b)

running 8 tests
test a_missing_parent_directory_is_a_backend_error ... ok
test a_git_working_tree_is_refused ... ok
test a_file_without_a_sidecar_loads_an_empty_record ... ok
test a_sidecar_that_is_not_utf8_is_an_error_not_a_lossy_record ... ok
test a_symlinked_directory_cannot_leave_the_root ... ok
test save_then_load_round_trips_beside_the_file ... ok
test a_symlinked_staging_file_cannot_redirect_the_write ... ok
test save_replaces_the_earlier_record ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/store.rs (<target>/debug/deps/unit_store-eab858085c3416e1)

running 23 tests
test a_committed_record_that_is_not_utf8_is_an_error ... ok
test a_csv_line_break_survives_commit_and_load ... ok
test a_missing_object_or_metadata_md_is_an_empty_record ... ok
test a_head_without_its_version_inventory_stays_refused ... ok
test an_object_root_that_could_escape_is_an_invalid_path ... ok
test a_stale_change_set_of_this_writer_is_discarded_before_staging ... ok
test a_pinned_version_that_is_not_the_head_is_a_conflict ... ok
test an_unchanged_record_publishes_nothing ... ok
test local_object::a_git_working_tree_is_refused ... ok
test local_object::a_new_object_directory_needs_an_identifier ... ok
test file_sha256_hashes_the_logical_file_through_filesquid_ocfl ... ok
test an_edit_never_modifies_an_earlier_version ... ok
test each_edit_commits_exactly_one_new_version ... ok
test local_object::another_identifier_is_refused ... ok
test metadata_md_is_a_logical_file_inside_a_version ... ok
test local_object::the_digest_of_a_missing_logical_file_is_an_error ... ok
test local_object::each_save_is_a_new_version_of_a_valid_object ... ok
test reading_a_root_that_holds_another_object_is_refused ... ok
test staging_is_cleared_after_each_edit ... ok
test local_object::an_edit_from_a_head_another_writer_moved_is_a_conflict ... ok
test the_first_edit_of_a_new_object_creates_v1 ... ok
test per_edit_versions_on_a_filesystem_backend ... ok
test records_larger_than_one_put_round_trip ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.48s

   Doc-tests filesquid_metadata

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

## PR #76 review fixes (Cubic round 1)

- P1: each `LocalObject` handle gets its own writer name (process id plus a process-wide sequence number), so two handles in one process never share a staging area and one handle's reset cannot discard or publish the other's change set (`two_local_objects_in_one_process_stage_apart`).
- P2: the test rig maps an object identifier to `objects/<sha256>`, so distinct identifiers never share a root.
- P2: the "no version should be published" step accepts only `Ok(None)` or the W010 `MissingVersionInventory` refusal.
- P2: an XMP packet must open with `<x:xmpmeta` followed by whitespace, `>` or `/` (`a_name_that_only_starts_like_xmpmeta_is_not_a_packet`).
- P2: XMP values decode `&quot;`, `&apos;`, decimal and hex character references, and CDATA sections (`xmp_values_decode_every_entity_reference_and_cdata`).
- P2: an element without its close ends the XMP scan, so a malformed packet costs one pass (`an_unclosed_element_ends_the_scan_without_an_error`).

## PR #76 review fixes (Cubic rounds 1 and 2, remaining threads)

- P2: `export json` and `import json` hash the `--file` logical file at the same version the record was loaded from (`LocalObject::file_sha256` and `read_file` take a version), so a concurrent edit cannot pair old statements with a new digest; `extract --file` reads that version too.
- P2: `extract` refuses a media path and `--file` together (`extract_refuses_a_path_and_a_logical_file_together`).
- P2 (not changed): a pinned edit may rebase onto a later head. OCFL-S-016 requires the change model to rebase when no intervening version changed a staged path, and to report a conflict otherwise; `metadata.md` is the only staged path, so any intervening change to it is a conflict and no record is overwritten. The behaviour is documented on `check_pinned`.
- P2: an unterminated CDATA section keeps its `<![CDATA[` opener (`an_unterminated_cdata_section_is_kept_as_written`); any whitespace after an element name marks attributes (`whitespace_before_attributes_does_not_end_the_scan`).
- P3: ADR 027 cites OCFL-S-018 for the refused base and OCFL-S-020 for no repair; `MetadataStore::save` says an unchanged record publishes nothing; the README separates read and edit commands and states that `--file` names a logical file the object already holds; the XMP feature step declares `rdf` and `dc`; the W010 fixtures delete only `v1/inventory.json`; `export_xmp_writes_the_head_record` covers the CLI XMP export; the coverage line above lists covered scenarios.

## PR #76 review fixes (Cubic round 3)

- P2: the pinned `read_file` test compares the `v1` bytes with the head bytes, so an implementation that ignores the version fails.
