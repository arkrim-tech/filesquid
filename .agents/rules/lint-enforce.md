---
description: Ensure all code is strictly linted and formatted before pushing or testing.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Linting and Formatting Enforcement

**Purpose:** This file instructs the AI agent on exactly how to lint and format the codebase to satisfy the Test-Driven Development (TDD) linting mandate in `AGENTS.md`.

## Active Tools

Based on the accepted Architecture Decision Records in `resources/architecture/` for coding style ([ADR 019](../../resources/architecture/019-coding-style-and-lint.md)):

- **rustfmt** (`cargo fmt`) for Rust formatting.
- **Clippy** (`cargo clippy -D warnings`) for Rust static analysis.
- **ESLint** for TypeScript static analysis.
- **Prettier** for TypeScript (and CSS) formatting.

## Execution Requirements

Before running any tests, committing code, or completing an execution task, you MUST execute the following CLI commands inside the correct workspace.

During the FileSquid MVP the pre-commit hook lints, type-checks and tests only `@filesquid/ui` (owner decision 2026-10-23, [ImageSquid during the MVP](./local-verification.md#imagesquid-during-the-mvp)). ImageSquid is still formatted, with `@filesquid/ui`'s Prettier, so these commands need no ImageSquid install.

### 1. Format each TypeScript UI package
Run in `apps/filesquid/ui/`, then format `apps/imagesquid/` with the same Prettier (Node runs it on every OS):
```bash
cd apps/filesquid/ui/
npx prettier --write .
cd ../../imagesquid/
node ../filesquid/ui/node_modules/prettier/bin/prettier.cjs --write .
```

### 2. Lint the FileSquid UI package (and auto-fix)
Run in `apps/filesquid/ui/`:
```bash
cd apps/filesquid/ui/
npx eslint . --fix
```

ImageSquid's ESLint is not required during the MVP. Whoever changes ImageSquid's code MAY run it by hand with `npm ci` and `npx eslint . --fix` in `apps/imagesquid/`; when ImageSquid work starts it becomes required again ([Task FS-3.8](../../resources/implementation-plan.md#task-fs-3.8)).

### 3. Format and lint the FileSquid Cargo workspace
Run once from the product-local workspace root. It covers every crate under `apps/filesquid/crates/`.
```bash
cd apps/filesquid/
cargo fmt --all
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Agent Resolution Mandate

If `eslint` or `cargo clippy` return a non-zero exit code, the agent **MUST NOT** proceed to testing or committing.

The agent MUST:

1. Review the CLI output.
2. Fix the violations.
3. Rerun the same commands until they exit 0.

Only when rustfmt, Clippy, Prettier, and ESLint all exit cleanly is the codebase valid for testing or PR submission.
