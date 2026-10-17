# Implementation Plan: filesquid-cli

The `filesquid-cli` crate builds `fsquid`, the one FileSquid command-line tool ([ADR 036](../../../../../resources/architecture/036-single-fsquid-cli.md)). It is not a module: it routes to each module library's command runner and may depend on every module; no module depends on it. OCFL and C2PA register their command groups here through their own tasks ([IO-D.4](../../ocfl/resources/plan.md), [CP-0.5, CP-0.7, CP-2.1, CP-2.3](../../c2pa/resources/plan.md)). Global sequence: [implementation-plan.md](../../../../../resources/implementation-plan.md).

Cargo commands run from `apps/filesquid/` with `-p filesquid-cli` (`cargo test -p filesquid-cli --locked`).

## Phase F0: One `fsquid` CLI (global Task FS-0.1)

- [x] **Task CLI-0.1: `fsquid` binary and command routing**
  - **Description:** Create the crate and the `fsquid` binary; route unique commands at the top level and `fsquid <module> <command>` always; `validate` always needs its module; guard against shared command names, module/command name clashes and any other CLI binary name; retire the `filesquid-metadata` binary.
  - **Estimated Time:** 6 hours
  - **Dependencies:** MD-0.3
  - **Related Requirements:** [F-050](../../../../../resources/prd.md#f-050-filesquid-platform-and-distributions), [SRS-F-050](../../../../../resources/srs.md#srs-f-050-filesquid-platform-and-distributions), [F-052](../../../../../resources/prd.md#f-052-metadata-module), [C-057](../../../../../resources/product-vision.md#module-rules)
  - **Related Tests:** [`single_fsquid_cli_F-050.feature`](../tests/features/single_fsquid_cli_F-050.feature), [`route.rs`](../tests/unit/route.rs)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task CLI-0.1. Then begin development, running Cargo from apps/filesquid/ with -p filesquid-cli (module directory apps/filesquid/crates/cli/). Write failing routing tests first. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/filesquid/crates/cli/resources/tasks/Task-CLI-0.1-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/filesquid/crates/cli/resources/plan.md. Then commit your changes and push the branch to origin.`
