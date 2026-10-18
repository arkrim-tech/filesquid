# Task CP-0.3 Walkthrough: Hard bindings and JPEG/PNG handlers

Global task: [Task 0.17](../../../../../../resources/implementation-plan.md) (third slice; CP-0.4 and CP-0.5 follow, so the global box stays open). Module plan: [plan.md](../plan.md). Branch: `feat/cp-0.3-bindings-handlers`. Builds on CP-0.2 ([walkthrough](./Task-CP-0.2-Walkthrough.md)).

## C2PA-S-005: Format support MUST go through a format-handler interface. Unsupported formats MUST be reported as unsupported

Full requirement text ([module SRS](../srs.md)): "Format support MUST go through a format-handler interface. Unsupported formats MUST be reported as unsupported." The new `format` module defines the `FormatHandler` trait (`format`, `recognizes`, `layout`) with `JpegHandler` (APP11) and `PngHandler` (`caBX`). `asset_layout` returns `None` for any other format, and `read::read_asset` turns that into `AssetManifest::Unsupported`: an explicit result, never an error and never a silent empty store.

## C2PA-S-002: The crate MUST validate hard bindings, COSE_Sign1 claim signatures, certificate chains against caller-supplied trust lists, RFC 3161 timestamps, available revocation status and ingredient chains, and MUST report the specification's validation status codes

Full requirement text: "The crate MUST validate hard bindings, COSE_Sign1 claim signatures, certificate chains against caller-supplied trust lists, RFC 3161 timestamps, available revocation status and ingredient chains, and MUST report the specification's validation status codes." This task delivers the **hard-binding** part: `binding::validate_hard_binding` checks data-hash, box-hash and BMFF-hash (v2, v3) assertions with their exclusion ranges and reports the C2PA 2.4 status codes. Signatures, certificate chains, timestamps and revocation are CP-0.4; the full report is CP-0.5.

## C2PA-S-001: The crate MUST read manifest stores embedded in assets, in sidecar `.c2pa` files and by remote reference, parsing JUMBF boxes and CBOR claims and assertions

Full requirement text: "The crate MUST read manifest stores embedded in assets, in sidecar `.c2pa` files and by remote reference, parsing JUMBF boxes and CBOR claims and assertions." CP-0.2 left the **embedded** part to this task. `read_asset` now locates and reads stores embedded in JPEG and PNG assets, so the scenario "Read a manifest store" (a JPEG), skipped since CP-0.2, now runs and passes. The test-only APP11 extractor of CP-0.2 (`tests/support/jpeg_app11.rs`) is promoted to the library's JPEG handler and deleted; its fill-byte and stray-byte behaviour and its truncation errors are kept and tested.

Also covered: [C2PA-S-008](../srs.md) (every asset byte goes through the caller's `ByteSource` and every hash through the caller's `CryptoBackend`; the CP-0.1 source-scan and isolation tests cover the new modules unchanged), [C2PA-S-009](../srs.md) (both `wasm32-unknown-unknown` builds ran in this task's test runs), [PRD F-054](../../../../../../resources/prd.md#f-054-c2pa-module), [ADR 026](../../../../../../resources/architecture/026-filesquid-provenance-engine.md) (bindings and format handlers are this crate's own code).

## Specification checks (C2PA 2.4)

Verified against the published HTML of the [C2PA 2.4 specification](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html):

- 13.1 and 15.4.1: the hash algorithm is the structure's `alg`, else the claim's `alg`; only `sha256`, `sha384` and `sha512` are allowed; otherwise `algorithm.unsupported`.
- 9.1, 11.2.2 and 15.12: exactly one hard binding per standard manifest (`claim.hardBindings.missing`, `assertion.multipleHardBindings`); an update manifest has none and its parent's applies, reached through its one `parentOf` ingredient (`activeManifest` in v3, `c2pa_manifest` in v1 and v2). 18.6.1: `c2pa.hash.bmff` (v1) is ignored by validators.
- 15.12.1 and 18.5: data-hash exclusions are ordered and do not overlap; "the data contained within the exclusion range containing the C2PA Manifest Store consists of only the C2PA Manifest Store and any appropriate padding ... If a validator encounters any data other than what is permitted, then the manifest shall be rejected with a failure code of `assertion.dataHash.mismatch`"; other exclusions give `assertion.dataHash.additionalExclusionsPresent`; with update manifests the store exclusion takes the current store length. JPEG: the APP11 segments including markers and lengths; PNG: the `caBX` chunk including length and type.
- 15.12.3 and 18.7: box-hash names in asset order (`mismatch` when out of order, `assertion.boxesHash.unknownBox` for a box not listed), `exclusions` relative to the box at `boxIndex`, `excluded` entries skipped, the store box `C2PA` never hashed, `PNGh` for the PNG signature, `c2pa.after` for bytes after the last box; JPEG names from ISO 10918-1 Table B.1 with `SOS` and `RSTn` including the entropy-coded data after them.
- 15.12.2, 18.6 and A.5.6: BMFF exclusions with the restricted xpath, `length`, `version`, `flags` + `exact`, `data` and `subset`; each root box not wholly excluded is hashed as its 64-bit big-endian offset followed by its included bytes; the C2PA `uuid` box, `ftyp` and `mfra` are the expected exclusions.
- A.3.4 (JPEG): APP11 packets `JP`, `En` (box instance), `Z` (sequence from 1), then the JUMBF box; continuation packets repeat the box header; packets are contiguous and in sequence. A.3.3 (PNG): the store is the data of a `caBX` chunk.
- 15.5.2.1: more than one embedded store is reported as `AssetManifest::MultipleStores`, not as a store.

## What changed

- **`src/format/`:** `FormatHandler`, `AssetFormat`, `AssetLayout` (each box with its C2PA box-hash name and byte range, and each embedded store's container, parts and length), `detect_format`, `asset_layout`, `read_store_bytes`, `FormatError` (with offsets and a fixed log category). `jpeg.rs` walks markers, skips `0xFF` fill and stray bytes, includes the entropy-coded data after `SOS` and each `RSTn`, and reassembles the C2PA APP11 packets with contiguity, sequence and length checks. `png.rs` walks chunks to `IEND`. `reader.rs` is a 64 KiB windowed reader over the caller's `ByteSource`.
- **`src/binding/`:** `validate_hard_binding`, `BindingReport`, `BindingStatus` (`Valid`, `Invalid`, `NotChecked`), `BindingKind`, `BindingError`; `select.rs` (manifest and assertion selection, update chain bounded by the manifest count), `data.rs`, `boxes.rs`, `bmff.rs`. Hashing goes through `CryptoBackend::digest_ranges` and, for BMFF, `incremental_hasher`.
- **`src/read.rs`:** `read_asset` and `AssetManifest` (`Unsupported`, `NotEmbedded`, `MultipleStores`, `Embedded`), `ReadError::Format`.
- **`src/manifest/mod.rs`:** `ParseLimits::max_asset_boxes` (default 262 144).
- **`src/crypto/mod.rs`:** `hash_range` is `pub(crate)` for the BMFF hasher.
- **Tests:** `unit_format`, `unit_binding`, `unit_bmff`, `performance_bindings` (new); `integration_conformance_parse` now reads JPEGs through `read_asset`, validates every hard binding and compares it with the corpus; synthetic JPEG, PNG and MP4 builders with independently computed hashes in `tests/support/assets.rs`; a test-only MP4 store locator in `tests/support/bmff_store.rs` until the BMFF handler (CP-2.2); `tests/support/jpeg_app11.rs` deleted.
- **BDD:** seven new scenarios (unsupported format, PNG read, JPEG data hash match and mismatch, PNG box hash match and unknown box, MP4 BMFF match); "Read a manifest store" (JPEG) is no longer skipped.
- **Docs:** crate README (status, public surface, format handlers and hard bindings, limits, conformance test), component diagram (format handlers and hard bindings built), module plan box, root checklist box.
- **Dependencies:** none added.

## Engine limits

| Limit | Default | Boundary tests |
|---|---|---|
| `ParseLimits::max_asset_boxes` (JPEG segments, each APP11 packet and RST marker; PNG chunks) | 262 144 | `unit_format::the_asset_box_limit_holds_at_the_boundary` (JPEG 7 and PNG 5 boxes pass at 7 and 5, fail at 6 and 4); `unit_manifest::default_limits_are_documented_values` |
| `ParseLimits::max_store_bytes` for an embedded store | 64 MiB | `unit_format::a_store_over_the_size_limit_is_rejected_before_reading_it` (`FormatError::StoreTooLarge` before any store byte is read) |
| BMFF xpath nodes | 16 | `unit_bmff::an_xpath_at_the_node_limit_is_accepted` (16 nodes), `malformed_bmff_maps_are_malformed` (17 nodes) |
| BMFF boxes walked per hash | 262 144 (`DEFAULT_MAX_ASSET_BOXES`) | covered by the shared `BoxBudget`; see open questions |
| Update-manifest chain | the number of manifests in the store | `unit_binding::an_update_manifest_without_a_parent_has_no_hard_binding` (orphan and cycle) |

Malformed and truncated input: every prefix and 2 000 random corruptions of a synthetic JPEG and PNG (`unit_format::truncated_and_corrupted_assets_never_panic`) and of the synthetic MP4 (`unit_bmff::truncated_and_corrupted_bmff_assets_never_panic`) are rejected or validated without a panic. Every scan loop moves forward through the asset.

## Conformance corpus (end to end)

`integration_conformance_parse` with `FILESQUID_C2PA_CONFORMANCE_DIR` set to a checkout of `c2pa-org/public-testfiles`: 25 stores located (24 JPEG through `read_asset`, 1 MP4 through the test-only locator), 41 manifests parsed, every hard binding checked. 22 agree with the `*Hash.match` or `*Hash.mismatch` verdict recorded in `detailed.json`, including the three recorded mismatches (`E-dat-CA`, `XCA`, `XCI`) and the MP4's BMFF hash.

Three Truepic JPEGs (`truepic-20230212-camera`, `-landscape`, `-library`) record a match, and their digest does match the bytes outside the exclusion. Their single exclusion, however, starts at byte 0 and ends after the C2PA APP11 segments, so SOI and the whole APP1 Exif segment are unhashed. C2PA 2.4 section 15.12.1 makes that `assertion.dataHash.mismatch`, and the test requires exactly that code with the reason "the store's exclusion covers other bytes" for those three files (`STRICTER_THAN_CORPUS`). The corpus has no PNG with a manifest, so PNG coverage is synthetic.

## SRS traceability

| Requirement | Tests |
|---|---|
| C2PA-S-005 | `unit_format::formats_are_detected_by_signature`, `assets_without_a_store_and_unsupported_formats_are_reported`; BDD "Unsupported formats are reported" |
| C2PA-S-001 (embedded) | `unit_format::a_jpeg_store_is_located_across_app11_segments`, `fill_bytes_and_stray_bytes_before_markers_are_skipped`, `other_app11_segments_are_ordinary_boxes`, `a_short_jp_packet_is_malformed`, `truncated_segments_are_errors`, `c2pa_segments_must_be_contiguous_and_in_sequence`, `a_reassembled_store_must_match_its_box_length`, `two_embedded_stores_are_reported_as_multiple`, `a_png_store_is_located_in_its_cabx_chunk`, `malformed_pngs_are_errors`, `two_cabx_chunks_are_two_stores`, `embedded_stores_are_read_and_parsed`; `integration_conformance_parse`; BDD "Read a manifest store", "Read a manifest store embedded in a PNG" |
| C2PA-S-002 algorithm and selection | `unit_binding::the_hash_algorithm_must_be_allowed_and_known`, `the_claim_alg_applies_when_the_assertion_has_none`, `hard_binding_presence_rules`, `bindings_the_engine_cannot_check_are_reported_as_not_checked`, `a_mismatch_with_a_multi_asset_hash_present_is_not_checked`, `an_update_manifest_uses_its_parent_binding_and_the_current_store_length`, `an_update_manifest_without_a_parent_has_no_hard_binding` |
| C2PA-S-002 data hash | `unit_binding::a_valid_data_hash_matches`, `a_changed_byte_outside_the_exclusion_is_a_data_hash_mismatch`, `overlapping_or_unordered_exclusions_are_malformed`, `an_exclusion_past_the_end_of_the_asset_is_a_mismatch`, `the_store_exclusion_must_cover_only_the_store`, `a_store_that_is_not_excluded_cannot_match`, `other_exclusions_are_reported_as_informational`, `malformed_exclusion_values_are_malformed`, `a_missing_hash_field_is_a_mismatch`, `a_sidecar_store_binds_a_whole_asset`; BDD "A JPEG's data hash matches its content", "A changed JPEG fails its data hash" |
| C2PA-S-002 box hash | `unit_format::every_jpeg_marker_has_its_iso_10918_name`, `scan_data_runs_to_the_next_marker_and_restart_markers_are_boxes`; `unit_binding::a_valid_box_hash_matches`, `a_changed_box_is_a_box_hash_mismatch`, `boxes_out_of_order_are_a_mismatch_and_unlisted_boxes_are_unknown`, `one_entry_can_hash_a_run_of_boxes`, `box_exclusions_are_applied_and_reported`, `bad_box_exclusions_are_malformed_or_mismatched`, `an_excluded_box_is_not_compared`, `malformed_box_maps_are_malformed`, `bytes_after_the_last_box_are_covered_by_c2pa_after`; BDD "A PNG's box hash matches its chunks", "A chunk the box hash does not list is rejected" |
| C2PA-S-002 BMFF hash | `unit_bmff::*` (12 tests); `integration_conformance_parse` (real MP4); BDD "An MP4's BMFF hash matches its boxes" |
| Performance | `performance_bindings::a_large_jpeg_is_laid_out_and_hashed_within_the_budget` (16 MiB scan with 20 000 RST markers, budget 60 s) |

## Mocked environment values

None. The corpus path is passed through `FILESQUID_C2PA_CONFORMANCE_DIR`; `c2patool` is not installed, so the oracle test is skipped and prints why (unchanged from CP-0.2).

## Not checked yet (reported as `BindingStatus::NotChecked`, never as a pass)

Collection data hashes, BMFF Merkle trees (`merkle`), compressed manifests (`c2cm`, Brotli placement pending the owner's decision), and the multi-asset fallback after a mismatch.

## Test output

### Failing run before the implementation (TDD)

The new tests were written first against the public API they describe; the build failed because the modules did not exist yet (`cargo test -p filesquid-c2pa`, error lines counted):

```text
      1 error: could not compile `filesquid-c2pa` (test "features") due to 2 previous errors
      1 error: could not compile `filesquid-c2pa` (test "integration_conformance_parse") due to 3 previous errors
      1 error: could not compile `filesquid-c2pa` (test "performance_bindings") due to 2 previous errors
      1 error: could not compile `filesquid-c2pa` (test "unit_binding") due to 2 previous errors
      1 error: could not compile `filesquid-c2pa` (test "unit_bmff") due to 8 previous errors
      1 error: could not compile `filesquid-c2pa` (test "unit_format") due to 3 previous errors
      5 error[E0432]: unresolved import `filesquid_c2pa::binding`
      2 error[E0432]: unresolved import `filesquid_c2pa::format`
      5 error[E0432]: unresolved imports `filesquid_c2pa::read::read_asset`, `filesquid_c2pa::read::AssetManifest`
      1 error[E0599]: no variant, associated function, or constant named `Format` found for enum `ReadError` in the current scope
      7 error[E0618]: expected function, found `Vec<u8>`
```

### Formatting, lint and documentation gates

From `apps/filesquid/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p filesquid-c2pa --no-default-features --all-targets --locked -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`:

```text
cargo fmt --all --check: no changes
    Finished `dev` profile [unoptimized] target(s) in 9.03s
    Finished `dev` profile [unoptimized] target(s) in 2.15s
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.57s
   Generated <target>/doc/filesquid_api/index.html and 9 other files
```

### Full crate run

`FILESQUID_C2PA_CONFORMANCE_DIR=<corpus> cargo test -p filesquid-c2pa --locked --no-fail-fast`: 279 tests passed, 0 failed; 38 scenarios (30 passed, 8 skipped: the UI panel scenarios, owned by `@filesquid/ui`). Without default features (`--no-default-features`, which leaves out the targets that need `rust-crypto`): 199 passed, 0 failed.

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_c2pa-d65bcc9dd41296d9)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs (<target>/debug/deps/features-8655cb2d4004b238)
SKIPPED c2patool oracle: c2patool not found: set FILESQUID_C2PATOOL or put c2patool on PATH (cargo install c2patool --locked)
38 scenarios (30 passed, 8 skipped)
97 steps (89 passed, 8 skipped)
     Running tests/integration/conformance_parse.rs (<target>/debug/deps/integration_conformance_parse-090ffb77eeb9717c)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.61s
     Running tests/integration/oracle.rs (<target>/debug/deps/integration_oracle-94666cdf1f8605b4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/wasm_build.rs (<target>/debug/deps/integration_wasm_build-35bc3b66d2b65aee)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s
     Running tests/performance/bindings.rs (<target>/debug/deps/performance_bindings-09be1d8c0ae554d1)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s
     Running tests/performance/parse.rs (<target>/debug/deps/performance_parse-f035af314bbfc222)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
     Running tests/unit/binding.rs (<target>/debug/deps/unit_binding-f93190ce513933d0)
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/bmff.rs (<target>/debug/deps/unit_bmff-27a5c7e6bd6f43d1)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
     Running tests/unit/cbor.rs (<target>/debug/deps/unit_cbor-c7040e8e8f10e2a2)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/unit/claim.rs (<target>/debug/deps/unit_claim-f9b9284008e60bf6)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/crypto.rs (<target>/debug/deps/unit_crypto-432f2a37df2000a3)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
     Running tests/unit/fetch.rs (<target>/debug/deps/unit_fetch-738758fb7279fcdb)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/format.rs (<target>/debug/deps/unit_format-30666b288ba03faa)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
     Running tests/unit/io.rs (<target>/debug/deps/unit_io-c003cdb6bb92b265)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-fe596040567ca446)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
     Running tests/unit/jumbf.rs (<target>/debug/deps/unit_jumbf-f69096fce7fab3ff)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/unit/manifest.rs (<target>/debug/deps/unit_manifest-e0b825c7c5ed2037)
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
     Running tests/unit/oracle_harness.rs (<target>/debug/deps/unit_oracle_harness-6d4b880861972b3f)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/read.rs (<target>/debug/deps/unit_read-d590eb7c9591cd5b)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/read_logs.rs (<target>/debug/deps/unit_read_logs-b14b662b7f43348d)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs (<target>/debug/deps/unit_source_scan-49491e813dee674f)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/uri.rs (<target>/debug/deps/unit_uri-bb9093e7b09a2280)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### New unit tests

```text
     Running tests/unit/binding.rs (<target>/debug/deps/unit_binding-f93190ce513933d0)
running 26 tests
test a_missing_hash_field_is_a_mismatch ... ok
test a_mismatch_with_a_multi_asset_hash_present_is_not_checked ... ok
test a_sidecar_store_binds_a_whole_asset ... ok
test a_changed_byte_outside_the_exclusion_is_a_data_hash_mismatch ... ok
test a_changed_box_is_a_box_hash_mismatch ... ok
test a_store_that_is_not_excluded_cannot_match ... ok
test a_valid_box_hash_matches ... ok
test an_excluded_box_is_not_compared ... ok
test an_exclusion_past_the_end_of_the_asset_is_a_mismatch ... ok
test an_update_manifest_without_a_parent_has_no_hard_binding ... ok
test a_valid_data_hash_matches ... ok
test bindings_the_engine_cannot_check_are_reported_as_not_checked ... ok
test bad_box_exclusions_are_malformed_or_mismatched ... ok
test an_update_manifest_uses_its_parent_binding_and_the_current_store_length ... ok
test boxes_out_of_order_are_a_mismatch_and_unlisted_boxes_are_unknown ... ok
test bytes_after_the_last_box_are_covered_by_c2pa_after ... ok
test box_exclusions_are_applied_and_reported ... ok
test malformed_exclusion_values_are_malformed ... ok
test hard_binding_presence_rules ... ok
test one_entry_can_hash_a_run_of_boxes ... ok
test other_exclusions_are_reported_as_informational ... ok
test the_claim_alg_applies_when_the_assertion_has_none ... ok
test malformed_box_maps_are_malformed ... ok
test the_hash_algorithm_must_be_allowed_and_known ... ok
test overlapping_or_unordered_exclusions_are_malformed ... ok
test the_store_exclusion_must_cover_only_the_store ... ok
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/bmff.rs (<target>/debug/deps/unit_bmff-27a5c7e6bd6f43d1)
running 12 tests
test a_changed_byte_is_a_mismatch ... ok
test a_uuid_box_with_other_data_is_not_excluded ... ok
test a_missing_hash_is_a_mismatch_and_merkle_trees_are_not_checked ... ok
test an_index_selects_one_sibling ... ok
test an_xpath_at_the_node_limit_is_accepted ... ok
test malformed_bmff_maps_are_malformed ... ok
test length_version_flags_and_data_conditions_select_boxes ... ok
test nested_boxes_can_be_excluded_and_are_reported ... ok
test moving_a_root_box_changes_the_hash ... ok
test the_required_exclusions_alone_match ... ok
test subsets_exclude_part_of_a_box ... ok
test truncated_and_corrupted_bmff_assets_never_panic ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
     Running tests/unit/format.rs (<target>/debug/deps/unit_format-30666b288ba03faa)
running 19 tests
test a_reassembled_store_must_match_its_box_length ... ok
test a_short_jp_packet_is_malformed ... ok
test a_jpeg_store_is_located_across_app11_segments ... ok
test assets_without_a_store_and_unsupported_formats_are_reported ... ok
test c2pa_segments_must_be_contiguous_and_in_sequence ... ok
test a_store_over_the_size_limit_is_rejected_before_reading_it ... ok
test embedded_stores_are_read_and_parsed ... ok
test every_jpeg_marker_has_its_iso_10918_name ... ok
test formats_are_detected_by_signature ... ok
test fill_bytes_and_stray_bytes_before_markers_are_skipped ... ok
test other_app11_segments_are_ordinary_boxes ... ok
test scan_data_runs_to_the_next_marker_and_restart_markers_are_boxes ... ok
test the_asset_box_limit_holds_at_the_boundary ... ok
test a_png_store_is_located_in_its_cabx_chunk ... ok
test malformed_pngs_are_errors ... ok
test truncated_segments_are_errors ... ok
test two_cabx_chunks_are_two_stores ... ok
test two_embedded_stores_are_reported_as_multiple ... ok
test truncated_and_corrupted_assets_never_panic ... ok
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
```

### New and enabled BDD scenarios

```text
  Scenario: Read a manifest store
   ✔  Given a JPEG with an embedded C2PA manifest store
   ✔  When I read it with filesquid-c2pa
   ✔  Then the active manifest, its claim and its assertions should be returned
  Scenario: Unsupported formats are reported
   ✔  Given an asset in a format without a handler
   ✔  When I read it with filesquid-c2pa
   ✔  Then the result should say the format is unsupported
  Scenario: Read a manifest store embedded in a PNG
   ✔  Given a PNG with an embedded C2PA manifest store
   ✔  When I read it with filesquid-c2pa
   ✔  Then the active manifest, its claim and its assertions should be returned
  Scenario: A JPEG's data hash matches its content
   ✔  Given a JPEG whose manifest binds its content with a data hash
   ✔  When I validate its hard binding with filesquid-c2pa
   ✔  Then the hard binding should report "assertion.dataHash.match"
  Scenario: A changed JPEG fails its data hash
   ✔  Given a JPEG whose manifest binds its content with a data hash
   ✔  And one byte of its image data is changed
   ✔  When I validate its hard binding with filesquid-c2pa
   ✔  Then the hard binding should report "assertion.dataHash.mismatch"
  Scenario: A PNG's box hash matches its chunks
   ✔  Given a PNG whose manifest binds its content with a box hash
   ✔  When I validate its hard binding with filesquid-c2pa
   ✔  Then the hard binding should report "assertion.boxesHash.match"
  Scenario: A chunk the box hash does not list is rejected
   ✔  Given a PNG whose box hash leaves out one chunk
   ✔  When I validate its hard binding with filesquid-c2pa
   ✔  Then the hard binding should report "assertion.boxesHash.unknownBox"
  Scenario: An MP4's BMFF hash matches its boxes
   ✔  Given an MP4 bound by a BMFF hash in a sidecar manifest store
   ✔  When I validate its hard binding with filesquid-c2pa
   ✔  Then the hard binding should report "assertion.bmffHash.match"
```

### Conformance corpus and performance (`--nocapture`)

`cargo test -p filesquid-c2pa --locked --test integration_conformance_parse --test performance_bindings -- --nocapture`:

```text
     Running tests/integration/conformance_parse.rs (<target>/debug/deps/integration_conformance_parse-090ffb77eeb9717c)
running 1 test
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-C.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CACA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CACAICAICICA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CAI.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CAIAIIICAICIICAIICICA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CAICA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CAICAI.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CI.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CICA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CICACACA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CIE-sig-CA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-CII.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-E-clm-CAICAI.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-E-dat-CA.jpg: Invalid { code: "assertion.dataHash.mismatch", reason: "the computed hash differs from the recorded hash" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-E-sig-CA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-E-uri-CA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-E-uri-CIE-sig-CA.jpg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-XCA.jpg: Invalid { code: "assertion.dataHash.mismatch", reason: "the computed hash differs from the recorded hash" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/adobe-20220124-XCI.jpg: Invalid { code: "assertion.dataHash.mismatch", reason: "the computed hash differs from the recorded hash" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/nikon-20221019-building.jpeg: Valid { code: "assertion.dataHash.match" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/truepic-20230212-camera.jpg: Invalid { code: "assertion.dataHash.mismatch", reason: "the store's exclusion covers other bytes" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/truepic-20230212-landscape.jpg: Invalid { code: "assertion.dataHash.mismatch", reason: "the store's exclusion covers other bytes" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/image/jpeg/truepic-20230212-library.jpg: Invalid { code: "assertion.dataHash.mismatch", reason: "the store's exclusion covers other bytes" }
  $FILESQUID_C2PA_CONFORMANCE_DIR/legacy/1.4/video/mp4/truepic-20230212-zoetrope.mp4: Valid { code: "assertion.bmffHash.match" }
conformance parse: 25 stores, 91 files without a store, 41 manifests parsed, 22 hard bindings agree with the corpus, 3 stricter under C2PA 2.4, 0 without a recorded verdict, 0 failures
test every_conformance_asset_is_located_parsed_and_binding_checked ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.60s
     Running tests/performance/bindings.rs (<target>/debug/deps/performance_bindings-09be1d8c0ae554d1)
running 1 test
laid out 20007 boxes in 73.731543ms, validated in 492.70672ms
test a_large_jpeg_is_laid_out_and_hashed_within_the_budget ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s
```

