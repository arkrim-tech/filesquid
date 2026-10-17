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

## Build and test

Run Cargo from the workspace root:

```bash
cd apps/filesquid
cargo test -p filesquid-ocfl --locked
cargo clippy -p filesquid-ocfl --all-targets --locked -- -D warnings
```

## References

- [Product vision: modules](../../../../resources/product-vision.md#modules)
- [PRD F-051: OCFL module](../../../../resources/prd.md#f-051-ocfl-module)
- [ADR 016](../../../../resources/architecture/016-rust-ocfl-library.md) and [ADR 023](../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)
- [Module SRS](./resources/srs.md) and [module plan](./resources/plan.md)
