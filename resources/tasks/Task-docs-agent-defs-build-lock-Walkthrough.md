# Task docs-agent-defs-build-lock Walkthrough: build-efficiency and process rules in the agent definitions

Documentation-only task on branch `docs/agent-defs-build-lock`. TDD-ENFORCE: Not applicable - documentation update. No crate, Cargo file or script is changed.

## Owner request (2026-10-18)

Add the build-efficiency and process rules decided on 2026-10-18 to the three agent definitions, without changing their scopes, names, descriptions or tools.

## Changes

- [Modules engineer](../../.claude/agents/filesquid-modules-engineer.md) and [platform engineer](../../.claude/agents/filesquid-platform-engineer.md): new "Rust builds" section (narrow cargo commands under the shared-target lock, one full hook run per push, the "could not find" retry, the `FULL=1` rule linked to [local-verification.md](../../.agents/rules/local-verification.md#change-aware-test-scope), conditional [ADR 039](../architecture/039-cargo-hakari-workspace-hack.md) workspace-hack regeneration) and a "Process rules" section (merge by merge commit on its own, no owner email in requests, model names only in the session's attribution trailer).
- [UI engineer](../../.claude/agents/filesquid-ui-engineer.md): a short "Rust builds" note that UI-only changes build no Rust, and the same "Process rules".

## Verification

Change scope is `docs` (only `.claude/` and `resources/` files). Environment: `CARGO_TARGET_DIR=/home/user/.cargo-target-shared`, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `TMPDIR=/dev/shm/docs-agent-defs`; `npm ci` run first in `apps/filesquid/ui` and `apps/imagesquid`. Raw output of `bash scripts/pre-commit.sh` (UTC, 2026-10-18):

```text
changed_scope: docs (only documentation or always-run checker scripts outside the crates and npm packages changed)
Running Pre-Commit Hooks... (test scope: docs)
... gates 0 to 9 passed (branch, ports, document sync, traceability 95/82, relative links, workspace boundaries,
    logging, formatting, documentation validation with 0 broken links in 194 files, change scope 166 assertions) ...
Starting Lint and Test Validation...
Test scope: docs
Skipped: Clippy, workspace tests, each module crate alone, rustdoc, desktop steps, ESLint/TypeScript/Vitest/Playwright (docs-only change)
Lint and test validation passed (test scope: docs).
All Pre-Commit Hooks Passed! Test scope: docs (13s)

(Output condensed from the full run; every gate printed its passed line.)

The run was repeated on the head commit before the PR was marked ready; the head-commit summary line is in the PR description.
```
