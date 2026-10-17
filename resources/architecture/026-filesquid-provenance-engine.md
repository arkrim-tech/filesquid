# Architecture Decision Record - FileSquid

026-filesquid-provenance-engine.md

## Title

filesquid-c2pa is a stand-alone FOSS C2PA engine, implemented from scratch; FileSquid is one caller

## Status

Accepted

Accepted: 2026-10-15T05:24:52Z. Owner directed a from-scratch Rust implementation of the C2PA specification that any developer can integrate without ImageSquid, in the same product family as `filesquid-ocfl` and `filesquid-metadata`. Supersedes the "official `c2pa` crate" choice in [ADR 006](./006-libvips-native-stills-not-imagemagick.md), [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md) and [ADR 015](./015-prov-o-jsonld-and-c2pa-sdk.md). Runs as a client engine under [ADR 025](./025-client-side-filesquid-engines.md). Revised: 2026-10-15T06:14:23Z. Amended by [ADR 028](./028-filesquid-ui-packages-and-panel-composition.md): the engine ships the `@filesquid/provenance-ui` Content Credentials sidebar panel. Revised: 2026-10-15T08:10:43Z. Owner renamed the engine from `filesquid-c2pa` to `filesquid-c2pa` (package, import, path, UI package and this ADR's filename) before any code exists, so the name does not tie the product to one standard. C2PA remains the engine's scope; the decision is otherwise unchanged.

Superseded in part 2026-10-15 by [ADR 029](./029-filesquid-unified-product-and-distributions.md): the module is renamed `filesquid-c2pa` (`apps/filesquid/crates/c2pa/`) and is a module of FileSquid in this repository, not a separate product with its own repository; the technical content is unchanged and this filename is kept as history. Amended by [ADR 032](./032-hosting-and-privacy-model.md): it MAY run in `filesquid-server`, never on the ciphertext-only `*.filesquid.tech` service.

Amended 2026-10-17 by [ADR 036](./036-single-fsquid-cli.md): every FileSquid CLI is the one `fsquid` binary built by `filesquid-cli`; this module's commands run there and the crate ships no binary of its own.

## Context / Requirement Reference

FileSquid creates import-time C2PA records, discloses actions, validates credentials, runs free collection scans, and later submits approved outputs for third-party signing ([F-043](../prd.md#f-043-import-time-c2pa-records-and-action-disclosure), [ADR 015](./015-prov-o-jsonld-and-c2pa-sdk.md)). The inspector mockups show a C2PA Track, cryptographic credentials, Merkle audit and a PROV-O graph on every file.

ADRs 006, 011 and 015 chose the official Rust `c2pa` crate inside desktop native processing. [ADR 025](./025-client-side-filesquid-engines.md) moved preservation and metadata engines to every client, which left C2PA as the one file-level engine still tied to the desktop. [ADR 024](./024-filesquid-metadata-engine.md) keeps C2PA out of `filesquid-metadata`. `imagesquid_core` already carries C2PA and PROV-O record types (`C2paImportRecord`, `ProvOSidecar`; task CO-0.8) but no C2PA logic.

The owner wants a C2PA engine that, like `filesquid-ocfl` for OCFL, is a complete stand-alone tool: a developer implementing the C2PA specification in their own application integrates with the `filesquid-c2pa` API and never installs FileSquid.

## Decision

FileSquid **authors `filesquid-c2pa` as a stand-alone FOSS C2PA engine** (library and CLI), implemented from scratch in Rust. FileSquid is one caller.

| | Value |
|---|---|
| Cargo package name | `filesquid-c2pa` |
| Rust import | `filesquid_c2pa` |
| MVP workspace path | `apps/filesquid/crates/c2pa/` |
| Specification | C2PA Technical Specification (2.4 at writing); the exact version is pinned at first implementation and recorded in the crate README |

**1. Engine scope (all C2PA processing).**

- **Read:** locate and parse manifest stores (JUMBF boxes, CBOR claims and assertions) embedded in assets, in sidecar `.c2pa` files, or referenced remotely.
- **Validate:** hard bindings (data hash, BMFF hash, box hash), claim signatures (COSE_Sign1), certificate chains against **caller-supplied** trust lists, RFC 3161 timestamps, revocation where available, ingredient chains. Results use the specification's validation status codes.
- **Mint:** build new claims and manifests (actions, ingredients, hard bindings, AI-generation and training/mining assertions, other standard and custom assertions) and sign them through a `Signer` interface. Signers can be local keys, hardware modules or remote/third-party services supplied by the caller.
- **Edit:** C2PA manifests are immutable, so edits create new manifests that reference the previous asset as a parent ingredient, including update manifests and assertion redaction as the specification allows.
- **Embed and export:** write manifests into supported asset formats or as sidecars, through a format-handler interface. JPEG and PNG first; HEIF/AVIF, BMFF (MP4/MOV), WebP and TIFF/DNG follow.
- **Report:** bounded collection scans and disclosure-safe reports (`scanCollection`, `exportScanReport` in [architecture.md](../architecture.md)).
- **PROV-O projection (optional feature):** map C2PA actions and ingredients to versioned PROV-O JSON-LD ([ADR 015](./015-prov-o-jsonld-and-c2pa-sdk.md)). Default off, so the core stays pure C2PA.

**2. From scratch means the C2PA logic, not the cryptography.** JUMBF, CBOR claim structures, assertions, bindings, manifest assembly, validation rules and format handlers are this crate's own code. Cryptographic primitives (hashes, signature algorithms, X.509 and ASN.1 parsing, timestamp tokens) MUST come from established, maintained crates behind a crypto-backend trait. The crate MUST NOT hand-roll cryptographic primitives. The backend trait lets the browser build use pure-Rust or WebCrypto-backed implementations.

**3. Interoperability oracle, not implementation.** The official `c2pa` crate, `c2patool` and the C2PA conformance test files MUST validate this engine's output and verdicts in CI (read each other's manifests; same validation results). They MUST NOT be the implementation or a runtime dependency. Seeking listing under the C2PA conformance program is a later distribution decision.

**4. Stand-alone and bytes-only.** Default `[dependencies]` MUST exclude Axum, Tauri, SQLite, libvips, FFmpeg, FileSquid domain crates, **and `filesquid-ocfl`**. The engine reads and writes through async byte sources and sinks supplied by the caller; it MUST NOT call `std::fs` or a network client in the core. Remote manifest and revocation fetches go through a caller-supplied fetcher. The CLI (`read`, `verify`, `sign`, `edit`, `embed`, `report`) is native-only.

**5. Client engine under ADR 025.** The crate MUST build for `wasm32-unknown-unknown`. FileSquid runs it in the Tauri desktop and in browser workers through `filesquid-api` (`apps/filesquid/crates/api/`; was `apps/client`), which reads assets and stores manifests and sidecars through `filesquid-ocfl`. The ciphertext-only `*.filesquid.tech` service MUST NOT depend on it; the owner-operated `filesquid-server` MAY ([ADR 032](./032-hosting-and-privacy-model.md)).

**6. Trust and signing stay with the caller.** The engine verifies against trust lists the caller supplies and signs with signers the caller supplies. FileSquid still does not operate a certificate authority; paid publication signing stays a third-party workflow ([ADR 015](./015-prov-o-jsonld-and-c2pa-sdk.md)).

**7. Portable types.** `filesquid-core` keeps its C2PA and PROV-O record types as portable DTOs mapped from engine results, without a second vocabulary (the same pattern as statement DTOs in [ADR 024](./024-filesquid-metadata-engine.md)).

**8. Distribution.** The crate is a FileSquid workspace module at `apps/filesquid/crates/c2pa/` ([ADR 029](./029-filesquid-unified-product-and-distributions.md)). It MUST build, test and publish on its own; during MVP it MAY carry `publish = false`. After the API is versioned, crates.io publication SHOULD follow.

## Rationale

C2PA is a file-level standard that applications beyond FileSquid need, just as OCFL is. An engine that FileSquid owns end to end can run identically on desktop and in the browser, follow FileSquid's bytes-only and client-only rules, and expose a clean developer API. Using battle-tested crypto crates keeps the security-critical primitives out of new code, while the C2PA-specific structures and rules, which are where integration pain sits, become a reusable product. Oracle testing against the official implementation keeps the engine interoperable without depending on it.

## Assumptions

- The C2PA specification is public and stable enough to implement independently; spec revisions are tracked by pinned versions.
- Maintained Rust crates exist for the required primitives (SHA-2, ECDSA/Ed25519/RSA-PSS, X.509, CMS/RFC 3161) and build for `wasm32`. Exact crates are chosen in the first implementation task and recorded in the crate README.
- The official `c2pa` crate and `c2patool` remain available as interoperability oracles in CI.
- Caller-supplied trust lists (for example the C2PA trust list) are the trust anchors; the engine ships no implicit trust store.

## Alternatives Considered

- **Keep the official `c2pa` crate as the engine (previous ADR 011/015 position):** Fastest path, but leaves C2PA as an external dependency outside the filesquid product family, with its own I/O, settings and platform model. Rejected.
- **Wrap the official crate behind a filesquid-c2pa API:** A façade without independent capability; developers would still inherit the wrapped crate's constraints. Rejected.
- **Implement cryptographic primitives from scratch too:** High security risk for no product gain. Rejected.
- **Depend on `filesquid-ocfl` inside the engine:** Forces OCFL on developers who only want C2PA. Rejected; FileSquid wires the two together in `filesquid-api` (`apps/filesquid/crates/api/`).
- **Put C2PA in `filesquid-metadata`:** Mixes descriptive metadata with cryptographic provenance and blocks C2PA-only adoption. Rejected ([ADR 024](./024-filesquid-metadata-engine.md)).
- **Run C2PA validation on the server:** The server would need plaintext assets. Rejected ([ADR 025](./025-client-side-filesquid-engines.md)).

## Consequences / Implications

Easier: one C2PA engine for desktop, browser and third-party developers; the inspector's C2PA panels use the same API everywhere; FileSquid controls format coverage and the validation report shape. Harder: implementing and maintaining a security-critical specification is a large, ongoing effort; spec revisions, new assertion types and format handlers must be tracked; interoperability must be proven continuously against the oracle; the browser bundle grows by the engine and crypto backend and MUST be measured. The FOSS/patent review gate for the linked installer ([ADR 011](./011-native-media-adapters-and-ffmpeg-license.md)) now covers this crate's dependencies instead of the official SDK.

## Related Decisions / Notes

- Supersedes the `c2pa` crate choice in: [ADR 006](./006-libvips-native-stills-not-imagemagick.md), [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md), [ADR 015](./015-prov-o-jsonld-and-c2pa-sdk.md) (PROV-O JSON-LD and "no graph database" remain).
- Related: [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 008](./008-browser-wasm-scope.md), [ADR 009](./009-cryptography-webcrypto-compatible-suite.md), [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md), [ADR 024](./024-filesquid-metadata-engine.md), [ADR 025](./025-client-side-filesquid-engines.md).
- Requirements: [PRD F-054](../prd.md#f-054-c2pa-module), [PRD F-043](../prd.md#f-043-import-time-c2pa-records-and-action-disclosure), [FE-072](../product-vision.md#independent-library), [U-67](../product-vision.md#c2pa-module), [C-054](../product-vision.md#module-rules).
- [C2PA specification](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html) (2.4 is the version ADR 015 cites; the engine pins its version at first implementation), [official c2pa crate (oracle)](https://crates.io/crates/c2pa).
