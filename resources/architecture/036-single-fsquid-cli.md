# Architecture Decision Record - FileSquid

036-single-fsquid-cli.md

## Title

One FileSquid command-line tool, `fsquid`, built by the `filesquid-cli` crate

## Status

Accepted

Accepted: 2026-10-17. Amends [ADR 016](./016-rust-ocfl-library.md), [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md), [ADR 024](./024-filesquid-metadata-engine.md) and [ADR 026](./026-filesquid-provenance-engine.md) where they give each module its own CLI.

Revised: 2026-10-20T05:00:00Z. The metadata module adds `namespace` (`register`, `list`) for its namespace registry (Task MD-0.7, MD-S-012); no other module defines it, so it runs at the top level.

## Context / Requirement Reference

- Requirements: [C-057](../product-vision.md#module-rules), [FE-069](../product-vision.md#independent-library), [FE-070](../product-vision.md#independent-library), [FE-072](../product-vision.md#independent-library), [U-40](../product-vision.md#ocfl-module), [U-63](../product-vision.md#metadata-module), [U-67](../product-vision.md#c2pa-module), [T-042](../product-vision.md#canonical-ocfl-state-and-evidence), [T-045](../product-vision.md#canonical-ocfl-state-and-evidence); [PRD F-050](../prd.md#f-050-filesquid-platform-and-distributions), [F-051](../prd.md#f-051-ocfl-module), [F-052](../prd.md#f-052-metadata-module), [F-054](../prd.md#f-054-c2pa-module).
- Earlier ADRs gave each module its own command line: `fsquid` for OCFL ([ADR 016](./016-rust-ocfl-library.md)), a metadata CLI that shipped as the `filesquid-metadata` binary ([ADR 024](./024-filesquid-metadata-engine.md), Task MD-0.3), and a C2PA CLI ([ADR 026](./026-filesquid-provenance-engine.md)).
- The owner decided on 2026-10-17 that every FileSquid CLI is invoked as `fsquid`, followed by commands and parameters, and that module context is added only where it is necessary.
- Command names planned across the modules collide only on `validate` (OCFL structural validation and metadata register validation). OCFL: `stage`, `status`, `diff`, `commit`, `log`, `checkout`, `revert`, `reinstate`, `list`, `cat`, `validate`. Metadata: `extract`, `get`, `set`, `export`, `import`, `namespace`, `validate`. C2PA: `read`, `verify`, `sign`, `edit`, `embed`, `report`.

## Decision

1. **One name.** Every FileSquid command-line tool is invoked as `fsquid`. No other CLI binary name is shipped. The `filesquid-desktop` and `filesquid-server` host binaries are applications, not CLI tools, and keep their names.
2. **One crate.** A workspace crate `filesquid-cli` (`apps/filesquid/crates/cli/`) builds the only `fsquid` binary and always includes the commands of every module. Module crates ship libraries only; each keeps its command logic in its library (for example `filesquid_metadata::cli::run`), and `filesquid-cli` routes to it. `filesquid-cli` is not a module: it may depend on every module, and no module depends on it.
3. **Top-level commands.** A command whose name only one module defines runs as `fsquid <command> [args]` (for example `fsquid get dcterms:title`).
4. **Module groups.** Every command also runs as `fsquid <module> <command> [args]` (for example `fsquid c2pa verify`). The group is required only for a command name that more than one module defines. `fsquid <module>` alone prints that module's help.
5. **`validate` always needs its group:** `fsquid ocfl validate`, `fsquid metadata validate`. `fsquid validate` fails with a message that names the grouped forms.
6. **Collision guard.** A test fails when two modules define the same command name and that name is not in the qualifier-only list, when a module name equals a command name, when any workspace crate other than the hosts and `filesquid-cli` declares a binary, or when `filesquid-cli` declares a binary not named `fsquid`.
7. **Unchanged:** FileSquid hosts link the module libraries and MUST NOT shell out to `fsquid` for canonical I/O ([T-043](../product-vision.md#canonical-ocfl-state-and-evidence)).

## Rationale

- One tool to install, learn and script, matching the owner's request.
- A single routing crate keeps module crates free of binary and argument-dispatch concerns while their command logic stays testable in the library.
- With one collision today, top-level commands stay short; groups exist for the cases that need them and for scripts that want to be explicit.

## Assumptions

- Users who install the CLI accept that `fsquid` includes all modules, even if they use only one (owner choice over per-module Cargo features).
- Future command names are chosen to avoid new collisions where possible; the collision guard makes any new one an explicit decision.

## Alternatives Considered

- **`filesquid-cli` with per-module Cargo features** (an OCFL-only `fsquid`). Not chosen by the owner; `fsquid` always includes every module.
- **Each module crate builds its own `fsquid`.** Rejected: two installed modules would overwrite each other's binary.
- **Module context as flags (`fsquid c2pa --verify`) or a scope flag (`fsquid verify --c2pa`).** Rejected in favour of the conventional subcommand group.
- **Top-level `validate` that runs every applicable validator, or renaming one of the two.** Rejected by the owner; `validate` always needs its module group.

## Consequences / Implications

- The `filesquid-metadata` binary is retired; its commands run as `fsquid <command>` or `fsquid metadata <command>`.
- Installing the CLI for independent use means installing `filesquid-cli`, which pulls in every module library. Each module library still builds, tests and publishes on its own ([C-047](../product-vision.md#module-rules), [C-048](../product-vision.md#module-rules)).
- OCFL and C2PA command tasks (IO-D.4, CP-0.5, CP-0.7, CP-2.1, CP-2.3) add their commands to the `filesquid-cli` module table instead of shipping binaries; release binaries are `fsquid` builds of `filesquid-cli`.

## Related Decisions / Notes

- [ADR 016](./016-rust-ocfl-library.md) and [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md) (OCFL engine and CLI), [ADR 024](./024-filesquid-metadata-engine.md) (metadata CLI), [ADR 026](./026-filesquid-provenance-engine.md) (C2PA CLI), [ADR 029](./029-filesquid-unified-product-and-distributions.md) (workspace of module crates).
