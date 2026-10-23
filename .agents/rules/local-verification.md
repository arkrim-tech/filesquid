---
trigger: always_on
description: Run every quality gate through the local pre-commit hook; never add hosted CI such as GitHub Actions.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Local Verification, No Hosted CI

**Purpose:** This rule enforces the "Local Verification, No Hosted CI" principle in `AGENTS.md`. FileSquid runs every quality gate on the developer's machine through `scripts/pre-commit.sh`. Hosted CI is slow, non-deterministic and outside the owner's control, so it is not used.

## MUST NOT

- Create `.github/workflows/`, `.gitlab-ci.yml`, or any other hosted CI configuration. If one exists, remove it rather than fixing it.
- Describe a check as "run in CI". Checks run in the pre-commit hook or not at all.
- Push a commit that fails `scripts/pre-commit.sh`.
- Bypass the hook with `--no-verify`. The only sanctioned shortcut is `FILESQUID_SKIP_TESTS=1` for a work-in-progress commit that will not be pushed until `scripts/verify_tests.sh` passes.
- Mark a pull request ready for review, or merge it, without the head-commit hook run required for its scope (below).
- Set the `CHANGED_SCOPE_*` fixture variables to shrink a hook run. The hook ignores them; they exist for `scripts/verify_changed_scope.sh` only.

## MUST

- Put every new gate in a `scripts/verify_*.sh` script and call it from `scripts/pre-commit.sh`. One script, one gate, non-zero exit on failure, a message that names the fix.
- Run `bash scripts/pre-commit.sh` from the repository root before every push, or install it once: `ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit`.
- Run the hook on the head commit (a) before a pull request is first marked ready for review (or first opened as ready), and (b) on the final commit before it is merged. When the PR's change scope (`scripts/changed_scope.sh` against `origin/main`) is `full` or includes `rust:`, that run MUST be the **full suite**, `FULL=1 bash scripts/pre-commit.sh`. When the PR's scope is `docs` or npm-only, a passing ordinary `bash scripts/pre-commit.sh` is enough. Paste the hook's summary line (`Test scope: <scope>`) in the PR or its walkthrough so a reviewer can see which run passed.
- Run every cargo command in a gate that builds, tests, documents or formats through `cargo_locked` from `scripts/lib/cargo_shared.sh`, so two gate runs that share one target directory never use it at the same time. `cargo metadata --no-deps` in the scope helper reads manifests only and is not locked. Where `flock` is not installed (for example macOS without util-linux) the gates warn and run unlocked; there, do not run two gate runs on one target directory at once.
- Keep the gates platform-agnostic ([platform-agnostic.md](./platform-agnostic.md)) and verify any command they run ([command-verification.md](./command-verification.md)).
- Document a new gate, its environment variables and its runtime in the root README "Run tests" section in the same change.

## The gates

`scripts/pre-commit.sh` runs, in order:

| Step | Script | Checks |
|---|---|---|
| 0 | `verify_branch.sh` | No direct commits to `main`; branch name is `<type>/<slug>` with a permitted Conventional Commit type ([task-workflow.md](./task-workflow.md)) |
| 1 | `verify_isolated_ports.sh` | Unique dev, preview and bind ports ([port-conflicts.md](./port-conflicts.md)) |
| 2 | `verify_doc_sync.sh` | Source changes ship with documentation changes ([sync-logic.md](./sync-logic.md)) |
| 3 | `verify_srs_traceability.sh` | PRD and SRS IDs resolve both ways |
| 4 | `verify_relative_links.sh` | No absolute paths in staged Markdown ([links-enforce.md](./links-enforce.md)) |
| 5 | `verify_workspace_boundaries.sh` | No language manifests at the root; distributions hold no Rust ([workspace-boundaries.md](./workspace-boundaries.md)) |
| 6 | `verify_centralized_logging.sh` | No file logging ([logging-enforce.md](./logging-enforce.md)) |
| 7 | `verify_formatting.sh` | `cargo fmt --check` per Cargo workspace (skipped when the scope is `docs` or npm-only; it runs for every `rust:` and `full` scope), `prettier --check` per npm package; `apps/imagesquid` is checked with `@filesquid/ui`'s Prettier, so the hook needs no ImageSquid install ([ImageSquid during the MVP](#imagesquid-during-the-mvp)) |
| 8 | `verify_docs.sh` | ADR structure, PRD allocation, relative links and anchors, FileSquid/ImageSquid document boundary, task cleanup (`scripts/docs/`), and the MVP progress merge-driver tests (`test_merge_mvp_progress.py`, throwaway git repositories under `TMPDIR`) |
| 9 | `verify_changed_scope.sh` | Tests of the change-scope helper and the hook libraries against a temporary git repository |
| 10 | `verify_tests.sh` | Clippy, Cargo tests, each module crate alone (ADR 029), rustdoc, then ESLint, `tsc` and Vitest for `@filesquid/ui` (`apps/filesquid/ui`), **limited to the change scope**; Playwright with `FILESQUID_E2E=1`. It runs no ImageSquid suite ([ImageSquid during the MVP](#imagesquid-during-the-mvp)) |

Steps 0 to 9 run on every commit and take seconds; only step 7's Rust format check depends on the scope. Step 10 is the gate that would otherwise be hosted CI and takes minutes on a cold Cargo cache. `FILESQUID_SKIP_TESTS=1` skips step 10 only.

## Change-aware test scope

`scripts/changed_scope.sh` diffs the merge-base with `origin/main` against the working tree (branch commits, staged, unstaged and untracked files together) and prints one scope. Step 10 runs only what that scope needs:

| Scope | When | Step 10 runs |
|---|---|---|
| `docs` | Only documentation changed **outside** the crates and npm packages: Markdown, `resources/`, `.agents/`, and the root `.claude/` and `docs/` (every file inside a crate or package, its README and `resources/` included, belongs to it, because its tests read them, for example `crates/api/resources/srs.md`); or a text checker that steps 0 to 9 run on every commit (the explicit list `verify_branch.sh`, `verify_isolated_ports.sh`, `verify_doc_sync.sh`, `verify_srs_traceability.sh`, `verify_relative_links.sh`, `verify_workspace_boundaries.sh`, `verify_centralized_logging.sh`, `verify_docs.sh`, `verify_task_cleanup.sh`, and `scripts/docs/`) | No Cargo or npm step; each skipped step is printed with the reason |
| `rust:<crates>` | Any file changed under `apps/filesquid/crates/<crate>/` | Clippy, tests and rustdoc with `-p` for the changed crates **and every workspace crate that depends on them** (`cargo metadata` reverse dependencies); each listed module crate alone; the desktop `test-webdriver` steps only when `filesquid-desktop` is listed |
| `npm:<packages>` | Files changed in `apps/filesquid/ui` or `apps/imagesquid`, their top-level `package.json` and `package-lock.json` included (no Rust build reads them; a nested manifest gives `full`). The only package step 10 tests is `apps/filesquid/ui`, so either gives `npm:apps/filesquid/ui`: a change confined to `apps/imagesquid` selects `@filesquid/ui`'s suite, whose distribution-table test reads ImageSquid's label table and token file, and a change to `@filesquid/ui` no longer adds ImageSquid | Lint, typecheck and tests for `@filesquid/ui` |
| `full` | `apps/filesquid/Cargo.toml` or `Cargo.lock`, a `rust-toolchain` file, any other `package.json` or `package-lock.json`, a script that decides or runs the scope (`pre-commit.sh`, `changed_scope.sh`, `verify_changed_scope.sh`, `verify_tests.sh`, `verify_formatting.sh`, `setup-dev-environment.sh`, `scripts/lib/`) or any script not in the `docs` list (a new one included), a generated contract file (`openapi.json`, `schema.d.ts`), any other unclassified file, `FULL=1`, or any detection failure (unknown `origin/main`, git or `cargo metadata` error) | Every step, exactly as before the scope existed |

Detection never falls back to running less: any doubt, and any answer outside the scope grammar, gives `full`. A scope handed to a gate with `--scope` is normalised the same way: a guarded package (`npm:apps/imagesquid`) becomes the suite that guards it (`npm:apps/filesquid/ui`), and any other package the hook does not know gives `full` (`normalise_scope` in `scripts/lib/hook_scope.sh`). The hook detects the scope once, hands it to steps 7 and 10 with `--scope`, and prints it in its first and last lines (`✅ All Pre-Commit Hooks Passed! Test scope: <scope>`).

## ImageSquid during the MVP

Owner decision, 2026-10-23, verbatim: "for ImageSquid checks decision use option 2". It answers the owner's question of the same day, verbatim: "why are we running imagesquid unit tests in the filesquid project, imagesquid is release 2". Option 2, as the orchestrator offered it, verbatim: "Narrow it for the MVP (my recommendation). Drop ImageSquid's lint, typecheck, Vitest and Playwright from the hook. Keep one cheap completeness test inside the FileSquid UI package that reads ImageSquid's label and token tables. Then MVP UI tasks only add keys to that table, and the full ImageSquid suite comes back when ImageSquid work starts."

Until ImageSquid work starts:

- The hook MUST NOT run ImageSquid's ESLint, `tsc`, Vitest or Playwright suite. `apps/imagesquid` keeps its npm scripts for anyone who runs them by hand there after `npm ci`; the hook needs no ImageSquid install.
- Step 7 MUST still run `prettier --check` over `apps/imagesquid`, with `@filesquid/ui`'s Prettier and ImageSquid's own `.prettierrc.json` and `.prettierignore`.
- `apps/filesquid/ui/tests/unit/distribution-tables.test.ts` MUST read ImageSquid's label table (`src/theme/labels.en.json`) and token file (`src/theme/tokens.css`) and fail when either lacks a key or token of the default table or token file, in light or dark (UI-S-005), or when a colour token is unreadable or a checked pair misses WCAG 2.2 AA, with no contrast exemption (UI-S-015). A change confined to `apps/imagesquid` MUST select `npm:apps/filesquid/ui`, so this test runs.
- A FileSquid UI change that adds a label key or a required token MUST add it to ImageSquid's label table and token file in the same change.

When ImageSquid work starts, the full ImageSquid lint, typecheck, Vitest and Playwright suite MUST return to the ImageSquid repository's own gates, or to this hook if ImageSquid is still in this repository ([Task FS-3.8](../../resources/implementation-plan.md#task-fs-3.8)).

## Shared Cargo target

Agents may share one `CARGO_TARGET_DIR`. Every cargo command in `verify_tests.sh`, the `cargo fmt` in `verify_formatting.sh` and the pre-warm in `scripts/setup-dev-environment.sh` holds an exclusive `flock` on `<target>.lock`, where the target is `CARGO_TARGET_DIR`, else `CARGO_BUILD_TARGET_DIR`, else the workspace's default `target/` (whose `target.lock` is gitignored), resolved to its physical path so a symlink and its real path share one lock. A `build.target-dir` in a Cargo config file is not read. Cargo commands run by hand or by an IDE do not take the lock. The wait is bounded by `FILESQUID_CARGO_LOCK_WAIT` seconds (default 1800); on timeout the command is not run and the message names the lock file. Where `flock` is missing (for example macOS) the gates print a warning and run unlocked. The same library exports `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=0` unless already set, so the hook and the setup pre-warm share artifacts.

The lock serialises cargo runs that take it. It does not isolate worktrees: in a shared target, two checkouts of the workspace produce artifacts with the same names, so Cargo can reuse a test binary built from another worktree's sources ([walkthrough](../../resources/tasks/Task-build-change-aware-pre-commit-Walkthrough.md#known-limitation-a-shared-target-across-worktrees)). Treat a result from a shared target as provisional when other worktrees are building, and when the PR's scope is `full` or includes `rust:`, run its final `FULL=1` gate with no other agent building in that target, or with a private `CARGO_TARGET_DIR` when disk allows.

## Agent resolution mandate

If any step exits non-zero, the agent MUST NOT commit or push. Read the output, fix the cause, and rerun `bash scripts/pre-commit.sh` until it exits 0. Do not weaken a verifier to make it pass; an owner decision is required to change what a gate checks ([no-silent-restore.md](./no-silent-restore.md)).
