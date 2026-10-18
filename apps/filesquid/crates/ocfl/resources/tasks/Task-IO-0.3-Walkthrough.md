# Task IO-0.3 Walkthrough: SHA-256 digest and inventory types

Module plan row: [IO-0.3](../plan.md). Global task: Task 0.3 in the [implementation plan](../../../../../../resources/implementation-plan.md). Branch `feat/io-0.3-digest-inventory`, based on `main` at `654050c` (IO-0.2 merged).

## OCFL-S-002: Digest algorithm is SHA-256: objects the engine writes MUST use `sha256` for content addressing and inventory sidecars. The engine MUST read objects that use `sha256` or `sha512`, the OCFL 1.1 content-addressing algorithms. Inventories, digest sidecars and NAMASTE declarations MUST be parsed strictly from untrusted bytes, with a bound on size and on JSON nesting and no repeated JSON keys, and every refusal MUST name its OCFL 1.1 validation code.

Sources: [OCFL-S-002](../srs.md), [PRD TC-004](../../../../../../resources/prd.md#tc-004-decision-status-and-repository-alignment) (shared SHA-256 test vectors), [ADR 009](../../../../../../resources/architecture/009-cryptography-webcrypto-compatible-suite.md) (SHA-256 for OCFL digests), [ADR 010](../../../../../../resources/architecture/010-ocfl-1-1-dual-profile.md), and the OCFL 1.1 specification, sections 3.2 to 3.7 and 4.2, with its validation-codes page. The SRS row previously read "Digest algorithm is SHA-256 (implementation later)"; it now states what was built (see the Sync note below).

## What changed

- **`src/digest.rs`.** `DigestAlgorithm` (`Sha256`, `Sha512`), with `ENGINE = Sha256`, OCFL names, hex lengths, an incremental `DigestHasher` for bounded chunks, `digest_hex`, and case-insensitive `matches`. `ETag` now reuses the same hex encoder.
- **`src/inventory/`.** `Inventory`, `Version`, `User`, `DigestMap` and `VersionNum` types, and `Inventory::parse` for untrusted bytes:
  - `json.rs`: one strict JSON value. The size cap (`MAX_INVENTORY_BYTES`, 64 MiB) is checked first; nesting deeper than `MAX_JSON_DEPTH` (8) is refused while parsing, so hostile nesting cannot exhaust the stack; a repeated key in any object is refused; trailing data and invalid UTF-8 are refused. All are E033.
  - `read.rs` and `blocks.rs`: every rule decidable from one inventory, in rule order, with its OCFL 1.1 code (listed in the traceability table).
  - `check.rs`: state digests exactly match manifest keys (E050, case-sensitive, as the spec and the `E050_manifest_digest_wrong_case` fixture require); every manifest digest is used (E107).
  - `version.rs`: `v` + positive integer (E104, E105), the first version's naming convention (E011, E012), `next()` within a padded width.
  - `timestamp.rs`: RFC 3339 `date-time` to the second with a zone, real calendar dates, leap seconds.
  - `sidecar.rs`: sidecar names (E059), contents, parsing (E061) and verification (E060).
  - `write.rs`: `Inventory::to_json()`, sorted keys, optional keys only when set; it parses back to an equal inventory.
- **`src/namaste.rs`.** `SpecVersion` (1.0 recognised, 1.1 written), `Declaration` for object and storage roots, with exact contents (E007, E080).
- **Dependencies.** `serde` 1 and `serde_json` 1 with default features off and `std` on, as `filesquid-c2pa` already uses them; both were already pinned in the workspace lockfile (`1.0.229`, `1.0.154`, current per `cargo search`). The lockfile gained only the two dependency edges. No ADR names a JSON library; these are the workspace's existing choice.
- **Engine rules kept.** No `std::fs`, no executor; the crate still builds for `wasm32-unknown-unknown` without default features, and the source scan passes.
- **Tests.** `tests/unit/digest.rs`, `tests/unit/inventory.rs`, `tests/unit/namaste_sidecar.rs`, `tests/integration/fixture_inventories.rs`, the shared example in `tests/support/inventory_example.rs`, and [`inventory_types_OCFL-S-002.feature`](../../tests/features/inventory_types_OCFL-S-002.feature) with steps in `tests/support/inventory_steps.rs`.
- **Docs.** Module [SRS](../srs.md) OCFL-S-002; crate [README](../../README.md) "OCFL file formats"; [component diagram](../component-diagram.html); module [plan](../plan.md) (IO-0.3 checked, test links); the repository-root implementation-plan checklist (IO-0.3 checked; Task 0.3 stays open).

## Split with structural validation (IO-0.5)

This task decides everything one inventory's bytes, one sidecar and one declaration can show. It does not look at files: content files against the manifest (E023, E092, E093), version directories and their inventories (E015, E046, E064, E066), content paths that sit in the right version's content directory (E014, E015), extensions and the storage hierarchy. Warnings (W###) are left to the validator; the types keep what it needs (for example a missing `message` or `address`, or a padded version name).

## Community fixture run

The pinned fixtures (`OCFL/fixtures` commit `d323fc207126becec066998b43408d998bf0fb1b`) were fetched read-only into a scratch directory outside the repository and the `1.1` tree was passed with `FILESQUID_OCFL_FIXTURES`. They are not committed; IO-0.5 adds them under `tests/`.

- All 12 `good-objects` and all 13 `warn-objects` root inventories parse, their sidecars verify, and their object declarations are exact.
- All 29 `bad-objects` whose fault is in the root inventory or its sidecar are refused. 28 are refused with a code their name lists. The gap: `E001_invalid_version_format` is refused with E104 (its version name `1` lacks the `v`); E001 itself is about the object root's directory listing, which this task does not read. OCFL-S-009 (c) allows a missing prefix code when the object is rejected.
- None of the other 26 bad fixtures (object-level faults) is refused by the parser for a wrong reason; two have no root inventory at all (`E003_E063_empty`, `E063_no_inv`).

## Remaining limits

- `FILESQUID_OCFL_FIXTURES` is unset in the pre-commit hook until IO-0.5 commits the fixtures, so there the fixture test reports that it skipped. The run above is recorded here instead.
- The parser stops at the first broken rule. Fixtures that list several codes (for example `E049_E050_E054_bad_version_block_values`) report one.
- Objects the engine writes use `sha256`, so validators report W004 ("SHOULD use sha512") for them. That is the accepted trade-off of ADR 009.

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-002: Digest algorithm is SHA-256 (engine writes `sha256`; shared test vectors, PRD TC-004) | `unit_digest`: `sha256_is_the_engine_algorithm`, `known_vectors_match`, `streaming_matches_one_shot`; BDD "SHA-256 is the engine's digest algorithm" |
| OCFL-S-002: Digest algorithm is SHA-256 (the engine reads `sha256` and `sha512`) | `unit_digest`: `known_vectors_match`, `names_and_lengths_follow_ocfl`, `digests_compare_without_regard_to_case`; fixture run (all 25 valid and warning fixtures, 23 of them `sha512`) |
| OCFL-S-002: Digest algorithm is SHA-256 (inventory types match OCFL 1.1: required keys, `contentDirectory` default, `head`, version blocks) | `unit_inventory`: `a_valid_inventory_becomes_typed_data`, `content_directory_defaults_to_content`, `optional_blocks_may_be_absent_or_empty`, `a_written_inventory_parses_back_to_the_same_data`, `an_ocfl_1_0_inventory_type_is_recognised`, `required_keys_are_enforced`, `unknown_keys_are_refused`, `type_and_digest_algorithm_are_checked`, `content_directory_must_be_a_direct_child_name`, `head_must_name_the_highest_version`; BDD "A valid OCFL 1.1 inventory becomes typed data" and the refusal outline |
| OCFL-S-002: Digest algorithm is SHA-256 (version names and sequence) | `unit_inventory`: `versions_must_be_a_non_empty_object`, `version_names_are_v_and_a_positive_integer`, `version_sequences_start_at_one_without_gaps`, `zero_padded_versions_keep_one_width`, `a_padded_head_outside_the_width_is_refused`, `version_numbers_parse_format_and_advance` |
| OCFL-S-002: Digest algorithm is SHA-256 (version blocks: `created`, `message`, `user`) | `unit_inventory`: `created_is_an_rfc3339_date_time_to_the_second`, `message_and_user_have_their_shapes` |
| OCFL-S-002: Digest algorithm is SHA-256 (digest-to-path maps: manifest, state, fixity) | `unit_inventory`: `manifest_structure_is_checked`, `manifest_digests_match_the_algorithm`, `a_digest_appears_once_regardless_of_case`, `content_paths_are_safe_and_unique`, `every_manifest_digest_is_used_by_a_version`, `state_digests_exactly_match_the_manifest`, `mixed_case_digests_are_kept_as_written`, `logical_paths_are_safe_and_unique`, `fixity_follows_the_manifest_structure` |
| OCFL-S-002: Digest algorithm is SHA-256 (strict parsing of untrusted bytes, size and nesting bounds) | `unit_inventory`: `malformed_json_is_refused`, `duplicate_json_keys_are_refused`, `deep_nesting_is_refused_without_exhausting_the_stack`, `oversized_input_is_refused_before_parsing`; BDD "An oversized inventory is refused before it is parsed" |
| OCFL-S-002: Digest algorithm is SHA-256 (sidecar digest check) | `unit_namaste_sidecar`: `sidecar_names_carry_the_algorithm`, `a_written_sidecar_verifies`, `sidecar_separators_and_case_are_accepted`, `a_sidecar_with_the_wrong_digest_is_refused`, `a_malformed_sidecar_is_refused`; BDD "The inventory digest sidecar must match the inventory" |
| OCFL-S-002: Digest algorithm is SHA-256 (NAMASTE declarations) | `unit_namaste_sidecar`: `declarations_have_their_namaste_names_and_contents`, `declaration_file_names_are_recognised`, `declaration_contents_must_be_exact`; BDD "An OCFL 1.1 object declares itself with NAMASTE" |
| OCFL-S-007: The engine MUST reach bytes only through an async, runtime-agnostic storage backend interface (no `std::fs`, `wasm32` build) | `unit_source_scan`; BDD "Storage goes through one async backend interface" and "Engine builds for the browser" |
| OCFL-S-011: Fixture pinning and handling (pinned community fixtures, root-inventory level) | `integration_fixture_inventories` with `FILESQUID_OCFL_FIXTURES` (run recorded above) |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. Every validation code the parser can return has a unit test that triggers it, except E031 (non-hex `sha512` digest, the same path as E030) and the `E033` cases for wrong JSON types of `id` and `address`, which share one helper with tested callers.

## TDD: failing run before implementation

Tests written first, with no `digest`, `inventory` or `namaste` module (`cargo test -p filesquid-ocfl --locked --no-fail-fast --test unit_digest --test unit_inventory --test unit_namaste_sidecar --test integration_fixture_inventories --test features`, error lines with counts):

```text
      3 error[E0432]: unresolved import `filesquid_ocfl::namaste`
      3 error[E0432]: unresolved import `filesquid_ocfl::inventory`
      3 error[E0432]: unresolved import `filesquid_ocfl::digest`
      1 error: could not compile `filesquid-ocfl` (test "unit_namaste_sidecar") due to 3 previous errors
      1 error: could not compile `filesquid-ocfl` (test "unit_digest") due to 1 previous error
      1 error: could not compile `filesquid-ocfl` (test "integration_fixture_inventories") due to 2 previous errors
      1 error: could not compile `filesquid-ocfl` (test "features") due to 3 previous errors
```

The first green run then failed five `unit_inventory` tests in the test helper `set`, which could not add a key that did not exist yet (`pointer_mut` only reaches existing keys); the helper was fixed to insert into the parent object.

## Gates

Run from `apps/filesquid/` with `CARGO_TARGET_DIR` on a shared target:

| Command | Result |
|---|---|
| `cargo fmt --all` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test -p filesquid-ocfl --locked` | all pass (counts below) |
| `cargo test -p filesquid-ocfl --locked --no-default-features` | all pass (below) |
| `cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown` | builds |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | clean |
| `bash scripts/pre-commit.sh` (repository root) | exit 0 |

## Mocked Environment Variables

None. `FILESQUID_OCFL_FIXTURES` is an optional input that names real fixture data, not a mocked value.

## Raw test output

### `cargo test -p filesquid-ocfl --locked` (summary lines)

```text
     Running unittests src/lib.rs
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.29s
     Running tests/features.rs
57 scenarios (48 passed, 9 skipped)
208 steps (199 passed, 9 skipped)
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/fs_conformance.rs
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.60s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.83s
     Running tests/unit/digest.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/git_guard.rs
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/unit/inventory.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/namaste_sidecar.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/storage_key.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### `cargo test -p filesquid-ocfl --locked --no-default-features` (summary lines)

```text
     Running unittests src/lib.rs
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.40s
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.96s
     Running tests/unit/digest.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/git_guard.rs
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/unit/inventory.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/unit/namaste_sidecar.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/source_scan.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/storage_key.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### `cargo test -p filesquid-ocfl --locked --test unit_digest --test unit_inventory --test unit_namaste_sidecar`

```text
running 5 tests
test sha256_is_the_engine_algorithm ... ok
test digests_compare_without_regard_to_case ... ok
test names_and_lengths_follow_ocfl ... ok
test known_vectors_match ... ok
test streaming_matches_one_shot ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s


running 31 tests
test a_digest_appears_once_regardless_of_case ... ok
test a_padded_head_outside_the_width_is_refused ... ok
test a_written_inventory_parses_back_to_the_same_data ... ok
test a_valid_inventory_becomes_typed_data ... ok
test an_ocfl_1_0_inventory_type_is_recognised ... ok
test content_directory_defaults_to_content ... ok
test content_directory_must_be_a_direct_child_name ... ok
test content_paths_are_safe_and_unique ... ok
test deep_nesting_is_refused_without_exhausting_the_stack ... ok
test every_manifest_digest_is_used_by_a_version ... ok
test duplicate_json_keys_are_refused ... ok
test fixity_follows_the_manifest_structure ... ok
test head_must_name_the_highest_version ... ok
test malformed_json_is_refused ... ok
test created_is_an_rfc3339_date_time_to_the_second ... ok
test manifest_digests_match_the_algorithm ... ok
test manifest_structure_is_checked ... ok
test message_and_user_have_their_shapes ... ok
test optional_blocks_may_be_absent_or_empty ... ok
test logical_paths_are_safe_and_unique ... ok
test state_digests_exactly_match_the_manifest ... ok
test mixed_case_digests_are_kept_as_written ... ok
test required_keys_are_enforced ... ok
test type_and_digest_algorithm_are_checked ... ok
test version_numbers_parse_format_and_advance ... ok
test version_sequences_start_at_one_without_gaps ... ok
test versions_must_be_a_non_empty_object ... ok
test zero_padded_versions_keep_one_width ... ok
test version_names_are_v_and_a_positive_integer ... ok
test unknown_keys_are_refused ... ok
test oversized_input_is_refused_before_parsing ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s


running 8 tests
test a_malformed_sidecar_is_refused ... ok
test declaration_contents_must_be_exact ... ok
test a_sidecar_with_the_wrong_digest_is_refused ... ok
test declarations_have_their_namaste_names_and_contents ... ok
test declaration_file_names_are_recognised ... ok
test sidecar_names_carry_the_algorithm ... ok
test a_written_sidecar_verifies ... ok
test sidecar_separators_and_case_are_accepted ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Cucumber: `inventory_types_OCFL-S-002.feature`

```text
Feature: SHA-256 digests and OCFL 1.1 inventory types
  Scenario: SHA-256 is the engine's digest algorithm
   ✔  When I digest "abc" with the engine's digest algorithm
   ✔  Then the algorithm should be named "sha256"
   ✔  And the digest should be "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
  Scenario: A valid OCFL 1.1 inventory becomes typed data
   ✔  Given the example OCFL 1.1 inventory
   ✔  When I parse the inventory
   ✔  Then the head should be "v2"
   ✔  And the content directory should be "content"
   ✔  And the inventory should have 2 versions
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that it has an unknown key
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E102"
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that it has no id
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E036"
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that its digest algorithm is md5
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E025"
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that its head is not the highest version
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E040"
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that its versions skip a number
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E010"
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that a version was created without a time zone
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E049"
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that a content path ends with a slash
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E100"
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that a state digest differs in case from the manifest
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E050"
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that two logical paths in a version conflict
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E095"
  Scenario Outline: An inventory that breaks an OCFL rule is refused with its code
   ✔  Given the example OCFL 1.1 inventory
   ✔  And the inventory is changed so that a digest appears twice in the manifest
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused with "E096"
  Scenario: An oversized inventory is refused before it is parsed
   ✔  Given an inventory one byte larger than the parser's bound
   ✔  When I parse the inventory
   ✔  Then the inventory should be refused as too large
  Scenario: The inventory digest sidecar must match the inventory
   ✔  Given the example OCFL 1.1 inventory
   ✔  When I write its sha256 sidecar
   ✔  Then the sidecar should verify against the inventory
   ✔  And a sidecar for different bytes should be refused with "E060"
  Scenario: An OCFL 1.1 object declares itself with NAMASTE
   ✔  When I make the OCFL 1.1 object declaration
   ✔  Then its file name should be "0=ocfl_object_1.1"
   ✔  And its contents should be "ocfl_object_1.1" and a newline
```

### Fixture run: `FILESQUID_OCFL_FIXTURES=<fixtures>/1.1 cargo test -p filesquid-ocfl --locked --test integration_fixture_inventories -- --nocapture --test-threads 1`

```text
running 3 tests
test inventory_level_bad_fixtures_are_refused ... E001_invalid_version_format: refused with E104 (named in the prefix: false)
E008_E036_no_versions_no_head: refused with E036 (named in the prefix: true)
E010_skipped_versions: refused with E010 (named in the prefix: true)
E011_E013_invalid_padded_head_version: refused with E011 (named in the prefix: true)
E017_invalid_content_dir: refused with E017 (named in the prefix: true)
E025_wrong_digest_algorithm: refused with E025 (named in the prefix: true)
E036_no_head: refused with E036 (named in the prefix: true)
E036_no_id: refused with E036 (named in the prefix: true)
E040_head_not_most_recent: refused with E040 (named in the prefix: true)
E040_wrong_head_doesnt_exist: refused with E040 (named in the prefix: true)
E040_wrong_head_format: refused with E040 (named in the prefix: true)
E041_no_manifest: refused with E041 (named in the prefix: true)
E049_E050_E054_bad_version_block_values: refused with E049 (named in the prefix: true)
E049_created_no_timezone: refused with E049 (named in the prefix: true)
E049_created_not_to_seconds: refused with E049 (named in the prefix: true)
E050_manifest_digest_wrong_case: refused with E050 (named in the prefix: true)
E050_state_digest_not_in_manifest: refused with E050 (named in the prefix: true)
E053_E052_invalid_logical_paths: refused with E053 (named in the prefix: true)
E058_no_sidecar: refused with E058 (named in the prefix: true)
E060_E064_root_inventory_digest_mismatch: refused with E060 (named in the prefix: true)
E061_invalid_sidecar: refused with E061 (named in the prefix: true)
E095_conflicting_logical_paths: refused with E095 (named in the prefix: true)
E095_non_unique_logical_paths: refused with E095 (named in the prefix: true)
E096_manifest_duplicate_digests: refused with E096 (named in the prefix: true)
E097_fixity_duplicate_digests: refused with E097 (named in the prefix: true)
E100_E099_fixity_invalid_content_paths: refused with E099 (named in the prefix: true)
E100_E099_manifest_invalid_content_paths: refused with E099 (named in the prefix: true)
E101_non_unique_content_paths: refused with E101 (named in the prefix: true)
E107_file_in_manifest_not_used: refused with E107 (named in the prefix: true)
ok
test other_bad_fixtures_report_how_their_root_inventory_reads ... E001_extra_dir_in_root: root inventory and sidecar read (object-level fault, IO-0.5)
E001_extra_file_in_root: root inventory and sidecar read (object-level fault, IO-0.5)
E001_v2_file_in_root: root inventory and sidecar read (object-level fault, IO-0.5)
E003_E063_empty: refused with read: No such file or directory (os error 2) (object-level fault, IO-0.5)
E003_no_decl: root inventory and sidecar read (object-level fault, IO-0.5)
E007_bad_declaration_contents: root inventory and sidecar read (object-level fault, IO-0.5)
E010_missing_versions: root inventory and sidecar read (object-level fault, IO-0.5)
E015_content_not_in_content_dir: root inventory and sidecar read (object-level fault, IO-0.5)
E019_inconsistent_content_dir: root inventory and sidecar read (object-level fault, IO-0.5)
E023_extra_file: root inventory and sidecar read (object-level fault, IO-0.5)
E023_old_manifest_missing_entries: root inventory and sidecar read (object-level fault, IO-0.5)
E037_inconsistent_id: root inventory and sidecar read (object-level fault, IO-0.5)
E040_wrong_version_in_version_dir: root inventory and sidecar read (object-level fault, IO-0.5)
E046_root_not_most_recent: root inventory and sidecar read (object-level fault, IO-0.5)
E060_version_inventory_digest_mismatch: root inventory and sidecar read (object-level fault, IO-0.5)
E063_no_inv: refused with read: No such file or directory (os error 2) (object-level fault, IO-0.5)
E064_different_root_and_latest_inventories: root inventory and sidecar read (object-level fault, IO-0.5)
E066_E092_old_manifest_digest_incorrect: root inventory and sidecar read (object-level fault, IO-0.5)
E066_algorithm_change_state_mismatch: root inventory and sidecar read (object-level fault, IO-0.5)
E066_inconsistent_version_state: root inventory and sidecar read (object-level fault, IO-0.5)
E067_file_in_extensions_dir: root inventory and sidecar read (object-level fault, IO-0.5)
E092_E093_content_path_does_not_exist: root inventory and sidecar read (object-level fault, IO-0.5)
E092_algorithm_change_incorrect_digest: root inventory and sidecar read (object-level fault, IO-0.5)
E092_content_file_digest_mismatch: root inventory and sidecar read (object-level fault, IO-0.5)
E093_fixity_digest_mismatch: root inventory and sidecar read (object-level fault, IO-0.5)
E103_older_spec_v2: root inventory and sidecar read (object-level fault, IO-0.5)
ok
test valid_and_warning_fixtures_parse_and_verify ... 25 valid and warning fixtures parsed and verified
ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```
