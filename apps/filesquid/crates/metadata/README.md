# filesquid-metadata

The metadata module of FileSquid. It extracts metadata from file bytes, creates, reads, updates and deletes namespaced statements, and creates and enhances Dublin Core and Schema.org records. Institutions can use it on its own.

| | |
|---|---|
| Cargo package | `filesquid-metadata` |
| Rust import | `filesquid_metadata` |
| Path | `apps/filesquid/crates/metadata/` (member of the `apps/filesquid/` Cargo workspace) |

Records persist through a pluggable `store::MetadataStore` that loads and saves a `Record` by an opaque `file_ref::FileRef` (object identifier, optional version, logical path; [ADR 027](../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md)). Two stores ship:

- `store::sidecar::SidecarStore` (the default `sidecar` feature, native only) keeps `<file>.metadata.md` beside a plain file under a root directory and refuses a Git working tree.
- `store::ocfl::OcflStore` (the optional `ocfl` feature) keeps `metadata.md` (YAML front matter + Markdown body) as a logical file in an OCFL object through [`filesquid-ocfl`](../ocfl/README.md), FileSquid's canonical store. Each edit is committed as exactly one new OCFL version through the `filesquid-ocfl` change model (`ChangeModel::stage_bytes`, then `commit`: the fenced `vN` reservation and head compare-and-swap); an unchanged record publishes nothing, earlier versions are never modified, and this crate builds no inventory and publishes no `vN` itself. A `FileRef` that pins a version reads that version. On save, an edit whose base head has moved is rebased onto the new head (OCFL-S-016), and only a concurrent change to the same `metadata.md` is reported as a `StoreError::Conflict`; nothing is overwritten. A head without its version inventory (W010) stays refused (`MissingVersionInventory`) and is never repaired here. `OcflStore::commit_record_with` takes `CommitOptions` (MD-S-014): the OCFL version `user`, the version message (FileSquid passes its `filesquid.operation.v1` message instead of `Edit metadata.md`) and version-bound files committed in the same version (such as the version's PROV-O record), and returns `Some(CommitReceipt)` for the caller's journal, or `None` when the record is unchanged and no version was written (journal nothing then); without options the stand-alone defaults apply. The host wires the store to its storage root, its shared version reservations, a staging backend outside the root, a clock and its object-root layout. With `ocfl-fs`, `store::ocfl::LocalObject` is the same store over one object directory on the local disk (leases and staging in `filesquid-metadata` under the system temporary directory), as the CLI uses it.

A caller, such as the browser host, can also implement `MetadataStore` itself. A native `sqlite` store is planned. Catalogs rebuild from the canonical store.

Extraction takes bytes or a reader plus a `FileRef`, never a path: `extract::extract_from_bytes` and `extract::extract_from_reader` read EXIF, ID3 and an embedded XMP packet (its `dc:` and `dcterms:` elements, mapped onto `dcterms:`) from what the caller supplies (for example a logical file read through `filesquid-ocfl`) and are independent of any store. The returned `Extracted` says which blocks were found and how many statements the register refused.

## File record and import record

FileSquid keeps one metadata record per file, `<file-uuid>.metadata.md`, beside the stored original `<file-uuid>.<ext>` in the file's OCFL object (the `.<ext>` suffix only for an extension of 1 to 16 ASCII letters or digits; otherwise the stored original is `<file-uuid>` with no suffix; [ADR 038](../../../../resources/architecture/038-import-file-record.md), MD-S-008, MD-S-013). `file_record::FileRecord` is that record as pure, `wasm32`-safe data: the owner's exact original file name (`raw::Raw`: UTF-8 as a string, any other bytes as base64 marked `encoding: base64`) with its Unicode normalisation form, the display name (initially the original name; a rename changes only this), the extension, the format and media type, the descriptive statements, and the append-only `import` section. The extension MUST be the original name's. The front matter opens with `schema: filesquid.original.v1` (so Schema.org statements are written under `schemaorg:` in a file record, and read from either key), then one JSON value per header line, the namespace maps, and last the `import` list with one JSON entry per line. `file_record::extension_of`, `stored_extension` and `record_name` are the naming rules.

Each `import::ImportEntry` is one acquisition: `Acquisition` (times, method, copied or referenced, job and operation IDs, the cited OCFL object, version, logical path, SHA-256 and size), `SourceFile` (name, paths, the size as the source reported it and as verified from the bytes read, times, owner and group, mode, ACL, flags, link, OS content type, extension), `SourceVolume`, `attributes::Attributes` and `Host`. The caller supplies every fact; the crate reads no filesystem. A fact the source or host could not provide is a `fact::Fact::Unavailable` with a `Reason`, never a guess. `ImportEntry::for_upload` builds the entry of a browser or API upload (the facts the client sent, the rest unavailable with the reason `not_provided_by_upload`). `ImportEntry::validate` rejects an entry whose source-reported size or whose cited acquisition size differs from the verified size (the bytes read and hashed), and any time that is not RFC 3339 UTC. `attributes::AttributeCollector` is the bookkeeping for a host that reads extended attributes: at most 256 names and 1 MiB of value bytes per entry, values over 64 KiB kept as size and SHA-256 only, and everything cut marked truncated (with the count enumerated) or its digest incomplete.

With the `ocfl` feature, name the record with `FileRef::with_record_path` (a `FileRef` without one keeps the record at `metadata.md`). `OcflStore::commit_import_with` commits the record over the content the host staged through `OcflStore::change_model`, so the record and the content land in one version; a failure leaves a retryable change set. `append_import_with` appends a re-import in a new version, and `commit_file_record_with` saves a whole record (a rename, for example). Every save is checked: it MUST keep the original name and every existing import entry (`StoreError::Record` with `RecordError::OriginalNameChanged`, `ImportEntryChanged` or `ImportEntryRemoved`, and nothing is published), every new entry MUST cite the object it is saved in; a managed (copied) entry also cites the logical path of the stored original (never the record's own path), and `commit_import_with` checks that the content staged there has the entry's SHA-256 (`append_import_with` checks the head's), and repeats the check inside the commit against the exact state the new version publishes, so content another writer replaced meanwhile is a `StoreError::Conflict` and nothing is published; a referenced entry cites no logical path and stages no content; and a statements-only `save` keeps the header and the import section. Inside the same commit check, every new entry's cited `object_version` MUST be the version being committed (ADR 038 Decision 10): an import or re-import whose base head moved would publish a later version, so it is not rebased but fails as a retryable `StoreError::Conflict` that publishes nothing, and the caller retries citing the new version. Statement edits still rebase onto a moved head (MD-S-008). `read_file_record` reads the whole record, at a pinned version or the head.

For the metadata routes, `FileRecord::facts()` is the read-only projection of API-S-027: names, extension, format, media type, size, SHA-256, import time and method, the path relative to the import root, the volume name and the OCFL object and version, with the display name after the one safe transformation of API-S-005 (`file_record::safe_display_name`), and never the absolute path, the import root, the host name or any account. `import::scrub::IdentityClass` and `ImportEntry::scrubbed` are the identity-field classes the outgoing disclosure policy of F-046 removes (paths, accounts, host name, numeric IDs, origin URLs, the quarantine record, Finder comments, and `OriginalName`, the original file name wherever the record holds it); each removed fact is listed as `withheld_by_policy`. `IdentityClass::ALL`, every class, is the default for shares and publications; a caller whose saved outgoing policy selects **Use original filename** leaves `OriginalName` out. `FileRecord::scrubbed(&classes)` is the outgoing copy of a whole record: every import entry scrubbed and, for `OriginalName`, the header's `original_name` and `display_name` written as `{"unavailable":"withheld_by_policy"}`. The copy renders and reads back (`names_withheld()` is true), but it is written only into exports and shares: the `ocfl` store refuses to save it (`RecordError::NamesWithheld`). `SourceFile::original_name` is a `Fact<Raw>` so an entry's source name can be withheld.

## Cargo features and WebAssembly

| Feature | Default | Adds |
|---|---|---|
| `sidecar` | yes | The `sidecar` store (native only; compiled out for `wasm32`) |
| `ocfl` | no | The `ocfl` store and the optional `filesquid-ocfl` dependency |
| `ocfl-fs` | no | `ocfl` plus the native path helpers over the local OCFL filesystem backend |
| `cli` | no | `ocfl-fs` plus the native `cli` module (the `fsquid` metadata commands) |

The crate builds for `wasm32-unknown-unknown` with default features off, alone or with `ocfl` (`kamadak-exif` and `id3` both compile there, so no adapter is gated). The CLI and the sidecar store are compiled out for `wasm32`. The ciphertext-only hosted service MUST NOT link this crate.

All FileSquid metadata handling goes through this module, and hosts reach it through `filesquid-api`. The stand-alone interface is the metadata commands of the one FileSquid CLI, `fsquid` (`fsquid extract`, `get`, `set`, `export`, `import`, `namespace`, and `fsquid metadata validate`); this crate ships no binary with CSV, XMP and JSON sidecar interchange. MODS and SQL import/export interchange are out of scope (the optional SQLite store is storage, not interchange). The module MUST NOT depend on Axum or Tauri.

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

## Commands and OCFL objects

`extract`, `get`, `set`, `export`, `import` and `namespace` work on an OCFL object directory (`--object`, default `.`); `help` needs no object, and `validate` needs one only to check its record or its registry. `get` and `export` only read the head. `set`, `extract` and `import` are edits: each commits one new OCFL version when the record changes (an unchanged record commits nothing) and prints the version. `--id` names the object: it is required to create an object in a directory that holds none, and an existing object's identifier must match it. `--file` names a logical file the object already holds (committed, for example, by an import through `filesquid-ocfl`); `extract` takes either a media file path or `--file`, not both.

```bash
cd apps/filesquid
# A directory without an object: the first edit creates it as v1.
cargo run -p filesquid-cli -- set dcterms:title Harbour --id urn:example:harbour --object <object-dir>
# An object that already holds the logical file photo.jpg:
cargo run -p filesquid-cli -- extract --file photo.jpg --object <object-dir>
```

## JSON sidecar

A JSON sidecar is one UTF-8 JSON document per OCFL object. It binds `{namespace, term, value}` statements to the OCFL object identifier (`objectId`) and the SHA-256 digest (`digestAlgorithm: "sha256"`, `digest`) of the described logical file. `--file` names a logical file the object already holds (here `photo.jpg`); its digest is read through `filesquid-ocfl` from the same version as the exported or edited record:

```bash
cd apps/filesquid
cargo run -p filesquid-cli -- export json harbour.json --file photo.jpg --object <object-dir>
cargo run -p filesquid-cli -- import json harbour.json --file photo.jpg --object <object-dir>
```

Import applies the `/terms/` register and the namespace registry: it accepts `dcterms`, `dc` (mapped to `dcterms`), `exif`, `schema` and registered extension prefixes, rejects any other prefix by name, and rejects a sidecar whose object identifier or digest does not match. A rejected import publishes no version. The sidecar is interchange only; `metadata.md` stays canonical.

## Namespace registry

`namespace::NamespaceRegistry` is the registry of extension namespaces (MD-S-012, [ADR 022](../../../../resources/architecture/022-dublin-core-descriptive-metadata.md)). An owner, a distribution or a module registers an extension namespace with `Registration::extension` (a prefix, an IRI and an optional term list); the owner declares a custom namespace of free keys with `Registration::custom`. The registry refuses:

- the built-in prefixes `dcterms`, `dc`, `schema` and `exif`, their IRIs under another prefix, and the record-format keys (`schemaorg`, `import`, `namespaces` and the file-record header keys);
- a prefix already registered with another IRI, an IRI already registered under another prefix, a change between extension and custom, and a re-registration that drops terms or the term list or gives an open namespace one (the same IRI may only add terms to a list);
- a prefix that is not 1 to 64 bytes of `[a-z][A-Za-z0-9_-]*` or starts with `xml`, an IRI without a scheme, over 2,048 bytes or holding a character RFC 3987 excludes, and a term that is not 1 to 255 bytes of `[A-Za-z_][A-Za-z0-9_.-]*`; at most 1,024 namespaces and 10,000 terms each.

An extension statement is `MetadataNamespace::Extension(prefix)`. `Record::set_registered` and `record::parse_curie_with` check it against the registry: an unregistered prefix is `MetadataError::UnregisteredNamespace` naming it, and a term outside the term list is `MetadataError::UnknownExtensionTerm`. `Record::set` and `parse_curie` take no registry, so every extension prefix is unregistered there. In `metadata.md` each extension prefix is its own YAML map after the built-in maps, and a record keeps the maps it holds when it is read. Extension statements round-trip through CSV (`csv::apply_csv_with`), JSON (`json::apply_json_with`) and XMP (`xmp::to_xmp_with`, which binds each prefix to its IRI, and `xmp::from_xmp_with`, which reads a namespace by its IRI under any packet prefix in one pass, and skips a packet prefix bound to two IRIs). They are never mapped onto `dcterms:`.

The registry persists as its own record, `namespaces.md` by default (front matter `schema: filesquid.namespaces.v1`, then one JSON registration per line), through `OcflStore::read_registry` and `OcflStore::commit_registry_with` at the logical path the caller names (`FileRef::with_record_path`). Each change is one OCFL version, and a save that drops a registration or changes its IRI is refused (`StoreError::Registry`), so a rebuilt index recovers every registration. No term list ships with the crate; a distribution registers the vocabulary it needs (RiC-O for Arkrim).

The CLI keeps the registry in `namespaces.md` of the object (or of the object `--registry` names). `set` and `import` resolve extension prefixes through it, `export xmp` binds each prefix to its registered IRI (CSV and JSON exports write prefixes as stored), and `validate` names every unregistered prefix:

```bash
cd apps/filesquid
cargo run -p filesquid-cli -- namespace register rico https://www.ica.org/standards/RiC/ontology# --terms title,hasCreator --id urn:example:harbour --object <object-dir>
cargo run -p filesquid-cli -- set rico:title "Fonds 12" --object <object-dir>
cargo run -p filesquid-cli -- metadata validate --object <object-dir>
cargo run -p filesquid-cli -- namespace list --object <object-dir>
```

## References

- [Product vision: modules](../../../../resources/product-vision.md#modules)
- [PRD F-052: Metadata module](../../../../resources/prd.md#f-052-metadata-module)
- [ADR 024](../../../../resources/architecture/024-filesquid-metadata-engine.md) and [ADR 022](../../../../resources/architecture/022-dublin-core-descriptive-metadata.md)
- [Module SRS](./resources/srs.md) and [module plan](./resources/plan.md)
