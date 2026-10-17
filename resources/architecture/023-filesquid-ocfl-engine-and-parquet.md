# Architecture Decision Record - FileSquid

023-filesquid-ocfl-engine-and-parquet.md

## Title

filesquid-ocfl is a stand-alone FOSS OCFL 1.1 product; FileSquid profile and later Parquet projection are layered

## Status

Accepted

Accepted: 2026-09-25T22:00:00Z. Revised: 2026-09-25T23:00:00Z. Owner confirmed `filesquid-ocfl` is always a stand-alone FOSS product that other users can install without ImageSquid. Revised: 2026-10-15T03:34:43Z. Amended by [ADR 025](./025-client-side-filesquid-engines.md): the backend trait is async and WASM-capable; ImageSquid profile construction moves to `apps/client`. Revised: 2026-10-15T06:14:23Z. Amended by [ADR 028](./028-filesquid-ui-packages-and-panel-composition.md): the engine adds a UI view API and the `@filesquid/ocfl-ui` Drive-style browser with sidebar panel slots; it owns storage facts only.

Superseded in part 2026-10-15 by [ADR 029](./029-filesquid-unified-product-and-distributions.md): `filesquid-ocfl` is a module of FileSquid in this repository, not a separate product with its own repository; module independence and publication on its own remain, and the technical content is unchanged. The private-client layer planned as `apps/client` is now planned in `filesquid-api` ([filesquid-api plan](../../apps/filesquid/crates/api/resources/plan.md)). Amended by [ADR 032](./032-hosting-and-privacy-model.md): the crate MAY run in `filesquid-server`.

Amended 2026-10-17 by [ADR 036](./036-single-fsquid-cli.md): every FileSquid CLI is the one `fsquid` binary built by `filesquid-cli`; this module's commands run there and the crate ships no binary of its own.

## Context / Requirement Reference

[ADR 016](./016-rust-ocfl-library.md) already names `filesquid-ocfl` as FileSquid’s authored OCFL library. The crate is intended for use by institutions and other programs without FileSquid types, application catalogs, or UI nouns ([U-40](../product-vision.md#ocfl-module), [FE-069](../product-vision.md#independent-library), [C-044](../product-vision.md#module-rules)). FileSquid package releases MUST pin a corresponding `filesquid-ocfl` version ([T-040](../product-vision.md#canonical-ocfl-state-and-evidence)).

A later paid dataset and preservation-workflow offering (Arkrim) is one post-1.0 candidate ([C-052](../product-vision.md#release-sequence)). It projects accepted OCFL logical state into source-linked Apache Parquet and other version-compliant collection jobs (including FAIR reporting) ([C-051](../product-vision.md#platforms-originals-and-service-limits)). That projection MUST live inside the same `filesquid-ocfl` crate as an optional feature, after FileSquid MVP. The first FileSquid MVP engine MUST already expose the read surface that projection needs ([T-039](../product-vision.md#canonical-ocfl-state-and-evidence), [T-041](../product-vision.md#canonical-ocfl-state-and-evidence), [C-045](../product-vision.md#module-rules), [C-046](../product-vision.md#module-rules)).

OCFL 1.1 is the layout of objects at rest ([ADR 010](./010-ocfl-1-1-dual-profile.md)). It does not define FileSquid collection snapshots, recipes, C2PA sidecars, catalog rebuild, or analytical tables. Putting those types in the engine core would make a third-party OCFL user take a FileSquid store. Walking version directories by name instead of inventory `state`/`manifest` would also break later projection of unchanged bytes stored in older `vN` directories.

## Decision

`filesquid-ocfl` is a **stand-alone FOSS OCFL 1.1 product** (a library whose commands run in the one FileSquid CLI, `fsquid`, built by `filesquid-cli` ([ADR 036](./036-single-fsquid-cli.md)); independent users install `filesquid-cli` for the command line) for the digital preservation community as well as FileSquid. FileSquid is one caller. Other programmes MUST be able to install and use it without installing FileSquid, and without compiling FileSquid domain types ([U-40](../product-vision.md#ocfl-module), [C-047](../product-vision.md#module-rules)).

The crate is a FileSquid workspace module at `apps/filesquid/crates/ocfl/` ([ADR 029](./029-filesquid-unified-product-and-distributions.md)). It MUST build, test and publish on its own, without FileSquid API, host or distribution crates. During MVP it MAY carry `publish = false`. After the engine API is versioned, crates.io publication SHOULD be the distribution; callers MUST then pin that published version ([T-042](../product-vision.md#canonical-ocfl-state-and-evidence)).

Existing community OCFL programs fell short as an embeddable engine. This product fills those gaps and goes further: crash-safe OCFL 1.1 version publication, first-class S3-compatible storage with compare-and-swap on inventory heads, and later optional Parquet projection. Layout remains OCFL 1.1.

**Engine core (FileSquid MVP, default Cargo features)**

The public API MUST provide:

1. A storage backend trait (put / get / head / list / exists, plus multipart and compare-and-swap where the backend requires them). The trait is async and runtime-agnostic and MUST NOT require `Send` on `wasm32`. Local filesystem (native only), S3-compatible (native and `wasm32`) and in-memory backends implement that trait; a browser cache backend MAY live in the caller. Callers select a backend; OCFL rules do not change ([ADR 025](./025-client-side-filesquid-engines.md)) ([ADR 016](./016-rust-ocfl-library.md), [ADR 012](./012-s3-compatible-ciphertext-storage.md)).
2. Open a storage root; discover object roots; load NAMASTE, inventories, and digest sidecars.
3. **Inventory-driven logical state** at a **caller-pinned object version** (`head` or a named `vN`). Resolve logical paths through inventory `state` and `manifest`. Unchanged bytes that remain in an older version directory MUST appear in the pinned logical state. Directory-name inference MUST NOT be the resolver.
4. Stream or copy a logical file without requiring the whole object in memory.
5. Structural validation; SHA-256 digest; `.git` prohibition; forward-delta write with `vN` reserved before object-root bytes and root inventory sidecar last.
6. An enumerator of committed logical state: object identifier, version, logical path, digest, and content locator. Callers (FileSquid catalog rebuild, later Parquet projection) consume these events. The enumerator MUST NOT require SQLite, PostgreSQL, or FileSquid catalog types.
7. A UI view API (folder listing, file details, version history, size, fixity, ranged reads, version-producing writes) that the `@filesquid/ocfl-ui` Drive-style browser uses. The engine owns storage facts only; descriptive and technical file facts belong to `filesquid-metadata`, Content Credentials to `filesquid-provenance`, and application meaning to the composite product ([ADR 028](./028-filesquid-ui-packages-and-panel-composition.md)).

Default `[dependencies]` MUST stay free of Apache Arrow, Apache Parquet, DuckDB, Iceberg, Axum, Tauri, libvips, FFmpeg, SQLite, and FileSquid domain crates. `publish` MAY remain false until the engine API is versioned; independent crates.io distribution, with its commands in `fsquid` (`filesquid-cli`), remains the product intent and MUST NOT be designed away ([T-042](../product-vision.md#canonical-ocfl-state-and-evidence)). The API MUST be written as a public crate surface.

Community OCFL fixtures and oracles (`rocfl`, `ocfl-py`) MUST validate emitted objects. They MUST NOT be the writer. The conformance suite is the `1.1/` tree of [OCFL/fixtures](https://github.com/OCFL/fixtures), pinned to one commit and used only as test data ([OCFL-S-009](../../apps/filesquid/crates/ocfl/resources/srs.md)–[OCFL-S-011](../../apps/filesquid/crates/ocfl/resources/srs.md)).

**FileSquid application profile (caller)**

The private-client library (`filesquid-api` (`apps/filesquid/crates/api/`; was `apps/client`), on desktop and in browser WASM) constructs ([ADR 025](./025-client-side-filesquid-engines.md)):

- One OCFL object per FileSquid **file** (originals, renditions, recipes, evidence, C2PA and metadata sidecars).
- Separate versioned **collection snapshot** objects.
- Dual-profile encryption: the engine stores **opaque bytes** the caller supplies. Owner-readable local media and ciphertext hosted packages are caller transforms, not engine object types ([ADR 010](./010-ocfl-1-1-dual-profile.md), [ADR 009](./009-cryptography-webcrypto-compatible-suite.md)).
- Intent in the operational catalog, then catalog update (SQLite on desktop, browser cache on web) from the published OCFL receipt ([ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 007](./007-durable-jobs-and-interactive-performance.md)).

Those types MUST NOT appear in `filesquid-ocfl`’s default public API.

**Later same-crate Parquet projection (after MVP)**

A Cargo feature `parquet` (default **off**) MAY add OCFL→Parquet projection inside `filesquid-ocfl`:

- Read committed logical state through the engine enumerator and stream APIs.
- Write Apache Parquet via Apache Arrow record batches to a **caller-chosen analytical location**.
- Keep primary media in preservation storage. Avoid one Parquet file per OCFL object.
- Pin source object versions in a generation manifest. Readers MUST NOT assemble a dataset from a mutable wildcard.
- Treat Parquet, DuckDB work tables, and optional later Iceberg tables as **derived**. OCFL remains authoritative for accepted archival state.
- Do not create analytics directories or application keys inside OCFL object or storage-root reserved paths (NAMASTE, inventories, version directories, optional `logs/`, optional `extensions/`). A dataset accepted for preservation re-enters through a normal OCFL object write of caller-supplied bytes.

DuckDB, dbt, and Iceberg/Polaris remain later analytical runtimes **outside** the engine default graph. They consume Parquet the feature emits. FileSquid MVP MUST NOT compile them.

**Coordinated releases**

Each FileSquid packaged release that links `filesquid-ocfl` MUST declare the exact `filesquid-ocfl` version in its lockfile and release notes. A filesquid-ocfl engine change that FileSquid needs MUST ship as a filesquid-ocfl version FileSquid then pins.

After extraction (global Task 10.4), FileSquid MUST depend on the published **Cargo package** and compile that library into the desktop native graph and the browser WASM bundle. GitHub Release **`fsquid` binaries** built by `filesquid-cli` are for independent install. FileSquid MUST NOT shell out to `fsquid` for canonical OCFL I/O ([T-043](../product-vision.md#canonical-ocfl-state-and-evidence)).

## Rationale

Existing community OCFL programs fell short as an embeddable Rust engine with crash-safe version publication and S3-compatible head CAS. Authoring a stand-alone FOSS product fills those gaps and still lets FileSquid (and later distributions such as Arkrim) be ordinary callers. A third-party user needs storage-root open, inventory-driven reads, validation, and forward-delta writes. Putting collection snapshots and ciphertext types in the core would force every consumer to take FileSquid’s object model.

Parquet projection is the same walk of committed logical state that catalog rebuild already needs. Exposing that enumerator in MVP lets a later `parquet` feature sit on a stable API. Compiling Arrow/Parquet into the default graph would tax FileSquid’s desktop installer and contradict “engine usable without FileSquid context” for sites that only want validate/ingest.

Inventory-driven resolution is required for both independent recovery and later analytics: OCFL stores unchanged bitstreams under earlier `vN` paths. A directory walk of the current version directory would drop those files from a dataset.

## Assumptions

- OCFL 1.1 remains the published standard at implementation.
- FileSquid MVP ships the engine core; the `parquet` feature is a later filesquid-ocfl increment consumed by a future dataset product (Arkrim), not a FileSquid consumer MVP feature.
- Apache Parquet and Arrow remain suitable interchange formats when that feature is added; exact crate versions are an integration gate then.
- Community validators continue to accept structurally valid objects this engine emits.

## Alternatives Considered

- **FileSquid types in the engine core:** Blocks institutional reuse and stand-alone install. Rejected.
- **Ship filesquid-ocfl only inside FileSquid:** Blocks users who need OCFL tooling without the full FileSquid application. Rejected.
- **Keep filesquid-ocfl permanently nested in the FileSquid Git repository:** Blocks digital-preservation programmes that want a small, independently versioned OCFL engine. Rejected as the long-term distribution model; MVP nesting is allowed until the API is versioned.
- **Parquet as a separate crate:** The owner directed the projection to live in this library. A Cargo feature keeps the default graph small. Rejected as the primary packaging.
- **Compile Arrow/Parquet/DuckDB into MVP default features:** Extra native weight and a second data model before the engine API exists. Rejected for FileSquid MVP.
- **Walk version directories by folder name:** Misses unchanged bytes stored in prior versions; breaks later projection. Rejected.
- **Write Parquet shards inside OCFL object `extensions/` or extra storage-root directories:** Violates OCFL layout and mixes analytical lifecycle with preservation. Rejected.
- **Second OCFL implementation for analytics:** Divergent layout and receipts. Rejected; one engine.
- **Make Parquet canonical:** Conflicts with owner-readable OCFL as source of accepted state ([ADR 010](./010-ocfl-1-1-dual-profile.md)). Rejected.

## Consequences / Implications

Easier: institutions can depend on `filesquid-ocfl` for OCFL 1.1; FileSquid and its distributions share one engine; catalog rebuild and Parquet projection share one enumerator. Harder: the private-client library owns collection snapshots and dual-profile encryption on desktop and web, and the desktop owns SQLite rebuild; IO-0.7/0.8 stay engine-generic; the first public API must be documented as if another team is the caller. FileSquid release engineering must pin and announce filesquid-ocfl versions. This record does not implement Parquet.

## Related Decisions / Notes

- [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 009](./009-cryptography-webcrypto-compatible-suite.md), [ADR 010](./010-ocfl-1-1-dual-profile.md), [ADR 012](./012-s3-compatible-ciphertext-storage.md), [ADR 016](./016-rust-ocfl-library.md), [ADR 021](./021-uuid-v4-identifiers.md)
- [PRD F-042](../prd.md#f-042-ocfl-preservation-and-portable-recovery), [PRD F-051](../prd.md#f-051-ocfl-module)
- [OCFL 1.1](https://ocfl.io/1.1/spec/), [Apache Parquet overview](https://parquet.apache.org/docs/overview/), [Arrow Rust ArrowWriter](https://arrow.apache.org/rust/parquet/arrow/arrow_writer/struct.ArrowWriter.html)
