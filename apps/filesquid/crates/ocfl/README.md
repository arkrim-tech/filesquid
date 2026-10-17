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

- Operations: `put`, `put_if` (compare-and-swap with `Precondition::IfAbsent` or `Precondition::IfMatch(etag)`), `get_range`, `head`, `exists`, paged `list`, `delete` (staging only), and multipart `create_multipart`, `upload_part`, `complete_multipart`, `abort_multipart`.
- The trait needs no executor. Its futures are `Send` on native targets and carry no `Send` bound on `wasm32`.
- Every transfer is bounded: ranged reads and multipart parts at most 8 MiB (`MAX_CHUNK_BYTES`), a single `put` at most 8 MiB (`MAX_PUT_BYTES`), at most 10 000 parts, listing pages of at most 1 000 keys. Use `storage::ChunkReader` to stream an object and `storage::MultipartWriter` to write a large one.
- Keys are `/`-separated relative paths; empty, absolute, `.`, `..`, backslash and NUL keys are rejected before any I/O.
- The shipped backends tag objects with the lowercase hex SHA-256 of their bytes.
- `FsBackend::new(root, staging)` keeps multipart staging outside the storage root and refuses a staging directory inside it. Replacing writes go through a flushed temporary file that is renamed into place.
- `write_logical_file`, `read_logical_file` and `reject_git_in_ocfl_root` (feature `fs`) keep their signatures for existing callers and now go through `FsBackend`.

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
