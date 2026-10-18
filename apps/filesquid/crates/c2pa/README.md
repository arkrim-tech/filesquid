# filesquid-c2pa

The C2PA module of FileSquid. It is a FOSS [C2PA](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html) engine, implemented from scratch in Rust. It reads, validates, mints, edits and embeds C2PA manifests, and produces collection scan reports. Any developer can add C2PA to their own application through its API.

| | |
|---|---|
| Cargo package | `filesquid-c2pa` |
| Rust import | `filesquid_c2pa` |
| Path | `apps/filesquid/crates/c2pa/` (member of the `apps/filesquid/` Cargo workspace) |
| Status | Foundation (CP-0.1: byte I/O, fetcher and crypto-backend traits, isolation tests, `wasm32` build check, `c2patool` oracle harness) and manifest store parsing (CP-0.2: JUMBF, CBOR, claims v1 and v2, assertions, sidecar and remote stores), JPEG and PNG format handlers and hard-binding validation (CP-0.3: data hash, box hash, BMFF hash). Signature, timestamp and trust validation (CP-0.4, CP-0.5), the BMFF format handler (CP-2.2), minting and editing do not exist yet. `publish = false` until the API is versioned (CP-F3.1). |
| Specification | Pinned specification: C2PA Technical Specification 2.4 (`filesquid_c2pa::C2PA_SPEC_VERSION`) |

- C2PA logic (JUMBF, CBOR claims and assertions, bindings, validation, manifest assembly, format handlers) is this module's own code. Cryptographic primitives come from established crates behind a crypto-backend trait; they are never hand-rolled.
- The official `c2pa` crate and `c2patool` are interoperability oracles in the local test gate only, never the implementation or a runtime dependency.
- Trust lists and signers are supplied by the caller. The module ships no implicit trust store and operates no certificate authority.
- No default dependency on `filesquid-ocfl`, any other FileSquid crate, Axum, Tauri, SQLite, libvips or FFmpeg. `filesquid-api` connects it to the OCFL module.
- Builds for `wasm32-unknown-unknown`, so the roadmap E2EE browser host stays reachable.
- Provides the `fsquid` commands `read`, `verify`, `sign`, `edit`, `embed`, `report` (the crate ships no binary; `filesquid-cli` builds `fsquid`). PROV-O JSON-LD projection is an optional feature.

The Content Credentials sidebar panel is part of `@filesquid/ui` in `apps/filesquid/ui/`.

## Public surface

| Module | Items | Purpose |
|---|---|---|
| `io` | `ByteSource`, `ByteSink`, `read_exact_at`, `MemorySource`, `MemorySink`, `ByteIoError` | The caller supplies every byte. The core never calls `std::fs`. |
| `fetch` | `Fetcher`, `FetchRequest`, `FetchPurpose`, `FetchResponse`, `FetchError`, `BoundedBody`, `OfflineFetcher`, `fetch_bounded`, `redacted_origin`, `DEFAULT_MAX_FETCH_BYTES` | Remote manifests, revocation data and timestamps go through the caller's fetcher. A fetcher MUST enforce `max_bytes` while reading (push chunks into `BoundedBody`); `fetch_bounded` checks again and logs failures with the redacted origin and an error category only. `OfflineFetcher` refuses everything. The core has no network client. |
| `crypto` | `CryptoBackend`, `IncrementalHasher`, `HashAlgorithm`, `SigningAlgorithm`, `CryptoError`, `RustCryptoBackend`, `DIGEST_CHUNK_BYTES` | Every hash and signature check goes through the backend trait. `digest_ranges` hashes ranges of a `ByteSource` in 64 KiB chunks without buffering the asset; an async-only backend such as WebCrypto overrides it. |
| `jumbf` | `parse_superbox`, `Superbox`, `DescriptionBox`, `ContentBox`, `JumbfChild`, `BoxRange`, `ByteRange`, `BoxType`, `TypeUuid`, `types`, `toggles`, `JumbfLimits`, `JumbfError`, `MAX_SUPPORTED_DEPTH` | JUMBF superboxes and description boxes (ISO 19566-5, C2PA 2.4 section 11.1) with the byte range of every box. `BoxRange::payload` is the superbox content without its header, which is what a hashed URI covers (section 8.4.2.3). |
| `cbor` | `decode`, `CborValue`, `CborFloat`, `ItemBudget`, `CborError`, `MAX_SUPPORTED_DEPTH` | A CBOR decoder written for this crate (RFC 8949). Accepts well-formed CBOR and rejects invalid UTF-8 and duplicate map keys. |
| `manifest` | `ManifestStore`, `ManifestEntry`, `Manifest`, `ManifestKind`, `ClaimSignature`, `Claim`, `ClaimVersion`, `HashedUri`, `GeneratorInfo`, `Assertion`, `AssertionContent`, `JumbfUri`, `IgnoredBox`, `IgnoreReason`, `ParseLimits`, `StoreError`, `ManifestError`, `ClaimError`, `AssertionError`, `UriError`, `ResolveError` | A parsed C2PA Manifest Store: every manifest in order (the last is active), each with its claim (`c2pa.claim.v2` or the legacy `c2pa.claim`), the claim's CBOR bytes, the raw `COSE_Sign1` claim signature and its assertions (CBOR decoded, JSON checked, embedded files and UUID boxes located). `ManifestStore::resolve` follows `self#jumbf` URIs. |
| `format` | `FormatHandler`, `JpegHandler`, `PngHandler`, `detect_format`, `asset_layout`, `read_store_bytes`, `AssetFormat`, `AssetLayout`, `AssetBox`, `EmbeddedStore`, `FormatError`, `HEAD_BYTES` | Format handlers lay out an asset's boxes (with their C2PA 2.4 box-hash names) and locate its embedded store: JPEG APP11 JUMBF packets (section A.3.1) and PNG `caBX` chunks (section A.3.2). Any other format is `None`: unsupported, never an error. |
| `read` | `read_manifest_store`, `read_asset`, `AssetManifest`, `fetch_remote_manifest_store`, `ReadError` | Read a store from a `ByteSource` (a sidecar `.c2pa` file), from an asset through its format handler (`AssetManifest::Unsupported`, `NotEmbedded`, `MultipleStores` or `Embedded`), or by remote reference through the caller's `Fetcher` (`http` and `https` only). |
| `binding` | `validate_hard_binding`, `BindingReport`, `BindingStatus`, `BindingKind`, `BindingError` | Validate the active manifest's hard binding (or, for an update manifest, its `parentOf` parent's) against the asset's bytes, with the specification's status code. Only assertions the claim references are considered. |

### Manifest store parsing

Every byte is treated as hostile. Errors come in layers. Manifest and assertion content errors do not hide other entries; framing, missing-label, size and engine-limit errors reject the whole store:

- `StoreError`: the store cannot be used at all (over `max_store_bytes`, broken JUMBF framing, not a requestable `c2pa` store, no manifests, two manifests with one label, a C2PA box without a label, or an engine limit).
- `ManifestError`, in `ManifestEntry::content`: one manifest is unusable. `status_code()` gives the C2PA 2.4 failure code: `claim.missing`, `claim.multiple`, `claim.cbor.invalid`, `claim.malformed` (including a signature or assertion reference that is not a `self#jumbf` URI, or a malformed generator `icon`), `claimSignature.missing`, `assertion.missing`, or `general.error` for a manifest box without the requestable toggle. C2PA 2.4 section 11.1.4.1.2 requires the requestable and label toggles on every C2PA box; a claim, signature or assertion store without them reports `claim.malformed`, `claimSignature.missing` or `assertion.missing`. A compressed manifest (`c2cm`) is `CompressedUnsupported` (no code): Brotli decompression is not implemented yet.
- `AssertionError`, in `Assertion::content`: one assertion is unusable (`assertion.cbor.invalid`, `assertion.json.invalid`, `assertion.missing` when the box is not requestable, or malformed embedded-file or UUID content, which has no code). Validation decides whether the claim references it.
- `ReadError`: reading failed. A remote store the fetcher cannot deliver is `manifest.inaccessible`.

Boxes the specification says to skip (unknown type UUIDs, time-stamp manifests `c2tm`, data box stores `c2db`, content boxes where only superboxes belong, C2PA boxes with an unexpected label) are listed in `ManifestStore::ignored`, never dropped silently. Hashes and signatures are not checked here; the parser keeps the byte ranges the validation tasks need. Error messages and logs never carry text from the input as is: labels are escaped and shortened, a fetch error shows only its category, and `tracing` logs record an error category and counts.

| Limit (`ParseLimits`) | Default | Exceeded |
|---|---|---|
| `max_store_bytes` | 64 MiB | `ReadError::TooLarge` before anything is read or allocated; a remote fetch uses it as `max_bytes`; `ManifestStore::from_bytes` returns `StoreError::TooLarge` |
| `max_box_depth` (store superbox is depth 1) | 16, never above 256 | `JumbfError::TooDeep` |
| `max_boxes` (every box header in the store) | 65 536 | `JumbfError::TooManyBoxes` |
| `max_cbor_depth` (a scalar is depth 1) | 64, never above 256 | `StoreError::CborLimit(CborError::TooDeep)` |
| `max_cbor_items` (shared by the CBOR decoded in the store; claim signatures are kept as raw bytes) | 1 048 576 | `StoreError::CborLimit(CborError::TooManyItems)` |
| `max_asset_boxes` (JPEG segments, counting each APP11 packet and each RST marker; PNG chunks; the boxes a BMFF-hash check walks) | 262 144 | `ReadError::Format(FormatError::TooManyBoxes)`; for a BMFF hash, `BindingStatus::NotChecked` |

An embedded store larger than `max_store_bytes` is `ReadError::Format(FormatError::StoreTooLarge)`, checked from the format's headers before the store is read. Its buffer is reserved fallibly, so a failed reservation is `ReadError::Allocation`, as for a sidecar store.

### Format handlers and hard bindings

`read_asset` picks a handler from the first `HEAD_BYTES` bytes. The JPEG handler names segments as ISO 10918-1 Table B.1 does (`SOI`, `APP0`, `DQT`, ...), includes the entropy-coded data after `SOS` and each `RSTn` in that box, skips `0xFF` fill bytes and stray bytes between segments as libjpeg does, and reassembles the C2PA APP11 packets (`JP`, box instance `En`, sequence `Z` from 1, contiguous and in order, each continuation repeating the first packet's box header). Skipped bytes belong to no box: section 18.7.1 hashes only the data within a separately listed box. The store is identified by its type UUID and the label `c2pa` (section 18.7.3.1); a store without the requestable and label toggles, or a first packet cut off inside its description box, is malformed. The PNG handler checks the signature, walks chunks to an empty `IEND`; chunk CRCs are not checked. The store's container (all its APP11 segments, or the `caBX` chunk with length and type) is reported as box `C2PA`. Truncated or malformed structure is a `FormatError` with its offset; two stores are `MultipleStores` (section 15.5.2.1).

`validate_hard_binding` takes the store, the asset as a `ByteSource`, the layout (or `None` for a sidecar or remote store), the `ParseLimits` and the caller's `CryptoBackend`. It considers only the assertions the claim references (`created_assertions` and `gathered_assertions`, or `assertions` in a v1 claim), both for the hard binding and for an update manifest's `parentOf` ingredient; an unreadable update manifest on the way is reported as not checked:

| Binding | Checked | Codes |
|---|---|---|
| `c2pa.hash.data` | Exclusions sorted and not overlapping; inside the asset; the exclusion holding the store holds only the store (section 15.12.1); update-manifest length adjustment; `sha256`, `sha384` or `sha512` from the assertion or the claim | `assertion.dataHash.match`, `.mismatch`, `.malformed`, `.additionalExclusionsPresent` (informational) |
| `c2pa.hash.boxes` | Names in asset order, each asset box listed, `exclusions` with `boxIndex`, `excluded` entries, `C2PA` entry not hashed, `c2pa.after` last | `assertion.boxesHash.match`, `.mismatch`, `.malformed`, `.unknownBox`, `.additionalExclusionsPresent` |
| `c2pa.hash.bmff.v2`, `.v3` | Restricted xpaths with `length`, `version`, `flags` + `exact`, `data` and `subset` conditions; each root box hashed as its 64-bit offset then its included bytes (section 18.6) | `assertion.bmffHash.match`, `.mismatch`, `.malformed`, `.additionalExclusionsPresent` |

All bindings also report `algorithm.unsupported`, `claim.hardBindings.missing` and `assertion.multipleHardBindings`. `c2pa.hash.bmff` (v1) is ignored as section 18.6.1 says. Not checked yet, and reported as `BindingStatus::NotChecked` with the reason rather than as a pass: collection data hashes, BMFF Merkle trees, compressed manifests and the multi-asset fallback after a mismatch.

A CBOR string, array or map that declares more data than remains fails as truncated, and an array or map that declares more items than the item budget has left fails at its head, both before anything is allocated; at most 1 024 slots are reserved up front. Map keys are compared as RFC 8949 section 5.6.1 says (`0.0` equals `-0.0`, map keys ignore pair order). JSON assertions are checked for well-formedness without building a value tree, and `serde_json` skips nested values iteratively, so JSON nesting needs no separate limit.

Trait methods return futures without a `Send` bound so single-threaded `wasm32` hosts can implement them; on native hosts the futures are `Send` whenever the implementation's are.

## Cryptographic primitive crates

Chosen in CP-0.1 ([ADR 026](../../../../resources/architecture/026-filesquid-provenance-engine.md), [ADR 009](../../../../resources/architecture/009-cryptography-webcrypto-compatible-suite.md)). All are pure Rust and build for `wasm32-unknown-unknown`. In the library they are used only in `src/crypto/rust_crypto.rs`, behind the default `rust-crypto` feature; a source-scan test fails if any other library module names a cryptography crate. (The tests also use `p256`, `p384` and `p521` as dev-dependencies, to sign test vectors.) With `--no-default-features` the crate links no cryptography crate and the caller supplies a `CryptoBackend`.

| Primitive | Crate | Status |
|---|---|---|
| SHA-256, SHA-384, SHA-512 | `sha2` 0.11 | Wired |
| ECDSA P-256 / P-384 / P-521 (ES256/384/512) | `p256`, `p384`, `p521` 0.14 (`ecdsa` 0.17) | Wired (verification) |
| Ed25519 | `ed25519-dalek` 3 | Wired (strict verification) |
| RSA-PSS (PS256/384/512) | `rsa` | Chosen; wired in CP-0.4. Until then the default backend returns `CryptoError::UnsupportedAlgorithm`. |
| X.509 certificates | `x509-cert` 0.3 | Chosen; wired in CP-0.4 |
| CMS / RFC 3161 timestamp tokens | `cms` | Chosen; wired in CP-0.4 |

## Build and test

Run Cargo from the workspace root:

```bash
cd apps/filesquid
cargo test -p filesquid-c2pa --locked
cargo build -p filesquid-c2pa --lib --target wasm32-unknown-unknown --no-default-features --locked
```

`cargo test` runs the unit tests (including fuzz tests over truncated and corrupted manifest stores), the dependency-isolation and source-scan tests, the `wasm32-unknown-unknown` build check (default features on and off, built into `filesquid-c2pa-wasm32-check/` under the Cargo target directory), the conformance parse test, the oracle run, the parsing performance test and the Cucumber features. The cucumber JUnit report is written to `tests/allure-results/cucumber-junit.xml`. The `wasm32` check **fails** when the target is missing; install it once with:

```bash
rustup target add wasm32-unknown-unknown
```

To skip the check on purpose, set `FILESQUID_SKIP_WASM32=1`. The integration tests then print `SKIPPED wasm32 build: ...` and the two `@wasm32` Cucumber scenarios are left out of the run with a `SKIPPED @wasm32 scenarios: ...` line; they are never reported as passed.

### Interoperability oracle

The oracle harness runs the official `c2patool` on the C2PA public conformance files and records its verdict and failure codes. It is built to compare them with the engine's and fail on any disagreement, but **no comparison happens yet**: the engine locates and parses stores and checks hard bindings (CP-0.3) but does not yet produce a full validation verdict (CP-0.5), so every file is counted as pending and only `c2patool`'s verdict is recorded. An unexpected `c2patool` exit or output that is not a report fails the test. Without the tool or the corpus the test is skipped and prints why.

From the repository root, in a POSIX shell (Linux, macOS, or Git Bash or WSL on Windows). The corpus is cloned next to the repository, and its path is passed as an absolute path because the test runs with the crate directory as its working directory:

```bash
cargo install c2patool --version 0.28.2 --locked
git clone --depth 1 https://github.com/c2pa-org/public-testfiles ../c2pa-public-testfiles
export FILESQUID_C2PA_CONFORMANCE_DIR="$(cd ../c2pa-public-testfiles && pwd)"
cd apps/filesquid
cargo test -p filesquid-c2pa --locked --test integration_oracle -- --nocapture
```

| Variable | Meaning |
|---|---|
| `FILESQUID_C2PATOOL` | Path to `c2patool`; default: `c2patool` on `PATH` |
| `FILESQUID_C2PA_CONFORMANCE_DIR` | Checkout of `c2pa-org/public-testfiles` (CC BY-SA 4.0; not vendored) |

The same corpus also drives the conformance parse test, which needs no `c2patool`. It reads every JPEG and PNG through `read_asset` (the MP4's store is located by test-only code until the BMFF handler exists), requires every manifest store and every manifest in it to parse completely, validates the selected hard binding of each store (the active manifest's, or its update chain's parent) and compares the result with the `*Hash.match` or `*Hash.mismatch` verdict recorded in the corpus's `detailed.json`. Every store must have a recorded verdict. Three Truepic JPEGs record a match although their data-hash exclusion starts at byte 0 and so leaves the APP1 Exif segment unhashed; C2PA 2.4 section 15.12.1 makes that a mismatch, and the test requires both the recorded match and exactly that result for those files. Without the variable it is skipped and prints why:

```bash
cd apps/filesquid
cargo test -p filesquid-c2pa --locked --test integration_conformance_parse -- --nocapture
```

`cargo install c2patool` compiles a vendored OpenSSL (`openssl-src`), so it needs a C compiler and Perl on `PATH` on every platform (on Windows, for example, the MSVC build tools and Strawberry Perl). It takes several minutes and about 3 GB of build space; the oracle never enters this crate's dependency graph.

## References

- [Product vision: modules](../../../../resources/product-vision.md#modules)
- [PRD F-054: C2PA module](../../../../resources/prd.md#f-054-c2pa-module)
- [ADR 026](../../../../resources/architecture/026-filesquid-provenance-engine.md)
- [Module SRS](./resources/srs.md), [module plan](./resources/plan.md) and [component diagram](./resources/component-diagram.html)
