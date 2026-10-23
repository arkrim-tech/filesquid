# Task build/imagesquid-checks-mvp-scope Walkthrough

## UI-S-005: A distribution's label table MUST define every key of the default table, and its token file every default token; a unit test MUST fail otherwise

Also covered: UI-S-015 (WCAG 2.2 AA for a distribution's tokens, with no contrast exemption for ImageSquid) and SRS-UI-007 (light and dark themes). Both are in [`apps/filesquid/ui/resources/srs.md`](../../apps/filesquid/ui/resources/srs.md) and [`resources/srs.md`](../srs.md#srs-ui-007-light-and-dark-themes).

### Owner decision

2026-10-23, verbatim: "for ImageSquid checks decision use option 2". It answers the owner's question, verbatim: "why are we running imagesquid unit tests in the filesquid project, imagesquid is release 2". Option 2, as offered, verbatim: "Narrow it for the MVP (my recommendation). Drop ImageSquid's lint, typecheck, Vitest and Playwright from the hook. Keep one cheap completeness test inside the FileSquid UI package that reads ImageSquid's label and token tables. Then MVP UI tasks only add keys to that table, and the full ImageSquid suite comes back when ImageSquid work starts." Recorded in [local-verification.md](../../.agents/rules/local-verification.md#imagesquid-during-the-mvp).

### What changed

- `scripts/verify_tests.sh`: step 10 runs ESLint, `tsc`, Vitest and (with `FILESQUID_E2E=1`) Playwright for `apps/filesquid/ui` only, and prints one skipped line for `apps/imagesquid` with the reason. `apps/imagesquid` keeps its npm scripts for running by hand.
- `scripts/changed_scope.sh`: `NPM_PACKAGES` is `apps/filesquid/ui`. `GUARDED_PACKAGES` maps `apps/imagesquid` to `apps/filesquid/ui`, so any change confined to `apps/imagesquid` (sources, tests, tables, docs, top-level `package.json` and `package-lock.json`) gives `npm:apps/filesquid/ui`. A nested manifest there still gives `full`. The `file:`-link expansion is removed: with one tested package it had no effect, and a `@filesquid/ui` change no longer adds ImageSquid. The scope grammar is unchanged and every doubt still gives `full`.
- `scripts/verify_changed_scope.sh`: the ImageSquid expectations changed to `npm:apps/filesquid/ui`, and a new `test_imagesquid_paths` covers the label table, the token file, ImageSquid's tests, docs, both top-level manifests, mixed scopes, a nested manifest (`full`) and the Cargo manifest (`full`). These were written first and failed (below).
- `apps/filesquid/ui/tests/unit/distribution-tables.test.ts` (new): reads ImageSquid's `src/theme/labels.en.json` and `src/theme/tokens.css` from its directory, with no ImageSquid install, and fails on a missing or empty default label key (both against `REQUIRED_LABEL_KEYS` and the keys of the default `labels.en.json`), a missing required token or a token the default file declares (light and dark), an unreadable colour token, a WCAG 2.2 AA failure with no exemption, or dark values that differ between the OS preference and `data-theme="dark"`. A missing table file fails the test; it is never skipped.
- `apps/imagesquid/tests/unit/theme.test.ts`: the checks moved to the new test (required keys, required tokens, contrast with and without an empty exemption list, readable colour tokens, matching dark blocks) are removed here, not duplicated. Its vocabulary, IS-S-004 wording, theme-control wording, Arkrim primary (IS-S-008) and spacing checks stay. The removed line `expect(light["--fs-primary"]).not.toBe("#16ac49")` is implied by two checks that remain: the exemption-free contrast check (that green on white is 2.99:1) and the Arkrim primary check (`#005db6`). `apps/imagesquid/tests/ui/distribution.test.ts` is unchanged: it renders the shell and checks ImageSquid's brand and vocabulary, not table completeness.
- `scripts/verify_formatting.sh`: Prettier for a package listed in `PRETTIER_FROM` runs from another package's `node_modules`; `apps/imagesquid` uses `apps/filesquid/ui`'s Prettier (both pin 3.9.10) with ImageSquid's own `.prettierrc.json` and `.prettierignore`. Every package's check now calls `node_modules/.bin/prettier` directly instead of `npx`, so a missing install fails with the `npm ci` instruction and never downloads a package.
- `scripts/setup-dev-environment.sh`: unchanged. No step fails without an ImageSquid install, and setup still installs both packages so ImageSquid's scripts can run by hand.
- Rules and docs: [local-verification.md](../../.agents/rules/local-verification.md) (gate table, scope table, new section "ImageSquid during the MVP" with the decision verbatim and dated), [lint-enforce.md](../../.agents/rules/lint-enforce.md) (FileSquid UI ESLint only; ImageSquid formatted with `@filesquid/ui`'s Prettier), the root [README](../../README.md) (install note, `npm:` scope bullet, `FILESQUID_E2E`, the UI test block and the completeness test command), [`apps/filesquid/ui/README.md`](../../apps/filesquid/ui/README.md), a new `apps/imagesquid/README.md`, the orchestrator and UI engineer agent files under `.claude/agents/`, an amendment line in [ADR 019](../architecture/019-coding-style-and-lint.md), one sentence in [Task FS-3.8](../implementation-plan.md#task-fs-3.8) and one in ImageSquid's own plan, and two error scenarios in [`theme_tokens_SRS-UI-007.feature`](../../apps/filesquid/ui/tests/features/theme_tokens_SRS-UI-007.feature).

### SRS traceability

| Requirement | Test |
|---|---|
| UI-S-005 label keys | `distribution-tables.test.ts`: "defines every required key", "defines every key of the default table, none empty" |
| UI-S-005 tokens, light and dark | `distribution-tables.test.ts`: "define every required token for light and dark", "declare every token the default token file declares, in light and in dark", "apply the same dark values for the OS preference and for data-theme=dark" |
| UI-S-015 and SRS-UI-007, no exemption for ImageSquid | `distribution-tables.test.ts`: "meet WCAG 2.2 AA for every checked pair in light and dark, with no exemption", "write every colour token as #rgb or #rrggbb in both themes" |
| Hook scope for ImageSquid changes | `scripts/verify_changed_scope.sh`: `test_imagesquid_paths`, `test_npm_manifests`, `test_file_lists`, `test_git_changes`, `test_hook_ignores_overrides` |

TDD-ENFORCE: the scope cases and the completeness test failed before the change (below). Coverage: the UI package has no coverage threshold configured; the change adds tests and removes no assertion from the hook's FileSquid suite.

### Mocked Environment Variables

None.

### Scope tests written first: failing run

`TMPDIR=<tmpdir> bash scripts/verify_changed_scope.sh`, with the new expectations and the old `changed_scope.sh`:

```text
Starting Change Scope Validation...
❌ files [apps/filesquid/ui/resources/tasks/Task-x-Walkthrough.md]: expected 'npm:apps/filesquid/ui', got 'npm:apps/filesquid/ui,apps/imagesquid'
❌ files [apps/imagesquid/resources/prd.md]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/filesquid/ui/src/main.ts]: expected 'npm:apps/filesquid/ui', got 'npm:apps/filesquid/ui,apps/imagesquid'
❌ files [apps/filesquid/ui/README.md]: expected 'npm:apps/filesquid/ui', got 'npm:apps/filesquid/ui,apps/imagesquid'
❌ files [apps/imagesquid/src/main.ts]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/filesquid/crates/c2pa/src/lib.rs apps/imagesquid/src/main.ts]: expected 'rust:filesquid-c2pa npm:apps/filesquid/ui', got 'rust:filesquid-c2pa npm:apps/imagesquid'
❌ files [apps/imagesquid/src/theme/labels.en.json]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/imagesquid/src/theme/tokens.css]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/imagesquid/tests/unit/theme.test.ts]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/imagesquid/tests/ui/distribution.test.ts]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/imagesquid/README.md apps/imagesquid/distribution.json]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/imagesquid/src/main.ts apps/filesquid/ui/src/main.ts]: expected 'npm:apps/filesquid/ui', got 'npm:apps/filesquid/ui,apps/imagesquid'
❌ files [apps/imagesquid/src/theme/labels.en.json README.md .agents/rules/rule.md]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/imagesquid/src/theme/tokens.css apps/filesquid/crates/server/src/main.rs]: expected 'rust:filesquid-server npm:apps/filesquid/ui', got 'rust:filesquid-server npm:apps/imagesquid'
❌ files [apps/filesquid/ui/package-lock.json]: expected 'npm:apps/filesquid/ui', got 'npm:apps/filesquid/ui,apps/imagesquid'
❌ files [apps/filesquid/ui/package.json]: expected 'npm:apps/filesquid/ui', got 'npm:apps/filesquid/ui,apps/imagesquid'
❌ files [apps/imagesquid/package.json]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/imagesquid/package-lock.json apps/imagesquid/src/main.ts]: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'
❌ files [apps/imagesquid/package-lock.json apps/filesquid/ui/package-lock.json]: expected 'npm:apps/filesquid/ui', got 'npm:apps/filesquid/ui,apps/imagesquid'
❌ files [apps/filesquid/ui/package-lock.json apps/filesquid/crates/c2pa/src/lib.rs]: expected 'rust:filesquid-c2pa npm:apps/filesquid/ui', got 'rust:filesquid-c2pa npm:apps/filesquid/ui,apps/imagesquid'
❌ files [.claude/agents/x.md docs/x.md apps/filesquid/ui/package-lock.json]: expected 'npm:apps/filesquid/ui', got 'npm:apps/filesquid/ui,apps/imagesquid'
❌ staged and unstaged: expected 'rust:filesquid-c2pa npm:apps/filesquid/ui', got 'rust:filesquid-c2pa npm:apps/imagesquid'
❌ hook ignores CHANGED_SCOPE_* overrides: expected 'npm:apps/filesquid/ui', got 'npm:apps/imagesquid'

🚨 VALIDATION FAILED: scripts/changed_scope.sh or a hook library broke an expectation above.
```

### Scope tests after the change

```text
Starting Change Scope Validation...
✅ Change scope validation passed (179 assertions).
exit 0
```

### Completeness test fails on a broken ImageSquid table

ImageSquid's tables were broken temporarily (label `virtualExhibits` removed, `monitorFilter` emptied, the dark `--fs-error` removed, light `--fs-primary` set to FileSquid's `#16ac49`), then restored. From `apps/filesquid/ui`, `npx vitest run tests/unit/distribution-tables.test.ts`:

```text

 RUN  v4.1.12 <repo>/apps/filesquid/ui

 ❯ tests/unit/distribution-tables.test.ts (7 tests | 6 failed) 30ms
     × defines every required key 13ms
     × defines every key of the default table, none empty 2ms
     × define every required token for light and dark 3ms
     × declare every token the default token file declares, in light and in dark 2ms
     ✓ write every colour token as #rgb or #rrggbb in both themes 1ms
     × meet WCAG 2.2 AA for every checked pair in light and dark, with no exemption 3ms
     × apply the same dark values for the OS preference and for data-theme=dark 3ms

⎯⎯⎯⎯⎯⎯⎯ Failed Tests 6 ⎯⎯⎯⎯⎯⎯⎯

 FAIL  tests/unit/distribution-tables.test.ts > 'ImageSquid' label table (UI-S-005) > defines every required key
AssertionError: expected [ 'virtualExhibits', 'monitorFilter' ] to deeply equal []

- Expected
+ Received

- []
+ [
+   "virtualExhibits",
+   "monitorFilter",
+ ]

 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/unit/distribution-tables.test.ts:80:35
     78|
     79|   it("defines every required key", () => {
     80|     expect(missingLabels(labels)).toEqual([]);
       |                                   ^
     81|   });
     82|   it("defines every key of the default table, none empty", () => {

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[1/6]⎯

 FAIL  tests/unit/distribution-tables.test.ts > 'ImageSquid' label table (UI-S-005) > defines every key of the default table, none empty
AssertionError: expected [ 'virtualExhibits', 'monitorFilter' ] to deeply equal []

- Expected
+ Received

- []
+ [
+   "virtualExhibits",
+   "monitorFilter",
+ ]

 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/unit/distribution-tables.test.ts:83:60
     81|   });
     82|   it("defines every key of the default table, none empty", () => {
     83|     expect(absentKeys(Object.keys(defaultLabels), labels)).toEqual([]);
       |                                                            ^
     84|   });
     85| });

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[2/6]⎯

 FAIL  tests/unit/distribution-tables.test.ts > 'ImageSquid' design tokens (UI-S-005, UI-S-015) > define every required token for light and dark
AssertionError: expected { light: [], dark: [ '--fs-error' ] } to deeply equal { light: [], dark: [] }

- Expected
+ Received

  {
-   "dark": [],
+   "dark": [
+     "--fs-error",
+   ],
    "light": [],
  }

 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/unit/distribution-tables.test.ts:91:38
     89|
     90|   it("define every required token for light and dark", () => {
     91|     expect(missingTokens(tokensCss)).toEqual({ light: [], dark: [] });
       |                                      ^
     92|   });
     93|   it("declare every token the default token file declares, in light an…

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[3/6]⎯

 FAIL  tests/unit/distribution-tables.test.ts > 'ImageSquid' design tokens (UI-S-005, UI-S-015) > declare every token the default token file declares, in light and in dark
AssertionError: expected [ '--fs-error' ] to deeply equal []

- Expected
+ Received

- []
+ [
+   "--fs-error",
+ ]

 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/unit/distribution-tables.test.ts:97:80
     95|     const own = tokenValues(tokensCss);
     96|     expect(Object.keys(defaults.light).filter((token) => !(token in ow…
     97|     expect(Object.keys(defaults.dark).filter((token) => !(token in own…
       |                                                                                ^
     98|   });
     99|   it("write every colour token as #rgb or #rrggbb in both themes", () …

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[4/6]⎯

 FAIL  tests/unit/distribution-tables.test.ts > 'ImageSquid' design tokens (UI-S-005, UI-S-015) > meet WCAG 2.2 AA for every checked pair in light and dark, with no exemption
AssertionError: expected [ { theme: 'light', …(3) }, …(6) ] to deeply equal []

- Expected
+ Received

- []
+ [
+   {
+     "background": "--fs-surface",
+     "foreground": "--fs-primary",
+     "ratio": 2.99,
+     "theme": "light",
+   },
+   {
+     "background": "--fs-surface-card",
+     "foreground": "--fs-primary",
+     "ratio": 2.74,
+     "theme": "light",
+   },
+   {
+     "background": "--fs-surface-overlay",
+     "foreground": "--fs-primary",
+     "ratio": 2.99,
+     "theme": "light",
+   },
+   {
+     "background": "--fs-primary",
+     "foreground": "--fs-on-primary",
+     "ratio": 2.99,
+     "theme": "light",
+   },
+   {
+     "background": "--fs-surface",
+     "foreground": "--fs-error",
+     "ratio": 2.8,
+     "theme": "dark",
+   },
+   {
+     "background": "--fs-surface-card",
+     "foreground": "--fs-error",
+     "ratio": 2.54,
+     "theme": "dark",
+   },
+   {
+     "background": "--fs-surface-container",
+     "foreground": "--fs-error",
+     "ratio": 2.54,
+     "theme": "dark",
+   },
+ ]

 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/unit/distribution-tables.test.ts:103:41
    101|   });
    102|   it("meet WCAG 2.2 AA for every checked pair in light and dark, with …
    103|     expect(contrastFailures(tokensCss)).toEqual([]);
       |                                         ^
    104|     expect(contrastFailures(tokensCss, [])).toEqual([]);
    105|   });

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[5/6]⎯

 FAIL  tests/unit/distribution-tables.test.ts > 'ImageSquid' design tokens (UI-S-005, UI-S-015) > apply the same dark values for the OS preference and for data-theme=dark
AssertionError: expected { '--fs-surface': '#121418', …(31) } to deeply equal { '--fs-surface': '#121418', …(30) }

- Expected
+ Received

@@ -1,10 +1,11 @@
  {
    "--fs-badge-data": "#fcd805",
    "--fs-badge-document": "#b42318",
    "--fs-badge-media": "#005db6",
    "--fs-brand": "#a9c7ff",
+   "--fs-error": "#ffb4ab",
    "--fs-focus-ring": "#a9c7ff",
    "--fs-icon": "#a6adb8",
    "--fs-icon-disabled": "#3a3f48",
    "--fs-icon-muted": "#858c96",
    "--fs-inverse-surface": "#2a2e36",

 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/unit/distribution-tables.test.ts:112:23
    110|     expect(media).not.toBeNull();
    111|     const fromMedia = tokenValues(`:root[data-theme="dark"] {${media?.…
    112|     expect(fromMedia).toEqual(tokenValues(tokensCss).dark);
       |                       ^
    113|   });
    114| });

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[6/6]⎯


 Test Files  1 failed (1)
      Tests  6 failed | 1 passed (7)
   Start at  20:33:55
   Duration  491ms (transform 196ms, setup 88ms, import 174ms, tests 30ms, environment 0ms)

JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
exit 1
```

### Completeness test on the real tables

```text

 RUN  v4.1.12 <repo>/apps/filesquid/ui

 ✓ tests/unit/distribution-tables.test.ts (7 tests) 13ms

 Test Files  1 passed (1)
      Tests  7 passed (7)
   Start at  20:45:45
   Duration  458ms (transform 224ms, setup 60ms, import 225ms, tests 13ms, environment 0ms)

JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
exit 0
```

### Format step without an ImageSquid install

`apps/imagesquid/node_modules` absent. `bash scripts/verify_formatting.sh --scope docs` with a scratch, badly formatted `apps/imagesquid/src/zz-scratch.ts` (removed afterwards):

```text
Starting Format Validation...
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: docs)
🔍 Validating Prettier formatting in ./apps/filesquid/ui...
✅ ./apps/filesquid/ui is properly formatted.
🔍 Validating Prettier formatting in ./apps/imagesquid...
❌ ERROR (./apps/imagesquid): Prettier format violation. Run '<repo>/apps/filesquid/ui/node_modules/.bin/prettier --write .' in ./apps/imagesquid.

🚨 VALIDATION FAILED: Test-Driven Development formatting rule violated.
Format each listed workspace before committing.
exit 1
```

The same run without the scratch file:

```text
Starting Format Validation...
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: docs)
🔍 Validating Prettier formatting in ./apps/filesquid/ui...
✅ ./apps/filesquid/ui is properly formatted.
🔍 Validating Prettier formatting in ./apps/imagesquid...
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
exit 0
```

### ImageSquid's own suite still runs by hand

From `apps/imagesquid` after `npm ci`, `npm run lint && npm run check && npm test`:

```text

> @imagesquid/ui@0.1.0 lint
> eslint .


> @imagesquid/ui@0.1.0 check
> tsc --noEmit


> @imagesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @imagesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <repo>/apps/imagesquid

 ✓ tests/unit/test-reporting.test.ts (12 tests) 15ms
 ✓ tests/unit/theme.test.ts (6 tests) 17ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 714ms
 ✓ tests/ui/distribution.test.ts (5 tests) 163ms

 Test Files  4 passed (4)
      Tests  32 passed | 1 skipped (33)
   Start at  20:34:39
   Duration  2.23s (transform 1.39s, setup 238ms, import 1.47s, tests 909ms, environment 1.99s)

JUNIT report written to <repo>/apps/imagesquid/tests/allure-results/vitest-junit.xml
exit 0
```

### Ordinary hook run on a scratch change confined to apps/imagesquid

A scratch clone of this branch (`<scratch-clone>`, deleted afterwards) with its `origin/main` set to the branch head, so the change scope is only the scratch change. `npm ci` ran in `apps/filesquid/ui` only; `apps/imagesquid/node_modules` did not exist. The change: `"stateRetry": "Try again"` became `"Try again now"` in `apps/imagesquid/src/theme/labels.en.json`. `TMPDIR=<tmpdir> bash scripts/pre-commit.sh`:

```text
changed_scope: npm:apps/filesquid/ui (changed crates and npm suites: apps/filesquid/ui)
========================================
Running Pre-Commit Hooks... (test scope: npm:apps/filesquid/ui)
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
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
Starting Portable Documentation Validation...
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: npm:apps/filesquid/ui)
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
  duration_ms: 9.976505
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.289611
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 0.764894
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
# duration_ms 74.963545
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
FileSquid: 263 vision IDs, 263 allocated, 133 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
🔍 Plan traceability...
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 291 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 174 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
......................................................
----------------------------------------------------------------------
Ran 54 tests in 5.801s

OK
✅ MVP queue and merge driver tests passed.
🔍 MVP progress bar...
✅ MVP progress bar passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (179 assertions).
Starting Lint and Test Validation...
ℹ️  Test scope: npm:apps/filesquid/ui
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/imagesquid (not run by the hook during the MVP (owner decision 2026-10-23); @filesquid/ui's distribution-table test checks its label table and token file)
⏭️  Skipped: Clippy (no Rust crate changed)
⏭️  Skipped: Workspace tests (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-core (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-ocfl (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-metadata (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-c2pa (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-prov (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-api (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-native (no Rust crate changed)
⏭️  Skipped: filesquid-metadata all-features tests and wasm32 builds (no Rust crate changed)
⏭️  Skipped: filesquid-prov all-features tests and wasm32 builds (no Rust crate changed)
⏭️  Skipped: Rustdoc (no Rust crate changed)
⏭️  Skipped: Clippy (desktop, test-instrumented) (no Rust crate changed)
⏭️  Skipped: Desktop tests (test-instrumented) (no Rust crate changed)
🔍 ESLint (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 lint
> eslint .

✅ ESLint passed (5s).
🔍 TypeScript (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 check
> tsc --noEmit

✅ TypeScript passed (6s).
🔍 Vitest (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <scratch-clone>/apps/filesquid/ui

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the error state and no host when the descriptor cannot be read
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <scratch-clone>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:329:46
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the denied state for an authorisation denial
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 401 (missing_credentials)
    at <scratch-clone>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:338:20
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 401,
  path: '/api/v1/host',
  code: 'unauthorized',
  reason: 'missing_credentials',
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the offline state with Retry when the read fails while the browser is offline
FileSquid host descriptor could not be read TypeError: Failed to fetch
    at <scratch-clone>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:357:29
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > reads again when Retry is pressed and recovers
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <scratch-clone>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:369:29
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> follows the host > reads again when a host.change payload breaks the contract
FileSquid host.change payload broke the contract; reading the descriptor

 ✓ tests/ui/host-indicator.test.ts (40 tests) 447ms
 ✓ tests/unit/theme.test.ts (36 tests) 69ms
 ✓ tests/ui/nav-tree.test.ts (30 tests) 1087ms
 ✓ tests/ui/file-browser-writes.test.ts (47 tests) 1724ms
 ✓ tests/ui/app-shell.test.ts (23 tests) 619ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1982ms
 ✓ tests/unit/files-source.test.ts (29 tests) 59ms
stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > words a refused start from the label table
OCFL validation could not start ApiError: FileSquid API /api/v1/admin/ocfl/validation-runs failed with HTTP 403 (not_permitted)
    at <scratch-clone>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:301:7
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 403,
  path: '/api/v1/admin/ocfl/validation-runs',
  code: undefined,
  reason: 'not_permitted',
  retryable: false
}

stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > shows the unavailable state with Retry when the findings route answers 503
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at <scratch-clone>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:315:29
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/ocfl-findings.test.ts (22 tests) 404ms
 ✓ tests/ui/library-start.test.ts (22 tests) 343ms
 ✓ tests/unit/files-memory-source.test.ts (35 tests) 45ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 449ms
stderr | tests/ui/monitor.test.ts > <fs-monitor> > keeps the other sections up when the findings route is unavailable
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at Object.fake.client.adminOcflFindings (<scratch-clone>/apps/filesquid/ui/tests/ui/monitor.test.ts:458:22)
    at FsOcflFindings.read (<scratch-clone>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:155:41)
    at FsOcflFindings.load (<scratch-clone>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:142:23)
    at FsOcflFindings.refresh (<scratch-clone>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:130:31)
    at FsOcflFindings.connectedCallback (<scratch-clone>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:111:15)
    at FsOcflFindings.invokeTheCallbackFunction (<scratch-clone>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/Function.js:19:26)
    at invokeCEReactions (<scratch-clone>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:190:31)
    at ceReactionsPostSteps (<scratch-clone>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:54:3)
    at ShadowRoot.replaceChildren (<scratch-clone>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/DocumentFragment.js:204:9)
    at replaceKeepingFocus (<scratch-clone>/apps/filesquid/ui/src/a11y/focus.ts:52:8) {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/monitor.test.ts (22 tests) 4986ms
     ✓ shows only the newest 200 rows after a long stream during a journal re-read  4409ms
 ✓ tests/unit/files-ops.test.ts (55 tests) 180ms
 ✓ tests/unit/component-styles.test.ts (193 tests) 106ms
 ✓ tests/ui/library-detail.test.ts (20 tests) 355ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 660ms
stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows the error state with Try again when the details cannot be read, and retries
FileSquid could not preview a file Error

stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows permission denied with the reason's label
FileSquid could not preview a file ApiError

 ✓ tests/ui/file-preview.test.ts (16 tests) 215ms
 ✓ tests/unit/client-io.test.ts (23 tests) 57ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 43ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 210ms
 ✓ tests/ui/settings.test.ts (10 tests) 232ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 41ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 377ms
Not implemented: HTMLMediaElement's pause() method
 ✓ tests/unit/media-handle.test.ts (14 tests) 104ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 27ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 115ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 37ms
 ✓ tests/unit/library-client.test.ts (18 tests) 40ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 27ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 43ms
 ✓ tests/unit/commands.test.ts (46 tests) 37ms
 ✓ tests/unit/files-model.test.ts (58 tests) 79ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 32ms
 ✓ tests/ui/states.test.ts (26 tests) 175ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 30ms
 ✓ tests/unit/files-window.test.ts (19 tests) 18ms
 ✓ tests/unit/focus.test.ts (16 tests) 71ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 17ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 657ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 30ms
 ✓ tests/unit/distribution-tables.test.ts (7 tests) 23ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 20ms
 ✓ tests/ui/library-route.test.ts (6 tests) 125ms
 ✓ tests/ui/files-route.test.ts (6 tests) 458ms
     ✓ shows the browser at the folder the hash names, now and after a hash change  303ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 81ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 74ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 14ms
 ✓ tests/unit/client.test.ts (8 tests) 19ms
 ✓ tests/unit/library-model.test.ts (7 tests) 16ms
 ✓ tests/unit/preferences.test.ts (10 tests) 20ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 81ms
 ✓ tests/unit/view-state.test.ts (5 tests) 13ms
 ✓ tests/unit/host-client.test.ts (4 tests) 14ms
 ✓ tests/unit/contrast.test.ts (13 tests) 14ms
 ✓ tests/unit/distribution.test.ts (3 tests) 8ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 29ms
 ✓ tests/unit/contract.test.ts (2 tests) 220ms

 Test Files  57 passed (57)
      Tests  1301 passed | 1 skipped (1302)
   Start at  20:42:26
   Duration  22.74s (transform 3.42s, setup 2.10s, import 7.75s, tests 17.46s, environment 30.24s)

JUNIT report written to <scratch-clone>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
✅ Vitest passed (25s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (57s)
========================================
```

The same clone with `virtualExhibits` deleted from ImageSquid's label table: the hook fails in `@filesquid/ui`'s suite. Lines filtered with `grep -E "Test scope|FAIL|AssertionError|virtualExhibits|❌|🚨|Skipped: ESLint"`:

```text
ℹ️  Test scope: npm:apps/filesquid/ui
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/imagesquid (not run by the hook during the MVP (owner decision 2026-10-23); @filesquid/ui's distribution-table test checks its label table and token file)
 FAIL  tests/unit/distribution-tables.test.ts > 'ImageSquid' label table (UI-S-005) > defines every required key
AssertionError: expected [ 'virtualExhibits' ] to deeply equal []
+   "virtualExhibits",
 FAIL  tests/unit/distribution-tables.test.ts > 'ImageSquid' label table (UI-S-005) > defines every key of the default table, none empty
AssertionError: expected [ 'virtualExhibits' ] to deeply equal []
+   "virtualExhibits",
❌ Vitest failed (26s).
🚨 VALIDATION FAILED (test scope: npm:apps/filesquid/ui): fix the lint or test failures listed above before committing.
❌ pre-commit failed: Lint and Test Validation (test scope: npm:apps/filesquid/ui)
```

### Ordinary hook run on a scratch change confined to apps/filesquid/ui

The same clone, ImageSquid reset, with one line appended to `apps/filesquid/ui/README.md`. `TMPDIR=<tmpdir> bash scripts/pre-commit.sh`:

```text
changed_scope: npm:apps/filesquid/ui (changed crates and npm suites: apps/filesquid/ui)
========================================
Running Pre-Commit Hooks... (test scope: npm:apps/filesquid/ui)
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
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
Starting Portable Documentation Validation...
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: npm:apps/filesquid/ui)
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
  duration_ms: 10.056477
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 1.873803
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 0.895038
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
# duration_ms 84.111151
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
FileSquid: 263 vision IDs, 263 allocated, 133 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
🔍 Plan traceability...
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 291 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 174 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
......................................................
----------------------------------------------------------------------
Ran 54 tests in 5.758s

OK
✅ MVP queue and merge driver tests passed.
🔍 MVP progress bar...
✅ MVP progress bar passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (179 assertions).
Starting Lint and Test Validation...
ℹ️  Test scope: npm:apps/filesquid/ui
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/imagesquid (not run by the hook during the MVP (owner decision 2026-10-23); @filesquid/ui's distribution-table test checks its label table and token file)
⏭️  Skipped: Clippy (no Rust crate changed)
⏭️  Skipped: Workspace tests (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-core (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-ocfl (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-metadata (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-c2pa (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-prov (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-api (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-native (no Rust crate changed)
⏭️  Skipped: filesquid-metadata all-features tests and wasm32 builds (no Rust crate changed)
⏭️  Skipped: filesquid-prov all-features tests and wasm32 builds (no Rust crate changed)
⏭️  Skipped: Rustdoc (no Rust crate changed)
⏭️  Skipped: Clippy (desktop, test-instrumented) (no Rust crate changed)
⏭️  Skipped: Desktop tests (test-instrumented) (no Rust crate changed)
🔍 ESLint (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 lint
> eslint .

✅ ESLint passed (5s).
🔍 TypeScript (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 check
> tsc --noEmit

✅ TypeScript passed (5s).
🔍 Vitest (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <scratch-clone>/apps/filesquid/ui

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the error state and no host when the descriptor cannot be read
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <scratch-clone>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:329:46
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the denied state for an authorisation denial
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 401 (missing_credentials)
    at <scratch-clone>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:338:20
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 401,
  path: '/api/v1/host',
  code: 'unauthorized',
  reason: 'missing_credentials',
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the offline state with Retry when the read fails while the browser is offline
FileSquid host descriptor could not be read TypeError: Failed to fetch
    at <scratch-clone>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:357:29
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > reads again when Retry is pressed and recovers
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <scratch-clone>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:369:29
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> follows the host > reads again when a host.change payload breaks the contract
FileSquid host.change payload broke the contract; reading the descriptor

 ✓ tests/ui/host-indicator.test.ts (40 tests) 448ms
 ✓ tests/unit/theme.test.ts (36 tests) 93ms
 ✓ tests/ui/nav-tree.test.ts (30 tests) 1083ms
 ✓ tests/ui/file-browser-writes.test.ts (47 tests) 1991ms
     ✓ uploads each chosen file with one PUT as its own file and reports the committed version  343ms
 ✓ tests/ui/app-shell.test.ts (23 tests) 683ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1749ms
 ✓ tests/unit/files-source.test.ts (29 tests) 83ms
stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > words a refused start from the label table
OCFL validation could not start ApiError: FileSquid API /api/v1/admin/ocfl/validation-runs failed with HTTP 403 (not_permitted)
    at <scratch-clone>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:301:7
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 403,
  path: '/api/v1/admin/ocfl/validation-runs',
  code: undefined,
  reason: 'not_permitted',
  retryable: false
}

stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > shows the unavailable state with Retry when the findings route answers 503
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at <scratch-clone>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:315:29
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<scratch-clone>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/ocfl-findings.test.ts (22 tests) 422ms
 ✓ tests/ui/library-start.test.ts (22 tests) 306ms
 ✓ tests/unit/files-memory-source.test.ts (35 tests) 65ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 409ms
 ✓ tests/unit/files-ops.test.ts (55 tests) 144ms
stderr | tests/ui/monitor.test.ts > <fs-monitor> > keeps the other sections up when the findings route is unavailable
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at Object.fake.client.adminOcflFindings (<scratch-clone>/apps/filesquid/ui/tests/ui/monitor.test.ts:458:22)
    at FsOcflFindings.read (<scratch-clone>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:155:41)
    at FsOcflFindings.load (<scratch-clone>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:142:23)
    at FsOcflFindings.refresh (<scratch-clone>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:130:31)
    at FsOcflFindings.connectedCallback (<scratch-clone>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:111:15)
    at FsOcflFindings.invokeTheCallbackFunction (<scratch-clone>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/Function.js:19:26)
    at invokeCEReactions (<scratch-clone>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:190:31)
    at ceReactionsPostSteps (<scratch-clone>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:54:3)
    at ShadowRoot.replaceChildren (<scratch-clone>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/DocumentFragment.js:204:9)
    at replaceKeepingFocus (<scratch-clone>/apps/filesquid/ui/src/a11y/focus.ts:52:8) {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/monitor.test.ts (22 tests) 5264ms
     ✓ shows only the newest 200 rows after a long stream during a journal re-read  4656ms
 ✓ tests/unit/component-styles.test.ts (193 tests) 85ms
 ✓ tests/ui/library-detail.test.ts (20 tests) 435ms
stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows the error state with Try again when the details cannot be read, and retries
FileSquid could not preview a file Error

stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows permission denied with the reason's label
FileSquid could not preview a file ApiError

 ✓ tests/ui/file-preview.test.ts (16 tests) 263ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 684ms
 ✓ tests/unit/client-io.test.ts (23 tests) 54ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 74ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 263ms
 ✓ tests/ui/settings.test.ts (10 tests) 238ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 35ms
Not implemented: HTMLMediaElement's pause() method
 ✓ tests/unit/media-handle.test.ts (14 tests) 121ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 394ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 43ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 100ms
 ✓ tests/unit/library-client.test.ts (18 tests) 49ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 32ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 24ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 29ms
 ✓ tests/unit/commands.test.ts (46 tests) 39ms
 ✓ tests/unit/files-model.test.ts (58 tests) 52ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 35ms
 ✓ tests/ui/states.test.ts (26 tests) 159ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 27ms
 ✓ tests/unit/files-window.test.ts (19 tests) 21ms
 ✓ tests/unit/focus.test.ts (16 tests) 96ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 665ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 18ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 23ms
 ✓ tests/unit/distribution-tables.test.ts (7 tests) 22ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 15ms
 ✓ tests/ui/library-route.test.ts (6 tests) 103ms
 ✓ tests/ui/files-route.test.ts (6 tests) 442ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 74ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 62ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 18ms
 ✓ tests/unit/client.test.ts (8 tests) 18ms
 ✓ tests/unit/library-model.test.ts (7 tests) 19ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 80ms
 ✓ tests/unit/preferences.test.ts (10 tests) 24ms
 ✓ tests/unit/view-state.test.ts (5 tests) 10ms
 ✓ tests/unit/host-client.test.ts (4 tests) 21ms
 ✓ tests/unit/contrast.test.ts (13 tests) 23ms
 ✓ tests/unit/distribution.test.ts (3 tests) 7ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 30ms
 ✓ tests/unit/contract.test.ts (2 tests) 253ms

 Test Files  57 passed (57)
      Tests  1301 passed | 1 skipped (1302)
   Start at  20:44:33
   Duration  23.48s (transform 3.37s, setup 2.23s, import 7.77s, tests 18.02s, environment 31.17s)

JUNIT report written to <scratch-clone>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
✅ Vitest passed (26s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (56s)
========================================
```

### Full hook run on the branch

This run was on commit `00843c75`, before `origin/main` (PRs #131 and #137) was merged in. The `FULL=1` run on the final head commit, after that merge and with this walkthrough committed, is recorded by its summary line in the pull request.

`TMPDIR=<tmpdir> CARGO_TARGET_DIR=<private-target> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 FULL=1 bash scripts/pre-commit.sh`, with `apps/imagesquid/node_modules` absent. The private target was deleted afterwards. The step lines of the raw output, filtered with `grep -E "^(🔍|✅|❌|⏭️|ℹ️|⚠️|🚨|=+$|Running Pre-Commit|Starting|exit )"`:

```text
========================================
Running Pre-Commit Hooks... (test scope: full)
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
Starting Document Sync Validation...
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
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
✅ PRD allocation passed.
🔍 Plan traceability...
✅ Plan traceability passed.
🔍 Relative links and anchors...
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
✅ MVP queue and merge driver tests passed.
🔍 MVP progress bar...
✅ MVP progress bar passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (179 assertions).
Starting Lint and Test Validation...
ℹ️  Test scope: full
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/imagesquid (not run by the hook during the MVP (owner decision 2026-10-23); @filesquid/ui's distribution-table test checks its label table and token file)
🔍 Clippy (apps/filesquid)...
✅ Clippy passed (129s).
🔍 Workspace tests (apps/filesquid)...
✅ Workspace tests passed (338s).
🔍 Module crate alone: filesquid-core (apps/filesquid)...
✅ Module crate alone: filesquid-core passed (26s).
🔍 Module crate alone: filesquid-ocfl (apps/filesquid)...
✅ Module crate alone: filesquid-ocfl passed (48s).
🔍 Module crate alone: filesquid-metadata (apps/filesquid)...
✅ Module crate alone: filesquid-metadata passed (4s).
🔍 Module crate alone: filesquid-c2pa (apps/filesquid)...
✅ Module crate alone: filesquid-c2pa passed (15s).
🔍 Module crate alone: filesquid-prov (apps/filesquid)...
✅ Module crate alone: filesquid-prov passed (7s).
🔍 Module crate alone: filesquid-api (apps/filesquid)...
✅ Module crate alone: filesquid-api passed (174s).
🔍 Module crate alone: filesquid-native (apps/filesquid)...
✅ Module crate alone: filesquid-native passed (24s).
🔍 Clippy (filesquid-metadata, all features) (apps/filesquid)...
✅ Clippy (filesquid-metadata, all features) passed (10s).
🔍 Tests (filesquid-metadata, all features) (apps/filesquid)...
✅ Tests (filesquid-metadata, all features) passed (15s).
🔍 wasm32 build (filesquid-metadata, default features off) (apps/filesquid)...
✅ wasm32 build (filesquid-metadata, default features off) passed (5s).
🔍 wasm32 build (filesquid-metadata, ocfl feature) (apps/filesquid)...
✅ wasm32 build (filesquid-metadata, ocfl feature) passed (7s).
🔍 Clippy (filesquid-prov, all features) (apps/filesquid)...
✅ Clippy (filesquid-prov, all features) passed (5s).
🔍 Tests (filesquid-prov, all features) (apps/filesquid)...
✅ Tests (filesquid-prov, all features) passed (3s).
🔍 wasm32 build (filesquid-prov) (apps/filesquid)...
✅ wasm32 build (filesquid-prov) passed (1s).
🔍 wasm32 build (filesquid-prov, ocfl feature) (apps/filesquid)...
✅ wasm32 build (filesquid-prov, ocfl feature) passed (3s).
🔍 Rustdoc (apps/filesquid)...
✅ Rustdoc passed (49s).
🔍 Clippy (desktop, test-instrumented) (apps/filesquid)...
✅ Clippy (desktop, test-instrumented) passed (61s).
🔍 Desktop tests (test-instrumented) (apps/filesquid)...
✅ Desktop tests (test-instrumented) passed (82s).
🔍 ESLint (apps/filesquid/ui)...
✅ ESLint passed (5s).
🔍 TypeScript (apps/filesquid/ui)...
✅ TypeScript passed (6s).
🔍 Vitest (apps/filesquid/ui)...
✅ Vitest passed (25s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: full).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: full (1065s)
========================================
exit 0
```
