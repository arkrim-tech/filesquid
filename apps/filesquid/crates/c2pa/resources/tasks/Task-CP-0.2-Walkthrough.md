# Task CP-0.2 Walkthrough: JUMBF and CBOR manifest store parsing

Global task: [Task 0.17](../../../../../../resources/implementation-plan.md) (second slice; CP-0.3 to CP-0.5 follow, so the global box stays open). Module plan: [plan.md](../plan.md). Branch: `feat/cp-0.2-manifest-parsing`. Builds on CP-0.1 ([walkthrough](./Task-CP-0.1-Walkthrough.md)).

## C2PA-S-001: The crate MUST read manifest stores embedded in assets, in sidecar `.c2pa` files and by remote reference, parsing JUMBF boxes and CBOR claims and assertions

Full requirement text ([module SRS](../srs.md)): "The crate MUST read manifest stores embedded in assets, in sidecar `.c2pa` files and by remote reference, parsing JUMBF boxes and CBOR claims and assertions." This task delivers the parsing, sidecar reading (any `ByteSource`) and remote references (through the caller's `Fetcher`). Locating a store **embedded in an asset** needs the JPEG and PNG format handlers of CP-0.3, so the scenario "Read a manifest store" (a JPEG) stays skipped until then. The conformance parse test below proves the parser on stores that real tools embedded in JPEGs, using test-only APP11 extraction.

## C2PA-S-008: The core MUST read and write through caller-supplied async byte sources and sinks and MUST NOT call `std::fs` or a network client directly

Full requirement text: "Default dependencies MUST exclude `filesquid-ocfl`, `filesquid-core` and every other FileSquid crate, Axum, Tauri, SQLite, libvips, FFmpeg and distribution code. The core MUST read and write through caller-supplied async byte sources and sinks and MUST NOT call `std::fs` or a network client directly." The new modules read only through `ByteSource` and `Fetcher`; the CP-0.1 source-scan and isolation tests cover them unchanged.

## C2PA-S-009: The crate MUST build for `wasm32-unknown-unknown` and MUST NOT require a server

Full requirement text: "The crate MUST build for `wasm32-unknown-unknown` and MUST NOT require a server. ..." The new dependencies (`serde`, `serde_json`) build for `wasm32-unknown-unknown`; both wasm builds ran in this task's test runs.

Also covered: [SRS-F-054](../../../../../../resources/srs.md#srs-f-054-c2pa-module) ("Remote fetches MUST go through a caller-supplied fetcher"), [PRD F-054](../../../../../../resources/prd.md#f-054-c2pa-module), [ADR 026](../../../../../../resources/architecture/026-filesquid-provenance-engine.md) (JUMBF and CBOR claim structures are this crate's own code).

## Specification checks (C2PA 2.4)

Verified against the published HTML of the [C2PA 2.4 specification](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html):

- 11.1.4.2: the store is labelled `c2pa` with UUID `63327061-0011-0010-8000-00AA00389B71`; one or more manifests; the last is the active manifest; other boxes MAY be present.
- 11.2: manifests are `c2ma` (standard), `c2md` (standard, "Manifest Consumers shall also accept"), `c2um` (update), `c2cm` (compressed, Brotli `brob`); `c2tm` time-stamp manifests are "not to be written by claim generators nor read by manifest consumers".
- 11.1.4.1.1: labels are NUL-terminated UTF-8 without U+0000 to U+001F, U+007F to U+009F, `/`, `;`, `?`, `#`, U+FEFF and U+FFFF.
- 11.1.4.3 and 11.1.4.4: assertion store `c2pa.assertions`; claim `c2pa.claim.v2` (or the deprecated `c2pa.claim`, same UUID `c2cl`), "a single CBOR Content Type box"; claim signature `c2pa.signature` (`c2cs`), a single CBOR box holding `COSE_Sign1_Tagged`.
- 11.1.2: a box with an unrecognised type UUID is skipped and ignored.
- 10.2.1 and 15.6.2: `claim-map-v2` requires `instanceID`, `claim_generator_info` (with `name`), `signature` and `created_assertions`; failure codes `claim.missing`, `claim.multiple`, `claim.cbor.invalid`, `claim.malformed`; 15.7 `claimSignature.missing`; 15.10 `assertion.missing`, `assertion.cbor.invalid`, `assertion.json.invalid`; 15.5.3 `manifest.inaccessible`.
- 8.4.2.1 and 8.4.2.3: `self#jumbf` URIs are absolute (`/` first) or relative to the manifest and never contain `..`; a referenced superbox is hashed without its own header.
- 11.4 and 15.5.3.2: remote stores are served as `application/c2pa` from `http` or `https` URIs; a `#jumbf=` fragment points into an asset.

## What changed

- **`src/cbor.rs`:** a bounded RFC 8949 decoder written for this crate (ADR 026: JUMBF and CBOR structures are the crate's own code). Integers, byte and text strings (definite and indefinite), arrays, maps, tags, simple values, half, single and double floats. Rejects reserved additional information, stray breaks, bad indefinite chunks, invalid UTF-8, duplicate map keys (compared on decoded values, so `1` and `0x1801` collide) and trailing bytes. A declared length longer than the remaining input fails before anything is allocated. Depth is capped by the caller and never above `MAX_SUPPORTED_DEPTH` (256, a stack bound); every item, including each string chunk, is charged to a shared `ItemBudget`. Deterministic encoding is not required (C2PA 10.1 asks generators for it; readers need well-formed CBOR).
- **`src/jumbf.rs`:** box headers (`LBox` 0 runs to the end of the container, 1 means `XLBox`, 2 to 7 and `XLBox < 16` are invalid), description boxes (type UUID, toggles, label, ID, signature, private box; reserved toggle bits and trailing bytes rejected), the superbox tree with the byte range of every box, the C2PA and ISO type UUIDs, and limits on superbox depth (never above 256) and box count.
- **`src/manifest/`:** `ManifestStore` (store, manifests in order, the last active, skipped boxes), `ManifestEntry` with a per-manifest `Result<Manifest, ManifestError>`, `Claim` for v1 and v2 (`Claim::from_cbor`), `ClaimSignature` (raw `COSE_Sign1_Tagged` range; verification is CP-0.4), `Assertion` with a per-assertion content result (CBOR decoded, JSON checked for well-formedness and kept as text, embedded files, UUID boxes, other content types), `JumbfUri` parsing and `ManifestStore::resolve`, `ParseLimits`. `ManifestError::status_code`, `AssertionError::status_code` and `ReadError::status_code` return the specification's failure codes. Unknown, historical (`c2tm`, `c2db`), misplaced content boxes and C2PA boxes with an unexpected label are listed in `ManifestStore::ignored`, never dropped silently. Compressed manifests (`c2cm`) are reported as `ManifestError::CompressedUnsupported`.
- **`src/read.rs`:** `read_manifest_store` reads a whole store from a `ByteSource` after checking its size against `max_store_bytes` (nothing is read or allocated when it is over, and memory is reserved with `try_reserve_exact`). `fetch_remote_manifest_store` accepts only `http` and `https` URLs, refuses `#jumbf=` references into assets, and fetches through `fetch_bounded` with `FetchPurpose::RemoteManifest` and `max_bytes = max_store_bytes`. Outcomes are logged with `tracing` (category and counts only).
- **Dependencies:** `serde` and `serde_json` (already in `Cargo.lock`, so the lock file only records the new edges), for JSON well-formedness through `IgnoredAny`. No new crate entered the graph; no cryptography is used in this task.
- **Tests:** `unit_cbor`, `unit_jumbf`, `unit_manifest`, `unit_claim`, `unit_uri`, `unit_read`, `unit_read_logs` (its own binary, because `tracing` caches call-site interest and another thread hitting the same call site without a subscriber could hide the event), `integration_conformance_parse`, `performance_parse`; fixtures in `tests/support/fixtures.rs` (test-only CBOR encoder and JUMBF builder) and `tests/support/jpeg_app11.rs` (test-only APP11 reassembly).
- **BDD:** nine C2PA-S-001 scenarios with step definitions.
- **Docs:** crate README (status, public surface, parsing errors, limits, conformance parse test), component diagram (JUMBF parser and manifest store built; the crypto and byte I/O boxes, built by CP-0.1, now drawn as built), module plan box.

## Engine limits

| `ParseLimits` field | Default | Boundary tests (limit passes, limit minus one fails) |
|---|---|---|
| `max_store_bytes` | 64 MiB | `unit_read::store_size_limit_holds_at_the_boundary_before_reading` (no read when over), `oversized_remote_stores_are_rejected_at_the_boundary`, `a_huge_reported_size_is_rejected_without_allocating`, `a_size_that_cannot_be_allocated_is_reported_without_reading` |
| `max_box_depth` | 16 (ceiling 256) | `unit_jumbf::superbox_depth_limit_holds_at_the_boundary`, `a_caller_depth_above_the_supported_ceiling_is_lowered_to_it`, `hostile_nesting_does_not_overflow_the_stack`; `unit_manifest::box_depth_limit_holds_at_the_boundary` |
| `max_boxes` | 65 536 | `unit_jumbf::box_count_limit_holds_at_the_boundary` (count from an independent walker), `private_boxes_count_toward_the_box_limit`; `performance_parse` (65 416 boxes) |
| `max_cbor_depth` | 64 (ceiling 256) | `unit_cbor::nesting_depth_limit_holds_at_the_boundary`, `a_caller_depth_above_the_supported_ceiling_is_lowered_to_it`, `deep_nesting_from_untrusted_input_does_not_overflow_the_stack`; `unit_manifest::cbor_depth_limit_rejects_the_whole_store` |
| `max_cbor_items` | 1 048 576 per store | `unit_cbor::item_budget_holds_at_the_boundary_and_is_shared`; `unit_manifest::cbor_item_budget_is_shared_across_the_store_and_holds_at_the_boundary` (count from an independent counter) |
| CBOR declared lengths | remaining input | `unit_cbor::declared_lengths_longer_than_the_input_fail_without_allocating` |
| JSON nesting | none needed: validation is iterative and builds no tree | `unit_manifest::deeply_nested_json_is_checked_without_recursion` (200 000 levels) |

Malformed and truncated input: every prefix and every single-byte corruption of a two-manifest store (`unit_jumbf`, `unit_manifest`), 4 000 random multi-byte corruptions and truncations of v1 and v2 stores (`unit_manifest::random_multi_byte_corruption_is_handled_without_panicking`) and 50 000 random CBOR inputs (`unit_cbor::random_input_is_decoded_or_rejected_without_panicking`) are rejected or parsed without a panic.

## SRS traceability

| Requirement | Tests |
|---|---|
| C2PA-S-001 JUMBF superboxes and description boxes | `unit_jumbf` (25 tests: header lengths, `XLBox`, `LBox` 0, overruns, description fields and toggles, reserved bits, labels and forbidden characters, trailing bytes, depth and box limits, prefixes and corruptions, display escaping) |
| C2PA-S-001 CBOR claims and assertions | `unit_cbor` (24 tests: RFC 8949 Appendix A vectors, indefinite lengths, floats, well-formedness errors, duplicate keys, UTF-8, truncation, limits, random input) |
| C2PA-S-001 manifest store, manifests, signature, assertion store, status codes | `unit_manifest` (34 tests); features "Read a sidecar manifest store", "A manifest with a broken claim is reported with its status code", "Unknown boxes are reported, not silently dropped", "A truncated manifest store is rejected", "Nesting deeper than the limit is rejected" |
| C2PA-S-001 claims v1 and v2 | `unit_claim` (10 tests: every required field of each version, CDDL types and sizes, hashed URIs, generator info, unknown fields kept); feature "Read a legacy v1 claim" |
| C2PA-S-001 `self#jumbf` URIs | `unit_uri` (7 tests: absolute and relative, `..`, empty segments, segment cap, resolution, ambiguity) |
| C2PA-S-001 sidecar `.c2pa` files | `unit_read::a_sidecar_store_is_read_from_a_byte_source` and the size, allocation and I/O tests; feature "A manifest store over the size limit is rejected before it is read" |
| C2PA-S-001 remote reference; SRS-F-054 caller-supplied fetcher | `unit_read` remote tests (purpose, `max_bytes`, `http`/`https` only, `#jumbf=` refused before fetching, `manifest.inaccessible`, messages without the URL); features "Read a remote manifest store through the caller fetcher", "A remote manifest store without a caller fetcher is inaccessible" |
| C2PA-S-001 real-world stores | `integration_conformance_parse`: 24 stores and 40 manifests (all v1 claims) from the C2PA public test files parse with no error |
| C2PA-S-008 bytes-only core | `unit_source_scan`, `unit_isolation` (unchanged, now covering `cbor`, `jumbf`, `manifest`, `read`); `unit_read_logs::rejection_logs_carry_categories_only` |
| C2PA-S-009 `wasm32` build | `integration_wasm_build` (both builds ran) |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. Every public function of `cbor`, `jumbf`, `manifest` and `read` is called by the tests above, and every error variant is produced by at least one test (`ReadError::Allocation` by `a_size_that_cannot_be_allocated_is_reported_without_reading`). This is a manual inventory, not a measured figure. The cucumber JUnit report is `tests/allure-results/cucumber-junit.xml`.

## Known gaps

- **Compressed manifests (`c2cm`):** reported as `ManifestError::CompressedUnsupported`, with no status code; Brotli decompression needs a decompressor crate and a size cap, and no plan task names it yet.
- **Embedded stores:** locating a store inside JPEG or PNG is CP-0.3; the oracle comparison stays pending until CP-0.5.
- **Requestable toggle:** C2PA 11.1.4.1.2 says C2PA description boxes set the requestable and label toggles. The parser requires labels on C2PA boxes and records the toggles, but does not reject a box without the requestable bit.

## Mocked Environment Variables

None. `FILESQUID_C2PA_CONFORMANCE_DIR` pointed at a shallow clone of `https://github.com/c2pa-org/public-testfiles` (CC BY-SA 4.0, not vendored) for the conformance parse run below. It is not a secret and is not stored anywhere. `c2patool` was not installed, so the oracle test printed its skip reason.

## TDD: failing run before implementation

`cargo build -p filesquid-c2pa --tests --keep-going` with the tests written and no `cbor`, `jumbf`, `manifest` or `read` module (counts of each error line):

```text
      1 error: could not compile `filesquid-c2pa` (test "features") due to 4 previous errors
      1 error: could not compile `filesquid-c2pa` (test "integration_conformance_parse") due to 1 previous error
      1 error: could not compile `filesquid-c2pa` (test "performance_parse") due to 2 previous errors
      1 error: could not compile `filesquid-c2pa` (test "unit_cbor") due to 2 previous errors
      1 error: could not compile `filesquid-c2pa` (test "unit_claim") due to 3 previous errors
      1 error: could not compile `filesquid-c2pa` (test "unit_jumbf") due to 3 previous errors
      1 error: could not compile `filesquid-c2pa` (test "unit_manifest") due to 4 previous errors
      1 error: could not compile `filesquid-c2pa` (test "unit_read") due to 3 previous errors
      1 error: could not compile `filesquid-c2pa` (test "unit_uri") due to 2 previous errors
      6 error[E0432]: unresolved import `filesquid_c2pa::cbor`
      3 error[E0432]: unresolved import `filesquid_c2pa::jumbf`
      7 error[E0432]: unresolved import `filesquid_c2pa::manifest`
      2 error[E0432]: unresolved import `filesquid_c2pa::read`
      1 error[E0433]: cannot find `jumbf` in `filesquid_c2pa`
```

After the implementation the first full run failed in one test, `json_nesting_limit_holds_at_the_boundary`, which assumed `serde_json` stops at 128 levels of nesting. It does not on the `IgnoredAny` path: nested values are skipped iteratively. The doc claim was removed and the test now proves what holds (200 000 levels are checked without recursion; an unbalanced input is rejected). The log-capture test then failed intermittently while it shared a test binary with other tests that hit the same `tracing` call site; it moved to its own binary (`unit_read_logs`) and passed in 10 repeated runs.

## Gates

Run from `apps/filesquid/` with `CARGO_TARGET_DIR` set to the shared target directory, `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=0`:

- `cargo fmt --all -- --check`: clean
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean (one `type_complexity` finding in test support fixed with a struct)
- `cargo clippy -p filesquid-c2pa --no-default-features --all-targets --locked -- -D warnings`: clean
- `cargo test -p filesquid-c2pa --locked`: passed (output below)
- `cargo test -p filesquid-c2pa --no-default-features --locked`: passed
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`: clean
- `bash scripts/pre-commit.sh` from the repository root (with `TMPDIR` on a tmpfs, because the `filesquid-api` health scenario needs 5 GiB free on the temporary volume): passed

## Raw test output

`FILESQUID_C2PA_CONFORMANCE_DIR=... cargo test -p filesquid-c2pa --locked` (target directory shown as `$CARGO_TARGET_DIR`, blank lines removed). Test binaries and results:

```text
     Running unittests src/lib.rs ($CARGO_TARGET_DIR/debug/deps/filesquid_c2pa-d65bcc9dd41296d9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-8655cb2d4004b238)
     Running tests/integration/conformance_parse.rs ($CARGO_TARGET_DIR/debug/deps/integration_conformance_parse-090ffb77eeb9717c)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/oracle.rs ($CARGO_TARGET_DIR/debug/deps/integration_oracle-94666cdf1f8605b4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/wasm_build.rs ($CARGO_TARGET_DIR/debug/deps/integration_wasm_build-35bc3b66d2b65aee)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
     Running tests/performance/parse.rs ($CARGO_TARGET_DIR/debug/deps/performance_parse-f035af314bbfc222)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/cbor.rs ($CARGO_TARGET_DIR/debug/deps/unit_cbor-c7040e8e8f10e2a2)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/claim.rs ($CARGO_TARGET_DIR/debug/deps/unit_claim-f9b9284008e60bf6)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/crypto.rs ($CARGO_TARGET_DIR/debug/deps/unit_crypto-432f2a37df2000a3)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
     Running tests/unit/fetch.rs ($CARGO_TARGET_DIR/debug/deps/unit_fetch-738758fb7279fcdb)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/io.rs ($CARGO_TARGET_DIR/debug/deps/unit_io-c003cdb6bb92b265)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/isolation.rs ($CARGO_TARGET_DIR/debug/deps/unit_isolation-fe596040567ca446)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
     Running tests/unit/jumbf.rs ($CARGO_TARGET_DIR/debug/deps/unit_jumbf-f69096fce7fab3ff)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/unit/manifest.rs ($CARGO_TARGET_DIR/debug/deps/unit_manifest-e0b825c7c5ed2037)
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
     Running tests/unit/oracle_harness.rs ($CARGO_TARGET_DIR/debug/deps/unit_oracle_harness-6d4b880861972b3f)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/read.rs ($CARGO_TARGET_DIR/debug/deps/unit_read-d590eb7c9591cd5b)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/read_logs.rs ($CARGO_TARGET_DIR/debug/deps/unit_read_logs-b14b662b7f43348d)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs ($CARGO_TARGET_DIR/debug/deps/unit_source_scan-49491e813dee674f)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/uri.rs ($CARGO_TARGET_DIR/debug/deps/unit_uri-bb9093e7b09a2280)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The Cucumber run from the same command:

```text
     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-8655cb2d4004b238)
Feature: Reusable filesquid-c2pa engine
  Scenario: Read a manifest store
   ?  Given a JPEG with an embedded C2PA manifest store
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:9:5
  Scenario: Read a sidecar manifest store
   ✔  Given a sidecar manifest store with an ingredient manifest and an active manifest
SKIPPED c2patool oracle: c2patool not found: set FILESQUID_C2PATOOL or put c2patool on PATH (cargo install c2patool --locked)
   ✔  When I read the sidecar with filesquid-c2pa
   ✔  Then the active manifest should be the last manifest in the store
   ✔  And its v2 claim and its CBOR, JSON and embedded-file assertions should be returned
  Scenario: Read a legacy v1 claim
   ✔  Given a sidecar manifest store whose claim uses the legacy c2pa.claim label
   ✔  When I read the sidecar with filesquid-c2pa
   ✔  Then the claim should be reported as version 1 with its assertions
  Scenario: Read a remote manifest store through the caller fetcher
   ✔  Given a caller fetcher that serves a manifest store
   ✔  When I read the remote manifest store "https://example.org/asset.c2pa"
   ✔  Then the active manifest should be the last manifest in the store
   ✔  And the fetcher should have been asked for a remote manifest
  Scenario: A remote manifest store without a caller fetcher is inaccessible
   ✔  Given no network fetcher supplied by the caller
   ✔  When I read the remote manifest store "https://example.org/asset.c2pa"
   ✔  Then the read should fail with status code "manifest.inaccessible"
  Scenario: A truncated manifest store is rejected
   ✔  Given a sidecar manifest store cut short by one byte
   ✔  When I read the sidecar with filesquid-c2pa
   ✔  Then the read should fail with a truncated JUMBF box error
  Scenario: A manifest store over the size limit is rejected before it is read
   ✔  Given a sidecar manifest store one byte larger than the store size limit
   ✔  When I read the sidecar with filesquid-c2pa
   ✔  Then the read should fail as too large without reading the source
  Scenario: Nesting deeper than the limit is rejected
   ✔  Given a sidecar manifest store nested one level deeper than the box depth limit
   ✔  When I read the sidecar with filesquid-c2pa
   ✔  Then the read should fail with a box depth limit error
  Scenario: A manifest with a broken claim is reported with its status code
   ✔  Given a sidecar manifest store whose ingredient claim is not valid CBOR
   ✔  When I read the sidecar with filesquid-c2pa
   ✔  Then the ingredient manifest should report "claim.cbor.invalid"
   ✔  And the active manifest should still be readable
  Scenario: Unknown boxes are reported, not silently dropped
   ✔  Given a sidecar manifest store with a vendor box of an unknown type
   ✔  When I read the sidecar with filesquid-c2pa
   ✔  Then the vendor box should be listed as ignored with an unknown type
  Scenario: Validate against a caller-supplied trust list
   ?  Given a signed asset and a trust list supplied by the caller
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:63:5
  Scenario: Mint and sign through a caller-supplied signer
   ?  Given an unsigned PNG and a test signer
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:69:5
  Scenario: Edits never change a signed manifest in place
   ?  Given a signed asset
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:75:5
  Scenario: Unsupported formats are reported
   ?  Given an asset in a format without a handler
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:80:5
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
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:152:5
  Scenario Outline: Content Credentials panel shows each credential state distinctly
   ?  Given a selected unsigned file in a host file browser
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:157:5
  Scenario Outline: Content Credentials panel shows each credential state distinctly
   ?  Given a selected signed file in a host file browser
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:157:5
  Scenario Outline: Content Credentials panel shows each credential state distinctly
   ?  Given a selected invalid file in a host file browser
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:157:5
  Scenario Outline: Content Credentials panel shows each credential state distinctly
   ?  Given a selected unavailable file in a host file browser
      Step skipped: tests/features/reusable_provenance_engine_F-054.feature:157:5
[Summary]
1 feature
32 scenarios (22 passed, 10 skipped)
74 steps (64 passed, 10 skipped)
```

`FILESQUID_C2PA_CONFORMANCE_DIR=... cargo test -p filesquid-c2pa --locked --test integration_conformance_parse --test performance_parse -- --nocapture`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.23s
     Running tests/integration/conformance_parse.rs ($CARGO_TARGET_DIR/debug/deps/integration_conformance_parse-090ffb77eeb9717c)
running 1 test
conformance parse: 24 stores, 91 files without a store, 40 manifests parsed, 0 manifest errors, 0 store errors
test every_conformance_manifest_store_parses ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/performance/parse.rs ($CARGO_TARGET_DIR/debug/deps/performance_parse-f035af314bbfc222)
running 1 test
parsed 21800 assertions and an 8 MiB thumbnail in 82.278618ms
test a_store_near_the_box_limit_parses_within_the_budget ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
```

Without the variable, the documented command prints its skip reason:

```text
SKIPPED conformance parse: FILESQUID_C2PA_CONFORMANCE_DIR is not set
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
