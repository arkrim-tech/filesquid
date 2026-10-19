# Architecture Decision Record - FileSquid

027-filesquid-metadata-pluggable-stores.md

## Title

filesquid-metadata uses pluggable stores; filesquid-ocfl becomes an optional store, and OCFL stays canonical for FileSquid

## Status

Accepted

Accepted: 2026-10-15T06:00:00Z. Owner confirmed filesquid-metadata is a separate product (not a feature of filesquid-ocfl) and a generic metadata tool usable without OCFL. Amends [ADR 024](./024-filesquid-metadata-engine.md).

Amended 2026-10-15 by [ADR 029](./029-filesquid-unified-product-and-distributions.md): `filesquid-metadata` is a module of FileSquid; its optional `ocfl` store is the one permitted cross-module dependency, and FileSquid uses it as canonical.

Amended 2026-10-18 (owner decision): the `sidecar` store writes one `<filename>.metadata.md` beside each plain file, not one `metadata.md` per folder, as the [metadata crate README](../../apps/filesquid/crates/metadata/README.md) describes. The `ocfl` store versions per edit: each metadata edit commits a new OCFL version through `filesquid-ocfl` and never modifies an earlier version (Decision 2; MD-0.2 in the [module plan](../../apps/filesquid/crates/metadata/resources/plan.md)).

Amended 2026-10-19 (owner decision): with the `ocfl` store, FileSquid keeps one metadata record per file, `<file-uuid>.metadata.md`, in the same directory as the stored original `<file-uuid>.<ext>` (the `<filename>.metadata.md` rule of the `sidecar` store, applied to the FileId name without the extension), instead of one logical `metadata.md` per object. It is THE file's metadata record: the owner's exact original file name, the file extension, the format and MIME type, the append-only `import` section that replaces the separate `file-record.md` of [ADR 038](./038-import-file-record.md), and the descriptive statements. The store still takes the logical path from the caller's `FileRef`, so the generic crate is unchanged; the MD-0.2 code on `main`, which FileSquid's callers point at a logical `metadata.md`, moves to the new path in MD-0.8. Each commit of the record also carries the version's PROV-O record and structured operation message ([ADR 040](./040-prov-o-history.md), MD-0.9).

## Context / Requirement Reference

[ADR 024](./024-filesquid-metadata-engine.md) made `filesquid-ocfl` the required storage engine for `filesquid-metadata`. That raised the question of whether a metadata engine that cannot work without OCFL should be a separate product at all, or a feature of `filesquid-ocfl`.

The code shows the coupling is small. `Record`/`Statement`, the Dublin Core register, EXIF/ID3 extraction, the `metadata.md` format, and CSV/XMP interchange work on plain data. Only `src/store.rs` (31 lines) calls `filesquid-ocfl` (`read_logical_file`, `write_logical_file`), and extraction (`src/extract.rs`) opens files directly. The owner chose three independent modules (`filesquid-ocfl`, `filesquid-metadata` and `filesquid-c2pa`, then named `filesquid-provenance`) that FileSquid combines behind one API; ImageSquid is a UI distribution of FileSquid ([product vision](../product-vision.md#modules), [ADR 029](./029-filesquid-unified-product-and-distributions.md)).

The owner also considered giving `filesquid-metadata` its own SQLite file as the canonical store. That would conflict with [U-03](../product-vision.md#platform-ownership-and-access) and [T-046](../product-vision.md#canonical-ocfl-state-and-evidence) for FileSquid: metadata history would leave OCFL versions, copying the OCFL root would no longer carry everything, and a lost database would lose metadata.

## Decision

1. **Separate product.** `filesquid-metadata` remains its own FOSS product. It is not merged into `filesquid-ocfl`. OCFL 1.1 does not define descriptive metadata, and the OCFL engine's default build stays free of format parsers ([ADR 023](./023-filesquid-ocfl-engine-and-parquet.md)).
2. **Pluggable store interface.** The core (statements, register, extraction from bytes, `metadata.md` format, CSV/XMP) depends on no store. A `MetadataStore` interface loads and saves records by an opaque `FileRef` supplied by the caller. Implementations:
   - `sidecar` (default on native): one `<filename>.metadata.md` beside each plain file (one per file, not one `metadata.md` per folder). Native only, because browsers have no ordinary filesystem; in WebAssembly the caller selects the `ocfl` store (as FileSquid does) or supplies its own `MetadataStore` implementation.
   - `ocfl` (optional Cargo feature): `metadata.md` as a logical file in an OCFL object through `filesquid-ocfl`; each edit is a new OCFL version. `FileRef` carries object id, version and logical path.
   - `sqlite` (optional Cargo feature, native only): records as rows in a caller-owned SQLite file, for stand-alone users who choose a database as their store.
3. **Optional index.** An optional SQLite index MAY sit beside any store for queries. It MUST be rebuildable from the canonical store and MUST NOT be the only copy.
4. **Dependency.** `filesquid-ocfl` moves from a required dependency to the optional `ocfl` feature. Default dependencies exclude it.
5. **FileSquid keeps OCFL canonical.** FileSquid MUST use the `ocfl` store as canonical. Its catalog and any SQLite index rebuild from `metadata.md` in OCFL ([U-03](../product-vision.md#platform-ownership-and-access), [T-046](../product-vision.md#canonical-ocfl-state-and-evidence)). FileSquid MUST NOT use the `sqlite` store as canonical.
6. **Extraction input.** Extraction takes bytes or a reader plus a `FileRef`, never a filesystem path in the core ([ADR 025](./025-client-side-filesquid-engines.md)). Path helpers stay in the native CLI.

## Rationale

The engine's value (extraction, a strict Dublin Core register with Schema.org overflow, a human-readable record, spreadsheet interchange) does not depend on OCFL. Making OCFL optional lets DAM developers, archivists and Obsidian-style users adopt it, and keeps the three engines symmetric: each stands alone and composite products wire them together. FileSquid's preservation guarantees are unchanged because it keeps the OCFL store canonical.

## Assumptions

- The store interface can be async and runtime-agnostic like the `filesquid-ocfl` backend trait, so it works natively and in WebAssembly.
- A SQLite crate suitable for the native `sqlite` feature exists; the feature is not built for `wasm32`.
- Existing tests that assert `filesquid-ocfl` is a dependency (`tests/unit/isolation.rs`, the F-052 feature) are updated by the implementation task, not by this record.

## Alternatives Considered

- **Keep `filesquid-ocfl` required (ADR 024 as written):** Leaves a metadata product that only OCFL users can adopt. Rejected.
- **Merge `filesquid-metadata` into `filesquid-ocfl` as a feature:** Fewer crates and releases, but puts format parsers into the OCFL engine and retires a product useful without OCFL. Rejected.
- **SQLite as the canonical store everywhere:** Breaks FileSquid U-03 and T-046 for FileSquid (history, portability, recovery). Rejected as FileSquid's canonical store; kept as an opt-in store for stand-alone users.

## Consequences / Implications

Easier: stand-alone adoption without OCFL; clearer product boundaries; the core is testable without storage. Harder: three store implementations to test; `FileRef` must stay generic; the `sqlite` store needs its own schema and migrations; FileSquid must configure the `ocfl` store explicitly.

## Related Decisions / Notes

- Amends [ADR 024](./024-filesquid-metadata-engine.md) (decision items 10 and 11). Related: [ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 022](./022-dublin-core-descriptive-metadata.md), [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md), [ADR 025](./025-client-side-filesquid-engines.md), [ADR 026](./026-filesquid-provenance-engine.md).
- Requirements: [PRD F-052](../prd.md#f-052-metadata-module), [U-63](../product-vision.md#metadata-module), [C-048](../product-vision.md#module-rules).
- Product vision: [filesquid-metadata](../product-vision.md#modules).
- Implementation: MD-0.4 in the [module plan](../../apps/filesquid/crates/metadata/resources/plan.md). MD-0.2 implements per-edit versioning: the `ocfl` store stages `metadata.md` and commits it through the `filesquid-ocfl` change model (IO-0.10), so each edit is one new version published by the engine's only write path; an unchanged record publishes nothing, an edit whose base head has moved is rebased onto the new head (OCFL-S-016), and only a concurrent change to the same `metadata.md` is reported as a conflict; nothing is overwritten (owner decision 2026-10-19), and a head without its version inventory (W010) stays refused as a commit base (OCFL-S-018) and is never repaired by the store (OCFL-S-020).
- Amended 2026-10-18 (owner decision): sidecar file naming and per-edit OCFL versioning in Decision 2; see Status.
