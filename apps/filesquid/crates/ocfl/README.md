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

Git MUST NOT be the OCFL store, and the storage hierarchy MUST stay plain files ([OCFL-S-001](./resources/srs.md), [ADR 016](../../../../resources/architecture/016-rust-ocfl-library.md)). The guards can be run against every backend and target, `wasm32` included. They check only when called: backend reads and writes do not run them. Callers MUST run them when they open a storage root or an object (object discovery and reads, IO-0.4) and before they publish a version (IO-0.6 onward).

**Scope (owner decision, 2026-10-18):** the guard covers the storage root, the storage-hierarchy directories between it and the object roots, and each object root. A `.git` inside an object's preserved content, such as an archived Git repository under `v1/content/`, is content the owner chose to keep, and the guard allows it.


- `check_storage_root(&backend)` runs the backend's hierarchy check, then pages through every key (at most `MAX_WALK_ENTRIES`, one million; a larger root is reported as over the limit, never passed). It refuses a `.git` entry in the storage root, in a storage-hierarchy directory above the objects, or in an object root (a directory holding an `0=ocfl_object_*` declaration that is not inside another object), and allows one inside object content.
- `reject_git_in_root(&backend, root)` checks one storage or object root for a `.git` file or any key under `.git/`.
- `StorageBackend::check_hierarchy(prefix)` refuses soft links (`StorageError::SymlinkForbidden`), hard links (`HardLinkForbidden`), empty directories below the root (`EmptyDirectoryForbidden`) and other non-regular entries. Object stores have neither links nor directories, so the default implementation only validates the prefix. `FsBackend` walks the directory under its root write lock, with the same bounds as listings. Hard links are detected where the standard library reports link counts (Unix, macOS included); on Windows they cannot be seen yet and pass.
- An empty `.git` directory holds no keys; on a file system it is refused as an empty directory.

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
