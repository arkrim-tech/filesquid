# Task build-narrow-rust-hook-scope: Walkthrough

## Owner-approved infrastructure change (2026-10-18): skip the Rust build when nothing that affects Rust changed

No PRD or SRS requirement. This task narrows the change-aware scope of the local verification gate in [local-verification.md](../../.agents/rules/local-verification.md) (AGENTS.md section 5), which the [change-aware pre-commit walkthrough](./Task-build-change-aware-pre-commit-Walkthrough.md) introduced. It also changes when a PR needs a `FULL=1` hook run. That rules change needs the owner's approval before merge.

## What changed

| File | Change |
|---|---|
| [`scripts/changed_scope.sh`](../../scripts/changed_scope.sh) | `scripts/` is split. The text checkers that steps 0 to 9 run on every commit are `docs`: an explicit allowlist (`DOCS_CHECKER_SCRIPTS`: `verify_branch.sh`, `verify_isolated_ports.sh`, `verify_doc_sync.sh`, `verify_srs_traceability.sh`, `verify_relative_links.sh`, `verify_workspace_boundaries.sh`, `verify_centralized_logging.sh`, `verify_docs.sh`, `verify_task_cleanup.sh`) plus everything under `scripts/docs/`. Every other script stays `full`: `pre-commit.sh`, `changed_scope.sh`, `verify_changed_scope.sh`, `verify_tests.sh`, `verify_formatting.sh`, `setup-dev-environment.sh`, `scripts/lib/`, and any new or unknown script. The top-level `package.json` or `package-lock.json` of a package in `NPM_PACKAGES` (`apps/filesquid/ui`, `apps/imagesquid`) is now `npm:<package>` plus its `file:` dependents. No Rust build reads them: the desktop `build.rs` and `tauri.conf` read only `ui/dist`. Any other `package.json` or `package-lock.json` stays `full`, as do `openapi.json`, `schema.d.ts` (the api contract test reads it), `Cargo.toml`, `Cargo.lock` and `rust-toolchain`. Root `.claude/` and `docs/` are `docs`. Two new small pure functions, `classify_script` and `classify_npm_manifest`, hold the new rules. Unknown paths and detection failures still give `full`. |
| [`scripts/verify_changed_scope.sh`](../../scripts/verify_changed_scope.sh) | Fixture tests for every new rule, written first (below). Each allowlisted checker and a new `scripts/docs/` file give `docs`. Each gate-logic script, a new `scripts/lib/` file and unknown new scripts give `full`. The UI and ImageSquid manifests give their npm scope; root, `apps/` and desktop e2e manifests stay `full`. `.claude/` and `docs/` give `docs` at the root only. Mixed cases combine them. 166 assertions. |
| [`scripts/verify_tests.sh`](../../scripts/verify_tests.sh), [`scripts/pre-commit.sh`](../../scripts/pre-commit.sh) | Header comments and the docs-scope skip reason match the new classification and the `FULL=1` rule. No behaviour change. |
| [local-verification.md](../../.agents/rules/local-verification.md), [task-workflow.md](../../.agents/rules/task-workflow.md), [pr-review-loop.md](../../.agents/rules/pr-review-loop.md) | `FULL=1 bash scripts/pre-commit.sh` is required before marking ready and on the final commit only when the PR's change scope is `full` or includes `rust:`. For a `docs` or npm-only PR, a passing ordinary run on the head commit is enough, and its `Test scope: <scope>` line is recorded the same way. The merge condition says the same. The scope table lists the new rules. |
| [README](../../README.md) | The "Run tests" scope list and the `FULL=1` rule match. |

Examples on the real repository (`CHANGED_SCOPE_FILES` names a file listing the paths shown first):

```text
# scripts/verify_docs.sh .claude/agents/x.md
changed_scope: docs (only documentation or always-run checker scripts outside the crates and npm packages changed)
docs
# apps/filesquid/ui/package-lock.json
changed_scope: npm:apps/filesquid/ui,apps/imagesquid (changed: apps/filesquid/ui)
npm:apps/filesquid/ui,apps/imagesquid
# scripts/verify_tests.sh
changed_scope: full (scripts/verify_tests.sh changed)
full
```

## Test-driven development

### Red: the new fixture tests against the old helper (`bash scripts/verify_changed_scope.sh`)

```text
Starting Change Scope Validation...
❌ files [scripts/verify_branch.sh]: expected 'docs', got 'full'
❌ files [scripts/verify_isolated_ports.sh]: expected 'docs', got 'full'
❌ files [scripts/verify_doc_sync.sh]: expected 'docs', got 'full'
❌ files [scripts/verify_srs_traceability.sh]: expected 'docs', got 'full'
❌ files [scripts/verify_relative_links.sh]: expected 'docs', got 'full'
❌ files [scripts/verify_workspace_boundaries.sh]: expected 'docs', got 'full'
❌ files [scripts/verify_centralized_logging.sh]: expected 'docs', got 'full'
❌ files [scripts/verify_docs.sh]: expected 'docs', got 'full'
❌ files [scripts/verify_task_cleanup.sh]: expected 'docs', got 'full'
❌ files [scripts/docs/check_links.py]: expected 'docs', got 'full'
❌ files [scripts/docs/validate-adrs.test.mjs]: expected 'docs', got 'full'
❌ files [scripts/docs/new_checker.py]: expected 'docs', got 'full'
❌ files [scripts/verify_docs.sh scripts/docs/check_links.py README.md .agents/rules/rule.md]: expected 'docs', got 'full'
❌ files [scripts/verify_branch.sh apps/filesquid/crates/c2pa/src/lib.rs]: expected 'rust:filesquid-c2pa', got 'full'
❌ files [apps/filesquid/ui/package-lock.json]: expected 'npm:apps/filesquid/ui,apps/imagesquid', got 'full'
❌ files [apps/filesquid/ui/package.json]: expected 'npm:apps/filesquid/ui,apps/imagesquid', got 'full'
❌ files [apps/imagesquid/package.json]: expected 'npm:apps/imagesquid', got 'full'
❌ files [apps/imagesquid/package-lock.json apps/imagesquid/src/main.ts]: expected 'npm:apps/imagesquid', got 'full'
❌ files [apps/filesquid/ui/package-lock.json apps/filesquid/crates/c2pa/src/lib.rs]: expected 'rust:filesquid-c2pa npm:apps/filesquid/ui,apps/imagesquid', got 'full'
❌ files [.claude/settings.json docs/diagram.svg]: expected 'docs', got 'full'
❌ files [.claude/agents/x.md docs/x.md scripts/verify_docs.sh resources/prd.md]: expected 'docs', got 'full'
❌ files [.claude/agents/x.md docs/x.md apps/filesquid/ui/package-lock.json]: expected 'npm:apps/filesquid/ui,apps/imagesquid', got 'full'

🚨 VALIDATION FAILED: scripts/changed_scope.sh or a hook library broke an expectation above.
```

The new `full` expectations (gate-logic scripts, unknown scripts, other manifests, `apps/docs/`, a nested `.claude/`) already passed against the old helper, which sent all of them to `full`.

### Green: after the change

```text
Starting Change Scope Validation...
✅ Change scope validation passed (166 assertions).
```

## Hook run

This branch changes gate-logic scripts, so its own scope is `full` and the hook builds and tests Rust. Status lines of `FULL=1 bash scripts/pre-commit.sh` on this branch (other agents held the shared Cargo lock part of the time), with `CARGO_TARGET_DIR` on the shared target, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and `TMPDIR` on tmpfs:

```text
Running Pre-Commit Hooks... (test scope: full)
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
✅ Traceability Validation passed: 95 PRD requirements, 82 resolved SRS references.
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
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Change scope validation passed (166 assertions).
ℹ️  Test scope: full
✅ Clippy passed (41s).
✅ Workspace tests passed (42s).
✅ Module crate alone: filesquid-core passed (29s).
✅ Module crate alone: filesquid-ocfl passed (21s).
✅ Module crate alone: filesquid-metadata passed (63s).
✅ Module crate alone: filesquid-c2pa passed (119s).
✅ Module crate alone: filesquid-api passed (16s).
✅ Module crate alone: filesquid-native passed (86s).
✅ Rustdoc passed (15s).
✅ Clippy (desktop, test-instrumented) passed (64s).
✅ Desktop tests (test-instrumented) passed (56s).
✅ ESLint passed (5s).
✅ TypeScript passed (7s).
✅ Vitest passed (18s).
✅ ESLint passed (1s).
✅ TypeScript passed (3s).
✅ Vitest passed (3s).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (646s)
```

## Mocked Environment Variables

None.
