# filesquid-metadata

The metadata module of FileSquid. It extracts metadata from file bytes, creates, reads, updates and deletes namespaced statements, and creates and enhances Dublin Core and Schema.org records. Institutions can use it on its own.

| | |
|---|---|
| Cargo package | `filesquid-metadata` |
| Rust import | `filesquid_metadata` |
| Path | `apps/filesquid/crates/metadata/` (member of the `apps/filesquid/` Cargo workspace) |

Today the crate persists records only through OCFL: `metadata.md` (YAML front matter + Markdown body) is read and written as a logical file in an OCFL object through [`filesquid-ocfl`](../ocfl/README.md), FileSquid's canonical store, and the dependency on `filesquid-ocfl` is unconditional. Planned, not yet available ([ADR 027](../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md)): a pluggable `MetadataStore` interface; a `sidecar` store (default on native) keeping `metadata.md` beside the file; the `ocfl` store as an optional feature; and an optional native `sqlite` store keeping records as rows in a caller-owned SQLite file. Catalogs rebuild from the canonical store.

All FileSquid metadata handling goes through this module, and hosts reach it through `filesquid-api`. The stand-alone interface is the metadata commands of the one FileSquid CLI, `fsquid` (`fsquid extract`, `get`, `set`, `export`, `import`, and `fsquid metadata validate`); this crate ships no binary with CSV, XMP and JSON sidecar interchange. MODS and SQL import/export interchange are out of scope (the optional SQLite store is storage, not interchange). The module MUST NOT depend on Axum or Tauri.

The Metadata sidebar panel is part of `@filesquid/ui` in `apps/filesquid/ui/`. Descriptive and technical file facts belong to this module; application meaning (collections, people, favorites, search indexes, recipes) belongs to FileSquid's application layer (C-055).

## Build and test

Run Cargo from the workspace root:

```bash
cd apps/filesquid
cargo test -p filesquid-metadata --locked
cargo run -p filesquid-cli -- metadata --help
```

This crate ships no binary. Its commands run in the one FileSquid CLI, `fsquid`, built by [`filesquid-cli`](../cli/README.md) ([ADR 036](../../../../resources/architecture/036-single-fsquid-cli.md)): `fsquid get`, or `fsquid metadata get`; `validate` always needs the module name (`fsquid metadata validate dcterms:title`).

## JSON sidecar

A JSON sidecar is one UTF-8 JSON document per OCFL object. It binds `{namespace, term, value}` statements to the OCFL object identifier (`objectId`) and the SHA-256 digest (`digestAlgorithm: "sha256"`, `digest`) of the described logical file. `--id` names the object and `--file` the logical file inside `--object`:

```bash
cd apps/filesquid
cargo run -p filesquid-cli -- export json harbour.json --id urn:example:harbour --file photo.jpg --object <object-dir>
cargo run -p filesquid-cli -- import json harbour.json --id urn:example:harbour --file photo.jpg --object <object-dir>
```

Import applies the `/terms/` register, rejects any namespace other than `dcterms`, `dc` (mapped to `dcterms`), `exif` and `schema`, and rejects a sidecar whose object identifier or digest does not match. A rejected import leaves `metadata.md` unchanged. The sidecar is interchange only; `metadata.md` stays canonical.

## References

- [Product vision: modules](../../../../resources/product-vision.md#modules)
- [PRD F-052: Metadata module](../../../../resources/prd.md#f-052-metadata-module)
- [ADR 024](../../../../resources/architecture/024-filesquid-metadata-engine.md) and [ADR 022](../../../../resources/architecture/022-dublin-core-descriptive-metadata.md)
- [Module SRS](./resources/srs.md) and [module plan](./resources/plan.md)
