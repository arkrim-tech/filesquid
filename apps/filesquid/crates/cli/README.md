# filesquid-cli

Builds `fsquid`, the one FileSquid command-line tool ([ADR 036](../../../../resources/architecture/036-single-fsquid-cli.md), [C-057](../../../../resources/product-vision.md#module-rules)). Every FileSquid CLI command is invoked as `fsquid`; no module ships a binary of its own.

- `fsquid <command> [args]` runs a command that only one module defines, for example `fsquid get dcterms:title`.
- `fsquid <module> <command> [args]` always works, for example `fsquid metadata get dcterms:title`. `fsquid <module>` prints that module's help.
- `validate` always needs its module: `fsquid metadata validate dcterms:title`. `fsquid validate` exits with code 2 and names the grouped forms.

Modules today: `metadata` (from [`filesquid-metadata`](../metadata/README.md)). OCFL and C2PA commands join through their own tasks. Each module keeps its command logic in its library; this crate only routes. A test fails when two modules define the same command name that is not qualifier-only, when a module name equals a command name, or when another crate declares a CLI binary.

FileSquid hosts link the module libraries and MUST NOT shell out to `fsquid` for canonical I/O.

Run Cargo from the workspace root:

```bash
cd apps/filesquid
cargo test -p filesquid-cli --locked
cargo run -p filesquid-cli -- --help
```

## References

- [Plan](./resources/plan.md)
- [PRD F-050](../../../../resources/prd.md#f-050-filesquid-platform-and-distributions) and [SRS-F-050](../../../../resources/srs.md#srs-f-050-filesquid-platform-and-distributions)
