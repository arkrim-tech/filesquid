# Architecture Decision Record - FileSquid

016-rust-ocfl-library.md

## Title

FileSquid-authored Rust OCFL library: read, write, browse on local filesystem and S3-compatible storage

## Status

Accepted

Accepted: 2026-09-18T05:11:07Z. Revised: 2026-09-25T22:00:00Z. Owner renamed the crate from `imagesquid-ocfl` to `filesquid-ocfl` for FileSquid product-family identity. Engine versus ImageSquid profile, coordinated releases, and later optional Parquet projection are [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md). Revised: 2026-10-15T03:34:43Z. Amended by [ADR 025](./025-client-side-filesquid-engines.md): the crate runs in Tauri and browser WASM through an async storage backend interface; Axum never links it. Revised: 2026-10-15T12:00:00Z. Owner added the forward-delta change model (automatic delta computation; stage, status, diff, commit, log, checkout, revert, reinstate) as a public library API, with `fsquid` built only on it ([product vision](../product-vision.md#modules)). Revised: 2026-10-16T23:20:22Z. Owner renamed the CLI binary from `filesquid-ocfl-cli` to `fsquid` for a short command name; the library crate stays `filesquid-ocfl`. `squid` was rejected because it is the binary name of the Squid caching proxy.

Amended 2026-10-15 by [ADR 029](./029-filesquid-unified-product-and-distributions.md): `filesquid-ocfl` is the `ocfl` module of FileSquid at `apps/filesquid/crates/ocfl/`; it still builds, tests and publishes on its own. Amended by [ADR 032](./032-hosting-and-privacy-model.md): it MAY run in the owner-operated `filesquid-server`, and never on the ciphertext-only `*.filesquid.tech` service.

Amended 2026-10-17 by [ADR 036](./036-single-fsquid-cli.md): every FileSquid CLI is the one `fsquid` binary built by `filesquid-cli`; this module's commands run there and the crate ships no binary of its own.

## Context / Requirement Reference

OCFL 1.1 is the canonical layout ([ADR 010](./010-ocfl-1-1-dual-profile.md), [FE-001](../product-vision.md#independent-library), [FE-033](../product-vision.md#portable-preservation), [U-10](../product-vision.md#platform-ownership-and-access), [U-03](../product-vision.md#platform-ownership-and-access)). The specification describes objects at rest. It does not ship software. The client engines own OCFL work on every client ([ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 025](./025-client-side-filesquid-engines.md)). Rebuild-from-OCFL, collection browse, and crash recovery all **read** objects, not only append versions. A completed change is intent, then a durable OCFL version and collection snapshot, then catalog update from that receipt ([ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 007](./007-durable-jobs-and-interactive-performance.md), [T-005](../product-vision.md#canonical-ocfl-state-and-evidence), [T-035](../product-vision.md#canonical-ocfl-state-and-evidence)). Requirement traces in this ADR stay at the vision; PRD F-numbers are allocated in the regenerated [PRD](../prd.md) ([F-042](../prd.md#f-042-ocfl-preservation-and-portable-recovery), [F-049](../prd.md#f-049-product-component-terminology-and-item-type)). No SRS exists yet.

Git looks similar (blobs, trees, commits). Community OCFL programs (`rocfl`, `ocfl`, `gocfl`, `ocfl-py`, `ocfl-java`) look like a ready-made engine. They fell short as FileSquid’s library: Git violates OCFL layout and owner-readable recovery; those programs do not provide an embeddable Rust engine with crash-safe `vN` publication, SHA-256 digest, S3-compatible compare-and-swap on inventory heads, a complete forward-delta change model as a library API, or a later path to OCFL→Parquet ([ADR 009](./009-cryptography-webcrypto-compatible-suite.md), [ADR 012](./012-s3-compatible-ciphertext-storage.md), [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md)).

This record freezes ownership, crate name, and scope of **`filesquid-ocfl`**, a stand-alone FOSS OCFL 1.1 product that FileSquid consumes. When this record was written, technical identifiers used an `imagesquid-*` prefix and the crate name was an owner-directed exception to it; since [ADR 029](./029-filesquid-unified-product-and-distributions.md) every crate is named `filesquid-*`, so the exception is historical. Exact on-disk path names remain for first implementation, as [ADR 010](./010-ocfl-1-1-dual-profile.md) already stated.

## Decision

FileSquid **authors its own** FOSS OCFL 1.1 module **`filesquid-ocfl`**, usable on its own by the digital preservation community as well as FileSquid. FileSquid is one caller. Other programmes MUST be able to install and use the library (and a later CLI) without installing FileSquid. The crate MUST build, test and publish on its own ([ADR 029](./029-filesquid-unified-product-and-distributions.md) Decision 4, [U-40](../product-vision.md#ocfl-module), [C-047](../product-vision.md#module-rules), [T-042](../product-vision.md#canonical-ocfl-state-and-evidence), [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md)).

The Cargo crate **`filesquid-ocfl`** lives in the client-engine family for FileSquid’s graph: native in Tauri and the owner-operated `filesquid-server`, `wasm32` in browser workers, never in the ciphertext-only `*.filesquid.tech` service ([ADR 032](./032-hosting-and-privacy-model.md), [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 013](./013-cargo-svelte-workspaces.md), [ADR 025](./025-client-side-filesquid-engines.md)). Its **default public API is a generic OCFL 1.1 engine**. Layout remains OCFL 1.1. Relative to existing community programs, this product fills embeddable-Rust, crash-safe publication and change-model gaps and goes further with first-class S3-compatible storage and later optional Parquet projection.

| | Value |
|---|---|
| Cargo package name | `filesquid-ocfl` |
| Rust import | `filesquid_ocfl` |
| Workspace path | `apps/filesquid/crates/ocfl/` |

It is not a module buried inside a larger native-processing crate. Other native crates **depend on** `filesquid-ocfl`; they do not reimplement OCFL. **`fsquid`** MUST link this crate as a stand-alone installable tool with git-style commands (stage, status, diff, commit, log, checkout, revert, reinstate, validate). The library remains the API; the CLI MUST use only the public library API and MUST NOT be the only API.

The library **reads, browses, validates, and writes** OCFL 1.1 storage roots and objects. Write is one operation, not the whole component. Git MUST NOT be the OCFL library. The crate is not git-annex, Git LFS, or a wrapper around a community OCFL CLI or Java/Python runtime.

The FileSquid desktop application and the browser WASM bundle **link `filesquid-ocfl`**. FileSquid owners do not need a separate community `ocfl` binary. Independent users install `filesquid-ocfl` (library and later CLI) without FileSquid.

**Storage backends (required, both)**

This crate MUST read and write OCFL on **both**:

1. **Local filesystem** — a directory that is an OCFL storage root on the owner's disk (default local profile: owner-readable media and inventories; [ADR 010](./010-ocfl-1-1-dual-profile.md)).
2. **S3-compatible object storage** — the same OCFL relative paths as object keys under a bucket or prefix (hosted / optional encrypted profile: ciphertext bodies; structurally readable inventories over ciphertext; [ADR 010](./010-ocfl-1-1-dual-profile.md), [ADR 012](./012-s3-compatible-ciphertext-storage.md)).

One async, runtime-agnostic storage interface (put / get / head / list / exists, plus multipart and compare-and-swap where the backend requires them). The S3-compatible backend MUST also build for `wasm32` over `fetch`; a browser cache backend (OPFS/IndexedDB) is staging only, never a canonical root ([ADR 025](./025-client-side-filesquid-engines.md)). Callers select a backend; OCFL rules do not change. The crate MUST NOT be filesystem-only with a separate non-OCFL S3 uploader. The crate MUST NOT require Amazon-only APIs ([ADR 012](./012-s3-compatible-ciphertext-storage.md)).

| | Local filesystem | S3-compatible |
|---|---|---|
| What is stored | OCFL storage root on disk | OCFL storage root as keys in a bucket or prefix |
| Default profile | Owner-readable (ADR 010 §1) | Ciphertext (ADR 010 §2) |
| Large objects | Ordinary files | Multipart upload (ADR 012) |
| Publishing `head` | Per-object lease, then inventory sidecar last | Reserve `vN` before content; same write order; CAS on the inventory head (ADR 012) |
| Who links the crate | Tauri desktop | Tauri desktop and browser WASM (hosted backup, restore, sync; the browser's canonical store) |

WASM does not mount OCFL on a local disk. It runs this same crate with the S3-compatible backend against the encrypted hosted root ([ADR 008](./008-browser-wasm-scope.md), [ADR 025](./025-client-side-filesquid-engines.md)). There is no second OCFL implementation. Browser replica browse MAY use decrypted shards as a derived cache. The ciphertext-only `*.filesquid.tech` service MUST NOT link this crate to read or write private library bytes ([ADR 004](./004-axum-hosted-ciphertext-mailbox.md), [ADR 032](./032-hosting-and-privacy-model.md)). Optional encrypted **local** packages use the ciphertext profile on the filesystem backend, not a third store.

**Read and browse (required)**

- Open a storage root; discover object roots; load NAMASTE, inventories, and digest sidecars.
- Resolve any version's logical state through the `manifest` to content paths; stream or copy a logical file.
- List logical paths; show version history (`created`, `message`, `user`); diff two versions, or staged changes against `head`; report structural validation.
- Enumerate committed logical state from the storage root alone so a caller can rebuild operational catalog indexes ([FE-065](../product-vision.md#independent-library), [U-10](../product-vision.md#platform-ownership-and-access), [U-03](../product-vision.md#platform-ownership-and-access), [T-039](../product-vision.md#canonical-ocfl-state-and-evidence)).

Inventory/types that are safe to share MAY live in portable contracts. Local filesystem I/O stays native; S3-compatible I/O runs in native and WASM clients and in `filesquid-server`; neither runs in the `*.filesquid.tech` service.

**Write (forward delta)**

1. The **caller** records intent in its operational catalog (operation ID, expected current `head`). The engine does not own SQLite or PostgreSQL.
2. **Reserve `vN` before any object-root bytes for that version are published.** Local filesystem: exclusive per-object lease ([ADR 007](./007-durable-jobs-and-interactive-performance.md)). S3-compatible: a compare-and-swap reservation or a PostgreSQL generation that **authorizes** `vN` keys **before** content upload ([ADR 012](./012-s3-compatible-ciphertext-storage.md)). Head CAS at the end is required and **not sufficient**. A second writer MUST NOT publish the same `vN` content prefix.
3. Never mutate `v1`…`v(N-1)` content or prior inventories.
4. Write only new or changed bitstreams under `vN/{contentDirectory}/`. Unchanged files keep existing content paths. Rename = same digest, new logical path in `state`. Delete = omit from the new `state` (bytes remain for history).
5. Build a new inventory: previous `versions` blocks kept; new `state` is the full logical tree; `manifest` extended; `head` = `vN`. Digest algorithm is SHA-256 ([ADR 009](./009-cryptography-webcrypto-compatible-suite.md)).
6. Write version inventory + sidecar, then root inventory + sidecar **last**. Assemble the next version in a workspace **outside** the object root, then publish.
7. The **caller** updates SQLite (desktop), the browser cache (web) or PostgreSQL receipts (hosted) from the **published** receipt. A crash after a published head and before this step reconciles to that receipt and MUST NOT duplicate `vN`.

**Change model (public API)**

Steps 2 to 6 sit behind a git-like change model. It is the only write path, used by the library API, `fsquid` and the view API's version-producing actions alike.

- **Delta computation.** Given a working state (a directory or a set of logical files) and a base version, the engine computes the change set by digest: added, changed, renamed, deleted and reinstated paths. It writes only bytes whose digest is not already in the object's `manifest`. Renames, deletes and reinstatements change `state` only (step 4).
- **Stage, status, diff, commit.** Changes accumulate in a staging area against a recorded base `head`. Staging lives outside the object root and the storage root: a caller-chosen directory on native, or browser staging storage on WASM ([ADR 025](./025-client-side-filesquid-engines.md)). It MUST NOT use an OCFL extension inside the object. `status` and `diff` report the staged change set before commit. Commit publishes exactly one `vN` through steps 2 to 6.
- **Log, checkout, revert, reinstate.** `log` reads `versions` from the inventory. Checkout writes a version's logical state to a caller location outside the storage root. Revert (restore the state of an earlier version) and reinstate (restore chosen paths) each commit a **new** version that points logical paths at existing manifest entries. They MUST NOT modify or remove earlier versions.
- **Base moved.** If the base `head` is no longer current at reservation or head CAS, the engine MUST rebase the staged change set onto the new `head` when no staged path was changed by the intervening versions, and otherwise report a conflict. It MUST NOT publish over another writer's version.

The staging area's on-disk format is specified at first implementation, like the layout extension. It is engine-private working data, not OCFL content, and is never required to read or recover an object.

**Published head (readers)**

A version is committed only when **all** of these hold:

- The root `inventory.json` sidecar digest matches the root inventory bytes.
- Root `head` is `vN`.
- When a version inventory exists, it equals the root inventory ([OCFL 1.1 §3.7](https://ocfl.io/1.1/spec/#inventory)).

A version-directory sidecar **alone** is not a commit. If `vN` content or a version inventory exists while the root `head` is still `v(N-1)`, or the root sidecar is missing or mismatched, the object is **incomplete**: readers MUST treat that `vN` as unpublished; recovery MUST reconcile or replace it under the same reservation and MUST NOT invent a second published `vN`.

**Object granularity**

The engine stores OCFL objects the caller defines as logical files in a version `state`. FileSquid's private-client library, `filesquid-api` (`apps/filesquid/crates/api/`; was `apps/client`), as a caller on desktop and in the browser, uses:

1. One OCFL object per coherent **file** (original, selected renditions, recipes, source/HTTP evidence, C2PA and metadata sidecars, history). ImageSquid UIs MAY label that file photo, image, or video.
2. Separate versioned **collection snapshot** objects (collections, order, relationships, confirmed identities, pointers to file-object ids and their head versions).

Those FileSquid object shapes MUST NOT be required types in the engine default API ([ADR 023](./023-filesquid-ocfl-engine-and-parquet.md)).

**Layout constraints**

- A `.git` directory MUST NOT appear in an OCFL object root or storage root (OCFL allows only NAMASTE, inventories, version directories, optional `logs/`, optional `extensions/` at the object; storage roots MUST NOT add extra directories besides the object hierarchy and optional `extensions/`).
- Hard links, soft links, and empty directories under the storage hierarchy MUST NOT be used.
- Owners copy a completed OCFL snapshot with ordinary file tools ([FE-065](../product-vision.md#independent-library)). Reconstruction MUST NOT require Git or FileSquid. Direct inspection and copy MUST NOT authorize in-place inventory or signed-original edits ([T-036](../product-vision.md#canonical-ocfl-state-and-evidence)).

**Community OCFL programs**

Independent validators and CLIs MAY be used in CI and for developer inspection (`rocfl validate`, `ocfl-py` / `ocfl-validate.py`, [OCFL fixtures](https://github.com/OCFL/fixtures)). They MUST NOT be FileSquid's library, a bundled sidecar, or a hosted mailbox dependency. Optional later export adapters MAY read OCFL into another packaging format; they MUST NOT become the store.

## Rationale

OCFL already is the content-addressed, append-only versioning system (manifest = blobs, `state` = tree, inventory + sidecar = commit, `head` = HEAD). Git duplicates that job with incompatible rules: a DAG and working tree instead of linear `vN`, SHA-1 packfiles instead of sha256 inventories, opaque objects instead of owner-readable files, and poor behaviour on large already-compressed media.

A per-collection Git repository cannot sit inside a valid OCFL root. Recovery that needs Git breaks FileSquid U-10. Git LFS and git-annex reinvent the manifest FileSquid already has.

`rocfl` and `ocfl-java` show that a dedicated OCFL API is the right shape (list, log, cat, diff, stage, commit). They fell short as an embeddable Rust engine with crash-safe `vN` publication (reserve before content, inventory last, unpublished `vN`), SHA-256, S3-compatible compare-and-swap on inventory heads, and a later OCFL→Parquet path. Each covers part of the change model and none covers it whole: `rocfl` stages explicit `cp`/`mv`/`rm` commands in its CLI only and gives no strong concurrency guarantees on S3; `ocfl-java` dedupes whole-directory puts and stages a mutable HEAD inside the object, but has no status or diff of staged changes, no revert, and needs a JVM. Authoring **`filesquid-ocfl` as its own FOSS product** fills those gaps and still emits standard OCFL 1.1 that community validators can check.

A write-only helper would leave catalog rebuild, repair, and version inspection to a second stack. Those are the same objects; they belong in one library.

A filesystem-only OCFL crate plus a separate S3 blob uploader would split layout, fixity, and head publication. Hosted roots are still OCFL; they must go through this crate.

## Assumptions

- OCFL 1.1 remains the published standard at implementation.
- Community validators accept objects this library emits when structure and ciphertext fixity are in spec; plaintext recovery remains a client check ([ADR 010](./010-ocfl-1-1-dual-profile.md)).
- `rocfl`, `ocfl-java`, and `ocfl-py` stay useful as oracles even if their APIs change; FileSquid does not depend on those APIs at runtime.
- Content-directory names, layout extension, and object-id → path mapping are specified at first implementation. This ADR does not freeze them. The crate name **`filesquid-ocfl`** is frozen. The former name `imagesquid-ocfl` MUST NOT return.
- At least one S3-compatible endpoint used in development (for example MinIO) exposes the put/get/list/multipart and conditional-write (or equivalent fence) behaviour [ADR 012](./012-s3-compatible-ciphertext-storage.md) requires.

## Alternatives Considered

- **Git working tree or sibling `.git` over the storage root:** Invalid OCFL (extra directory); mutable working tree vs immutable version dirs; two HEADs; recovery needs Git. Rejected.
- **Git LFS or git-annex for binaries, Git for inventories:** Second manifest, second hash, second remote; still not S3 CAS or ciphertext envelopes. Rejected.
- **Git only for collection JSON:** Second versioning protocol for data OCFL already versions; `.git` still cannot live in the storage root. Rejected.
- **Shell out to `rocfl`, `ocfl` (ocfl-tools), or `gocfl`:** Extra process; unofficial staging extensions; no dual profile, receipts, hosted CAS, or in-app browse. Rejected as FileSquid's library.
- **Vendor the `rocfl` crate:** CLI last published on crates.io as 1.7.0 (2022); not an embeddable library other crates depend on; staging uses unspecified `rocfl-staging` / `rocfl-locks`. Rejected as the engine.
- **`ocfl-java` (Fedora 6) or `ocfl-py` / `ocflcore`:** Mature libraries in Java or Python. Extra runtime against ADR 001 / 013. Rejected as application code.
- **Write-only helper; browse via a community CLI:** Two engines for one tree; repair and rebuild would depend on a sidecar. Rejected.
- **Filesystem-only OCFL crate; S3 as a raw object uploader:** Hosted preservation would not be OCFL (no inventories, no forward-delta, no CAS head). Rejected.
- **Two OCFL implementations (one for disk, one for S3):** Divergent layout and receipts. Rejected; one crate, two backends.
- **Crate name `ocfl`, `ocfl-rs`, or `rocfl`:** Sounds official or collides with [rocfl](https://crates.io/crates/rocfl). Rejected.
- **Crate name `imagesquid-ocfl`:** Original freeze under the ImageSquid identifier prefix. The owner renamed the crate to `filesquid-ocfl` so the preservation engine already uses the later FileSquid product-family name. Rejected as the current name.
- **OCFL as a private module of a catch-all native crate:** Blocks independent tests, a stand-alone CLI, and install without FileSquid. Rejected; `filesquid-ocfl` is its own FOSS product.
- **filesquid-ocfl available only inside the FileSquid installer:** Contradicts independent institutional use. Rejected.
- **Store Git packfiles as OCFL content:** Breaks parsability and completeness (rebuild would require Git). Rejected.

## Consequences / Implications

Easier: one named crate (`filesquid-ocfl`) for ingest, inspect, repair, rebuild, local disk, and hosted S3-compatible roots, on desktop and in the browser; standard validators can check emitted objects; Git remains the VCS for FileSquid source only. Harder: FileSquid owns inventory construction, path safety, digest sidecars, dual-profile encryption, logical-state resolution, crash reconciliation, multipart, version reservation, and head CAS; first implementation still needs a layout/path ADR and must scaffold `apps/filesquid/crates/ocfl/`. Concurrent writers are serialized by per-object reservation **before** `vN` bytes, then CAS on the root inventory; not `git push`. This record does not create the crate.

Cache eviction MUST NOT delete or rewrite authoritative OCFL content ([T-021](../product-vision.md#caching)). Ephemeral cache generation MUST NOT become permanent PROV-O history unless the output is retained or disclosed ([T-037](../product-vision.md#canonical-ocfl-state-and-evidence)). Optional technical-user export to git-annex or BagIt is an exit adapter, not canonical storage.

## Related Decisions / Notes

- [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 007](./007-durable-jobs-and-interactive-performance.md), [ADR 008](./008-browser-wasm-scope.md), [ADR 009](./009-cryptography-webcrypto-compatible-suite.md), [ADR 010](./010-ocfl-1-1-dual-profile.md), [ADR 012](./012-s3-compatible-ciphertext-storage.md), [ADR 015](./015-prov-o-jsonld-and-c2pa-sdk.md), [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md)
- [OCFL 1.1](https://ocfl.io/1.1/spec/), [implementation notes](https://ocfl.io/1.1/implementation-notes/), [implementations wiki](https://github.com/OCFL/spec/wiki/Implementations)
- Inspection oracles (not FileSquid's library): [rocfl](https://github.com/pwinckles/rocfl), [ocfl-java](https://github.com/OCFL/ocfl-java), [ocfl-py](https://github.com/zimeon/ocfl-py), [ocfl-tools](https://github.com/srerickson/ocfl-tools)
