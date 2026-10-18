# Task IO-0.8 Walkthrough: Opaque bytes; dual profile is a caller

Module plan row: [IO-0.8](../plan.md). Global task: [Task 0.4](../../../../../../resources/implementation-plan.md#task-0.4) (stays open: IO-0.9 and IO-0.10 remain).

## OCFL-S-020: Opaque bytes; the dual profile is a caller

## OCFL-S-002: Digest algorithm is SHA-256 (digests over the stored bytes)

## OCFL-S-003: Default public API is generic OCFL 1.1 (no profile or encryption types)

## OCFL-S-010: Read and write conformance (b) — sealed content fixtures valid in the engine and an independent community validator

Sources: [OCFL-S-020, OCFL-S-002, OCFL-S-003, OCFL-S-007, OCFL-S-010](../srs.md) (OCFL-S-020 is a new row, see Sync), [PRD F-042](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), [PRD TC-002](../../../../../../resources/prd.md#tc-002-endpoint-processing-and-hosted-encryption), [ADR 010](../../../../../../resources/architecture/010-ocfl-1-1-dual-profile.md) Decision 2, [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md) "Dual-profile encryption".

## What changed

- **No engine change was needed.** The IO-0.6 writer and the IO-0.4 reader already treat content and logical paths as bytes and strings: they never decode, transform or inspect content, and digest exactly what was staged. This task proves that and records it as OCFL-S-020. The crate rustdoc (`src/lib.rs`) gains an "Opaque bytes" section.
- **Test caller transform.** `tests/support/opaque.rs` holds `ToyCipher`, a keyed SHA-256 counter-mode keystream XOR. It is test-only and NOT cryptography. Playing the hosted-profile caller, it seals content (nonce: the plaintext path, so unchanged files seal to the same bytes), seals each logical path into one lowercase hex segment, gives the object an opaque `urn:opaque:` id, and seals `message` and `user` text (a user's `address` becomes an opaque `urn:opaque:` URI). `created` stays an RFC 3339 timestamp, as OCFL requires. The owner-readable profile is the identity: the same `write_fixture` call with the plaintext.
- **Integration tests** (`tests/integration/opaque_bytes.rs`, new target `integration_opaque_bytes`):
  - `every_byte_value_and_ocfl_lookalike_content_round_trips` (memory and filesystem): stores every byte value, an empty file, bytes that are not UTF-8, a copy of a pinned `inventory.json`, a sidecar-looking file and a `0=ocfl_object_1.1` file as content. Each validates, streams back byte-identical and is digested as stored.
  - `a_file_above_one_chunk_streams_back_in_bounded_chunks`: 8 MiB + 17 bytes, staged by multipart, read back in two chunks of at most 8 MiB.
  - `sealed_content_fixtures_validate_and_only_the_caller_recovers_them` (filesystem): all 7 pinned content fixtures, sealed. For each, the engine validator reports zero errors, the stored state is exactly what the caller gave, the manifest digests are SHA-256 of the sealed bytes, and the caller opens every version to its source. No plaintext name, object name, object id, metadata (user address included) or content shows. ocfl-py and rocfl accept every object.
  - `both_profiles_use_the_same_engine_calls`: `spec-ex-full` stored plain and sealed through the same calls.
  - `unchanged_sealed_bytes_are_not_written_again`: sealed bytes are deduplicated by digest like plaintext (`cf3`: v2 writes one file, v3 none; `spec-ex-full`: 3, 1, 0).
  - `a_changed_sealed_file_is_refused_with_e092`: manifest digest verification over ciphertext needs no key.
  - `the_engine_has_no_profile_or_encryption_types`: no `encrypt`, `decrypt`, `cipher`, `plaintext`, `profile` or `aead` in engine code (comments and strings removed, case ignored), and no encryption crate in `Cargo.toml`, whether declared by key, as a `[dependencies.<name>]` table or renamed through `package = "…"`.
  - Two oracle checks show the leak check and the scan find what they look for: `the_plaintext_check_finds_an_owner_readable_object` and `the_profile_scan_finds_profile_code_and_encryption_crates`.
- **BDD.** New `tests/features/opaque_bytes_OCFL-S-020.feature` (6 scenarios) with steps in `tests/support/opaque_steps.rs`.
- **Test support.** `content_fixtures::draft_for` stages through `write_whole`, so files above one put (8 MiB) are staged by multipart.
- **No new dependency.** The transform uses `sha2`, already a dependency. Fixtures are unchanged (OCFL-S-011). The `wasm32-unknown-unknown` build with default features off still passes (BDD "Engine builds for the browser").

## TDD

The tests were written before any engine change. The first run failed 5 of 6 tests; every cause was in the test harness, not the engine:

```text
test both_profiles_use_the_same_engine_calls ... FAILED
test every_byte_value_and_ocfl_lookalike_content_round_trips ... FAILED
test sealed_content_fixtures_validate_and_only_the_caller_recovers_them ... FAILED
test unchanged_sealed_bytes_are_not_written_again ... ok
test the_engine_has_no_profile_or_encryption_types ... FAILED
test a_file_above_one_chunk_streams_back_in_bounded_chunks ... FAILED
```

- The test read whole files with a chunk size above the 8 MiB bound.
- The profile scan matched `nonce` inside `FnOnce` once whitespace was removed, so `nonce` was dropped from the word list.
- The fixture helper staged with one `put`, which is capped at 8 MiB, so it now uses `write_whole`.

After those harness fixes every test passed against the unchanged engine; OCFL-S-020 needed no engine change. The two oracle tests show the leak and profile checks fail when their target is present.

## Constraint gates

| Constraint | Gate |
|---|---|
| C1 every byte value, empty, non-UTF-8 and OCFL-looking bytes are content | `every_byte_value_and_ocfl_lookalike_content_round_trips`; BDD "Bytes that look like OCFL files or are not text are stored as content" |
| C2 digests are SHA-256 of the stored bytes | `assert_digests_are_of_stored_bytes` in the round-trip, chunk and sealed tests; BDD "its inventory should parse with sha256 digests of the stored bytes" |
| C3 a file above one 8 MiB chunk | `a_file_above_one_chunk_streams_back_in_bounded_chunks` |
| C4 opaque logical paths stored as given | sealed tests (`assert_state_matches` over sealed names) |
| C5 one engine API for both profiles | `both_profiles_use_the_same_engine_calls`; BDD "The owner-readable profile uses the same engine calls" |
| C6 inventory parses; engine and community validators accept sealed objects | `sealed_content_fixtures_validate_and_only_the_caller_recovers_them`; BDD "A sealed object stays structurally readable OCFL" |
| C7 no plaintext in keys, inventory or content | `assert_no_plaintext`, oracle `the_plaintext_check_finds_an_owner_readable_object`; BDD "A sealed object shows no plaintext" |
| C8 caller recovers every version | `assert_caller_recovers`; BDD "the caller should recover the plaintext of every version" |
| C9 no profile or encryption types or crates | `the_engine_has_no_profile_or_encryption_types`, oracle `the_profile_scan_finds_profile_code_and_encryption_crates`; BDD "The engine has no profile or encryption types" |
| C10 unchanged sealed bytes stored once | `unchanged_sealed_bytes_are_not_written_again` |
| C11 wasm32 build, no other module dependency | BDD "Engine builds for the browser", "it should build and publish without other FileSquid crates" |
| Manifest digest verification over ciphertext (E092) | `a_changed_sealed_file_is_refused_with_e092`; BDD "A changed sealed file is refused without a key" |

## Test output

`FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1 cargo test -p filesquid-ocfl --locked --test integration_opaque_bytes -- --nocapture --test-threads=1` (ocfl-py 2.1.0 and rocfl 1.7.0 on `PATH`):

```text
running 9 tests
test a_changed_sealed_file_is_refused_with_e092 ... ok
test a_file_above_one_chunk_streams_back_in_bounded_chunks ... ok
test both_profiles_use_the_same_engine_calls ... ok
test every_byte_value_and_ocfl_lookalike_content_round_trips ... ok
test sealed_content_fixtures_validate_and_only_the_caller_recovers_them ... filesquid-ocfl sealed cf1 at bcf213: VALID
filesquid-ocfl sealed cf2 at bcf210: VALID
filesquid-ocfl sealed cf3 at bcf211: VALID
filesquid-ocfl sealed cf4 at bcf216: VALID
filesquid-ocfl sealed spec-ex-diff-paths at ace4476fd89d1812af5a63a67c35bc1f4022: VALID
filesquid-ocfl sealed spec-ex-full at ace4476fd89d1812ad4669ac: VALID
filesquid-ocfl sealed spec-ex-minimal at ace4476fd89d1812a65a6ba93c24b1: VALID
ocfl-py sealed cf1: VALID
ocfl-py sealed cf2: VALID
ocfl-py sealed cf3: VALID
ocfl-py sealed cf4: VALID
ocfl-py sealed spec-ex-diff-paths: VALID
ocfl-py sealed spec-ex-full: VALID
ocfl-py sealed spec-ex-minimal: VALID
rocfl sealed cf1: VALID
rocfl sealed cf2: VALID
rocfl sealed cf3: VALID
rocfl sealed cf4: VALID
rocfl sealed spec-ex-diff-paths: VALID
rocfl sealed spec-ex-full: VALID
rocfl sealed spec-ex-minimal: VALID
test the_engine_has_no_profile_or_encryption_types ... ok
test the_plaintext_check_finds_an_owner_readable_object ... ok
test the_profile_scan_finds_profile_code_and_encryption_crates ... ok
test unchanged_sealed_bytes_are_not_written_again ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.36s
```

`opaque_bytes_OCFL-S-020.feature` from `FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1 cargo test -p filesquid-ocfl --locked`:

```text
Feature: The engine stores opaque bytes and the dual profile is a caller
  Scenario: A sealed object stays structurally readable OCFL
   ✔  When a caller writes the content fixture "spec-ex-full" sealed with its own key
   ✔  Then the written object should validate with zero errors in filesquid-ocfl
   ✔  And its inventory should parse with sha256 digests of the stored bytes
   ✔  And the caller should recover the plaintext of every version
  Scenario: The owner-readable profile uses the same engine calls
   ✔  When a caller writes the content fixture "spec-ex-full" as plaintext
   ✔  Then the written object should validate with zero errors in filesquid-ocfl
   ✔  And its inventory should parse with sha256 digests of the stored bytes
   ✔  And every version should read back as its plaintext source
  Scenario: Bytes that look like OCFL files or are not text are stored as content
   ✔  When a caller writes files holding every byte value, no bytes, bytes that are not UTF-8 and an OCFL inventory
   ✔  Then the written object should validate with zero errors in filesquid-ocfl
   ✔  And every version should read back as its plaintext source
  Scenario: A sealed object shows no plaintext
   ✔  When a caller writes the content fixture "spec-ex-full" sealed with its own key
   ✔  Then no stored key, inventory text or content file should show the plaintext
  Scenario: A changed sealed file is refused without a key
   ✔  Given a caller wrote the content fixture "spec-ex-minimal" sealed with its own key
   ✔  When one byte of its sealed content file changes
   ✔  Then reading that file should be refused with OCFL code "E092"
  Scenario: The engine has no profile or encryption types
   ✔  When I inspect filesquid-ocfl library dependencies
   ✔  Then no profile or encryption type or encryption crate should be part of the engine
```

Whole crate, same command:

```text
106 scenarios (103 passed, 3 skipped)
461 steps (458 passed, 3 skipped)
     Running unittests src/lib.rs (/home/user/.cargo-target-shared/debug/deps/filesquid_ocfl-9ee12e3a2a8ac091)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.35s
     Running tests/features.rs (/home/user/.cargo-target-shared/debug/deps/features-660d3beb4ac78a47)
     Running tests/integration/content_fixtures.rs (/home/user/.cargo-target-shared/debug/deps/integration_content_fixtures-eb54ca1aa5b99bf0)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.20s
     Running tests/integration/fixture_conformance.rs (/home/user/.cargo-target-shared/debug/deps/integration_fixture_conformance-e155409d48cae74b)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
     Running tests/integration/fixture_inventories.rs (/home/user/.cargo-target-shared/debug/deps/integration_fixture_inventories-e2c8132ecc88ab92)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/fixture_reads.rs (/home/user/.cargo-target-shared/debug/deps/integration_fixture_reads-0781dec467712594)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.47s
     Running tests/integration/fs_conformance.rs (/home/user/.cargo-target-shared/debug/deps/integration_fs_conformance-53113de746b3ccb6)
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.94s
     Running tests/integration/hierarchy_guard.rs (/home/user/.cargo-target-shared/debug/deps/integration_hierarchy_guard-8487ef9b8986f271)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/integration/logical_file.rs (/home/user/.cargo-target-shared/debug/deps/integration_logical_file-85503f2116830ce8)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.65s
     Running tests/integration/memory_conformance.rs (/home/user/.cargo-target-shared/debug/deps/integration_memory_conformance-62ff572b6c54f901)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s
     Running tests/integration/opaque_bytes.rs (/home/user/.cargo-target-shared/debug/deps/integration_opaque_bytes-f58cee03ad590c72)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.13s
     Running tests/integration/read_objects.rs (/home/user/.cargo-target-shared/debug/deps/integration_read_objects-f3ca64edfc55818c)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.48s
     Running tests/integration/validate_objects.rs (/home/user/.cargo-target-shared/debug/deps/integration_validate_objects-d8b9f446fae33dcb)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
     Running tests/integration/write_order.rs (/home/user/.cargo-target-shared/debug/deps/integration_write_order-c53d51931c61d547)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.74s
     Running tests/unit/digest.rs (/home/user/.cargo-target-shared/debug/deps/unit_digest-6975f6529ef18af2)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/fixity.rs (/home/user/.cargo-target-shared/debug/deps/unit_fixity-9197e75113c84979)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs (/home/user/.cargo-target-shared/debug/deps/unit_git_guard-baeea4437ada9384)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/inventory.rs (/home/user/.cargo-target-shared/debug/deps/unit_inventory-8e91e5efcdd5c241)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/unit/lease.rs (/home/user/.cargo-target-shared/debug/deps/unit_lease-7b482c6c5384d2e7)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/namaste_sidecar.rs (/home/user/.cargo-target-shared/debug/deps/unit_namaste_sidecar-8cded6eb4632033d)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/read_state.rs (/home/user/.cargo-target-shared/debug/deps/unit_read_state-bab33eba56254469)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs (/home/user/.cargo-target-shared/debug/deps/unit_source_scan-da347b40e774f677)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/storage_key.rs (/home/user/.cargo-target-shared/debug/deps/unit_storage_key-c5a229a0a5758ac8)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo clippy -p filesquid-ocfl --all-targets --locked -- -D warnings` and `cargo doc -p filesquid-ocfl --no-deps` pass. `bash scripts/pre-commit.sh` passes before the push.

## Review round 1 (Cubic)

- The SRS row, README and rustdoc now limit "stored unchanged" to content bytes: logical paths are UTF-8 strings stored as given that must pass the OCFL 1.1 path rules (E052, E053), and a caller that hides names encodes them. They call E092 manifest digest verification (E093 is the fixity block), say the `filesquid-api` transforms are planned, and say only the hosted package is sealed.
- `ToyCipher` keeps a user's `address`, sealed into a `urn:opaque:` URI.
- The leak check also looks for the plaintext object name, object id and user address; its oracle test now expects the id and address.
- The dependency scan also lists `[dependencies.<name>]` tables and `package = "…"` renames; its oracle test covers both.
- "Remaining limits" states the confidentiality cost of deterministic sealing.

`FILESQUID_OCFL_COMMUNITY_VALIDATOR=rocfl FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1 cargo test -p filesquid-ocfl --locked --test integration_opaque_bytes` (rocfl 1.7.0):

```text
running 9 tests
test a_changed_sealed_file_is_refused_with_e092 ... ok
test both_profiles_use_the_same_engine_calls ... ok
test every_byte_value_and_ocfl_lookalike_content_round_trips ... ok
test the_plaintext_check_finds_an_owner_readable_object ... ok
test the_profile_scan_finds_profile_code_and_encryption_crates ... ok
test unchanged_sealed_bytes_are_not_written_again ... ok
test the_engine_has_no_profile_or_encryption_types ... ok
test sealed_content_fixtures_validate_and_only_the_caller_recovers_them ... ok
test a_file_above_one_chunk_streams_back_in_bounded_chunks ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.22s
```

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. Every constraint above has at least one test. No engine line changed.

## Sync Execution Report

- **Module SRS:** new row **OCFL-S-020** (opaque bytes; the dual profile is a caller). It restates the IO-0.8 plan description, ADR 010 Decision 2 and ADR 023 "Dual-profile encryption". It adds two testable consequences of those sources: digests are over the stored bytes (OCFL-S-002), and the engine takes no encryption dependency.
- **Module plan:** IO-0.8 checked; Related Requirements add OCFL-S-020; Related Tests add the new feature, the integration test and this Walkthrough.
- **Repository-root implementation-plan checklist:** IO-0.8 checked. Global Task 0.4 stays open (IO-0.9 and IO-0.10).
- **Crate README:** new "Opaque bytes" section. **Root README:** "Run tests" names `integration_opaque_bytes`.
- **Crate rustdoc:** "Opaque bytes" section in `src/lib.rs`.
- **Component diagram:** "Opaque bytes; dual profile is a caller" is now BUILT (IO-0.8).
- **PRD, global SRS, product vision, ADRs:** unchanged. ADR 010 and ADR 023 already make the dual profile a caller transform; no new technology or dependency.
- **Server feature** `endpoint_processing_and_hosted_encryption_TC-002.feature` is owned by `filesquid-server` and not edited.

## Mocked Environment Variables

None. No credential or `.env` value was mocked. The `ToyCipher` key is a fixed test string, not a credential.

## Remaining limits and owner decisions

- **The hosted profile's real encryption is not here.** ADR 009's suite, key custody and name encryption belong to the caller (`filesquid-api` and the private-client library, ADR 023). That caller must keep OCFL path rules for the names it produces (this test uses one hex segment per path) and must keep `created` an RFC 3339 timestamp.
- **Deterministic sealing is a confidentiality trade-off, not only a space one.** `ToyCipher` uses the plaintext path as the nonce, so every version of a path reuses one keystream. Changed content at that path leaks the XOR of the two plaintexts, and unchanged ciphertext shows that the content did not change. This is acceptable only in a test transform. The real caller MUST seal with a fresh nonce per encryption (an AEAD from the ADR 009 suite), and then every file is stored again in every version. Keeping deduplication over ciphertext needs a deliberate owner decision to accept the equality leak (for example convergent or SIV-style sealing, which reveals that two files are equal but not their XOR). The engine is correct either way.
- **Numbering:** PR #37 (open) also adds a module SRS row after OCFL-S-019. Whichever merges second renumbers its row; this one is OCFL-S-020.
