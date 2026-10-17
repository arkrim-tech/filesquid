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

## MUST

- Put every new gate in a `scripts/verify_*.sh` script and call it from `scripts/pre-commit.sh`. One script, one gate, non-zero exit on failure, a message that names the fix.
- Run `bash scripts/pre-commit.sh` from the repository root before every push, or install it once: `ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit`.
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
| 7 | `verify_formatting.sh` | `cargo fmt --check` per Cargo workspace, `prettier --check` per npm package |
| 8 | `verify_docs.sh` | ADR structure, PRD allocation, relative links and anchors, FileSquid/ImageSquid document boundary, task cleanup (`scripts/docs/`) |
| 9 | `verify_tests.sh` | Clippy, `cargo test --workspace`, each module crate alone (ADR 029), rustdoc, then ESLint, `tsc` and Vitest per UI package; Playwright with `FILESQUID_E2E=1` |

Steps 0 to 8 take seconds. Step 9 is the gate that would otherwise be hosted CI and takes minutes on a cold Cargo cache. `FILESQUID_SKIP_TESTS=1` skips step 9 only.

## Agent resolution mandate

If any step exits non-zero, the agent MUST NOT commit or push. Read the output, fix the cause, and rerun `bash scripts/pre-commit.sh` until it exits 0. Do not weaken a verifier to make it pass; an owner decision is required to change what a gate checks ([no-silent-restore.md](./no-silent-restore.md)).
