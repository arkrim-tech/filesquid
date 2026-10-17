# Architecture Decision Record - FileSquid

024-filesquid-metadata-engine.md

## Title

filesquid-metadata is a stand-alone FOSS metadata CRUD engine; persist through pluggable stores, with filesquid-ocfl canonical for FileSquid (ADR 027)

## Status

Accepted

Accepted: 2026-09-26T00:30:00Z. Revised: 2026-09-26T01:15:00Z. Owner confirmed the engine extracts, creates, reads, updates and deletes metadata for files stored through filesquid-ocfl, including creating and enhancing Dublin Core and Schema.org records. ImageSquid MUST divert all metadata handling to this crate. Revised: 2026-10-15T03:34:43Z. Amended by [ADR 025](./025-client-side-filesquid-engines.md): the crate builds for `wasm32` and runs in the browser as well as the desktop; extraction takes bytes from `filesquid-ocfl`. Revised: 2026-10-15T06:00:00Z. Amended by [ADR 027](./027-filesquid-metadata-pluggable-stores.md): storage is pluggable (`sidecar`, optional `ocfl`, optional `sqlite`); ImageSquid keeps the `ocfl` store canonical. Revised: 2026-10-15T06:14:23Z. Amended by [ADR 028](./028-filesquid-ui-packages-and-panel-composition.md): the properties API is delivered as the `@filesquid/metadata-ui` sidebar panel; descriptive and technical file facts belong to this engine; Content Credentials belong to `filesquid-provenance` and application meaning to the composite product.

Superseded in part 2026-10-15 by [ADR 029](./029-filesquid-unified-product-and-distributions.md): `filesquid-metadata` is a module of FileSquid at `apps/filesquid/crates/metadata/`, not a separate product with its own repository; module independence and publication on its own remain, and the technical content is unchanged. Amended by [ADR 032](./032-hosting-and-privacy-model.md): the crate MAY run in `filesquid-server`.

Amended 2026-10-17 by [ADR 036](./036-single-fsquid-cli.md): every FileSquid CLI is the one `fsquid` binary built by `filesquid-cli`; this module's commands run there and the crate ships no binary of its own.

## Context / Requirement Reference

[ADR 022](./022-dublin-core-descriptive-metadata.md) closed the consumer descriptive vocabulary to ISO 15836-2 `/terms/` with Schema.org overflow. FileSquid already authors [filesquid-ocfl](./023-filesquid-ocfl-engine-and-parquet.md) as a stand-alone OCFL product. Metadata that lives only in FileSquid portable contracts or desktop codecs cannot be reused by institutions and cannot be the single write path for OCFL sidecars.

[U-63](../product-vision.md#metadata-module) and [FE-070](../product-vision.md#independent-library) require a stand-alone **`filesquid-metadata`** engine that extracts metadata from files going through `filesquid-ocfl`, performs full CRUD, and supports creating and enhancing Dublin Core and Schema.org records. [C-048](../product-vision.md#module-rules) forbids a parallel FileSquid extract/write path.

## Decision

FileSquid **authors `filesquid-metadata` as a stand-alone FOSS metadata CRUD engine** (library and later CLI). FileSquid is one caller. Institutions MUST be able to install and use it without FileSquid.

| | Value |
|---|---|
| Cargo package name | `filesquid-metadata` |
| Rust import | `filesquid_metadata` |
| MVP workspace path | `apps/filesquid/crates/metadata/` |

**Engine core**

1. **Extract** embedded and sidecar metadata from logical files stored through `filesquid-ocfl` (EXIF, ID3, IPTC, XMP, and other packets the engine supports). Map extracted descriptive facts onto `dcterms:` or `schema:`; keep native tags as `exif:` (or the packet’s prefix) observations. Unknown formats MUST leave the OCFL object intact and record an explicit unsupported/unknown observation.
2. **Create, read, update, delete** namespaced statements `{namespace, term, value}` with prefixes `exif:`, `dcterms:`, `schema:`.
3. Closed ISO 15836-2 `/terms/` register (`dcterms:`). Unknown local names MUST be rejected. Collection-domain `accrual*` MUST NOT be authored on a file. The fifteen `dc:` names MUST map to `dcterms:` on write ([ADR 022](./022-dublin-core-descriptive-metadata.md), [T-038](../product-vision.md#descriptive-metadata)).
4. Create and enhance Dublin Core records. Unmatched intent MUST suggest the closest Schema.org term; store overflow as `schema:` after confirmation. 1:1 `dcterms:`↔Schema.org map; close matches MUST NOT auto-alias.
5. Authoring API MUST support an Obsidian-style properties UI: populated key/value rows, namespace prefixes (`dcterms:title`), and **Add property**. Empty `/terms/` keys MUST stay hidden. Add searches `/terms/` first, then Schema.org. FileSquid’s inspector is a host of that API. Svelte and Tauri MUST NOT live in this crate.
6. **Stand-alone UI is the CLI**, shipped in Foundation: `extract`, `get`, `set`, `export`, `import`, `validate`. Spreadsheet users and scripts use it without FileSquid.
7. **Canonical metadata** is a Markdown logical file in the OCFL object (logical path `metadata.md`) with YAML front matter, Obsidian-style. Namespaces are YAML maps:

```yaml
---
dcterms:
  title: Harbour at dusk
  creator: George
exif:
  GPSLatitude: "49.3"
schema:
  certification: WERT level1
---

Optional Markdown body (long-form notes; MAY hold dcterms:description).
```

The UI shows `dcterms:title` as a property row. FileSquid MUST use this Markdown/YAML shape for metadata stored in OCFL unless a named requirement or integration profile explicitly requires JSON, XML, XMP, or CSV as the object payload ([T-046](../product-vision.md#canonical-ocfl-state-and-evidence)). Those formats remain interchange.

**Indexes.** SQLite, PostgreSQL, and Elasticsearch MUST be derived from `metadata.md` (and the OCFL object). Query and saved-query performance come from those indexes. The Markdown file is not scanned at query time after a successful rebuild. A catalog MUST be fully rebuildable from OCFL if it is lost.
8. **CSV import/export** (one row per object; headers `prefix:term`) and **XMP sidecar** read/write are Foundation interchange.
9. Interchange MUST NOT include MODS or SQL.
10. Persist through a pluggable store ([ADR 027](./027-filesquid-metadata-pluggable-stores.md)). With the `ocfl` store, which FileSquid uses as canonical, `metadata.md` is an ordinary logical file in the object and metadata-only edits are new OCFL versions. The crate MUST NOT implement storage-root layout, inventories, or `vN` publication.
11. Default `[dependencies]` MUST exclude Axum, Tauri, SQLite, and FileSquid domain crates (`filesquid-core`, the private-client library). `filesquid-ocfl` is an optional dependency behind the `ocfl` store feature ([ADR 027](./027-filesquid-metadata-pluggable-stores.md)). Format extract adapters belong to this crate (optional Cargo features as needed). They MUST NOT pull FileSquid native processing (`libvips` stills pipeline, FFmpeg worker, PhotoKit). They MUST take caller-supplied bytes or readers plus a `FileRef` ([ADR 027](./027-filesquid-metadata-pluggable-stores.md); in FileSquid the bytes come through `filesquid-ocfl`) and build for `wasm32-unknown-unknown`, or be feature-gated and reported as unsupported in the browser. The CLI binary is native-only ([ADR 025](./025-client-side-filesquid-engines.md)).

**FileSquid as caller**

FileSquid MUST divert **all** metadata handling to `filesquid-metadata`: extract, inspect, correct, author Dublin Core/Schema.org, batch edit, privacy-export field selection, and OCFL sidecar writes. Catalog/search indexes consume statements the engine returns. Pixel decode and recipes remain other modules; C2PA is the `filesquid-provenance` engine ([ADR 026](./026-filesquid-provenance-engine.md)). The ciphertext-only `*.filesquid.tech` service MUST NOT link `filesquid-metadata`; the owner-operated `filesquid-server` MAY ([ADR 032](./032-hosting-and-privacy-model.md)). The browser WASM bundle links `filesquid-metadata` and `filesquid-ocfl` with injected storage backends ([ADR 025](./025-client-side-filesquid-engines.md)); statement DTOs MAY be copied into portable contracts without a second vocabulary.

**Distribution**

The crate is a FileSquid workspace module at `apps/filesquid/crates/metadata/` ([ADR 029](./029-filesquid-unified-product-and-distributions.md)). It MUST build, test and publish on its own; during MVP it MAY carry `publish = false`. After the API is versioned, crates.io publication SHOULD be the distribution. FileSquid MUST pin the published Cargo package and MUST NOT shell out to a metadata CLI for canonical writes ([T-044](../product-vision.md#canonical-ocfl-state-and-evidence)).

## Rationale

OCFL already has a stand-alone engine. Metadata extract and authoring that stay in FileSquid would duplicate that pattern and block institutional reuse. One engine that reads file bytes from `filesquid-ocfl`, maps them onto `exif:` / `dcterms:` / `schema:`, and writes sidecars back through the same OCFL API gives FileSquid and other programmes the same CRUD surface.

## Assumptions

- `filesquid-ocfl` remains the OCFL writer FileSquid and `filesquid-metadata` share.
- Dublin Core `/terms/` remains the authoring vocabulary ([ADR 022](./022-dublin-core-descriptive-metadata.md)).
- Host UIs (FileSquid inspector, later CLI) consume the engine’s populated-key authoring API rather than inventing a second schema.

## Alternatives Considered

- **Register in `filesquid-core` only:** Cannot persist to OCFL or ship without FileSquid. Rejected as the engine home; T-038 moves to `filesquid-metadata`.
- **Desktop codecs extract; engine only persists:** Leaves a parallel FileSquid metadata path and blocks stand-alone extract. Rejected.
- **Metadata adapters buried in Media/Catalog:** Blocks independent tests, CLI, and install without FileSquid. Rejected.
- **Reimplement OCFL inside filesquid-metadata:** Divergent layout and receipts. Rejected; persist via `filesquid-ocfl`.
- **Ship only inside the FileSquid installer:** Blocks institutions that want a metadata CRUD engine without the full FileSquid application. Rejected.
- **Put Svelte/Tauri in the crate:** Couples the engine to FileSquid’s UI host. Rejected; host UIs call the library. The stand-alone UI is the CLI.
- **MODS or SQL interchange:** Out of scope. CSV, JSON, XMP, and embedded extract are interchange. Canonical storage is Markdown with YAML front matter in OCFL ([C-049](../product-vision.md#module-rules), [T-046](../product-vision.md#canonical-ocfl-state-and-evidence)).
- **Catalog-only metadata (SQLite/Postgres/Elasticsearch as source of truth):** Rejected. Those layers MUST rebuild from `metadata.md` in OCFL, the same way the catalog rebuilds from OCFL objects ([U-03](../product-vision.md#platform-ownership-and-access)).

## Consequences / Implications

Easier: one extract/CRUD/persist path for FileSquid and independent users; Dublin Core and Schema.org authoring is a library feature. Harder: format extract adapters live in this crate; FileSquid must not keep a second EXIF writer; catalog rebuild consumes statements from OCFL logical files; release engineering pins both engine versions. This record does not pick every extract crate.

## Related Decisions / Notes

- [ADR 016](./016-rust-ocfl-library.md), [ADR 022](./022-dublin-core-descriptive-metadata.md), [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md)
- [F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports), [F-052](../prd.md#f-052-metadata-module)
- [DCMI Metadata Terms](https://www.dublincore.org/specifications/dublin-core/dcmi-terms/), [ISO 15836 KB](../../.agents/kb/iso-15836-dublin-core.md)
