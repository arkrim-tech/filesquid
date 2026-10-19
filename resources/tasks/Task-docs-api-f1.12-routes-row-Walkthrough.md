# Task docs-api-f1.12-routes-row Walkthrough: API-F1.12 library and repair routes planned

Documentation-only task on branch `docs/api-f1.12-library-repair-routes`. TDD-ENFORCE: Not applicable - plan and requirements update; no code changed.

## Owner decisions (2026-10-19)

1. CL-1.1 defines the local-library commands and the version-inventory repair, but no route. A new api module task, **API-F1.12 "Library and repair routes"**, adds versioned routes (desktop custom scheme and `filesquid-server` HTTP) and the OpenAPI 3.1 contract, with regenerated TypeScript types and the drift test, for create, open, copy completed snapshot and rebuild from files, plus the W010 repair-candidate query and the consented repair command. Global Task 1.1 dispatches it after CL-1.1 and before DE-1.1 and UI-1.1, which both depend on it.
2. Two won't-fix answers from [PR #65](https://github.com/arkrim-tech/filesquid/pull/65) (CL-1.1) are accepted: library create is not atomic against a file that appears between the emptiness check and the declaration write (the result is still a valid library; no backend offers an atomic create-if-empty), and server library owners cannot run the repair until API-F1.9 (every server caller is refused until then).

## Changes

- [api plan](../../apps/filesquid/crates/api/resources/plan.md): new section "Phase F2: Library and repair routes (global Task 1.1)" with the API-F1.12 row (Description, 10-hour estimate, Dependencies `Task CL-1.1, API-F1.2, API-F1.7, API-F1.3`, Related Requirements, Related Tests, Agent Prompt), in the format of API-F1.10 and API-F1.11. The repair rules carried into the row: desktop owner only on the desktop; every server caller, the administrator included, refused until API-F1.9; recorded consent (`RepairConsent`, statement `version_inventory_repair.v1`); `ocfl.repair.version_inventory` journal rows; the `nothing_to_repair` refusal; every refusal journaled once; no file or SQL access in `filesquid-api` and no dependency on `filesquid-native`. The CL-1.1 Status line now records the two accepted won't-fix answers and points to API-F1.12. The Summary Timeline critical path names API-F1.12.
- [api SRS](../../apps/filesquid/crates/api/resources/srs.md): new row **API-S-025 Local library and repair routes**. API-S-019 covers the library catalog commands and API-S-023 the repair itself, but no row required the CL-1.1 library lifecycle or the repair to be reachable as routes. The row states no timeline: a host that cannot identify a library's owner as an authenticated principal refuses every caller; the API-F1.9 timing stays in the plan ([decoupled-execution.md](../../.agents/rules/decoupled-execution.md)). Parents: F-001, SRS-F-001, F-042, SRS-F-042, F-056.
- [Product SRS](../srs.md): SRS-F-001 and SRS-F-042 each gain a bullet linking API-S-025, so the trace runs both ways.
- [Implementation plan](../implementation-plan.md): Task 1.1 dispatches API-F1.12 after CL-1.1, names API-S-025, depends on Task FS-1.2 (API-F1.3 route conventions), and its estimate rises from 62 to 72 hours (Phase F2 in the Summary Timeline from 342 to 352); the Agent Prompt asks for failing tests for the routes; the dispatch example and fan-out table list API-F1.12; the totals rise to about 2,673 hours overall and 826 for the MVP, with a sentence that names the 10 hours.
- [Checklist](../implementation-plan-checklist.md): the API-F1.12 row sits under Task 1.1 after CL-1.1 and before DE-1.1 and UI-1.1; the MVP progress block and bar were redrawn with `python3 scripts/docs/update_mvp_progress.py`.
- [UI plan](../../apps/filesquid/ui/resources/plan.md) UI-1.1 and [desktop plan](../../apps/filesquid/crates/desktop/resources/plan.md) DE-1.1: both depend on API-F1.12 and cite API-S-025; UI-1.1 calls the repair through its API-F1.12 route, and DE-1.1 handles the API-F1.12 routes.

## Cubic review

- Round 1 (3 P2, 1 P3), all fixed: the Summary Timeline's Phase F2 row rises from 342 to 352 hours, so the phase rows add up to the new totals; the DE-1.1 Description now says that the desktop host wires the storage root, the repair ledger and the desktop owner principal into the router for the API-F1.12 repair route; and the API-F1.12 Related Tests no longer say that route scenarios were added to the existing API-S-023 feature (the task writes them in a new API-S-025 feature).
- Round 2 (1 P3), fixed: the transcript below is now the 162s run after the round 1 fixes, labelled with the commit it covers.

## Hook

The change touches files inside `apps/filesquid/crates/api/`, `apps/filesquid/crates/desktop/` and `apps/filesquid/ui/`, which belong to those crates and that package, so the change scope (`scripts/changed_scope.sh` against `origin/main`) is `rust:filesquid-api,filesquid-desktop,filesquid-native,filesquid-server npm:apps/filesquid/ui,apps/imagesquid`, not `docs`. A scope that includes `rust:` needs a full-suite run: `FULL=1 bash scripts/pre-commit.sh`, after `npm ci` in `apps/filesquid/ui` and `apps/imagesquid`.

The first full run (2026-10-19 UTC) failed two tests in code this change does not touch: `an_oversized_response_is_journaled_without_its_detail` (`filesquid-desktop`) and `failure_logs_carry_no_secrets` (`filesquid-c2pa`). Both were built in a Cargo target shared with other worktrees ([local-verification.md](../../.agents/rules/local-verification.md#shared-cargo-target)). The second full run passed with no change in between (`Test scope: full (159s)`). After the Cubic round 1 fixes, `FULL=1 bash scripts/pre-commit.sh` passed again: `✅ All Pre-Commit Hooks Passed! Test scope: full (162s)`.

Step lines of the `FULL=1 bash scripts/pre-commit.sh` run on the tree committed as 87169d4 (after the Cubic round 1 fixes; the later commits change only this walkthrough), exit 0:

```text
changed_scope: full (FULL=1 is set)
Running Pre-Commit Hooks... (test scope: full)
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
✅ Traceability Validation passed: 95 PRD requirements, 83 resolved SRS references.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
✅ ./apps/filesquid is properly formatted.
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ FileSquid and ImageSquid document boundary passed.
✅ MVP queue and merge driver tests passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Change scope validation passed (166 assertions).
ℹ️  Test scope: full
✅ Clippy passed (1s).
✅ Workspace tests passed (51s).
✅ Module crate alone: filesquid-core passed (1s).
✅ Module crate alone: filesquid-ocfl passed (25s).
✅ Module crate alone: filesquid-metadata passed (0s).
✅ Module crate alone: filesquid-c2pa passed (5s).
✅ Module crate alone: filesquid-api passed (6s).
✅ Module crate alone: filesquid-native passed (1s).
✅ Clippy (filesquid-metadata, all features) passed (0s).
✅ Tests (filesquid-metadata, all features) passed (3s).
✅ wasm32 build (filesquid-metadata, default features off) passed (0s).
✅ wasm32 build (filesquid-metadata, ocfl feature) passed (1s).
✅ Rustdoc passed (3s).
✅ Clippy (desktop, test-instrumented) passed (1s).
✅ Desktop tests (test-instrumented) passed (18s).
✅ ESLint passed (3s).
✅ TypeScript passed (4s).
✅ Vitest passed (12s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
✅ ESLint passed (2s).
✅ TypeScript passed (2s).
✅ Vitest passed (3s).
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (162s)
```
