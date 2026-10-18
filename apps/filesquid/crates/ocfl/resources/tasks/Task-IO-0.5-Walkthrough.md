# Task IO-0.5 Walkthrough: Structural validation

Module plan row: [IO-0.5](../plan.md). Global task: Task 0.3 in the [implementation plan](../../../../../../resources/implementation-plan.md), which this slice completes (IO-0.1 to IO-0.5 are done). Branch `feat/io-0.5-validation`, based on `main` at `5d81f0f` (IO-0.4 merged as PR #25).

## OCFL-S-009: Validator conformance. The engine validator MUST be tested against every object in the `1.1/` tree of the community OCFL fixtures at the pinned commit

Every good object validates with zero errors, every warning object with zero errors and its prefix warning codes, and every bad object is rejected; errors and warnings use the OCFL 1.1 codes (`E###`, `W###`); no fixture is skipped. Sources: [OCFL-S-009](../srs.md) (a) to (i), [PRD F-051](../../../../../../resources/prd.md#f-051-ocfl-module), [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md), [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md), [OCFL 1.1 validation codes](https://ocfl.io/1.1/spec/validation-codes.html).

## OCFL-S-011: Fixture pinning and handling. The fixtures MUST be pinned to one commit of `OCFL/fixtures`, kept as test data under `tests/`, never packaged or loaded at runtime, run offline by `cargo test --locked` and by the local pre-commit hook, with CC-BY-4.0 attribution kept

Sources: [OCFL-S-011](../srs.md), [local-verification.md](../../../../../../.agents/rules/local-verification.md).

## OCFL-S-004: Only the root inventory commits a version (a version-directory inventory and sidecar alone are not a commit)

Sources: [OCFL-S-004](../srs.md), ADR 016 "Published head (readers)", module plan IO-0.5 Description.

## What changed

- **`src/validate/` (new public module `validate`).** `validate_object(&backend, root)` returns a `ValidationReport` of `Finding`s (`code`, `at`, `message`); `is_valid()` means no errors. It builds on the IO-0.4 `read` module (`root_dir`, `key_in`, `read_small`, `choose_declaration`, `check_declaration`) and the guard's `next_start`; those `read` submodules are now `pub(crate)`. It returns `Err(ReadError)` only when the object cannot be validated (backend failure, listing bound, OCFL-S-017 bound, OCFL version other than 1.1). No `std::fs`; builds for `wasm32-unknown-unknown`.
  - `listing.rs`: one paged listing of the object (at most `MAX_WALK_ENTRIES` keys and `MAX_WALK_ENTRIES / MAX_LIST_PAGE + 1` pages; continuations checked); the backend hierarchy check maps an empty directory to E024 and a link to E090.
  - `root.rs`: declaration (E003, E007); root inventory (E063, every `Inventory::parse` code, E038) and sidecar (E058, E060, E061); root entries (E001; E046 for a version directory the root inventory does not list; E010 for a listed version without a directory; E067 and W013 for `extensions`; `logs` allowed).
  - `warnings.rs`: W001, W004, W005, W007, W008, W009 (a URI is an RFC 3986 scheme, a colon and a rest).
  - `versions.rs`: version directories (E015, W002, W010) and version inventories (all inventory codes with their own sidecar, E040, E037, E019, E103, E064, E066, W011, E023 against each earlier manifest, W004).
  - `content.rs`: content files against the manifest (E023, E042) and every stated digest against the bytes: root manifest, earlier version manifests in whatever algorithm (E092), root fixity (E093). Each file is streamed once in chunks of at most 8 MiB through one hasher per algorithm it needs; the chunk loop is bounded by the object size bound. A missing content path is E092 and, when the fixity block lists it, E093 (the case IO-0.4 left open).
- **`src/fixity.rs` (new public module `fixity`).** `FixityAlgorithm` (md5, sha1, sha256, sha512, blake2b-512: the OCFL 1.1 fixity table, E027); other algorithms are `None` and ignored (E028).
- **Dependencies.** `md-5`, `sha1` and `blake2` 0.11 (RustCrypto, the `sha2` 0.11 family already used; current per `cargo search`; pure Rust, the `wasm32` build passes). A search of `resources/architecture/` found no rule against them: ADR 009 fixes the digest FileSquid writes (SHA-256), not the fixity values a validator must read.
- **Pinned fixtures committed.** The read-only upstream clone outside the repository (used by IO-0.3 and IO-0.4) was at `d323fc207126becec066998b43408d998bf0fb1b`. Its `1.1/` tree (good-objects, warn-objects, bad-objects, content), `README.md` and `.zenodo.json` (the CC-BY-4.0 licence record) were copied unchanged (`diff -r` identical, 641 files) to `tests/fixtures/ocfl/`, with a [NOTICE](../../tests/fixtures/ocfl/NOTICE.md) giving source, commit, date and attribution. `1.0/` is not copied. `Cargo.toml` `exclude`s `tests/fixtures/`. The root `.gitignore` allow-list gained `!**/tests/fixtures/`, since `**/tests/*` ignored the new directory.
- **Tests read the committed fixtures.** `tests/support/fixtures.rs` resolves the tree (`FILESQUID_OCFL_FIXTURES` may name another copy) and fails when it is missing. `integration_fixture_inventories`, `integration_fixture_reads` and the `@fixtures` BDD feature no longer skip; `tests/features.rs` no longer gates `@fixtures` on the variable.
- **New tests.** `tests/unit/fixity.rs` (known-answer vectors), `tests/integration/validate_objects.rs` (one built object per code, memory and filesystem backends), `tests/integration/fixture_conformance.rs` (every fixture object, per-fixture output, pinned counts, expected gaps), [`structural_validation_OCFL-S-009.feature`](../../tests/features/structural_validation_OCFL-S-009.feature) and the validation and pin scenarios of [`OCFL_1.1_fixture_conformance.feature`](../../tests/features/OCFL_1.1_fixture_conformance.feature) (`tests/support/validate_steps.rs`).
- **Docs.** Crate [README](../../README.md) "Structural validation" and "Pinned OCFL community fixtures"; root README "Run tests"; module [SRS](../srs.md) OCFL-S-009 (f) to (i); module [plan](../plan.md); [component diagram](../component-diagram.html); implementation plan Task 0.3 and the checklist.

## Fixture conformance results

All 80 objects of the pinned `1.1` tree were validated: 12 `good-objects`, 13 `warn-objects`, 55 `bad-objects`. The 7 `content` fixtures are source directories for writing objects (OCFL-S-010 (b), IO-0.10), not objects, so they are not validated here.

- **good-objects:** 12 of 12 valid with zero errors (and zero warnings).
- **warn-objects:** 13 of 13 valid with zero errors; each reports every warning code its name starts with.
- **bad-objects:** 55 of 55 rejected. 49 report every error code their name starts with.
- **Excluded fixtures:** none.

### Prefix codes not reported

Each of these is rejected, with the first broken rule of one inventory. `Inventory::parse` checks one inventory to its first broken rule (IO-0.3), and the fixtures README allows a validator to stop early and miss a named code. The test pins this exact list (`EXPECTED_GAPS`), so a change in either direction fails it.

| Fixture | Reported | Not reported | Why |
|---|---|---|---|
| `E008_E036_no_versions_no_head` | E036 | E008 | The inventory lacks `head` and has no versions; the required-key check (E036) comes before the versions block (E008). |
| `E011_E013_invalid_padded_head_version` | E011 | E013 | `head` breaks the zero-padded convention (E011). E013 is the same fault seen as a writer rule (an operation that adds a version must keep the convention); there is no second fault to find. |
| `E049_E050_E054_bad_version_block_values` | E049 | E050, E054 | The version block's `created` (E049) is checked before its `state` digests (E050) and `user` (E054). |
| `E053_E052_invalid_logical_paths` | E053 | E052 | The first broken logical path begins or ends with `/` (E053); the path with a bad element (E052) is not reached. |
| `E100_E099_fixity_invalid_content_paths` | E099 | E100 | The first broken fixity content path has a bad element (E099); the one with an edge slash (E100) is not reached. |
| `E100_E099_manifest_invalid_content_paths` | E099 | E100 | As above, in the manifest. |

### Codes reported beyond the prefix

Some bad fixtures break further rules, which the validator also reports: `E001_invalid_version_format` (E104 for the version name `1`), `E003_E063_empty` (E001 for its `.keep` file), `E015_content_not_in_content_dir` and `E019_inconsistent_content_dir` (E042: their manifest paths are outside the content directory), `E025_wrong_digest_algorithm` (E001: with no valid inventory, its `inventory.json.md5` is not a sidecar), and `E060_E064_root_inventory_digest_mismatch` (W011). Warnings on bad fixtures (W002, W004, W007, W009, W011, W013) are what those objects also do.

### Per-fixture output

`cargo test -p filesquid-ocfl --locked --test integration_fixture_conformance -- --nocapture --test-threads=1`, from `apps/filesquid` (Cargo progress lines removed; each test's first fixture line follows its `test ... ` header):

```text
     Running tests/integration/fixture_conformance.rs

running 4 tests
test every_bad_fixture_is_rejected ... bad-objects/E001_extra_dir_in_root: invalid; errors: E001; warnings: W007
bad-objects/E001_extra_file_in_root: invalid; errors: E001; warnings: W007
bad-objects/E001_invalid_version_format: invalid; errors: E001, E104; warnings: none
bad-objects/E001_v2_file_in_root: invalid; errors: E001; warnings: none
bad-objects/E003_E063_empty: invalid; errors: E001, E003, E063; warnings: none
bad-objects/E003_no_decl: invalid; errors: E003; warnings: none
bad-objects/E007_bad_declaration_contents: invalid; errors: E007; warnings: W007
bad-objects/E008_E036_no_versions_no_head: invalid; errors: E036; warnings: none
bad-objects/E010_missing_versions: invalid; errors: E010; warnings: none
bad-objects/E010_skipped_versions: invalid; errors: E010; warnings: none
bad-objects/E011_E013_invalid_padded_head_version: invalid; errors: E011; warnings: none
bad-objects/E015_content_not_in_content_dir: invalid; errors: E015, E042; warnings: none
bad-objects/E017_invalid_content_dir: invalid; errors: E017; warnings: none
bad-objects/E019_inconsistent_content_dir: invalid; errors: E019, E042; warnings: W002
bad-objects/E023_extra_file: invalid; errors: E023; warnings: W009
bad-objects/E023_old_manifest_missing_entries: invalid; errors: E023; warnings: none
bad-objects/E025_wrong_digest_algorithm: invalid; errors: E001, E025; warnings: none
bad-objects/E036_no_head: invalid; errors: E036; warnings: none
bad-objects/E036_no_id: invalid; errors: E036; warnings: none
bad-objects/E037_inconsistent_id: invalid; errors: E037; warnings: none
bad-objects/E040_head_not_most_recent: invalid; errors: E040; warnings: none
bad-objects/E040_wrong_head_doesnt_exist: invalid; errors: E040; warnings: none
bad-objects/E040_wrong_head_format: invalid; errors: E040; warnings: none
bad-objects/E040_wrong_version_in_version_dir: invalid; errors: E040; warnings: none
bad-objects/E041_no_manifest: invalid; errors: E041; warnings: none
bad-objects/E046_root_not_most_recent: invalid; errors: E046; warnings: none
bad-objects/E049_E050_E054_bad_version_block_values: invalid; errors: E049; warnings: none
bad-objects/E049_created_no_timezone: invalid; errors: E049; warnings: none
bad-objects/E049_created_not_to_seconds: invalid; errors: E049; warnings: none
bad-objects/E050_manifest_digest_wrong_case: invalid; errors: E050; warnings: none
bad-objects/E050_state_digest_not_in_manifest: invalid; errors: E050; warnings: none
bad-objects/E053_E052_invalid_logical_paths: invalid; errors: E053; warnings: none
bad-objects/E058_no_sidecar: invalid; errors: E058; warnings: none
bad-objects/E060_E064_root_inventory_digest_mismatch: invalid; errors: E060, E064; warnings: W011
bad-objects/E060_version_inventory_digest_mismatch: invalid; errors: E060; warnings: none
bad-objects/E061_invalid_sidecar: invalid; errors: E061; warnings: none
bad-objects/E063_no_inv: invalid; errors: E063; warnings: none
bad-objects/E064_different_root_and_latest_inventories: invalid; errors: E064; warnings: none
bad-objects/E066_E092_old_manifest_digest_incorrect: invalid; errors: E066, E092; warnings: none
bad-objects/E066_algorithm_change_state_mismatch: invalid; errors: E066; warnings: W004
bad-objects/E066_inconsistent_version_state: invalid; errors: E066; warnings: none
bad-objects/E067_file_in_extensions_dir: invalid; errors: E067; warnings: W007, W013
bad-objects/E092_E093_content_path_does_not_exist: invalid; errors: E092, E093; warnings: none
bad-objects/E092_algorithm_change_incorrect_digest: invalid; errors: E092; warnings: W004
bad-objects/E092_content_file_digest_mismatch: invalid; errors: E092; warnings: none
bad-objects/E093_fixity_digest_mismatch: invalid; errors: E093; warnings: none
bad-objects/E095_conflicting_logical_paths: invalid; errors: E095; warnings: none
bad-objects/E095_non_unique_logical_paths: invalid; errors: E095; warnings: none
bad-objects/E096_manifest_duplicate_digests: invalid; errors: E096; warnings: none
bad-objects/E097_fixity_duplicate_digests: invalid; errors: E097; warnings: none
bad-objects/E100_E099_fixity_invalid_content_paths: invalid; errors: E099; warnings: none
bad-objects/E100_E099_manifest_invalid_content_paths: invalid; errors: E099; warnings: none
bad-objects/E101_non_unique_content_paths: invalid; errors: E101; warnings: none
bad-objects/E103_older_spec_v2: invalid; errors: E103; warnings: none
bad-objects/E107_file_in_manifest_not_used: invalid; errors: E107; warnings: none
gap: bad-objects/E008_E036_no_versions_no_head: E008 not reported
gap: bad-objects/E011_E013_invalid_padded_head_version: E013 not reported
gap: bad-objects/E049_E050_E054_bad_version_block_values: E050, E054 not reported
gap: bad-objects/E053_E052_invalid_logical_paths: E052 not reported
gap: bad-objects/E100_E099_fixity_invalid_content_paths: E100 not reported
gap: bad-objects/E100_E099_manifest_invalid_content_paths: E100 not reported
ok
test every_good_fixture_validates_with_zero_errors ... good-objects/diff_files_same_md5: valid; errors: none; warnings: none
good-objects/minimal_content_dir_called_stuff: valid; errors: none; warnings: none
good-objects/minimal_logs_directory_one_log_file: valid; errors: none; warnings: none
good-objects/minimal_mixed_digests: valid; errors: none; warnings: none
good-objects/minimal_no_content: valid; errors: none; warnings: none
good-objects/minimal_one_version_one_file: valid; errors: none; warnings: none
good-objects/minimal_uppercase_digests: valid; errors: none; warnings: none
good-objects/ocfl_object_all_fixity_digests: valid; errors: none; warnings: none
good-objects/spec-ex-full: valid; errors: none; warnings: none
good-objects/spec-ex-minimal: valid; errors: none; warnings: none
good-objects/updates_all_actions: valid; errors: none; warnings: none
good-objects/updates_three_versions_one_file: valid; errors: none; warnings: none
ok
test every_warning_fixture_has_zero_errors_and_its_prefix_warnings ... warn-objects/W001_W004_W005_zero_padded_versions: valid; errors: none; warnings: W001, W004, W005
warn-objects/W001_zero_padded_versions: valid; errors: none; warnings: W001
warn-objects/W002_extra_dir_in_version_dir: valid; errors: none; warnings: W002
warn-objects/W004_uses_sha256: valid; errors: none; warnings: W004
warn-objects/W004_versions_diff_digests: valid; errors: none; warnings: W004
warn-objects/W005_id_not_uri: valid; errors: none; warnings: W005
warn-objects/W007_no_message_or_user: valid; errors: none; warnings: W007
warn-objects/W007_spec-ex-diff-paths: valid; errors: none; warnings: W007
warn-objects/W008_user_no_address: valid; errors: none; warnings: W008
warn-objects/W009_user_address_not_uri: valid; errors: none; warnings: W009
warn-objects/W010_no_version_inventory: valid; errors: none; warnings: W010
warn-objects/W011_version_inv_diff_metadata: valid; errors: none; warnings: W011
warn-objects/W013_unregistered_extension: valid; errors: none; warnings: W013
ok
test the_tree_is_the_pinned_one_with_no_object_left_out ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
```

## TDD: failing run before implementation

The tests were written first, with no `fixity` or `validate` module (`cargo test -p filesquid-ocfl --no-fail-fast --test unit_fixity --test integration_validate_objects --test integration_fixture_conformance --test features`, error lines with counts):

```text
      1 error: could not compile `filesquid-ocfl` (test "features") due to 1 previous error
      1 error: could not compile `filesquid-ocfl` (test "integration_fixture_conformance") due to 7 previous errors
      1 error: could not compile `filesquid-ocfl` (test "integration_validate_objects") due to 2 previous errors
      1 error: could not compile `filesquid-ocfl` (test "unit_fixity") due to 1 previous error
      5 error[E0277]: the size for values of type `str` cannot be known at compilation time
      1 error[E0308]: mismatched types
      2 error[E0432]: unresolved import `filesquid_ocfl::fixity`
      3 error[E0432]: unresolved import `filesquid_ocfl::validate`
```

The `str` and mismatched-type errors follow from the missing types. The first run with the modules in place failed 30 of 34 `integration_validate_objects` tests: `Listing::key("")` returned `obj/`, so the declaration key had an empty segment; it now returns the root. The first conformance run then showed that the pinned `bad-objects` group holds 55 objects (the test said 56) and that the two `E100_E099` fixtures report E099, not E100 (the expected-gap list had them the other way round). Both expectations were corrected against the fixtures; no validator change was needed.

## Raw test output

`cargo test -p filesquid-ocfl --locked`, from `apps/filesquid` (result lines; the four skipped BDD scenarios are the IO-0.10 write scenario and three F-051 scenarios whose steps belong to later tasks, as before):

```text
     Running unittests src/lib.rs
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.27s
     Running tests/features.rs
7 features
85 scenarios (81 passed, 4 skipped)
345 steps (341 passed, 4 skipped)
     Running tests/integration/fixture_conformance.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.39s
     Running tests/integration/fs_conformance.rs
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.88s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.66s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.79s
     Running tests/integration/read_objects.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/integration/validate_objects.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/fixity.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/inventory.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/unit/namaste_sidecar.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/read_state.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/storage_key.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests filesquid_ocfl
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Lint, docs and the browser build, from `apps/filesquid`:

```text
$ cargo fmt --all --check
exit 0
$ cargo clippy --workspace --all-targets --locked -- -D warnings
exit 0
$ RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
exit 0
$ cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown
exit 0
```

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-009 (a): every `good-objects` object validates with zero errors | `integration_fixture_conformance`: `every_good_fixture_validates_with_zero_errors`; BDD "Valid OCFL 1.1 fixture objects pass validation"; `integration_validate_objects`: `a_valid_object_has_no_errors_or_warnings`, `a_valid_object_on_a_filesystem_has_no_errors`; BDD "A valid object validates with zero errors and zero warnings" |
| OCFL-S-009 (b): every `warn-objects` object has zero errors and reports its prefix warning codes | `every_warning_fixture_has_zero_errors_and_its_prefix_warnings`; BDD "Warning OCFL 1.1 fixture objects report their warning codes"; `a_sha256_object_is_valid_with_w004`, `inventory_warnings_have_their_codes` (W005, W007, W008, W009), `a_missing_version_inventory_is_w010`, `prior_version_metadata_that_differs_is_w011`, `version_directory_entries_are_e015_and_w002`, `extensions_hold_only_extension_directories` (W013); BDD "A missing version inventory is a warning, not an error", "An unregistered object extension is a warning" |
| OCFL-S-009 (c): every `bad-objects` object is rejected; unreported prefix codes listed per fixture | `every_bad_fixture_is_rejected` (with `EXPECTED_GAPS`); BDD "Invalid OCFL 1.1 fixture objects are rejected", whose last step checks that every gap fixture is named in this Walkthrough |
| OCFL-S-009 (d): errors and warnings use the OCFL 1.1 codes | every `integration_validate_objects` test asserts a code: E001, E003, E007, E010, E015, E019, E023, E024, E033, E036, E037, E038, E040, E042, E046, E058, E060, E061, E063, E064, E066, E067, E090, E092, E093, E103; `every_finding_carries_an_ocfl_code_and_its_location`; BDD outline "A broken rule is reported with its OCFL 1.1 validation code" |
| OCFL-S-009 (e): no fixture skipped or excluded | `the_tree_is_the_pinned_one_with_no_object_left_out` and the pinned counts (12, 13, 55) in every group test |
| OCFL-S-009 (f): every finding reported with code and location; errors only when validation cannot run | `every_finding_carries_an_ocfl_code_and_its_location`, `an_ocfl_1_0_object_is_outside_the_validators_scope`, `a_root_that_is_not_a_key_prefix_is_refused` |
| OCFL-S-009 (g) and OCFL-S-004: only the root inventory commits a version (E046) | `a_version_directory_inventory_alone_is_not_a_commit`; fixture `E046_root_not_most_recent`; BDD "A version directory that the root inventory does not list is not a commit" |
| OCFL-S-009 (h): fixity in the OCFL 1.1 table verified, other algorithms ignored | `unit_fixity` (4 tests); `fixity_values_are_verified_and_unknown_algorithms_ignored`, `a_missing_content_path_that_has_fixity_is_e092_and_e093`; fixtures `E093_fixity_digest_mismatch`, `E092_E093_content_path_does_not_exist`, `ocfl_object_all_fixity_digests` |
| OCFL-S-009 (i): every stated digest checked, earlier manifests in any algorithm included | `content_that_does_not_match_its_digest_is_e092`, `a_missing_content_file_is_e092`; fixtures `E092_algorithm_change_incorrect_digest`, `E066_E092_old_manifest_digest_incorrect`, `W004_versions_diff_digests` |
| OCFL-S-011: pinned commit, CC-BY-4.0 attribution, test data only, not packaged or loaded, offline in `cargo test --locked` and the hook | BDD "Community fixtures are pinned test data only" (NOTICE pin and licence, `.zenodo.json`, no `1.0/`, `exclude`, no `src/` reference); `tests/support/fixtures.rs` fails when the tree is missing; `scripts/verify_tests.sh` runs `cargo test -p filesquid-ocfl --locked` |
| OCFL-S-010 (a): reads of every good fixture version (IO-0.4, now on the committed fixtures) | `integration_fixture_reads` (3 tests); BDD "Valid OCFL 1.1 fixture objects are readable at every version" |
| OCFL-S-007: bytes only through the backend; `wasm32` build | `unit_source_scan` (covers `src/validate/` and `src/fixity.rs`); BDD "Engine builds for the browser"; the `wasm32` build above |
| OCFL-S-017: engine bounds are errors without a code | `read_small` bounds reused for inventories (64 MiB), sidecars (4 KiB) and declarations (64 bytes); `ValidationReport::record` passes them through as errors |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. Every code the validator emits is triggered by a built-object test or by a fixture: E001, E003, E007, E010, E015, E019, E023, E024, E033 and the other `Inventory::parse` codes, E037, E038, E040, E042, E046, E058, E060, E061, E063, E064, E066, E067, E090, E092, E093, E103, W001, W002, W004, W005, W007, W008, W009, W010, W011, W013.

## Sync Execution Report

- Module SRS OCFL-S-009 gains (f) to (i): findings rather than first-fault refusals (one inventory still to its first fault), E046 for version directories the root inventory does not list, the fixity table (md5, sha1, sha256, sha512, blake2b-512; others ignored), and every stated digest checked. These state OCFL 1.1 rules (E027, E028, E046, E092) and ADR 016's published-head rule as built; nothing beyond them was added.
- Module plan IO-0.5 checked, with its tests and this Walkthrough; implementation plan Task 0.3 and both checklist rows checked.
- Crate README, root README "Run tests" (fixtures committed; the variable is now an optional override) and the component diagram updated.
- PRD, global SRS, product vision and ADRs: unchanged. SRS-F-051 already requires structural validation and the fixture suite. No ADR is needed for `md-5`, `sha1` and `blake2`: they are digest primitives of the RustCrypto family already used (`sha2`), read only, for the OCFL 1.1 fixity table.

## Mocked Environment Variables

None. `FILESQUID_OCFL_FIXTURES` was set once, to the `1.1` directory of that upstream clone, only to verify the documented override command; the committed tree is the default.

## Remaining limits

- One inventory is checked to its first broken rule (`Inventory::parse`), so the six gaps above remain until the parser collects every fault of an inventory.
- Fixity blocks of earlier version inventories are parsed and checked for shape but not verified against content; the root fixity block is.
- Storage-root validation (E069 to E090, W014 to W016 across a whole storage root) is not part of this module yet; `StorageRoot::open` checks the storage-root declaration and guards.
- `FsBackend::list` still re-walks the directory for each page (unchanged, as in IO-0.4); the validator lists each object once.
- The registered extension names (W013) are the OCFL extensions registry as of this task; a newly registered name warns until it is added.
