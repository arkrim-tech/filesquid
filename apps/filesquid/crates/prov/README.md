# filesquid-prov

The `prov` module of FileSquid: a W3C [PROV-O](https://www.w3.org/TR/prov-o/) history of OCFL versions and changes, written in the versioned JSON-LD profile `filesquid-prov/1`. Preservation programmes can use it without FileSquid.

| | |
|---|---|
| Cargo package | `filesquid-prov` |
| Rust import | `filesquid_prov` |
| Path | `apps/filesquid/crates/prov/` (member of the `apps/filesquid/` Cargo workspace) |
| Requirements | PROV-S-001 to PROV-S-005 of the module SRS, PRD F-067, ADR 040 |

The crate is stand-alone. Its dependencies are `serde`, `serde_json` and `thiserror`; it depends on no other FileSquid crate (not even `filesquid-core`), links no Axum, Tauri, SQLite, libvips or FFmpeg, does no filesystem or network I/O, and builds for `wasm32-unknown-unknown`. It has no Cargo features yet. An optional `ocfl` feature, off by default, will add `filesquid-ocfl` to read and write records in OCFL objects (Task PV-0.2). No feature may ever add `filesquid-native` or `filesquid-c2pa`; the isolation tests fail if one does.

## Model

One `Record` describes one committed operation on one OCFL object version: exactly one `prov:Activity`, its `prov:Agent`s and its `prov:Entity`s.

- **Activity** `urn:filesquid:activity:<operation-id>:<object-id>:<version>` with `fsq:operation`, `fsq:kind`, `fsq:objectVersion` (object ID and version), `prov:startedAtTime`, `prov:endedAtTime` (the inventory `created` time), `fsq:journalEntry` (opaque journal entry IDs), an optional `fsq:message` (the version message), and, for a record reconstructed after the fact, `fsq:derivation: "reconstructed"` with its own `fsq:recordedAt`.
- **Entity** `urn:filesquid:entity:<object-id>:<version>:<percent-encoded logical path>` with `fsq:objectId`, `fsq:version`, `fsq:path` and `fsq:sha256`. Percent-encoding keeps RFC 3986 unreserved characters (`A-Z a-z 0-9 - . _ ~`) and writes every other UTF-8 byte as upper-case `%XX`.
- **Agents.** The owner is the installation agent, a plain `prov:Agent` `urn:uuid:<installation-uuid>` (UUID v4, lower case). There is no `prov:Person`, person identity or display name anywhere. The FileSquid host is a `prov:SoftwareAgent` `urn:filesquid:software:<product>:<distribution>:<version>` that `prov:actedOnBehalfOf` the installation agent; it carries product, distribution and version and never a host name. A tool or model `prov:SoftwareAgent` (`urn:filesquid:tool:<name>:<version>`) is reserved: the profile reads and writes it, the builder never produces it. An agent the caller did not observe is stated as unrecorded (`urn:filesquid:agent:unrecorded:installation` or `:host`), never guessed.
- **Relations** `prov:used`, `prov:wasAssociatedWith` (on the activity), `prov:wasGeneratedBy`, `prov:wasDerivedFrom`, `prov:wasInvalidatedBy`, `prov:wasAttributedTo` (on entities) and `prov:actedOnBehalfOf` (on software agents).

Times are RFC 3339 in UTC (`Z`), with an optional fraction of 1 to 9 digits.

## Builder

`build_record(&VersionFacts)` turns the facts of one committed version into a record: object ID, version, base version, `created`, OCFL `user`, message, the base and new logical states (logical path to SHA-256), moves (old path to new path), operation ID and kind, started time, the installation UUID and host if observed, and the journal entry IDs.

| Change | Entities and relations |
|---|---|
| Added path | New state `wasGeneratedBy` the activity |
| Changed path | New state generated and `wasDerivedFrom` the previous state, which the activity `used` |
| Moved path | New state generated and derived from the old path's base state, which the activity `used` (not invalidated) |
| Removed path | Base state `wasInvalidatedBy` the activity |
| Unchanged path | No entity |

Generated states are `wasAttributedTo` the installation agent, and the activity `wasAssociatedWith` every agent. Without an installation UUID, an OCFL `user.address` of the form `urn:uuid:<uuid v4>` names the installation agent; the `user.name` is never written. Logical paths under `provenance/`, where the records themselves live, are left out. Refusals carry fixed codes: `PROV_INVALID_FACT` (naming the field), `PROV_INVALID_MOVE`, `PROV_TIME_ORDER`, `PROV_TOO_LARGE` and `PROV_INCONSISTENT_FACTS`.

## Profile `filesquid-prov/1`

`to_json_ld(&Record)` writes one compact UTF-8 JSON-LD document ending in a newline, with an embedded `@context` (`prov` = `http://www.w3.org/ns/prov#`, `xsd` = `http://www.w3.org/2001/XMLSchema#`, `fsq` = `https://filesquid.tech/ns/prov/v1#`), `fsq:profile: "filesquid-prov/1"` and a `@graph`. Output is deterministic: every node's members are written in a fixed order, and the graph lists the activity, then the agents and the entities, each sorted by identifier, so equal inputs give equal bytes.

`parse_record(&[u8])` reads every known profile version (only `/1` exists; `Profile` and one reader module per version leave room for `/2`) and refuses, with a fixed code whose message names no stored value:

| Code | Refused |
|---|---|
| `PROV_TOO_LARGE` | A record above 1 MiB (1,048,576 bytes) |
| `PROV_TOO_DEEP` | JSON nesting deeper than 16 levels |
| `PROV_DUPLICATE_KEY` | A member name repeated in one object (a strict reader on top of `serde_json`) |
| `PROV_UNKNOWN_PROFILE` | A missing or unknown `fsq:profile` |
| `PROV_INVALID_NODE` | A node that breaks the profile: wrong context, unknown member, forged identifier, dangling reference, more or fewer than one activity |
| `PROV_INVALID_JSON` | Malformed JSON or UTF-8 |

`parse_record(to_json_ld(&record).as_bytes()) == Ok(record)` for every record.

## History index

`HistoryIndex` keeps records in memory and answers `HistoryQuery::Object`, `Entity`, `Operation`, `Agent` and `TimeRange { from, to }` (from inclusive, to exclusive). Results are ordered by end time, then activity identifier, in pages of 1 to 1000 activities (`PageRequest::new`; other sizes give `PROV_PAGE_SIZE`). `Page::next` is the cursor of the next page; `Cursor::to_token` and `Cursor::from_token` carry it across a transport (`PROV_INVALID_CURSOR` for a token it did not make). Inserting a different record under an indexed activity identifier gives `PROV_CONFLICTING_RECORD`.

## Build and test

Run Cargo from the workspace root:

```bash
cd apps/filesquid
cargo test -p filesquid-prov --locked
cargo clippy -p filesquid-prov --all-targets --locked -- -D warnings
cargo check -p filesquid-prov --lib --target wasm32-unknown-unknown --locked
```

`cargo test` runs the unit tests under `tests/unit/` and the BDD features under `tests/features/` (JUnit in `tests/allure-results/cucumber-junit.xml`). The `@wasm32` scenario builds the crate for `wasm32-unknown-unknown` into `filesquid-prov-wasm32-check/` under the Cargo target directory; set `FILESQUID_SKIP_WASM32=1` to skip it explicitly. The pre-commit hook runs the same `wasm32` check whenever `filesquid-prov` is in the change scope.

## The `ocfl` feature

`filesquid-prov` with `--features ocfl` (off by default; adds `filesquid-ocfl` with its default features off, so it still builds for `wasm32-unknown-unknown`) keeps records in OCFL objects (PROV-S-006, ADR 040 Decisions 3 and 7):

- `ocfl::RecordHook` is the change model's version-bound file function: handed to `ChangeModel::commit_with`, it writes `provenance/<version>.jsonld` in the same commit, named after the reserved version and its inventory `created` time, and is called again after a rebase.
- `ocfl::read_record`, `list_records` and `missing_records` read records back through the engine.
- `ocfl::reconstruct` derives a record from an inventory alone for a version committed before the history existed (marked `reconstructed`, with its own `recordedAt`, inventing no agent); `stage_reconstructed` stages those records through the change model, at most a given number per run and none for versions that already have one, for the caller to commit in one version with the record of the run itself.

Nothing in the feature writes outside the change model.

```bash
cd apps/filesquid
cargo test -p filesquid-prov --all-features --locked
cargo check -p filesquid-prov --lib --features ocfl --target wasm32-unknown-unknown --locked
```
