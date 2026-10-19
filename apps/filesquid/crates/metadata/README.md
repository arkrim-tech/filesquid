# filesquid-metadata

The metadata module of FileSquid. It extracts metadata from file bytes, creates, reads, updates and deletes namespaced statements, and creates and enhances Dublin Core and Schema.org records. Institutions can use it on its own.

| | |
|---|---|
| Cargo package | `filesquid-metadata` |
| Rust import | `filesquid_metadata` |
| Path | `apps/filesquid/crates/metadata/` (member of the `apps/filesquid/` Cargo workspace) |

Records persist through a pluggable `store::MetadataStore` that loads and saves a `Record` by an opaque `file_ref::FileRef` (object identifier, optional version, logical path; [ADR 027](../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md)). Two stores ship:

- `store::sidecar::SidecarStore` (the default `sidecar` feature, native only) keeps `<file>.metadata.md` beside a plain file under a root directory and refuses a Git working tree.
- `store::ocfl::OcflStore` (the optional `ocfl` feature) keeps `metadata.md` (YAML front matter + Markdown body) as a logical file in an OCFL object through [`filesquid-ocfl`](../ocfl/README.md), FileSquid's canonical store. Each edit is committed as exactly one new OCFL version through the `filesquid-ocfl` change model (`ChangeModel::stage_bytes`, then `commit`: the fenced `vN` reservation and head compare-and-swap); an unchanged record publishes nothing, earlier versions are never modified, and this crate builds no inventory and publishes no `vN` itself. A `FileRef` that pins a version reads that version; saving from a pinned version that is no longer the head is a `StoreError::Conflict`. A head without its version inventory (W010) stays refused (`MissingVersionInventory`) and is never repaired here. The host wires the store to its storage root, its shared version reservations, a staging backend outside the root, a clock and its object-root layout. With `ocfl-fs`, `store::ocfl::LocalObject` is the same store over one object directory on the local disk (leases and staging in `filesquid-metadata` under the system temporary directory), as the CLI uses it.

A caller, such as the browser host, can also implement `MetadataStore` itself. A native `sqlite` store is planned. Catalogs rebuild from the canonical store.

Extraction takes bytes or a reader plus a `FileRef`, never a path: `extract::extract_from_bytes` and `extract::extract_from_reader` read EXIF, ID3 and an embedded XMP packet (its `dc:` and `dcterms:` elements, mapped onto `dcterms:`) from what the caller supplies (for example a logical file read through `filesquid-ocfl`) and are independent of any store. The returned `Extracted` says which blocks were found and how many statements the register refused.

## Cargo features and WebAssembly

| Feature | Default | Adds |
|---|---|---|
| `sidecar` | yes | The `sidecar` store (native only; compiled out for `wasm32`) |
| `ocfl` | no | The `ocfl` store and the optional `filesquid-ocfl` dependency |
| `ocfl-fs` | no | `ocfl` plus the native path helpers over the local OCFL filesystem backend |
| `cli` | no | `ocfl-fs` plus the native `cli` module (the `fsquid` metadata commands) |

The crate builds for `wasm32-unknown-unknown` with default features off, alone or with `ocfl` (`kamadak-exif` and `id3` both compile there, so no adapter is gated). The CLI and the sidecar store are compiled out for `wasm32`. The ciphertext-only hosted service MUST NOT link this crate.

All FileSquid metadata handling goes through this module, and hosts reach it through `filesquid-api`. The stand-alone interface is the metadata commands of the one FileSquid CLI, `fsquid` (`fsquid extract`, `get`, `set`, `export`, `import`, and `fsquid metadata validate`); this crate ships no binary with CSV, XMP and JSON sidecar interchange. MODS and SQL import/export interchange are out of scope (the optional SQLite store is storage, not interchange). The module MUST NOT depend on Axum or Tauri.

The Metadata sidebar panel is part of `@filesquid/ui` in `apps/filesquid/ui/`. Descriptive and technical file facts belong to this module; application meaning (collections, people, favorites, search indexes, recipes) belongs to FileSquid's application layer (C-055).

## Build and test

Run Cargo from the workspace root:

```bash
cd apps/filesquid
cargo test -p filesquid-metadata --locked
cargo test -p filesquid-metadata --all-features --locked
cargo check -p filesquid-metadata --lib --no-default-features --target wasm32-unknown-unknown --locked
cargo run -p filesquid-cli -- metadata --help
```

The default test run skips the tests that need the `ocfl`, `ocfl-fs` or `cli` features; `--all-features` runs them, and the BDD feature (`--test features`) needs `cli`. The pre-commit hook runs the all-features tests and both `wasm32` checks whenever `filesquid-metadata` is in the change scope. This crate ships no binary. Its commands run in the one FileSquid CLI, `fsquid`, built by [`filesquid-cli`](../cli/README.md) ([ADR 036](../../../../resources/architecture/036-single-fsquid-cli.md)): `fsquid get`, or `fsquid metadata get`; `validate` always needs the module name (`fsquid metadata validate dcterms:title`).

## JSON sidecar

Every command works on an OCFL object directory (`--object`, default `.`) and commits each edit as a new OCFL version. `--id` names the object: it is required to create an object in a directory that holds none, and an existing object's identifier must match it.

```bash
cd apps/filesquid
cargo run -p filesquid-cli -- set dcterms:title Harbour --id urn:example:harbour --object <object-dir>
cargo run -p filesquid-cli -- extract --file photo.jpg --object <object-dir>
```

A JSON sidecar is one UTF-8 JSON document per OCFL object. It binds `{namespace, term, value}` statements to the OCFL object identifier (`objectId`) and the SHA-256 digest (`digestAlgorithm: "sha256"`, `digest`) of the described logical file. `--file` names the logical file of the object's head, read through `filesquid-ocfl`:

```bash
cd apps/filesquid
cargo run -p filesquid-cli -- export json harbour.json --file photo.jpg --object <object-dir>
cargo run -p filesquid-cli -- import json harbour.json --file photo.jpg --object <object-dir>
```

Import applies the `/terms/` register, rejects any namespace other than `dcterms`, `dc` (mapped to `dcterms`), `exif` and `schema`, and rejects a sidecar whose object identifier or digest does not match. A rejected import publishes no version. The sidecar is interchange only; `metadata.md` stays canonical.

## References

- [Product vision: modules](../../../../resources/product-vision.md#modules)
- [PRD F-052: Metadata module](../../../../resources/prd.md#f-052-metadata-module)
- [ADR 024](../../../../resources/architecture/024-filesquid-metadata-engine.md) and [ADR 022](../../../../resources/architecture/022-dublin-core-descriptive-metadata.md)
- [Module SRS](./resources/srs.md) and [module plan](./resources/plan.md)
