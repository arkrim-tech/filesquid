# Task CP-0.1 Walkthrough: Scaffold, isolation, crypto backend and oracle harness

Global task: [Task 0.17](../../../../../../resources/implementation-plan.md) (first slice; CP-0.2 to CP-0.5 follow, so the global box stays open). Module plan: [plan.md](../plan.md). Branch: `feat/cp-0.1-scaffold`.

## C2PA-S-006: Cryptographic primitives MUST come from established crates behind a crypto-backend trait and MUST NOT be hand-rolled

Full requirement text ([module SRS](../srs.md)): "Cryptographic primitives MUST come from established crates behind a crypto-backend trait and MUST NOT be hand-rolled." [PRD F-054](../../../../../../resources/prd.md#f-054-c2pa-module) adds that the chosen crates MUST be recorded in the module README.

## C2PA-S-007: The official `c2pa` crate, `c2patool` and C2PA conformance files MUST verify interoperability in CI and MUST NOT be runtime dependencies

Full requirement text: "The official `c2pa` crate, `c2patool` and C2PA conformance files MUST verify interoperability in CI and MUST NOT be runtime dependencies." This repository has no hosted CI (owner decision 2026-10-17, [local-verification.md](../../../../../../.agents/rules/local-verification.md)), so the oracle runs in the local `scripts/pre-commit.sh` gate through `cargo test --workspace`.

## C2PA-S-008: Default dependencies MUST exclude every other FileSquid crate and the core MUST read and write through caller-supplied async byte sources and sinks

Full requirement text: "Default dependencies MUST exclude `filesquid-ocfl`, `filesquid-core` and every other FileSquid crate, Axum, Tauri, SQLite, libvips, FFmpeg and distribution code. The core MUST read and write through caller-supplied async byte sources and sinks and MUST NOT call `std::fs` or a network client directly."

## C2PA-S-009: The crate MUST build for `wasm32-unknown-unknown` and MUST NOT require a server

Full requirement text: "The crate MUST build for `wasm32-unknown-unknown` and MUST NOT require a server. Any developer MAY embed it wherever Rust runs. In FileSquid it runs behind `filesquid-api` in the desktop host, in `filesquid-server` and in the encrypted browser host, and MUST NOT be linked into the ciphertext-only hosted service."

Also covered: [SRS-F-054](../../../../../../resources/srs.md#srs-f-054-c2pa-module) (remote fetches through a caller-supplied fetcher; the module pins a specification version), [ADR 026](../../../../../../resources/architecture/026-filesquid-provenance-engine.md), [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md).

## What changed

- **Crate scaffold:** `apps/filesquid/crates/c2pa/Cargo.toml` (`publish = false`, MIT), added to the workspace members. Feature `rust-crypto` (default) carries every cryptography crate; with `--no-default-features` the crate links none.
- **`src/lib.rs`:** module docs and `C2PA_SPEC_VERSION = "2.4"`. The README pins "C2PA Technical Specification 2.4"; a scenario checks both agree.
- **`src/io.rs`:** `ByteSource` (`size`, `read_at`) and `ByteSink` (`write_all`, `finish`) with `impl Future` returns and no `Send` bound, so single-threaded `wasm32` hosts can implement them; `read_exact_at` (bounded: each iteration reads at least one byte, offset overflow checked); `MemorySource`, `MemorySink`; `ByteIoError`.
- **`src/fetch.rs`:** `Fetcher` trait, `FetchRequest` (`get`/`post`, `FetchPurpose::{RemoteManifest, Revocation, Timestamp}`, `max_bytes`), `FetchResponse`, `FetchError::{Offline, TooLarge, Failed}`, `OfflineFetcher` (refuses everything), `BoundedBody` (a fetcher pushes chunks into it and stops at the cap, so an oversized body is never buffered; the `Fetcher` contract requires this), `fetch_bounded` (checks the cap again and logs a failure with `tracing::warn!` carrying only the redacted origin `scheme://host[:port]`, the purpose and an error category, never the URL path, query, userinfo or the caller's error text), `redacted_origin`, `FetchError::category`, `DEFAULT_MAX_FETCH_BYTES` = 16 MiB.
- **`src/crypto/mod.rs`:** `CryptoBackend` (`incremental_hasher`, default `digest`, default `digest_ranges` that hashes ranges of a `ByteSource` in 64 KiB chunks without buffering the asset and that an async-only backend such as WebCrypto overrides, `verify_signature` over an SPKI DER key and the COSE `r || s` signature form), `IncrementalHasher`, `HashAlgorithm` (C2PA names, output lengths), `SigningAlgorithm` (COSE ids -7, -35, -36, -37, -38, -39, -8 and C2PA names), `CryptoError`.
- **`src/crypto/rust_crypto.rs`:** `RustCryptoBackend` on `sha2` 0.11, `p256`/`p384`/`p521` 0.14 and `ed25519-dalek` 3 (strict verification). No RNG is pulled in, so it builds for `wasm32-unknown-unknown` without `getrandom`. RSA-PSS, X.509 and CMS crates are chosen and recorded in the README and wired in CP-0.4; until then PS256/384/512 return `CryptoError::UnsupportedAlgorithm`.
- **Isolation (`tests/unit/isolation.rs`, `tests/support/deps.rs`):** `cargo tree -e normal,build --target all` on the resolved graph (default, `--all-features` and `--no-default-features`), checked against a rule table: any `filesquid-*` crate but this one, `imagesquid*`, `arkrim*`, `axum*`, `tauri*`, `rusqlite`, `sqlite`, `libsqlite3-sys`, `sqlx*`, `libvips*`, `ffmpeg*`, `c2pa`, `c2pa-*`, `c2patool`, and network clients or runtimes (`reqwest`, `hyper*`, `ureq`, `isahc`, `surf`, `curl*`, `attohttpc`, `tokio*`, `async-std`, `smol`, `async-executor`, `async-global-executor`, `async-io`, `async-net`, `actix-rt`, `glommio`, `mio`). Every rule is tested with a name generated from the rule table, so a new rule cannot go untested. Dev-dependencies are not checked, so the official crate MAY be an oracle there.
- **Source scan (`tests/unit/source_scan.rs`):** `src/` (bounded walk) has no `std::fs`, `std::net`, `std::process`, `tokio`, `async_std`, `reqwest`, `hyper`, `ureq`, `isahc`, `File::open` or `File::create` in code (line and nested block comments and whitespace removed first; grouped imports such as `use std::{fs, io};` expanded), and no cryptography crate outside `src/crypto/rust_crypto.rs`.
- **wasm32 (`tests/integration/wasm_build.rs`, `tests/support/wasm.rs`):** `cargo build -p filesquid-c2pa --lib --target wasm32-unknown-unknown`, with default features on and off, into `filesquid-c2pa-wasm32-check/` under the Cargo target directory (its own lock, so it never waits on the surrounding `cargo test`). A missing target fails the check; only `FILESQUID_SKIP_WASM32=1` skips it, printing `SKIPPED wasm32 build: ...` and leaving the two `@wasm32` scenarios out of the Cucumber run with a printed line, so a skip is never reported as a pass.
- **Oracle (`tests/integration/oracle.rs`, `tests/support/oracle.rs`, `tests/unit/oracle_harness.rs`):** runs `c2patool` 0.28.2 out of process on each conformance file, reduces its JSON report to a verdict (`NoManifest`, `Valid`, `Trusted`, `Invalid`) plus sorted failure codes, and is built to compare it with the engine's report and fail on disagreement. **No comparison happens in this task**: the engine has no manifest reader, so every file is counted as "pending" and only `c2patool`'s verdict is recorded until CP-0.2 to CP-0.5 wire `engine_report`. An unexpected `c2patool` exit, or JSON without a report field (for example `{}`), is an error, not a verdict. Skipped with a printed reason when the tool or the corpus is absent.
- **BDD:** steps for "Stand-alone dependencies" and "Engine builds for the browser", plus twelve new scenarios for C2PA-S-006, -007, -008, -009 and the specification pin. The reading, minting, editing and panel scenarios belong to later tasks and stay skipped.
- **Docs:** crate README (status, public surface, primitive crates, specification pin, build/test, oracle setup), root README "Run tests" (wasm32 check and oracle variables), component diagram (scaffolded traits), module plan box.

## SRS traceability

| Requirement | Tests |
|---|---|
| C2PA-S-006 crypto behind a backend trait | `unit_source_scan::crypto_crates_stay_behind_the_backend`, `scanner_flags_crypto_crates`; `unit_isolation::no_default_features_graph_has_no_crypto_crate`; `unit_crypto::caller_supplied_backend_uses_the_default_digest`, `async_only_backend_can_supply_its_own_range_digest`, `digest_ranges_rejects_a_reversed_range`; feature "Cryptography stays behind the crypto backend" |
| C2PA-S-006 established crates, not hand-rolled | `unit_crypto::sha256_matches_fips_vector`, `sha384_matches_fips_vector`, `sha512_matches_fips_vector`, `incremental_hash_equals_one_shot_hash`, `ed25519_accepts_rfc8032_vector`, `ed25519_rejects_a_tampered_message`, `ecdsa_signatures_verify_for_every_curve`, `ecdsa_key_of_the_wrong_curve_is_an_invalid_key`, `malformed_public_key_is_reported`, `malformed_signature_is_reported`, `rsa_pss_is_reported_unsupported_until_cp_0_4`, `hash_algorithm_names_follow_c2pa`, `signing_algorithms_map_to_cose_identifiers`; features "The default crypto backend accepts a valid Ed25519 signature", "... rejects a tampered message" |
| C2PA-S-007 oracle harness (comparison **pending**: no engine verdicts until CP-0.5, so this task does not yet verify interoperability) | `integration_oracle::engine_agrees_with_c2patool_on_the_conformance_files` (records 115 `c2patool` verdicts, 115 pending); `unit_oracle_harness` (13 tests); feature "The official c2pa tools are oracles, not dependencies" |
| C2PA-S-007 official crate not a runtime dependency | `unit_isolation::every_rule_rejects_a_matching_name`, `rules_reject_every_named_crate`, `async_runtimes_and_network_clients_are_rules`, `a_forbidden_dependency_would_fail_the_check`, `default_runtime_graph_is_isolated`, `all_features_runtime_graph_is_isolated`; feature "Stand-alone dependencies" |
| C2PA-S-008 no FileSquid, distribution, host, SQLite, media crates | same isolation tests; `unit_isolation::manifest_is_not_published_yet` (`publish = false`, no path outside `apps/filesquid/`); feature "Stand-alone dependencies" |
| C2PA-S-008 async byte sources and sinks | `unit_io` (13 tests); `unit_crypto::digest_ranges_streams_included_ranges_across_chunks`, `digest_ranges_reports_a_short_source`; features "The engine reads through a caller-supplied byte source", "Reading past the end of a byte source is reported" |
| C2PA-S-008 no `std::fs` or network client | `unit_source_scan::core_has_no_direct_io`, `scanner_flags_direct_filesystem_and_network_use`, `scanner_ignores_comment_lines`, `scanner_ignores_block_comments_including_nested_ones`, `scanner_sees_code_after_a_block_comment_on_the_same_line`, `scanner_sees_code_after_a_url_in_a_string`, `scanner_flags_grouped_std_imports`, `scanner_allows_harmless_grouped_imports`; network crates in the isolation rule table; feature "The core performs no direct file or network access" |
| SRS-F-054 caller-supplied fetcher, bounded and redacted | `unit_fetch` (14 tests, including `streaming_fetcher_stops_reading_at_the_cap`, `bounded_fetch_rejects_a_fetcher_that_ignores_the_cap`, `failure_logs_carry_no_secrets`); features "Remote fetches without a caller fetcher are refused", "Oversized remote responses are rejected" |
| C2PA-S-009 builds for `wasm32-unknown-unknown` | `integration_wasm_build::builds_for_wasm32_with_default_features_off`, `builds_for_wasm32_with_the_rust_crypto_backend`; features "Engine builds for the browser" (both) |
| SRS-F-054 pinned specification version | feature "The C2PA specification version is pinned" |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. The tests above exercise every public function and method of `io`, `fetch` and `crypto` and every error variant, including the caller-side ones (`ByteIoError::Backend` in `unit_io::storage_errors_propagate_through_read_exact_at`, `FetchError::Failed` in `unit_fetch::caller_failures_pass_through_with_a_category`, `CryptoError::Backend` in `unit_crypto::async_only_backend_can_supply_its_own_range_digest`). This is a manual inventory, not a measured coverage figure. The cucumber JUnit report is `tests/allure-results/cucumber-junit.xml`.

## Oracle environment and skips

- `c2patool` 0.28.2 was installed for this run with `cargo install c2patool --version 0.28.2 --locked` (it compiles a vendored OpenSSL; about 3 GB of build space). The corpus was a shallow clone of `https://github.com/c2pa-org/public-testfiles` (CC BY-SA 4.0, not vendored).
- With `FILESQUID_C2PATOOL` and `FILESQUID_C2PA_CONFORMANCE_DIR` set, 115 assets ran: oracle verdicts 13 `Valid`, 11 `Invalid`, 91 `NoManifest` (the 91 are ingredient and thumbnail files in the legacy 1.4 folders, which carry no claim); 0 agree, 115 pending engine reader, 0 disagree.
- In `scripts/pre-commit.sh` neither variable is set by default and `c2patool` is not on `PATH`, so the oracle test and the oracle step of the BDD scenario print `SKIPPED c2patool oracle: c2patool not found: set FILESQUID_C2PATOOL or put c2patool on PATH (cargo install c2patool --locked)` and pass.
- The `wasm32-unknown-unknown` target is installed here, so both wasm builds ran and `FILESQUID_SKIP_WASM32` was not set (artifacts under `filesquid-c2pa-wasm32-check/wasm32-unknown-unknown/debug/`).

## Mocked Environment Variables

None. `FILESQUID_C2PATOOL` and `FILESQUID_C2PA_CONFORMANCE_DIR` pointed at a real tool and a real corpus for the oracle run below; they are not secrets and are not stored anywhere.

## TDD: failing run before implementation

`cargo test -p filesquid-c2pa --no-fail-fast` with the tests written and `src/lib.rs` holding only its module comment:

```text
error[E0432]: unresolved import `filesquid_c2pa::crypto`
error: could not compile `filesquid-c2pa` (test "unit_crypto") due to 1 previous error
error[E0432]: unresolved import `filesquid_c2pa::io`
error: could not compile `filesquid-c2pa` (test "unit_io") due to 1 previous error
error[E0432]: unresolved import `filesquid_c2pa::crypto`
error[E0432]: unresolved import `filesquid_c2pa::fetch`
error[E0432]: unresolved import `filesquid_c2pa::io`
error[E0425]: cannot find value `C2PA_SPEC_VERSION` in crate `filesquid_c2pa`
error: could not compile `filesquid-c2pa` (test "features") due to 4 previous errors
```

After the library existed, the BDD run failed once more until the README pinned the specification:

```text
      Step panicked. Captured output: README must pin C2PA 2.4
23 scenarios (12 passed, 10 skipped, 1 failed)
```

## Gates

Run from `apps/filesquid/` with `CARGO_TARGET_DIR` set to a shared target directory:

- `cargo fmt --all`: applied, then clean
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean (one `manual_is_multiple_of` finding in a test helper fixed)
- `cargo clippy -p filesquid-c2pa --no-default-features --all-targets --locked -- -D warnings`: clean (the crypto unit tests and the BDD runner declare `required-features = ["rust-crypto"]`)
- `cargo test -p filesquid-c2pa --no-default-features --locked`: passed
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`: clean
- `bash scripts/pre-commit.sh` from the repository root: passed (all steps; `filesquid-c2pa` runs in `cargo test --workspace` and alone through `MODULE_CRATES`). The first run failed only in the existing `filesquid-api` scenario "A stalled process degrades health live": the build machine had 1.4 GB free on the volume holding the temporary directory, under the API's 5 GiB `DEGRADED_FREE_BYTES` threshold, so health started `Degraded`. Rerun with `TMPDIR` on a tmpfs with free space, the whole gate passed. No code outside this crate was changed.

## Raw test output

`FILESQUID_C2PATOOL=... FILESQUID_C2PA_CONFORMANCE_DIR=... cargo test -p filesquid-c2pa --locked` after the Cubic round 1 fixes (target directory shown as `$CARGO_TARGET_DIR`, blank lines removed):

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.61s
     Running unittests src/lib.rs ($CARGO_TARGET_DIR/debug/deps/filesquid_c2pa-464563f1e2e5510c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-2cebfaed5509fcd4)
Feature: Reusable filesquid-c2pa engine
  Scenario: Read a manifest store
   ?  Given a JPEG with an embedded C2PA manifest store
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:9:5
  Scenario: Validate against a caller-supplied trust list
   ?  Given a signed asset and a trust list supplied by the caller
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:14:5
  Scenario: Mint and sign through a caller-supplied signer
   ?  Given an unsigned PNG and a test signer
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:20:5
  Scenario: Edits never change a signed manifest in place
   ?  Given a signed asset
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:26:5
  Scenario: Unsupported formats are reported
   ?  Given an asset in a format without a handler
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:31:5
  Scenario: Stand-alone dependencies
   ✔  When I inspect filesquid-c2pa library dependencies
   ✔  Then filesquid-ocfl, axum, tauri, sqlite, libvips, ffmpeg, and filesquid-core should be absent from default dependencies
   ✔  And no runtime dependency on the official c2pa crate should exist
  Scenario: Engine builds for the browser
   ✔  When I build filesquid-c2pa for wasm32-unknown-unknown with default features off
   ✔  Then the build should succeed
  Scenario: Engine builds for the browser with the default crypto backend
   ✔  When I build filesquid-c2pa for wasm32-unknown-unknown with default features on
   ✔  Then the build should succeed
  Scenario: Cryptography stays behind the crypto backend
   ✔  When I inspect the filesquid-c2pa sources
   ✔  Then only the crypto backend should name a cryptography crate
  Scenario: The default crypto backend accepts a valid Ed25519 signature
   ✔  Given the RFC 8032 Ed25519 test key and signature
   ✔  When I verify the signature through the default crypto backend
   ✔  Then the signature should be accepted
  Scenario: The default crypto backend rejects a tampered message
   ✔  Given the RFC 8032 Ed25519 test key and signature
   ✔  When I verify the signature over a different message through the default crypto backend
   ✔  Then verification should fail
  Scenario: The core performs no direct file or network access
   ✔  When I inspect the filesquid-c2pa sources
   ✔  Then no direct filesystem, network or process call should be present
  Scenario: The engine reads through a caller-supplied byte source
   ✔  Given a caller-supplied byte source holding "c2pa manifest bytes"
   ✔  When the engine reads 8 bytes at offset 5
   ✔  Then it should receive "manifest"
  Scenario: Reading past the end of a byte source is reported
   ✔  Given a caller-supplied byte source holding "short"
   ✔  When the engine reads 8 bytes at offset 2
   ✔  Then an unexpected end of data should be reported
  Scenario: Remote fetches without a caller fetcher are refused
   ✔  Given no network fetcher supplied by the caller
   ✔  When the engine requests a remote manifest of at most 16 bytes
   ✔  Then the request should be refused as offline
  Scenario: Oversized remote responses are rejected
   ✔  Given a caller fetcher that returns 32 bytes
   ✔  When the engine requests a remote manifest of at most 16 bytes
   ✔  Then the response should be rejected as too large
  Scenario: The official c2pa tools are oracles, not dependencies
   ✔  When I inspect filesquid-c2pa library dependencies
   ✔  Then no runtime dependency on the official c2pa crate should exist
   ✔  And the c2patool oracle harness should report verdicts or a skip reason
  Scenario: The C2PA specification version is pinned
   ✔  When I inspect the filesquid-c2pa specification pin
   ✔  Then the library and the README should both name C2PA 2.4
  Scenario: Content Credentials panel plugs into a file browser sidebar
   ?  Given a selected signed file in a host file browser
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:103:5
  Scenario Outline: Content Credentials panel shows each credential state distinctly
   ?  Given a selected unsigned file in a host file browser
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:108:5
  Scenario Outline: Content Credentials panel shows each credential state distinctly
   ?  Given a selected signed file in a host file browser
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:108:5
  Scenario Outline: Content Credentials panel shows each credential state distinctly
   ?  Given a selected invalid file in a host file browser
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:108:5
  Scenario Outline: Content Credentials panel shows each credential state distinctly
   ?  Given a selected unavailable file in a host file browser
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:108:5
[Summary]
1 feature
23 scenarios (13 passed, 10 skipped)
44 steps (34 passed, 10 skipped)
     Running tests/integration/oracle.rs ($CARGO_TARGET_DIR/debug/deps/integration_oracle-4935b8363a2b30a6)
running 1 test
test engine_agrees_with_c2patool_on_the_conformance_files ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.73s
     Running tests/integration/wasm_build.rs ($CARGO_TARGET_DIR/debug/deps/integration_wasm_build-647cbeae8bcdac72)
running 2 tests
test builds_for_wasm32_with_default_features_off ... ok
test builds_for_wasm32_with_the_rust_crypto_backend ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
     Running tests/unit/crypto.rs ($CARGO_TARGET_DIR/debug/deps/unit_crypto-c47591bb80e8e3c3)
running 18 tests
test digest_ranges_reports_a_short_source ... ok
test digest_ranges_rejects_a_reversed_range ... ok
test caller_supplied_backend_uses_the_default_digest ... ok
test ecdsa_key_of_the_wrong_curve_is_an_invalid_key ... ok
test digest_ranges_streams_included_ranges_across_chunks ... ok
test ed25519_accepts_rfc8032_vector ... ok
test hash_algorithm_names_follow_c2pa ... ok
test incremental_hash_equals_one_shot_hash ... ok
test malformed_public_key_is_reported ... ok
test malformed_signature_is_reported ... ok
test rsa_pss_is_reported_unsupported_until_cp_0_4 ... ok
test sha256_matches_fips_vector ... ok
test sha384_matches_fips_vector ... ok
test sha512_matches_fips_vector ... ok
test signing_algorithms_map_to_cose_identifiers ... ok
test async_only_backend_can_supply_its_own_range_digest ... ok
test ed25519_rejects_a_tampered_message ... ok
test ecdsa_signatures_verify_for_every_curve ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/fetch.rs ($CARGO_TARGET_DIR/debug/deps/unit_fetch-d5bc82a7f3171fb4)
running 14 tests
test bounded_body_accepts_exactly_the_cap ... ok
test bounded_body_keeps_nothing_of_an_overflowing_chunk ... ok
test bounded_fetch_rejects_a_fetcher_that_ignores_the_cap ... ok
test bounded_fetch_returns_a_response_within_the_bound ... ok
test default_bound_is_sixteen_mebibytes ... ok
test caller_failures_pass_through_with_a_category ... ok
test get_request_carries_purpose_and_bound ... ok
test offline_and_too_large_have_categories ... ok
test failure_logs_carry_no_secrets ... ok
test offline_fetcher_refuses_every_request ... ok
test post_request_carries_body_and_content_type ... ok
test redacted_origin_drops_userinfo_path_query_and_fragment ... ok
test redacted_origin_hides_text_that_is_not_a_url ... ok
test streaming_fetcher_stops_reading_at_the_cap ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/io.rs ($CARGO_TARGET_DIR/debug/deps/unit_io-605cbc5fe649e3c8)
running 13 tests
test memory_sink_finish_twice_is_an_error ... ok
test memory_sink_collects_writes_until_finished ... ok
test memory_sink_rejects_writes_after_finish ... ok
test memory_source_reads_a_range ... ok
test memory_source_rejects_offset_past_the_end ... ok
test memory_source_short_read_at_the_end ... ok
test memory_source_reports_its_size ... ok
test memory_source_returns_zero_at_end_of_data ... ok
test read_exact_at_reports_offset_overflow ... ok
test read_exact_at_with_empty_buffer_reads_nothing ... ok
test read_exact_at_fills_the_buffer_across_short_reads ... ok
test read_exact_at_reports_unexpected_end_of_data ... ok
test storage_errors_propagate_through_read_exact_at ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/isolation.rs ($CARGO_TARGET_DIR/debug/deps/unit_isolation-f6d9f8d195517a46)
running 10 tests
test a_forbidden_dependency_would_fail_the_check ... ok
test async_runtimes_and_network_clients_are_rules ... ok
test every_rule_rejects_a_matching_name ... ok
test manifest_is_not_published_yet ... ok
test package_names_parse_cargo_tree_lines ... ok
test rules_allow_this_crate_and_primitive_crates ... ok
test rules_reject_every_named_crate ... ok
test all_features_runtime_graph_is_isolated ... ok
test no_default_features_graph_has_no_crypto_crate ... ok
test default_runtime_graph_is_isolated ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
     Running tests/unit/oracle_harness.rs ($CARGO_TARGET_DIR/debug/deps/unit_oracle_harness-f117ca3018d301e1)
running 13 tests
test asset_extensions_are_case_insensitive ... ok
test comparison_agrees_on_identical_reports ... ok
test comparison_reports_a_different_verdict ... ok
test comparison_without_engine_report_is_pending ... ok
test comparison_reports_different_failure_codes ... ok
test json_without_report_fields_is_an_error ... ok
test corpus_discovery_finds_assets_and_skips_git ... ok
test malformed_json_and_unknown_state_are_errors ... ok
test corpus_discovery_is_bounded ... ok
test trusted_state_is_recognised ... ok
test legacy_status_without_state_is_invalid_on_failure ... ok
test parses_validation_state_and_failure_codes ... ok
test unexpected_tool_exit_is_an_error_not_a_verdict ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/source_scan.rs ($CARGO_TARGET_DIR/debug/deps/unit_source_scan-91dbf49001e29c79)
running 12 tests
test scanner_allows_harmless_grouped_imports ... ok
test grouped_import_findings_carry_the_line_of_the_group ... ok
test scanner_flags_crypto_crates ... ok
test scanner_flags_direct_filesystem_and_network_use ... ok
test scanner_flags_grouped_std_imports ... ok
test scanner_ignores_block_comments_including_nested_ones ... ok
test scanner_ignores_comment_lines ... ok
test scanner_sees_code_after_a_block_comment_on_the_same_line ... ok
test scanner_sees_code_after_a_url_in_a_string ... ok
test sources_include_the_crypto_backend_module ... ok
test crypto_crates_stay_behind_the_backend ... ok
test core_has_no_direct_io ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests filesquid_c2pa
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Oracle run with output shown (`cargo test -p filesquid-c2pa --locked --test integration_oracle -- --nocapture`, same variables):

```text
     Running tests/integration/oracle.rs ($CARGO_TARGET_DIR/debug/deps/integration_oracle-17f7214a90152126)
running 1 test
c2patool oracle: 0 agree, 115 pending engine reader, 0 disagree; oracle verdicts {"Invalid": 11, "NoManifest": 91, "Valid": 13}
test engine_agrees_with_c2patool_on_the_conformance_files ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.78s
```

Without the variables (as in the pre-commit gate):

```text
SKIPPED c2patool oracle: c2patool not found: set FILESQUID_C2PATOOL or put c2patool on PATH (cargo install c2patool --locked)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
