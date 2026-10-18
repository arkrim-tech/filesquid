# Task build-change-aware-pre-commit: Walkthrough

## Owner-approved infrastructure change (2026-10-18): change-aware pre-commit gate, shared-target Cargo lock and session setup script

No PRD or SRS requirement. This task changes the local verification gate described in [local-verification.md](../../.agents/rules/local-verification.md) (AGENTS.md section 5) and bounds its waits (AGENTS.md section 22).

## What changed

| File | Change |
|---|---|
| [`scripts/changed_scope.sh`](../../scripts/changed_scope.sh) | New helper. It diffs the merge-base with `origin/main` against the working tree (committed, staged, unstaged and untracked files) and prints `docs`, `rust:<crates>`, `npm:<packages>` (or both) or `full`. Rust scope adds reverse dependencies from `cargo metadata --no-deps --format-version 1`. npm scope adds packages that link a changed package by `file:` (ImageSquid links `@filesquid/ui`). Every file inside a crate or npm package, its README and `resources/` included, belongs to that crate or package, because tests read them (`crates/api/resources/srs.md`, `crates/c2pa/README.md`). Every failure prints `full`. |
| [`scripts/lib/hook_scope.sh`](../../scripts/lib/hook_scope.sh) | New. The gates call the helper through this file. It strips the fixture-only `CHANGED_SCOPE_*` overrides and validates the answer against the scope grammar: `docs`, `full`, `rust:<list>`, `npm:<list>`, or both, with non-empty lists and one line. Anything else becomes `full`. `scope_or_detect` takes the scope the hook detected once (`--scope`), so steps 7 and 10 and the summary line agree. |
| [`scripts/lib/cargo_shared.sh`](../../scripts/lib/cargo_shared.sh) | New. `cargo_locked` and `with_cargo_lock` hold `flock -w ${FILESQUID_CARGO_LOCK_WAIT:-1800}` on descriptor 9 of a subshell, on `<target>.lock`. The target is `CARGO_TARGET_DIR`, else `CARGO_BUILD_TARGET_DIR`, else `target`, whose `target.lock` is gitignored. The command does not inherit the descriptor. A timeout prints a message naming the lock and exits 75; a command's own exit 75 is never reported as a timeout. Without `flock` the command runs unlocked after a warning. Sourcing the file also defaults `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=0`. |
| [`.gitignore`](../../.gitignore) | Ignores `**/target.lock`, so the default lock never shows up as an unclassified file (which would force `full`). |
| [`scripts/verify_tests.sh`](../../scripts/verify_tests.sh) | Follows the scope. `full` keeps the previous steps unchanged. `rust:` runs scoped Clippy, tests and rustdoc, each listed module crate alone, and the desktop `test-webdriver` steps only for `filesquid-desktop`. `npm:` runs only the listed packages. `docs` prints every skipped step and the reason. Every cargo call runs under the lock. |
| [`scripts/verify_formatting.sh`](../../scripts/verify_formatting.sh) | `cargo fmt --check` runs under the lock. It is skipped when the scope is `docs` or npm-only, because no `.rs` file changed, and runs for every `rust:` and `full` scope. Prettier still runs on every commit because it also checks Markdown. On failure it prints the cargo output. A lock timeout gets its own message; any other failure is reported as "a format violation, or cargo fmt could not run", and the printed output shows which. |
| [`scripts/verify_changed_scope.sh`](../../scripts/verify_changed_scope.sh) | New gate (hook step 9) with 122 assertions. It builds a temporary git repository with a fake `origin/main` and the real crate graph, and checks every fixture git step. Cases:<br>- every scope, crate and package resources included<br>- every `full` trigger<br>- every fallback: `FULL=1`, an unknown base, no git repository, a `cargo metadata` failure, an unknown crate, a missing file list<br>- renames and ignored files<br>- the hook ignoring the overrides<br>- the scope grammar and `--scope`, including `--scope=…`, a misspelled option and extra arguments<br>- the pure parsers<br>- the lock path, including the `CARGO_BUILD_TARGET_DIR` fallback, and that the default lock is gitignored<br>- the bounded lock wait (exit 75), a symlinked target sharing the real target's lock, an inherited `CDPATH` not redirecting the lock, and no timeout message for a command that exits 75<br>- one real `cargo metadata` case |
| [`scripts/pre-commit.sh`](../../scripts/pre-commit.sh) | Detects the scope once, passes it to steps 7 and 10 with `--scope`, and prints it in its first line and in the summary line (`✅ All Pre-Commit Hooks Passed! Test scope: <scope> (<s>s)`). Adds step 9. |
| [`scripts/setup-dev-environment.sh`](../../scripts/setup-dev-environment.sh) | New session setup script. It is idempotent and bounded (each step needs `timeout` or `gtimeout` for its time limit and warns without one), and logs every step to stdout with RFC 3339 UTC timestamps. A toolchain failure fails the step; there is no `rustup show` fallback. Required steps: the toolchain from `rust-toolchain.toml` plus `wasm32-unknown-unknown`, `cargo fetch --locked`, and `npm ci` in both UI packages. Optional steps: `ocfl-py==2.1.0` with pip when `python3` exists, and the pre-warm `cargo build --workspace --tests --locked` under the same lock and Cargo environment as the hook. |
| [local-verification.md](../../.agents/rules/local-verification.md), [task-workflow.md](../../.agents/rules/task-workflow.md), [pr-review-loop.md](../../.agents/rules/pr-review-loop.md) | Run `FULL=1 bash scripts/pre-commit.sh` before a PR is first marked ready and on the final commit before merge, and record the `Test scope: full` summary line. These rules also describe the scope table, the shared-target lock and the new step 9. |
| [README](../../README.md) | Documents the setup script with its one-line command, the change-aware test scope, `FULL=1`, the lock and `FILESQUID_CARGO_LOCK_WAIT`. |

## Timings

All runs used `CARGO_TARGET_DIR` set to the shared target, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and `TMPDIR` on tmpfs. The Cargo cache was warm. Other agents were building in the same target at the same time (they still ran the old, unlocked hook), so the Rust timings include waits on Cargo's own build-directory lock.

| Run | Scope | Hook total | Lint and test step |
|---|---|---|---|
| Before: every commit ran the full suite (`FULL=1` runs the same steps) | `full` | 310 s | about 300 s: Clippy 57 s, workspace tests 46 s, module crates alone 43 s, rustdoc 65 s, desktop instrumented 62 s, UI packages 27 s |
| After: docs-only commit | `docs` | 10 s | 0 s (all skipped and listed) |
| After: single-crate commit (`filesquid-c2pa`, which nothing depends on) | `rust:filesquid-c2pa` | 108 s | 94 s: Clippy 16 s, tests 55 s, rustdoc 23 s. This figure includes compiling the crate in a fresh clone; a warm rerun is faster |
| Setup script, first run in this session | n/a | 72 s | pre-warm build 57 s |
| Setup script, rerun after the Cubic round 1 fixes (idempotence check) | n/a | 20 s | pre-warm build 8 s; every step passed |

The docs-only and single-crate runs used a scratch clone of this branch. Its local `origin/main` pointed at a commit containing this change, so the diff held only the demonstration edit. On this branch the hook always selects `full`, because `scripts/` changed.

## Known limitation: a shared target across worktrees

In the full run, the cucumber output for `filesquid-api`, `-c2pa`, `-core` and `-metadata` printed feature-file paths from **another agent's worktree** (`Step skipped: <other-agent-worktree>/apps/filesquid/crates/api/tests/features/...`). Those test binaries were built from that worktree's sources. Two checkouts of one workspace give their crates the same artifact names in a shared target, and Cargo judges freshness by relative path and modification time. So Cargo can reuse, or another process can overwrite, a binary built from different sources. The new lock covers the build and the test run of every locked cargo command, which stops concurrent runs that use the lock from corrupting each other. It cannot protect against processes that do not take it (every agent still on the old hook), and it does not stop Cargo from treating another worktree's artifact as fresh. This branch changes no Rust or TypeScript source, so its own result does not depend on this. It is still an owner decision; see the PR description.

The pre-commit run on the staged branch showed both effects. The first `FULL=1` attempt failed in `Module crate alone: filesquid-api`, with ``could not find `library` in `filesquid_api` ``: the test targets compiled against a `filesquid-api` library that another agent's unlocked `cargo test -p filesquid-api` had just rebuilt from a branch without that module. The immediate rerun passed (`✅ All Pre-Commit Hooks Passed! Test scope: full (248s)`). Its output still held 270 feature paths from the other worktree and 21 from this one.

## Mocked Environment Variables

None. No credentials or dummy values were used.

## Review fixes (Cubic round 1)

Cubic's first review raised 20 findings, and all were fixed:
- crate and package `resources/` and READMEs now count as code;
- the default lock file is gitignored;
- the toolchain step has no masking fallback;
- the lock is fd-based, reads `CARGO_BUILD_TARGET_DIR`, and never mislabels a command's own exit 75;
- the scope grammar is strict, and the scope is detected once and passed with `--scope`;
- formatting failures name their cause;
- the fixture git steps are checked;
- the documentation is reworded where it overstated the lock, the timeouts, the docs scope or the step 7 exception.

Cubic's second review raised 8 findings, and all were fixed:
- the lock key is the target's physical path, so a symlink and its real path share one lock;
- `--scope` handling is strict: any other argument runs `full` with a message;
- the setup toolchain step adds `rustfmt` and `clippy` explicitly;
- the setup script blames an old rustup only when the detected version is older than 1.28;
- the documentation says exactly which commands are timed and which are locked.

Cubic's third review raised 3 findings, and all were fixed:
- the gates clear an inherited `CDPATH`, and the physical-path lookup uses `CDPATH='' cd --`;
- creating the target directory is its own named step (`create_target_dir`), separate from `physical_dir_path`;
- the README documents that the lock sits beside the physical target path.

The runs below were recorded before these fixes. The scope logic they show is unchanged for the edits they made (a root `resources/tasks/` file and `crates/c2pa/src/lib.rs`). Only the reason text of the docs scope now adds "outside the crates and npm packages".

## Raw output (complete): docs-only run (`bash scripts/pre-commit.sh`, one Markdown edit)

Run at 2026-10-18T17:29:12Z. Absolute paths are replaced with `<repo>`, `<demo-clone>`, `<other-agent-worktree>` and `<CARGO_TARGET_DIR>`. This is the complete output.

```text
========================================
Running Pre-Commit Hooks... (test scope: docs)
========================================
✅ Branch Validation passed.
Starting Port Isolation Validation...
🔍 Validating ./apps/filesquid/ui (Vite)...
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
🔍 Validating ./apps/imagesquid (Vite)...
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
🔍 Validating filesquid-server (default bind)...
✅ filesquid-server uniquely bound to 8083

🚀 VALIDATION PASSED: All modules maintain strict port isolation.
Starting Document Sync Validation...
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 95 PRD requirements, 82 resolved SRS references.
Starting Portable Documentation Validation...
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: docs)
🔍 Validating Prettier formatting in ./apps/filesquid/ui...
✅ ./apps/filesquid/ui is properly formatted.
🔍 Validating Prettier formatting in ./apps/imagesquid...
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
Starting Documentation Validation...
🔍 ADR structure (ISO 42010)...
TAP version 13
# Subtest: stack ADRs exist with ISO 42010 sections and accepted vocabulary
ok 1 - stack ADRs exist with ISO 42010 sections and accepted vocabulary
  ---
  duration_ms: 9.658416
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.291564
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 1.35588
  type: 'test'
  ...
1..3
# tests 3
# suites 0
# pass 3
# fail 0
# cancelled 0
# skipped 0
# todo 0
# duration_ms 100.415379
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
FileSquid: 260 vision IDs, 260 allocated, 130 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
🔍 Plan traceability...
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 186 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 161 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (52 assertions).
Starting Lint and Test Validation...
changed_scope: docs (only Markdown, resources/ or .agents/ files changed)
ℹ️  Test scope: docs
⏭️  Skipped: Clippy (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Workspace tests (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-core (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-ocfl (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-metadata (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-c2pa (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-api (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-native (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Rustdoc (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Clippy (desktop, test-instrumented) (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Desktop tests (test-instrumented) (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/filesquid/ui apps/imagesquid (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
⏭️  Skipped: Desktop packaged-window e2e (docs-only change: only Markdown, resources/ or .agents/ files changed; set FULL=1 to run)
✅ Lint and test validation passed (test scope: docs).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: docs (10s)
========================================
```

## Output excerpt: single-crate run (`bash scripts/pre-commit.sh`, one edit in `crates/c2pa/src/lib.rs`)

Run at 2026-10-18T17:33:38Z. This is an excerpt of the raw output (739 lines in all): every hook line is kept verbatim, and Cargo compile and progress lines and per-test lines are left out. The complete log was not committed because it is mostly cargo noise and holds machine-specific absolute paths ([links-enforce.md](../../.agents/rules/links-enforce.md)).

```text
========================================
Running Pre-Commit Hooks... (test scope: rust:filesquid-c2pa)
========================================
✅ Branch Validation passed.
Starting Port Isolation Validation...
🔍 Validating ./apps/filesquid/ui (Vite)...
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
🔍 Validating ./apps/imagesquid (Vite)...
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
🔍 Validating filesquid-server (default bind)...
✅ filesquid-server uniquely bound to 8083
🚀 VALIDATION PASSED: All modules maintain strict port isolation.
Starting Document Sync Validation...
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 95 PRD requirements, 82 resolved SRS references.
Starting Portable Documentation Validation...
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
🔍 Validating Rust formatting in ./apps/filesquid...
✅ ./apps/filesquid is properly formatted.
🔍 Validating Prettier formatting in ./apps/filesquid/ui...
✅ ./apps/filesquid/ui is properly formatted.
🔍 Validating Prettier formatting in ./apps/imagesquid...
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
Starting Documentation Validation...
🔍 ADR structure (ISO 42010)...
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
FileSquid: 260 vision IDs, 260 allocated, 130 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
🔍 Plan traceability...
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 186 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 161 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (52 assertions).
Starting Lint and Test Validation...
changed_scope: rust:filesquid-c2pa (changed: crates/c2pa)
ℹ️  Test scope: rust:filesquid-c2pa
ℹ️  Rust scope: filesquid-c2pa
🔍 Clippy (scoped) (apps/filesquid)...
✅ Clippy (scoped) passed (16s).
🔍 Scoped tests (apps/filesquid)...
✅ Scoped tests passed (55s).
⏭️  Skipped: Module crate alone: filesquid-core (not in the change scope)
⏭️  Skipped: Module crate alone: filesquid-ocfl (not in the change scope)
⏭️  Skipped: Module crate alone: filesquid-metadata (not in the change scope)
⏭️  Skipped: Module crate alone: filesquid-c2pa (identical to the scoped test run above)
⏭️  Skipped: Module crate alone: filesquid-api (not in the change scope)
⏭️  Skipped: Module crate alone: filesquid-native (not in the change scope)
🔍 Rustdoc (scoped) (apps/filesquid)...
✅ Rustdoc (scoped) passed (23s).
⏭️  Skipped: Clippy (desktop, test-instrumented) (filesquid-desktop is not in the change scope)
⏭️  Skipped: Desktop tests (test-instrumented) (filesquid-desktop is not in the change scope)
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/filesquid/ui (not in the change scope)
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/imagesquid (not in the change scope)
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: rust:filesquid-c2pa).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: rust:filesquid-c2pa (108s)
========================================
```

## Output excerpt: full run (`FULL=1 bash scripts/pre-commit.sh` on this branch)

This is an excerpt of the raw output (7,360 lines in all): the hook and step result lines verbatim, with the cucumber scenario totals. The individual test lines are left out, for the same reasons as above.

```text
========================================
Running Pre-Commit Hooks... (test scope: full)
========================================
Starting Port Isolation Validation...
Starting Document Sync Validation...
Starting PRD/SRS Traceability Validation...
Starting Portable Documentation Validation...
Starting Workspace Boundary Validation...
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
Starting Documentation Validation...
Starting Change Scope Validation...
✅ Change scope validation passed (52 assertions).
Starting Lint and Test Validation...
changed_scope: full (FULL=1 is set)
ℹ️  Test scope: full
✅ Clippy passed (57s).
195 scenarios (75 passed, 120 skipped)
38 scenarios (30 passed, 8 skipped)
7 scenarios (7 passed)
63 scenarios (59 passed, 4 skipped)
18 scenarios (15 passed, 3 skipped)
3 scenarios (3 passed)
100 scenarios (97 passed, 3 skipped)
✅ Workspace tests passed (46s).
63 scenarios (59 passed, 4 skipped)
✅ Module crate alone: filesquid-core passed (1s).
100 scenarios (97 passed, 3 skipped)
✅ Module crate alone: filesquid-ocfl passed (30s).
18 scenarios (15 passed, 3 skipped)
✅ Module crate alone: filesquid-metadata passed (3s).
38 scenarios (30 passed, 8 skipped)
✅ Module crate alone: filesquid-c2pa passed (5s).
195 scenarios (75 passed, 120 skipped)
✅ Module crate alone: filesquid-api passed (3s).
3 scenarios (3 passed)
✅ Module crate alone: filesquid-native passed (1s).
✅ Rustdoc passed (65s).
✅ Clippy (desktop, test-instrumented) passed (38s).
9 scenarios (9 passed)
✅ Desktop tests (test-instrumented) passed (24s).
✅ ESLint passed (4s).
✅ TypeScript passed (5s).
 Test Files  20 passed (20)
      Tests  328 passed (328)
✅ Vitest passed (11s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
✅ ESLint passed (1s).
✅ TypeScript passed (3s).
 Test Files  2 passed (2)
      Tests  13 passed (13)
✅ Vitest passed (3s).
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: full).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: full (310s)
========================================
```

## Raw output (complete): `bash scripts/setup-dev-environment.sh` (first run in this session)

```text
[2026-10-18T17:23:38Z] FileSquid development environment setup
[2026-10-18T17:23:38Z] ▶ Rust toolchain (rust-toolchain.toml) and wasm32 target
info: using existing install for 'stable-x86_64-unknown-linux-gnu'
info: the active toolchain `stable-x86_64-unknown-linux-gnu` has been installed
info: it's active because: overridden by '<repo>/apps/filesquid/rust-toolchain.toml'
info: component 'rust-std' for target 'wasm32-unknown-unknown' is up to date
[2026-10-18T17:23:39Z] ✅ Rust toolchain (rust-toolchain.toml) and wasm32 target (1s)
[2026-10-18T17:23:39Z] ▶ cargo fetch --locked
    Blocking waiting for file lock on package cache
 Downloading crates ...
  Downloaded objc2-javascript-core v0.3.2
  Downloaded objc2-core-video v0.3.2
  Downloaded objc2-security v0.3.2
[2026-10-18T17:23:41Z] ✅ cargo fetch --locked (2s)
[2026-10-18T17:23:41Z] ▶ npm ci in apps/filesquid/ui
npm warn deprecated eslint@9.39.5: This version is no longer supported. Please see https://eslint.org/version-support for other options.

added 224 packages in 6s
[2026-10-18T17:23:47Z] ✅ npm ci in apps/filesquid/ui (6s)
[2026-10-18T17:23:47Z] ▶ npm ci in apps/imagesquid
npm warn deprecated eslint@9.39.5: This version is no longer supported. Please see https://eslint.org/version-support for other options.

added 202 packages in 5s
[2026-10-18T17:23:52Z] ✅ npm ci in apps/imagesquid (5s)
[2026-10-18T17:23:52Z] ▶ ocfl-py 2.1.0 (community OCFL validator) (optional)
[2026-10-18T17:23:53Z] ocfl-py 2.1.0 already installed
[2026-10-18T17:23:53Z] ✅ ocfl-py 2.1.0 (community OCFL validator) (1s)
[2026-10-18T17:23:53Z] ▶ Pre-warm Cargo target (cargo build --workspace --tests --locked) (optional)
[2026-10-18T17:23:53Z] CARGO_TARGET_DIR=<CARGO_TARGET_DIR> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
    Blocking waiting for file lock on build directory
   Compiling filesquid-desktop v0.1.0 (<repo>/apps/filesquid/crates/desktop)
   Compiling filesquid-ocfl v0.1.0 (<repo>/apps/filesquid/crates/ocfl)
    Finished `dev` profile [unoptimized] target(s) in 57.10s
[2026-10-18T17:24:50Z] ✅ Pre-warm Cargo target (cargo build --workspace --tests --locked) (57s)
[2026-10-18T17:24:50Z] ✅ Setup complete (72s)
```

## Scope helper tests (`bash scripts/verify_changed_scope.sh`)

```text
Starting Change Scope Validation...
✅ Change scope validation passed (122 assertions).
```
