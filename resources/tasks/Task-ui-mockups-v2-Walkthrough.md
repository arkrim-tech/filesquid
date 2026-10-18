# Task ui-mockups-v2: Walkthrough

Documentation-only task (design artifacts). Output: 37 SVG mockups and an index in [`resources/ui-mockups/v2/`](../ui-mockups/v2/README.md).

## What was done

- Worked out the MVP scope from [C-056](../product-vision.md#release-sequence), [ADR 035](../architecture/035-macos-first-desktop-mvp.md) and phases F0 to F3 of the [implementation plan](../implementation-plan.md), down to the module rows (UI-0.x, UI-F2.1 to UI-F2.13, UI-1.1, UI-1.2, API-F1.3 to API-F2.5, CL-1.1, CL-1.4, DE-0.4, DE-0.5, DE-1.1, DE-1.2, DE-F3.1, IO-1.1, MD-0.7). Anything assigned to phase F3R, F4 or I1, or with no MVP task, was left out. The v2 README lists what was removed from v1 and why.
- Drew 34 distinct screens plus 3 dark variants at 1440 × 900 in a macOS window, following the [FileSquid Design Guide](../assets/FileSquid-design-guide-v1.html) tokens and the v1 layouts.
- Used the coloured logo [`FileSquid-logo.svg`](../assets/FileSquid-logo.svg), at the owner's request after a black-and-white draft: coloured shapes in both modes, with a black outline and wordmark on light surfaces and a white one on dark surfaces.
- Did not mark v2 as the active suite in the [mockups README](../ui-mockups/README.md). The owner approves that, because an active suite becomes an SRS requirement source.

## How the mockups were produced

A Python generator in the session scratch area (not committed, as for v1) holds the tokens, icons and components, so every screen uses the same values. Each SVG was rendered with Chromium and checked for overlap, clipping and token misuse, then fixed and re-rendered.

## Verification

```text
$ for f in resources/ui-mockups/v2/*.svg; do xmllint --noout "$f"; done
(no output: all 37 files parse)

$ ./scripts/verify_isolated_ports.sh       -> 🚀 VALIDATION PASSED
$ ./scripts/verify_doc_sync.sh             -> ✅ Document Sync Validation passed.
$ ./scripts/verify_srs_traceability.sh     -> ✅ Traceability Validation passed: 95 PRD requirements, 81 resolved SRS references.
$ ./scripts/verify_relative_links.sh       -> ✅ Portable Documentation validation passed.
$ ./scripts/verify_workspace_boundaries.sh -> ✅ Workspace Boundary validation passed.
$ ./scripts/verify_centralized_logging.sh  -> ✅ Logging rule validation passed (no relevant staged files).
$ ./scripts/verify_formatting.sh           -> ✅ Format validation passed.
$ ./scripts/verify_docs.sh                 -> ✅ Documentation validation passed.
$ ./scripts/verify_tests.sh                -> ✅ Lint and test validation passed.
```

The test gate was rerun on 2026-10-17 on macOS, in a fresh worktree of this branch. The first run of `verify_tests.sh` failed two `filesquid-c2pa` wasm32 build scenarios because the Rust target was not installed. Raw output of the failing `filesquid-c2pa` section (absolute paths shortened to `./`):

```text
  Scenario: Engine builds for the browser
   ✔  When I build filesquid-c2pa for wasm32-unknown-unknown with default features off
   ✘  Then the build should succeed
      Step failed:
      Defined: tests/features/reusable_provenance_engine_F-054.feature:43:5
      Matched: crates/c2pa/tests/features.rs:145:1
      Step panicked. Captured output: wasm32 build check failed:
      Rust target wasm32-unknown-unknown is not installed: run `rustup target add wasm32-unknown-unknown` (or set FILESQUID_SKIP_WASM32=1 to skip this check explicitly)
  Scenario: Engine builds for the browser with the default crypto backend
   ✔  When I build filesquid-c2pa for wasm32-unknown-unknown with default features on
   ✘  Then the build should succeed
      Step failed:
      Defined: tests/features/reusable_provenance_engine_F-054.feature:49:5
      Matched: crates/c2pa/tests/features.rs:145:1
      Step panicked. Captured output: wasm32 build check failed:
      Rust target wasm32-unknown-unknown is not installed: run `rustup target add wasm32-unknown-unknown` (or set FILESQUID_SKIP_WASM32=1 to skip this check explicitly)
[Summary]
1 feature
23 scenarios (11 passed, 10 skipped, 2 failed)
44 steps (32 passed, 10 skipped, 2 failed)
error: test failed, to rerun pass `-p filesquid-c2pa --test features`

Caused by:
  process didn't exit successfully: `./apps/filesquid/target/debug/deps/features-28d2f95f507417d4` (exit status: 1)
❌ Workspace tests failed.
```

After `rustup target add wasm32-unknown-unknown` and `npm ci` in both UI packages (the new worktree had no `node_modules`), the gate passed. Raw output of the rerun: the `filesquid-c2pa` wasm32 scenarios and summary, then the end of the gate. Playwright e2e was skipped (`FILESQUID_E2E` not set).

```text
  Scenario: Engine builds for the browser
   ✔  When I build filesquid-c2pa for wasm32-unknown-unknown with default features off
   ✔  Then the build should succeed
  Scenario: Engine builds for the browser with the default crypto backend
   ✔  When I build filesquid-c2pa for wasm32-unknown-unknown with default features on
   ✔  Then the build should succeed
[Summary]
1 feature
23 scenarios (13 passed, 10 skipped)
44 steps (34 passed, 10 skipped)
...
✅ Rustdoc passed.
🔍 ESLint (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 lint
> eslint .

✅ ESLint passed.
🔍 TypeScript (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 check
> tsc --noEmit

✅ TypeScript passed.
🔍 Vitest (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 pretest
> rm -rf tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 ./apps/filesquid/ui

 ✓ tests/unit/theme.test.ts (6 tests) 3ms
 ✓ tests/unit/client.test.ts (8 tests) 4ms
 ✓ tests/unit/admin-client.test.ts (7 tests) 7ms
 ✓ tests/unit/contract.test.ts (1 test) 26ms
 ✓ tests/ui/monitor-route.test.ts (3 tests) 9ms
 ✓ tests/ui/app-shell.test.ts (4 tests) 19ms
 ✓ tests/ui/monitor.test.ts (8 tests) 64ms

 Test Files  7 passed (7)
      Tests  37 passed (37)
   Start at  16:47:10
   Duration  501ms (transform 136ms, setup 0ms, import 529ms, tests 133ms, environment 1.56s)

JUNIT report written to ./apps/filesquid/ui/tests/allure-results/vitest-junit.xml
✅ Vitest passed.
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
🔍 ESLint (apps/imagesquid)...

> @imagesquid/ui@0.1.0 lint
> eslint .

✅ ESLint passed.
🔍 TypeScript (apps/imagesquid)...

> @imagesquid/ui@0.1.0 check
> tsc --noEmit

✅ TypeScript passed.
🔍 Vitest (apps/imagesquid)...

> @imagesquid/ui@0.1.0 pretest
> rm -rf tests/allure-results


> @imagesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 ./apps/imagesquid

 ✓ tests/unit/theme.test.ts (4 tests) 2ms
 ✓ tests/ui/distribution.test.ts (1 test) 10ms

 Test Files  2 passed (2)
      Tests  5 passed (5)
   Start at  16:47:13
   Duration  339ms (transform 32ms, setup 0ms, import 45ms, tests 12ms, environment 242ms)

JUNIT report written to ./apps/imagesquid/tests/allure-results/vitest-junit.xml
✅ Vitest passed.
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
✅ Lint and test validation passed.
```

`scripts/verify_branch.sh` (step 0) rejected the original session branch name `claude/stoic-gates-famwt6`. With the owner's approval it was renamed to `docs/ui-mockups-v2`, which closed the first pull request ([#15](https://github.com/arkrim-tech/filesquid/pull/15)); review continues on a new pull request from the renamed branch.

## Open questions for the owner

- Approve v2 as the active suite, or trim it first.
- Whether the New menu on the desktop should say "Import" (as drawn) or "Upload" (as UI-S-006 words it).
- Whether the People search chip should map to `creator:` on the desktop; v2 leaves it out.
