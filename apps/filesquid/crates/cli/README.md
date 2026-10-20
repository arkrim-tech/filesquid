# filesquid-cli

Builds `fsquid`, the one FileSquid command-line tool ([ADR 036](../../../../resources/architecture/036-single-fsquid-cli.md), [C-057](../../../../resources/product-vision.md#module-rules)). Every FileSquid CLI command is invoked as `fsquid`; no module ships a binary of its own.

- `fsquid <command> [args]` runs a command that only one module defines, for example `fsquid get dcterms:title`.
- `fsquid <module> <command> [args]` always works, for example `fsquid metadata get dcterms:title`. `fsquid <module>` prints that module's help.
- `validate` always needs its module: `fsquid metadata validate dcterms:title`. `fsquid validate` exits with code 2 and names the grouped forms.

Modules today: `metadata` (from [`filesquid-metadata`](../metadata/README.md)). OCFL and C2PA commands join through their own tasks. Each module keeps its command logic in its library; this crate only routes. A test fails when two modules define the same command name that is not qualifier-only, when a module name equals a command name, or when another crate declares a CLI binary.

FileSquid hosts link the module libraries and MUST NOT shell out to `fsquid` for canonical I/O.

Every command is journaled ([ADR 036](../../../../resources/architecture/036-single-fsquid-cli.md) Decision 8, Task CLI-0.2). Before a command runs, `fsquid` opens `filesquid.sqlite` in `FILESQUID_DATA_DIR`, or else in the desktop app's data directory, through `filesquid-native`'s SQLite journal store in WAL mode, so it writes beside a running desktop app. A command that changes something or fails writes one redacted entry, `cli.<module>.<command>`, with its outcome, error code and the OCFL object identifier and version it committed; a command line that names no module command writes one failed `cli.route` entry; a command that changes nothing writes none, and help needs no journal. Exit codes: the module's own code, 2 for usage errors, 3 when the journal cannot be opened (one stdout line marked `"journal_unavailable": true`, and nothing runs), 4 when a command succeeded but its entry could not be written (the entry printed to stdout marked `"journal_write_failed": true`). The root README's `## View logs` shows how to read the entries.

Run Cargo from the workspace root:

```bash
cd apps/filesquid
cargo test -p filesquid-cli --locked
cargo run -p filesquid-cli --bin fsquid -- --help
```

## References

- [Plan](./resources/plan.md)
- [PRD F-050](../../../../resources/prd.md#f-050-filesquid-platform-and-distributions) and [SRS-F-050](../../../../resources/srs.md#srs-f-050-filesquid-platform-and-distributions)
- [PRD F-061](../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring), [SRS-F-061](../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring) and [API-S-031](../api/resources/srs.md) (the journaled commands)
