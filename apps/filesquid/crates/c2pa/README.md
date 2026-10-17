# filesquid-c2pa

The C2PA module of FileSquid. It is a FOSS [C2PA](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html) engine, implemented from scratch in Rust. It reads, validates, mints, edits and embeds C2PA manifests, and produces collection scan reports. Any developer can add C2PA to their own application through its API.

| | |
|---|---|
| Cargo package | `filesquid-c2pa` |
| Rust import | `filesquid_c2pa` |
| Path | `apps/filesquid/crates/c2pa/` (member of the `apps/filesquid/` Cargo workspace) |
| Status | Foundation only (CP-0.1): byte I/O, fetcher and crypto-backend traits, isolation tests, `wasm32` build check and `c2patool` oracle harness. No C2PA reading, validation, minting or editing exists yet; manifest reading starts in CP-0.2. `publish = false` until the API is versioned (CP-F3.1). |
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

`cargo test` runs the unit tests, the dependency-isolation and source-scan tests, the `wasm32-unknown-unknown` build check (default features on and off, built into `filesquid-c2pa-wasm32-check/` under the Cargo target directory), the oracle run and the Cucumber features. The cucumber JUnit report is written to `tests/allure-results/cucumber-junit.xml`. The `wasm32` check **fails** when the target is missing; install it once with:

```bash
rustup target add wasm32-unknown-unknown
```

To skip the check on purpose, set `FILESQUID_SKIP_WASM32=1`. The integration tests then print `SKIPPED wasm32 build: ...` and the two `@wasm32` Cucumber scenarios are left out of the run with a `SKIPPED @wasm32 scenarios: ...` line; they are never reported as passed.

### Interoperability oracle

The oracle harness runs the official `c2patool` on the C2PA public conformance files and records its verdict and failure codes. It is built to compare them with the engine's and fail on any disagreement, but **no comparison happens yet**: the engine cannot read manifests until CP-0.2 to CP-0.5, so every file is counted as pending and only `c2patool`'s verdict is recorded. An unexpected `c2patool` exit or output that is not a report fails the test. Without the tool or the corpus the test is skipped and prints why.

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

`cargo install c2patool` compiles a vendored OpenSSL (`openssl-src`), so it needs a C compiler and Perl on `PATH` on every platform (on Windows, for example, the MSVC build tools and Strawberry Perl). It takes several minutes and about 3 GB of build space; the oracle never enters this crate's dependency graph.

## References

- [Product vision: modules](../../../../resources/product-vision.md#modules)
- [PRD F-054: C2PA module](../../../../resources/prd.md#f-054-c2pa-module)
- [ADR 026](../../../../resources/architecture/026-filesquid-provenance-engine.md)
- [Module SRS](./resources/srs.md), [module plan](./resources/plan.md) and [component diagram](./resources/component-diagram.html)
