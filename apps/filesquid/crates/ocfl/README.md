# filesquid-ocfl

The OCFL module of FileSquid. It is a FOSS [OCFL 1.1](https://ocfl.io/1.1/spec/) library with a generic API; its commands are planned for the one FileSquid CLI, `fsquid` (Task IO-D.4), and it has no FileSquid or ImageSquid types. Digital preservation programmes can use the library on its own.

| | |
|---|---|
| Cargo package | `filesquid-ocfl` |
| Rust import | `filesquid_ocfl` |
| Path | `apps/filesquid/crates/ocfl/` (member of the `apps/filesquid/` Cargo workspace) |

FileSquid hosts reach this module through `filesquid-api`. The module itself MUST NOT depend on Axum, Tauri or another FileSquid module by default, and it MUST keep building for `wasm32-unknown-unknown` so the roadmap E2EE browser host stays reachable. Once Task IO-D.4 lands, independent programmes install the `fsquid` binaries from GitHub Releases. FileSquid MUST NOT shell out to `fsquid` for canonical OCFL I/O.

The crate stays independently publishable. It moves to its own repository only as part of extracting `apps/filesquid/` as a whole.

The Drive-style file browser that shows this module's storage facts (paths, versions, agents, digests, sizes) is part of `@filesquid/ui` in `apps/filesquid/ui/`.

## Storage backends

The engine reaches bytes only through the async, runtime-agnostic `storage::StorageBackend` trait ([OCFL-S-007](./resources/srs.md), [ADR 025](../../../../resources/architecture/025-client-side-filesquid-engines.md)). The host picks the backend; OCFL rules do not change.

| Backend | Targets | Feature |
|---|---|---|
| `storage::MemoryBackend` | every target, `wasm32` included | always on (for tests) |
| `storage::FsBackend` | native only | `fs` (default on) |
| S3-compatible | native and `wasm32` | planned (IO-5.1) |

- Operations: `put`, `put_if` (compare-and-swap with `Precondition::IfAbsent` or `Precondition::IfMatch(etag)`), `get_range`, `head`, `exists`, paged `list`, `delete`, and multipart `create_multipart`, `upload_part`, `complete_multipart`, `abort_multipart`. `delete` removes any key it is given; it is intended for staging and recovery, and the engine never uses it on published OCFL versions.
- The trait needs no executor. Its futures are `Send` on native targets and carry no `Send` bound on `wasm32`. `storage::complete_immediately` drives the futures of the in-memory and filesystem backends, which never wait, from synchronous code.
- Every transfer is bounded: ranged reads at most 8 MiB (`MAX_CHUNK_BYTES`), a single `put` at most 8 MiB (`MAX_PUT_BYTES`), multipart parts at most 64 MiB (`MAX_PART_BYTES`), at most 10 000 parts, so objects of at most 625 GiB (`MAX_OBJECT_BYTES`), and listing pages of at most 1 000 keys. Use `storage::ChunkReader` to read an object chunk by chunk and `storage::MultipartWriter` to write a large one part by part.
- Keys are `/`-separated relative paths; empty, absolute, `.`, `..`, backslash and NUL keys are rejected before any I/O.
- The shipped backends tag objects with the lowercase hex SHA-256 of their bytes.
- `storage::write_whole` writes a byte buffer of any size (single put, or multipart above 8 MiB, aborting a failed upload) and `storage::read_verified` reads one object back as a single version: the read is bracketed by two `head` calls and accepted only when both return the same entity tag (opaque tags such as S3 `ETag`s work), and backends that report `content_addressed_tags()` (the shipped ones) must also return bytes that hash to the tag; otherwise it retries up to 3 times. They suit small objects such as sidecars; stream large content instead.
- `storage::default_staging_dir(root)` picks `filesquid-ocfl-staging` in the system temporary directory, or the sibling `<root>.fsquid-staging` when the temporary directory lies inside the root (for example a root of `/tmp`); `FsBackend::with_default_staging(root)` uses it.

`FsBackend::new(root, staging)`:

- Resolves both directories (symbolic links, `.` and `..`) and refuses a staging directory that resolves to the root or inside it.
- Writes through a flushed temporary file that is renamed into place. Temporary files use the reserved `.fsquid-` name prefix; keys may not use it and listings skip it. Listings return regular files only, never links, FIFOs or other special files.
- Serialises every mutation of published objects (`put`, `put_if`, `delete`, `complete_multipart`) with an operating-system lock on a file in the staging directory, named after the resolved root. All writers to one root, in any process, MUST share one staging directory. Multipart staging (`create_multipart`, `upload_part`, `abort_multipart`) does not take this lock: each upload has its own directory under `staging/uploads/<sha256 of the resolved root>/`, so an upload ID from one root is unknown to every other root sharing the staging directory, parts are only written into an existing upload directory, and completing or aborting first renames that directory (an atomic step), so a late part can never reopen a closed upload. Every internal path (staging, upload and lock directories, key records, parts, lock files, temporary files) and every object file is opened or created without following links, as a regular file only, with the open handle checked against the path (new files are created with `create_new`, which never follows or reuses an existing entry). Internal records are size-capped, and `create_multipart` refuses a key longer than that cap (64 KiB). `Precondition::IfAbsent` publishes with a hard link, which the operating system refuses when the target exists, so two creators never both win even without the shared lock.
- Refuses a key whose path below the root passes through an existing symbolic link. On Windows it also refuses drive and stream names (`:`), reserved characters, device names such as `CON` or `COM1`, and names ending in a dot or space. The checks run before each operation, so the root MUST NOT be writable by untrusted processes.

Path-based helpers (feature `fs`) keep their signatures for existing callers:

- `write_logical_file` and `read_logical_file` go through `FsBackend`. Reads use `read_verified`, so a file replaced mid-read is read again and never returned as a mix of two versions.
- `reject_git_in_ocfl_root` checks for a `.git` entry (directory, file or link, without following links) directly on the filesystem; it does not use a storage backend.

## Git guard

Git MUST NOT be the OCFL store, and the storage hierarchy MUST stay plain files ([OCFL-S-001](./resources/srs.md), [ADR 016](../../../../resources/architecture/016-rust-ocfl-library.md)). The guards can be run against every backend and target, `wasm32` included. They check only when called: backend reads and writes do not run them. `read::StorageRoot::open` and `read::Object::open` run them on every open (IO-0.4); other callers MUST run them when they open a storage root or an object, and before they publish a version (IO-0.6 onward).

**Scope (owner decision, 2026-10-18):** the guard covers the storage root, the storage-hierarchy directories between it and the object roots, and each object root. A `.git` inside an object's preserved content, such as an archived Git repository under `v1/content/`, is content the owner chose to keep, and the guard allows it. It may hold only ordinary files and non-empty directories: soft links, hard links and empty directories inside it are refused like anywhere else in the storage hierarchy.


- `check_storage_root(&backend)` runs the backend's hierarchy check, then pages through every key (at most `MAX_WALK_ENTRIES`, one million; a larger root is reported as over the limit, never passed). Each continuation must be the last key of its page and move forward; a backend that skips keys, sends an empty page with a continuation or stalls is refused, so no `.git` can be skipped. It refuses a `.git` entry in the storage root, in a storage-hierarchy directory above the objects, or in an object root (a directory holding an `0=ocfl_object_*` declaration that is not inside another object), and allows one inside object content (ordinary files and non-empty directories only, as above).
- `reject_git_in_root(&backend, root)` checks one storage or object root for a `.git` file or any key under `.git/`.
- `StorageBackend::check_hierarchy(prefix)` refuses soft links (`StorageError::SymlinkForbidden`), hard links (`HardLinkForbidden`), empty directories below the root (`EmptyDirectoryForbidden`) and other non-regular entries. Object stores have neither links nor directories, so the default implementation only validates the prefix. `FsBackend` walks the directory under its root write lock, with the same bounds as listings. Hard links are detected where the standard library reports link counts (Unix, macOS included); on Windows they cannot be seen yet and pass.
- An empty `.git` directory holds no keys; on a file system it is refused as an empty directory.

## OCFL file formats

Pure parsing and writing, available on every target, `wasm32` included ([OCFL-S-002](./resources/srs.md), [ADR 009](../../../../resources/architecture/009-cryptography-webcrypto-compatible-suite.md)):

- `digest::DigestAlgorithm`: `sha256` is the engine's algorithm (`DigestAlgorithm::ENGINE`), so objects it writes use SHA-256 and validators report OCFL's SHOULD-use-`sha512` warning (W004) for them. `sha512` is read, because OCFL 1.1 allows both and most existing objects use it. `hasher()` digests content in bounded chunks. `matches` compares two digests of the algorithm's length and hexadecimal digits without regard to case, as a sidecar check needs; inside an inventory a state digest must match its manifest key exactly (E050).
- `inventory::Inventory::parse(bytes)` reads one untrusted `inventory.json`. It refuses input above 64 MiB (`MAX_INVENTORY_BYTES`) before parsing ([OCFL-S-017](./resources/srs.md); these engine bounds carry no OCFL code), JSON nested deeper than 8 levels (`MAX_JSON_DEPTH`) while parsing, and any JSON object that repeats a key. It then checks every rule one inventory can decide and names the OCFL 1.1 validation code of the first one broken (`FormatError::code`): required keys (E036, E041, E043, E048), no other keys (E102), `type` (E038), `digestAlgorithm` (E025), `contentDirectory` (E017, E018, E108), version names, padding and sequence (E008 to E012, E104, E105), `head` (E040), `created` as RFC 3339, with a leap second only at 23:59 UTC (E049), `message` and `user` (E054, E094), manifest digests and content paths (E030, E031, E039, E092, E096, E099 to E101, E106, E107), state digests and logical paths (E050, E052, E053, E095) and fixity (E057, E097, E111). Digests keep the case they were written in, and a state digest must match its manifest key exactly. `to_json()` writes an inventory with sorted keys that parses back to the same value.
- `inventory::VersionNum`: `v1`, `v2`, ... or zero-padded `v001`, ...; OCFL sets no digit limit, so the number is a validated decimal string and any valid name parses. `next()` returns `None` rather than overflow a padded width; otherwise it returns a name `parse` accepts.
- Sidecars: `sidecar_name`, `sidecar_contents` (`DIGEST inventory.json` and a newline), `verify_sidecar` (refuses input above 4 KiB, `MAX_SIDECAR_BYTES`; E061 malformed, E060 wrong digest) and `check_sidecar_name` (E059).
- `namaste::Declaration`: `0=ocfl_object_1.1` and `0=ocfl_1.1`, with their exact contents (E007, E080).

Object-level validation (files against the manifest, version directories, earlier version inventories) is the `validate` module, built on these types ([Structural validation](#structural-validation)). `tests/integration/fixture_inventories.rs` checks the root inventories, sidecars and declarations of the [pinned OCFL 1.1 community fixtures](#pinned-ocfl-community-fixtures).

## Read and browse

The `read` module opens storage roots and objects through any storage backend, on every target, `wasm32` included ([OCFL-S-004](./resources/srs.md), [ADR 016](../../../../resources/architecture/016-rust-ocfl-library.md), [ADR 023](../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)):

- `StorageRoot::open(&backend)` runs the storage-root guard, then checks the `0=ocfl_1.1` declaration (E069, E080; another version, such as `0=ocfl_2.0`, is `ReadError::UnsupportedVersion`; a root with more than 16 `0=` files is E069, and the scan keeps no more than 16 of their names) and, from the guard's one paged listing (at most `MAX_WALK_ENTRIES` keys), discovers the outermost object roots: directories with an `0=ocfl_object_*` declaration that are not inside another object. `objects()` lists them; `open_object(root)` opens one.
- `Object::open(&backend, root)` runs the Git guard and hierarchy check on the object root, requires exactly one object declaration among the files directly in the root, with exact contents (E003, E007), and refuses a root declared as another OCFL version (`ReadError::UnsupportedVersion`; OCFL 1.0 objects are out of scope, OCFL-S-011), reads the root inventory (E063 when missing; refused above 64 MiB before it is read) and its sidecar (E058, E060, E061; refused above 4 KiB before it is read), and requires the head version's inventory, when present, to be the same file (E064). A sidecar mismatch is read again, at most 3 times (`READ_ATTEMPTS`), in case a writer replaced the inventory between the two reads. Declarations above 64 bytes (`MAX_DECLARATION_BYTES`) are refused before they are read. Each of these small files is read as one consistent version and its size bound is checked on every attempt before any byte is read, so a file that grows during the read is never buffered past its bound. A head version inventory that disappears during the read counts as missing.
- Only the root inventory commits versions (ADR 016 "Published head (readers)"): content or a version inventory for a `vN` above the root `head` is unpublished, and pinning it is `ReadError::VersionNotFound`. A root inventory whose sidecar still holds the digest of the previous version's inventory (a writer is publishing, or crashed before writing the root sidecar) is not committed either: the object opens at that previous version and `Object::unpublished_head()` names the claimed head. Any other sidecar mismatch is E060.
- `Object::state(&version)` (or the pure `read::resolve_state(&inventory, &version)`) resolves the logical state of a version the caller pins through `state` and `manifest` only. A file whose bytes were stored in an earlier version resolves to that earlier content path; files the inventory does not list are never part of the state. Each `LogicalFile` has its logical path, digest and content path; the state has the object id and version.
- `Object::stream_file(&version, path, chunk_bytes)` returns a `LogicalFileReader` that reads in chunks of at most 8 MiB and checks the digest when the stream ends: bytes are unverified until `next_chunk()` returns `None`, and a mismatch or missing content file is E092. `Object::copy_file(&version, path, &dest, &key, precondition)` streams into a multipart upload on any backend and publishes it only when the digest matched; otherwise the upload is aborted, no new object is published at `key`, and any object already there stays unchanged.
- `Object::history()` lists every version with `created`, `message` and `user`; `Object::diff(&from, &to)` lists added, deleted, modified and renamed paths (a rename is a deleted and an added path with the same digest), in path order.
- Reading is not validation: a reader refuses only the faults it meets. Structural validation is the `validate` module.

`tests/integration/fixture_reads.rs` resolves every version of every `good-objects` and `warn-objects` fixture and streams every logical file against its digest, and checks that a reader refuses the bad fixtures whose faults it meets, with the code their names list ([OCFL-S-010](./resources/srs.md) (a)), on the [pinned fixtures](#pinned-ocfl-community-fixtures).

## Structural validation

`validate::validate_object(&backend, root)` validates one OCFL 1.1 object through any storage backend, on every target ([OCFL-S-009](./resources/srs.md)). It returns a `ValidationReport`: each broken rule it reaches is a `Finding` with its OCFL 1.1 validation code (`E###` errors, `W###` warnings), the storage key (and JSON pointer, for an inventory value) where it is, and a message. `is_valid()` is true when there are no errors; warnings never make an object invalid. The function returns an error only when the object cannot be validated: a backend failure, a listing above `MAX_WALK_ENTRIES` keys, a file above the engine's parsing bounds ([OCFL-S-017](./resources/srs.md); no OCFL code), or an object declared as another OCFL version than 1.1.

It checks, in order:

- the hierarchy under the object root: an empty directory is E024 (validation goes on), a soft or hard link E090 (validation stops there, since the listing cannot be trusted);
- the declaration (E003, E007), the root inventory (E063 when missing, every code `Inventory::parse` gives, E038 for a type other than 1.1) and its sidecar (E058, E060, E061);
- the object root's entries: only the declaration, the inventory and its sidecar, version directories, `logs` and `extensions` (E001); a version directory the root inventory does not list is E046, even with an inventory and sidecar of its own, because only the root inventory commits a version; a listed version without a directory is E010; `extensions` holds only directories (E067), each SHOULD be a registered extension name (W013);
- inventory warnings: zero-padded versions (W001), `sha256` (W004), an id that is not a URI (W005), a version without `message` or `user` (W007), a user without an address (W008) or with one that is not a URI (W009);
- each version directory: only its inventory, its sidecar and the content directory (E015; another directory is W002 and ignored). When the version inventory parses, only the sidecar for its `digestAlgorithm` is allowed; a sidecar without an inventory is E015; a malformed inventory reports its own error and allows any recognised (`sha256`, `sha512`) sidecar; a missing inventory is W010; a version inventory must be valid with its own sidecar, name its own version as `head` (E040), keep the id (E037) and content directory (E019), not go back to an older specification (E103), equal the root inventory for the head version (E064), describe every earlier version's state as the root inventory does (E066; `created`, `message` or `user` differences are W011), keep every manifest path inside a content directory (E042) and list every content file of its versions (E023);
- content: every file in a content directory is in the manifest (E023), every manifest path is inside a version's content directory (E042), and every file is streamed once, in chunks of at most 8 MiB, and checked against every digest stated for it: the root manifest and the manifests of earlier version inventories, whatever their algorithm (E092), and the root fixity block (E093). A content path that does not exist is E092 (and E093 when the fixity block lists it).

Fixity values are checked in every algorithm of the OCFL 1.1 fixity table (`fixity::FixityAlgorithm`: `md5`, `sha1`, `sha256`, `sha512`, `blake2b-512`, E027); values in other algorithms are ignored (E028). md5 and sha1 are only read: the engine writes `sha256`.

One inventory is checked to its first broken rule, so an object whose inventory breaks several rules reports the first; the [fixtures README](./tests/fixtures/ocfl/README.md) allows this. Storage-root validation (E069 to E090 across a whole storage root) is not part of this module yet; `StorageRoot::open` checks the declaration and guards.

## Write

The `write` module publishes new versions through any storage backend, on every target ([OCFL-S-018](./resources/srs.md), [ADR 016](../../../../resources/architecture/016-rust-ocfl-library.md) "Write (forward delta)", [ADR 007](../../../../resources/architecture/007-durable-jobs-and-interactive-performance.md), [ADR 012](../../../../resources/architecture/012-s3-compatible-ciphertext-storage.md)). It is the engine's only write path.

- A `VersionDraft` names the next version's whole logical state: each path is staged bytes on the caller's staging backend (outside the storage root) or a digest the manifest already holds. Paths left out are deleted from the new state; their bytes stay.
- `ObjectWriter::new(&storage, &reservations, holder).commit(root, expected_head, &staging, &draft)` first assembles the version without writing: it hashes the staged bytes (sha256), builds the new inventory and refuses an invalid draft (`created` not RFC 3339 UTC, an unknown digest, another object's id, a path OCFL forbids) or a base that is not sha256. It then reserves `vN`, re-reads the committed head (`CommitError::HeadMoved` when it is not `expected_head`), runs the Git and hierarchy guards, and writes in this order: the declaration (new objects), each new bitstream under `vN/<contentDirectory>/` (`content` unless the base inventory names another; only digests the manifest lacks, once each), the version inventory and its sidecar, then the root inventory (compare-and-swap on the base) and the root sidecar last (compare-and-swap on the sidecar read before the root inventory was replaced). A base whose committed head has no version inventory of its own is refused (`CommitError::MissingVersionInventory`), because readers need that file to read past an interrupted publication. Content is streamed in chunks of at most 8 MiB into parts of at most 64 MiB and re-checked against the assembled digest (`CommitError::StagingChanged`). Nothing under `vN/` is overwritten (`IfAbsent`).
- Every write is preceded by a fence check (`VersionReservations::confirm`). A writer whose reservation expired, was released or passed to another writer gets `ReservationError::Lost` at its next write: it writes no more content and never publishes the head.
- `LeaseStore::new(backend, prefix, clock, LeaseConfig { ttl_ms, margin_ms })` keeps one lease record per object in any backend with compare-and-swap, outside the storage root and shared by every writer of that root (for a local root, an `FsBackend` over a directory such as the staging directory). Each new holder gets a larger fencing token; a confirm renews the lease once less than half its TTL is left and fails once no more than the margin is left (defaults 60 s and 10 s). A host can implement `VersionReservations` over a PostgreSQL generation instead. `wasm32` callers pass their own `Clock`; native callers can use `SystemClock`.
- A crash while publishing `vN` of an existing object leaves `vN` unpublished: readers keep seeing `v(N-1)`. Until it is recovered, the validator reports what the crash left: the unlisted `vN` directory (E046) when the root inventory was not yet replaced, or the root sidecar that does not match the root inventory (E060) when it was. A crash during the first commit leaves no committed version: opening the object is E058 (or E063 before the root inventory is written), and only a commit for the same object id can recover it. After the crashed writer's lease lapses, the next commit of that version restores a root inventory that was never committed, keeps leftover content whose bytes match, deletes the rest of `vN/`, and publishes `vN` once.
- `CommitReceipt` (object id, version, `created`, inventory digest, content paths written) is what the caller records in its own operational store; the engine owns no database.

Fencing is checked by the writer, not enforced by the storage: a writer that stalls for longer than the margin between a fence check and its next write could still make that one write. It cannot replace another writer's content, inventories or head (writes under `vN/` never overwrite, and the root inventory and root sidecar are compare-and-swaps), but a file it adds under `vN/` after the new holder's last listing check would be published as an extra file that the validator reports (E023), and a stalled recovering writer could delete a file the new holder already verified. Choose a margin well above the longest single write. The owner accepted this gap on 2026-10-18 only while a library cannot be written by more than one host process or by more than one user ([OCFL-S-018](./resources/srs.md) "Known gap"; background in the [IO-0.6 Walkthrough](./resources/tasks/Task-IO-0.6-Walkthrough.md#remaining-limits)). Before that changes, a storage-side check that refuses a stale fencing token closes it: the S3-compatible backend (IO-5.1) and the multi-user server (SV-F1.2) carry it as an acceptance criterion. Until then, every validator error, E023 included, reaches the host's health status and the Monitor ([API-S-019](../api/resources/srs.md)).

A base whose committed head has no version inventory (W010) stays refused. A one-time repair, run only after the owner consents and recorded in the activity journal, writes a copy of the root inventory and its sidecar into the head version directory so the object becomes editable ([OCFL-S-019](./resources/srs.md), [API-S-020](../api/resources/srs.md); planned as IO-1.4 in the [module plan](./resources/plan.md)).

## Pinned OCFL community fixtures

The `1.1/` tree of the [OCFL community fixtures](https://github.com/OCFL/fixtures) is committed unchanged under `tests/fixtures/ocfl/1.1/`, pinned to commit `d323fc207126becec066998b43408d998bf0fb1b` (2024-12-05T17:25:06Z), with the upstream README, its CC-BY-4.0 licence record (`.zenodo.json`) and a [NOTICE](./tests/fixtures/ocfl/NOTICE.md) ([OCFL-S-011](./resources/srs.md)). They are test data: the package `exclude`s them and no library code loads them. Moving the pin replaces the whole tree in its own reviewed change.

`cargo test -p filesquid-ocfl --locked` runs them offline, so the local pre-commit hook does too:

- `integration_fixture_conformance` validates every object: all 12 `good-objects` and all 13 `warn-objects` with zero errors (each warning fixture also reports the warning codes its name starts with), and all 55 `bad-objects` are rejected. A code a bad fixture's name lists but the validator does not report is allowed only when it is in the test's `EXPECTED_GAPS` list; the [IO-0.5 Walkthrough](./resources/tasks/Task-IO-0.5-Walkthrough.md) explains each one.
- `integration_fixture_inventories` and `integration_fixture_reads` check root inventories and reads ([Read and browse](#read-and-browse)); the BDD feature `OCFL_1.1_fixture_conformance.feature` covers validation, reads and the pin.

To see one line per fixture:

```bash
cd apps/filesquid
cargo test -p filesquid-ocfl --locked --test integration_fixture_conformance -- --nocapture --test-threads=1
```

`FILESQUID_OCFL_FIXTURES` may name another copy of a `1.1` tree, for example an `OCFL/fixtures` checkout at a new commit before the pin moves (in PowerShell, `$env:FILESQUID_OCFL_FIXTURES = "C:\path\to\fixtures\1.1"` first):

```bash
cd apps/filesquid
FILESQUID_OCFL_FIXTURES=/path/to/fixtures/1.1 cargo test -p filesquid-ocfl --locked --test integration_fixture_conformance
```

## Build and test

Run Cargo from the workspace root:

```bash
cd apps/filesquid
cargo test -p filesquid-ocfl --locked
cargo clippy -p filesquid-ocfl --all-targets --locked -- -D warnings
cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown
```

The `wasm32` build needs `rustup target add wasm32-unknown-unknown` once. `cargo test` also runs that build (the "Engine builds for the browser" feature scenario), so the local pre-commit gate fails if the crate stops building for the browser.

## References

- [Product vision: modules](../../../../resources/product-vision.md#modules)
- [PRD F-051: OCFL module](../../../../resources/prd.md#f-051-ocfl-module)
- [ADR 016](../../../../resources/architecture/016-rust-ocfl-library.md) and [ADR 023](../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)
- [Module SRS](./resources/srs.md) and [module plan](./resources/plan.md)
- [OCFL 1.1 validation codes](https://ocfl.io/1.1/spec/validation-codes.html)
