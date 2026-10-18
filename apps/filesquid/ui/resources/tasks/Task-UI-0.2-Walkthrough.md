# Task UI-0.2 Walkthrough

## SRS-UI-007: Light and dark themes

Plan row: [UI-0.2 Official branding and light/dark tokens](../plan.md), part of global [Task FS-2.1](../../../../../resources/implementation-plan.md#task-fs-2.1). UI-0.5 and UI-F2.1 remain, so the global box stays open. Requirements: [SRS-UI-007](../../../../../resources/srs.md#srs-ui-007-light-and-dark-themes), [SRS-F-058](../../../../../resources/srs.md#srs-f-058-white-label-distributions), [NFR-006](../../../../../resources/prd.md#nfr-006-accessibility-and-usability), [UI-S-005](../srs.md), [UI-S-015](../srs.md) (new), ImageSquid IS-S-008 and IS-S-009, [ADR 031](../../../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md). Scenarios: [`theme_tokens_SRS-UI-007.feature`](../../tests/features/theme_tokens_SRS-UI-007.feature).

SRS-UI-007 in full: tokens MUST be CSS custom properties with the `--fs-` prefix and the Arkrim token role names; each distribution MUST define every required token for both a light and a dark theme, and a missing token in either theme MUST fail the token test; FileSquid custom elements MUST style colour, type, spacing and radius only through tokens, and component stylesheets MUST NOT contain literal colour values; the theme MUST follow the operating-system colour scheme by default, a theme control MUST switch light and dark, and the choice MUST persist across launches; each distribution sets its own values under these token names; contrast of primary actions and body text on their surfaces MUST meet WCAG 2.2 AA in both themes, in every distribution; a theme MUST NOT be required to match mockup pixels.

## Changes

- **Official FileSquid tokens** ([`tokens.css`](../../src/theme/tokens.css)) from the [FileSquid Design Guide](../../../../../resources/assets/FileSquid-design-guide-v1.html), adjusted where the guide misses AA:

  | Token | Light | Dark |
  |---|---|---|
  | `--fs-surface` / `--fs-surface-container` | `#ffffff` / `#f5f5f5` | `#141414` / `#1c1c1c` |
  | `--fs-on-surface` / `--fs-on-surface-variant` | `#111111` / `#555555` | `#e0e0e0` / `#a0a0a0` |
  | `--fs-outline` | `#e0e0e0` | `#2a2a2a` |
  | `--fs-primary` / `--fs-on-primary` | `#0b7a32` / `#ffffff` (5.46:1) | `#16ac49` / `#0d2b18` (5.11:1) |
  | `--fs-primary-container` / `--fs-on-primary-container` | `#f0faf4` / `#0b7a32` (5.12:1) | `#0d2b18` / `#4aca74` (7.26:1) |
  | `--fs-error` | `#c62828` (5.62:1) | `#ef5350` (5.28:1) |
  | `--fs-font-sans`, `--fs-font-heading` (new), `--fs-font-mono`, `--fs-radius` | system sans, Georgia serif, SF Mono stack, `8px` | same |

  The guide's white text on the brand green `#16ac49` is 2.99:1 and its light error `#e53935` on white is 4.23:1, both below AA, so the light primary and error are darker; the brand green stays the dark primary, with dark ink on it.
- **Contrast contract** ([`contrast.ts`](../../src/theme/contrast.ts), [`tokens.ts`](../../src/theme/tokens.ts)): `contrastRatio` (WCAG 2.2), `tokenValues`, `CONTRAST_PAIRS` (nine text-on-surface pairs) and `contrastFailures(css)`, exported from `@filesquid/ui/theme` so every distribution's token test checks AA in both themes. `--fs-font-heading` joins `REQUIRED_TOKENS`.
- **Theme store** ([`theme-store.ts`](../../src/theme/theme-store.ts)): `ThemeStore extends EventTarget` (ADR 031 Decision 6). Follows `prefers-color-scheme` by default (no `data-theme`); Light or Dark sets `data-theme` and is kept under `filesquid.theme` in Web Storage; System removes both. Storage failures are logged with `console.warn` and the theme still applies. `dispose()` removes the OS listener.
- **`<fs-theme-control>`** ([`fs-theme-control.ts`](../../src/components/fs-theme-control.ts)): a `fieldset` with a `legend` and three native radio buttons worded from the label table (`theme`, `themeSystem`, `themeLight`, `themeDark`), so the arrow keys work without custom key handling (ready for UI-0.5).
- **Brand slots** ([`fs-app-shell.ts`](../../src/components/fs-app-shell.ts), [`distribution.ts`](../../src/distribution.ts)): `configure({ distribution, client, theme })`; the header holds the logo, the product name in `--fs-font-heading` and the theme control. `logo.srcDark` (optional) is shown in the dark theme. The shell subscribes to the store on connect and unsubscribes on disconnect.
- **Official mark**: [`filesquid-mark.svg`](../../public/filesquid-mark.svg) and [`filesquid-mark-dark.svg`](../../public/filesquid-mark-dark.svg), cropped from the official [`FileSquid-logo.svg`](../../../../../resources/assets/FileSquid-logo.svg) to the mark (black outline on light, white on dark, shapes never recoloured); they replace the raster `filesquid-logo.png` in the header and favicon. It renders 32 px tall (guide minimum 24 px).
- **ImageSquid**: tokens use IS-S-008 (light `#005db6` / `#ffffff`, dark `#a9c7ff` / `#003063`) instead of the earlier navy `#1d3f73`; `--fs-font-heading` and the theme labels added; `id` and `productName` come from `distribution.json` (IS-S-009); `main.ts` starts the theme store.
- **Docs**: new [UI-S-015](../srs.md); [SRS-UI-007](../../../../../resources/srs.md#srs-ui-007-light-and-dark-themes) points to it; [UI README](../../README.md) section "Theme, tokens and brand"; the feature file rewritten with the feature-writer skill; plan row and [checklist](../../../../../resources/implementation-plan-checklist.md) ticked.

## Decisions and assumptions for the platform engineer

1. **Theme persistence is device-local.** No preferences route exists in the API. The choice lives in Web Storage under `filesquid.theme`; it is a display preference, not library data or a credential. When per-user preferences arrive (UI-F2.8), an API-backed `ThemeStorage` can replace it without changing the control. Desktop (WKWebView) keeps Web Storage for the custom-scheme origin across launches; this should be confirmed on the packaged macOS build (DE-F3.1).
2. **No pixel matching.** Every check is a DOM, computed-style or contrast assertion; no screenshot is compared.
3. **IS-S-008 versus the ImageSquid screenshot.** The screenshot shows a navy "Edit" button; IS-S-008 and the UI-0.2 row require `#005db6`. The written values were applied, since the ImageSquid plan calls the screenshots informative for colour.

## SRS traceability

| Constraint | Tests |
|---|---|
| `--fs-` prefix, Arkrim roles, `--fs-font-heading` required | `theme.test.ts` "uses the --fs- prefix and requires a heading font" |
| Missing token in light or dark fails (FileSquid and ImageSquid) | `theme.test.ts` "defines every required token", "reports tokens a distribution forgot", "reports a colour token missing from the dark theme only", "does not count commented-out declarations"; ImageSquid `theme.test.ts` "define every required token" |
| No literal colour in component stylesheets | `component-styles.test.ts` (6) |
| Follows the OS by default | `theme-store.test.ts` "follows the OS by default", "follows OS changes while the choice is system"; `theme-control.test.ts` "checks System by default"; e2e "the theme follows the OS by default ..." |
| Control switches light and dark; choice persists across launches | `theme-store.test.ts` "persists an explicit choice", "a new store over the same storage starts with the saved choice"; `theme-control.test.ts` (7); e2e reload test in both packages |
| Keyboard operation | e2e: arrow keys move from Dark to Light to System, each chosen as focused |
| Storage refused | `theme-store.test.ts` "still applies the choice when storage throws", "works without storage"; `createThemeStore` "falls back to no storage" |
| Each distribution sets its own values | `theme.test.ts` "uses the official FileSquid values"; ImageSquid "use the Arkrim primary role values (IS-S-008)"; e2e computed values in light and dark in both packages |
| WCAG 2.2 AA in both themes, every distribution | `contrast.test.ts` (13); `theme.test.ts` "passes for the default tokens", "reports the official brand green with white text (2.99:1)"; ImageSquid "meet WCAG 2.2 AA"; e2e in-page contrast of all nine pairs and of the rendered chosen option and status line |
| Brand slots: product name and logo from the distribution, dark variant | `app-shell.test.ts` "shows the dark-surface logo", "keeps the one logo of a distribution without a dark variant", "follows a theme store given after a first configure", "stops following the theme once disconnected"; e2e "the official mark fills the brand slot" (image loaded, at least 24 px tall, favicon) |
| ImageSquid name and mark from its brand assets (IS-S-009) | ImageSquid `distribution.test.ts` (3); e2e ImageSquid brand test |
| UI-S-011, UI-S-013: built bundle, custom elements defined, no console errors | e2e: 13 FileSquid and 5 ImageSquid tests against `npm run build` and `filesquid-server` |

| Spacing, type size and radius only from tokens; every colour token hex in both themes (review round 1) | `component-styles.test.ts` "takes spacing, type size and radius from tokens" (per component); `theme.test.ts` "requires theme-independent spacing and type-size tokens", "reports a colour token outside the contrast pairs ..."; ImageSquid "write every colour token as #rgb or #rrggbb" |

Coverage: no coverage reporter is approved for Vitest in [ADR 017](../../../../../resources/architecture/017-test-runners-and-reporting.md). Every new exported function has a direct unit test: `logoSource` in `tests/unit/distribution.test.ts` (added in review round 1; before that it was covered only through the shell tests), and the contrast, token, store and control functions in the files named above.

## Review round 1 (Cubic, 12 findings)

All 12 findings were valid and fixed in one commit. The regression tests for the reported defects were written first and failed first: the 10 failures in the run below. Some tests added in this round passed before the fixes, because they cover behaviour that was already correct and had no direct test: the `logoSource` unit test (`tests/unit/distribution.test.ts`, added for the coverage finding), the exact nine-pair `CONTRAST_PAIRS` assertion, the stylesheet-only and named-colour checks of `component-styles.test.ts`, the shared-fakes refactor, the "does not notify" store test and the ImageSquid `data-resolved` assertion.

- `theme.test.ts` now requires exactly the nine UI-S-015 contrast pairs.
- Type size, spacing and radius come from tokens. New theme-independent tokens are `--fs-font-size-small`, `--fs-font-size-body`, `--fs-font-size-title` and `--fs-space-1` to `--fs-space-5` (4, 8, 12, 16 and 24 px in both distributions). `<fs-theme-control>`, `<fs-app-shell>` and `<fs-monitor>` use them. `component-styles.test.ts` fails on any literal margin, padding, gap, font size or border radius.
- `invalidColourTokens(css)` reports any colour token that is not `#rgb` or `#rrggbb` in either theme, including `--fs-outline`, which no contrast pair reads. Both packages run it.
- `tokenValues` reads a last declaration that has no closing semicolon.
- `ThemeStore.start()` dispatches `change` when the restored choice or theme differs from the one subscribers last saw.
- `setChoice("system")` removes a saved value even when System is already shown.
- `component-styles.test.ts` now reads only each component's `STYLE` template and matches every CSS named colour, `transparent` and colour function, case-insensitively.
- The README says which tokens the dark blocks must repeat (colours) and which they need not (`THEME_INDEPENDENT_TOKENS`).
- `theme-store.test.ts` uses the shared `tests/support/theme-fakes.ts`, which can now start from saved values and count its listeners.
- The ImageSquid dark-theme test also asserts the control's `data-resolved="dark"`.
- `logoSource` has a direct unit test, and the coverage line above was corrected.

Failing run before the round 1 fixes (`npx vitest run` in `apps/filesquid/ui`, then `apps/imagesquid`):

```text
     × 'fs-app-shell.ts' takes spacing, type size and radius from tokens
     × 'fs-monitor.ts' takes spacing, type size and radius from tokens
     × 'fs-theme-control.ts' takes spacing, type size and radius from tokens
     × choosing System removes a saved value even when System is already shown
     × finds no unreadable colour token in the default sheet
     × notifies subscribers when start restores a different choice or theme
     × reads a last declaration without a closing semicolon
     × reports a colour token outside the contrast pairs that is not #rgb or #rrggbb
     × requires theme-independent spacing and type-size tokens
 Test Files  3 failed | 13 passed (16)
      Tests  9 failed | 218 passed (227)

     × write every colour token as #rgb or #rrggbb in both themes
TypeError: invalidColourTokens is not a function
 Test Files  1 failed | 1 passed (2)
      Tests  1 failed | 11 passed (12)
```

After the fixes (commit `d8907dc`), `npm test` in `apps/filesquid/ui`, then in `apps/imagesquid`:

```text
 Test Files  16 passed (16)
      Tests  227 passed (227)
JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml

 Test Files  2 passed (2)
      Tests  12 passed (12)
JUNIT report written to <repo>/apps/imagesquid/tests/allure-results/vitest-junit.xml
```

Both Playwright suites passed in `FILESQUID_E2E=1 bash scripts/pre-commit.sh` on that commit (13/13 and 5/5). In this round the FileSquid UI suite (`apps/filesquid/ui`) grew from 211 to 227 tests (16 new tests), and the ImageSquid suite (`apps/imagesquid`) grew from 11 to 12 (1 new test). The 211 and 11 totals under "Passing runs" below are from the first commit `48ebd0f` and are superseded by these.

## Review round 2 (Cubic, 3 findings)

All 3 findings were valid documentation fixes, made in one commit. TDD-ENFORCE: not applicable, documentation only.

- The README type-size row names `--fs-font-size-small`, `--fs-font-size-body` and `--fs-font-size-title` in full.
- Round 1 no longer claims that every new test failed first. It names the tests that passed before the fixes (including `logoSource`) and why.
- The round-1 totals now come with their own transcripts at `d8907dc`, and the earlier "Passing runs" are labelled as the first commit's, superseded.

## TDD: failing run before the implementation

`npx vitest run` in `apps/filesquid/ui`, after writing the tests and before any source change:

```text
     × words the theme control
     × uses the --fs- prefix and requires a heading font
     × uses the official FileSquid values, adjusted for AA
     × applies the same dark values for the OS preference and for data-theme=dark
     × checks primary actions and body text on their surfaces
     × passes for the default tokens in light and dark
     × reports the official brand green with white text (2.99:1) as a failure
     × reports a colour it cannot read instead of passing it
     × is exported for distributions from @filesquid/ui/theme
     × cover every component source
 FAIL  tests/ui/app-shell.test.ts [ tests/ui/app-shell.test.ts ]
Error: Failed to resolve import "../../src/theme/theme-store" from "tests/support/theme-fakes.ts". Does the file exist?
 FAIL  tests/ui/theme-control.test.ts [ tests/ui/theme-control.test.ts ]
Error: Failed to resolve import "../../src/theme/theme-store" from "tests/support/theme-fakes.ts". Does the file exist?
 FAIL  tests/unit/contrast.test.ts [ tests/unit/contrast.test.ts ]
Error: Cannot find module '../../src/theme/contrast' imported from <repo>/apps/filesquid/ui/tests/unit/contrast.test.ts
 FAIL  tests/unit/theme-store.test.ts [ tests/unit/theme-store.test.ts ]
Error: Failed to resolve import "../../src/theme/theme-store" from "tests/unit/theme-store.test.ts". Does the file exist?
 FAIL  tests/unit/component-styles.test.ts > component stylesheets > cover every component source
AssertionError: expected [ Array(3) ] to deeply equal ArrayContaining{…}
 FAIL  tests/unit/theme.test.ts > default label table > words the theme control
AssertionError: expected [ 'repository', 'library', …(48) ] to include 'theme'
 FAIL  tests/unit/theme.test.ts > default design tokens > uses the --fs- prefix and requires a heading font
AssertionError: expected [ '--fs-surface', …(12) ] to include '--fs-font-heading'
 FAIL  tests/unit/theme.test.ts > default design tokens > uses the official FileSquid values, adjusted for AA
TypeError: tokenValues is not a function
 FAIL  tests/unit/theme.test.ts > WCAG 2.2 AA contrast > passes for the default tokens in light and dark
TypeError: contrastFailures is not a function
 Test Files  6 failed | 9 passed (15)
      Tests  10 failed | 145 passed (155)
```

`npx vitest run` in `apps/imagesquid` before its token change:

```text
     × define every required token for light and dark
     × use the Arkrim primary role values (IS-S-008)
 FAIL  tests/unit/theme.test.ts > ImageSquid design tokens > define every required token for light and dark
AssertionError: expected { Object (light, dark) } to deeply equal { light: [], dark: [] }
 FAIL  tests/unit/theme.test.ts > ImageSquid design tokens > use the Arkrim primary role values (IS-S-008)
AssertionError: expected [ '#1d3f73', '#ffffff' ] to deeply equal [ '#005db6', '#ffffff' ]
 Test Files  1 failed | 1 passed (2)
      Tests  2 failed | 9 passed (11)
```

The first Playwright run also failed twice and changed the tests, not the code: a minified build writes `#ffffff` as `#fff`, so the e2e helpers now resolve each token through a probe element's computed `rgb()`; and the keyboard step now starts from the chosen radio, as a native radio group does.

## Passing runs

These transcripts are from the first commit `48ebd0f`, before review round 1. They are superseded by the round-1 transcripts above (227 and 12 Vitest tests at `d8907dc`); the Playwright counts did not change.

`npx vitest run --reporter=verbose` for the new and changed test files in `apps/filesquid/ui`:

```text
 ✓ tests/unit/theme.test.ts > default label table > covers every internal name
 ✓ tests/unit/theme.test.ts > default label table > uses the internal names as FileSquid nouns
 ✓ tests/unit/theme.test.ts > default label table > lists required keys for repository through file
 ✓ tests/unit/theme.test.ts > default label table > words the theme control
 ✓ tests/unit/theme.test.ts > default design tokens > defines every required token for light and dark
 ✓ tests/unit/theme.test.ts > default design tokens > uses the --fs- prefix and requires a heading font
 ✓ tests/unit/theme.test.ts > default design tokens > reports tokens a distribution forgot
 ✓ tests/unit/theme.test.ts > default design tokens > reports a colour token missing from the dark theme only
 ✓ tests/unit/theme.test.ts > default design tokens > does not count commented-out declarations
 ✓ tests/unit/theme.test.ts > default design tokens > uses the official FileSquid values, adjusted for AA
 ✓ tests/unit/theme.test.ts > default design tokens > applies the same dark values for the OS preference and for data-theme=dark
 ✓ tests/unit/theme.test.ts > WCAG 2.2 AA contrast > checks primary actions and body text on their surfaces
 ✓ tests/unit/theme.test.ts > WCAG 2.2 AA contrast > passes for the default tokens in light and dark
 ✓ tests/unit/theme.test.ts > WCAG 2.2 AA contrast > reports the official brand green with white text (2.99:1) as a failure
 ✓ tests/unit/theme.test.ts > WCAG 2.2 AA contrast > reports a colour it cannot read instead of passing it
 ✓ tests/unit/theme.test.ts > WCAG 2.2 AA contrast > is exported for distributions from @filesquid/ui/theme
 ✓ tests/unit/theme-store.test.ts > parseThemeChoice and resolveTheme > reads "light" as light
 ✓ tests/unit/theme-store.test.ts > parseThemeChoice and resolveTheme > reads "dark" as dark
 ✓ tests/unit/theme-store.test.ts > parseThemeChoice and resolveTheme > reads "system" as system
 ✓ tests/unit/theme-store.test.ts > parseThemeChoice and resolveTheme > reads null as system
 ✓ tests/unit/theme-store.test.ts > parseThemeChoice and resolveTheme > reads "sepia" as system
 ✓ tests/unit/theme-store.test.ts > parseThemeChoice and resolveTheme > reads 42 as system
 ✓ tests/unit/theme-store.test.ts > parseThemeChoice and resolveTheme > resolves system from the OS preference and keeps an explicit choice
 ✓ tests/unit/theme-store.test.ts > parseThemeChoice and resolveTheme > names the storage key and media query
 ✓ tests/unit/theme-store.test.ts > ThemeStore > follows the OS by default: no data-theme and the OS scheme resolved
 ✓ tests/unit/theme-store.test.ts > ThemeStore > restores a stored choice at start and applies it to the root
 ✓ tests/unit/theme-store.test.ts > ThemeStore > ignores an unknown stored value
 ✓ tests/unit/theme-store.test.ts > ThemeStore > persists an explicit choice and removes it for system
 ✓ tests/unit/theme-store.test.ts > ThemeStore > a new store over the same storage starts with the saved choice (next launch)
 ✓ tests/unit/theme-store.test.ts > ThemeStore > dispatches change with the new resolved theme
 ✓ tests/unit/theme-store.test.ts > ThemeStore > follows OS changes while the choice is system, and ignores them otherwise
 ✓ tests/unit/theme-store.test.ts > ThemeStore > start is idempotent and dispose removes the OS listener
 ✓ tests/unit/theme-store.test.ts > ThemeStore > still applies the choice when storage throws, and logs why
 ✓ tests/unit/theme-store.test.ts > ThemeStore > works without storage (choice lasts for this page only)
 ✓ tests/unit/theme-store.test.ts > createThemeStore > uses the window's localStorage, matchMedia and document element
 ✓ tests/unit/theme-store.test.ts > createThemeStore > falls back to no storage when reading localStorage throws
 ✓ tests/unit/contrast.test.ts > parseHexColour > reads #rrggbb and #rgb in any case
 ✓ tests/unit/contrast.test.ts > parseHexColour > refuses "16ac49"
 ✓ tests/unit/contrast.test.ts > parseHexColour > refuses "#12345"
 ✓ tests/unit/contrast.test.ts > parseHexColour > refuses "#gggggg"
 ✓ tests/unit/contrast.test.ts > parseHexColour > refuses "rgb(0, 0, 0)"
 ✓ tests/unit/contrast.test.ts > parseHexColour > refuses ""
 ✓ tests/unit/contrast.test.ts > parseHexColour > refuses "#1234"
 ✓ tests/unit/contrast.test.ts > relativeLuminance > is 0 for black and 1 for white
 ✓ tests/unit/contrast.test.ts > contrastRatio > is 21 for black on white and symmetric
 ✓ tests/unit/contrast.test.ts > contrastRatio > is 1 for a colour on itself
 ✓ tests/unit/contrast.test.ts > contrastRatio > matches known WCAG values
 ✓ tests/unit/contrast.test.ts > contrastRatio > is undefined when either colour cannot be read
 ✓ tests/unit/contrast.test.ts > contrastRatio > names the AA threshold for normal text
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > is registered as a custom element
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > renders the distribution's product name, logo and labels in its shadow DOM
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > puts the theme control in the header, worded from the label table
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > shows the dark-surface logo when the resolved theme is dark
 ✓ tests/ui/theme-control.test.ts > <fs-theme-control> > is registered as a custom element
 ✓ tests/ui/theme-control.test.ts > <fs-theme-control> > renders a labelled group of three radios worded from the label table
 ✓ tests/ui/theme-control.test.ts > <fs-theme-control> > checks System by default, following the OS
 ✓ tests/ui/theme-control.test.ts > <fs-theme-control> > sets the store's choice when a radio is chosen
 ✓ tests/ui/theme-control.test.ts > <fs-theme-control> > follows a choice made elsewhere (another control or a restored setting)
 ✓ tests/ui/theme-control.test.ts > <fs-theme-control> > stops listening once removed from the document
 ✓ tests/ui/theme-control.test.ts > <fs-theme-control> > renders nothing until configured
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > keeps the one logo of a distribution without a dark variant
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > stops following the theme once disconnected
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > follows a theme store given after a first configure
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > sets the product name in the heading font token
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > shows the API health status once the client answers
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > shows an alert when the API is unreachable
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > shows HTTP 401 unauthorized (missing_credentials) with its own label
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > shows HTTP 403 forbidden (invalid_credentials) with its own label
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > shows HTTP 403 forbidden (not_permitted) with its own label
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > reloads the API status on refreshHealth (after a token is entered)
 ✓ tests/ui/app-shell.test.ts > <fs-app-shell> > ignores a stale pre-sign-in answer that arrives after the signed-in one
 ✓ tests/unit/component-styles.test.ts > component stylesheets > cover every component source
 ✓ tests/unit/component-styles.test.ts > component stylesheets > 'denial.ts' contains no literal colour
 ✓ tests/unit/component-styles.test.ts > component stylesheets > 'fs-app-shell.ts' contains no literal colour
 ✓ tests/unit/component-styles.test.ts > component stylesheets > 'fs-monitor.ts' contains no literal colour
 ✓ tests/unit/component-styles.test.ts > component stylesheets > 'fs-theme-control.ts' contains no literal colour
 ✓ tests/unit/component-styles.test.ts > component stylesheets > the check catches a literal colour
 Test Files  6 passed (6)
      Tests  77 passed (77)
```

`npm test` in `apps/filesquid/ui` (whole suite):

```text
 Test Files  15 passed (15)
      Tests  211 passed (211)
JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

`npx vitest run --reporter=verbose` in `apps/imagesquid`:

```text
 ✓ tests/unit/theme.test.ts > ImageSquid label table > defines every required key
 ✓ tests/unit/theme.test.ts > ImageSquid label table > uses repo, album, gallery and exhibit for the internal nouns
 ✓ tests/unit/theme.test.ts > ImageSquid label table > carries the screenshot wording (IS-S-004)
 ✓ tests/unit/theme.test.ts > ImageSquid label table > words the theme control
 ✓ tests/unit/theme.test.ts > ImageSquid design tokens > define every required token for light and dark
 ✓ tests/unit/theme.test.ts > ImageSquid design tokens > use the Arkrim primary role values (IS-S-008)
 ✓ tests/unit/theme.test.ts > ImageSquid design tokens > meet WCAG 2.2 AA for primary actions and body text in light and dark
 ✓ tests/unit/theme.test.ts > ImageSquid design tokens > apply the same dark values for the OS preference and for data-theme=dark
 ✓ tests/ui/distribution.test.ts > takes its id and displayed name from distribution.json (IS-S-009)
 ✓ tests/ui/distribution.test.ts > renders ImageSquid brand and vocabulary in the shared shell
 ✓ tests/ui/distribution.test.ts > keeps the official mark in the dark theme and shows the theme control
 Test Files  2 passed (2)
      Tests  11 passed (11)
```

`npm run test:e2e` in `apps/filesquid/ui` (built UI, real `filesquid-server`):

```text
Running 13 tests using 2 workers

  ✓   1 [chromium] › tests/e2e/theme.spec.ts:91:1 › official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA (341ms)
  ✓   2 [chromium] › tests/e2e/app.spec.ts:44:1 › shell mounts with FileSquid brand, internal vocabulary and live API health (398ms)
  ✓   3 [chromium] › tests/e2e/theme.spec.ts:115:1 › the official mark fills the brand slot, with its dark-surface variant in dark (273ms)
  ✓   4 [chromium] › tests/e2e/app.spec.ts:68:1 › design tokens switch between light and dark (191ms)
  ✓   6 [chromium] › tests/e2e/app.spec.ts:79:1 › client-side routes fall back to the distribution index (247ms)
  ✓   5 [chromium] › tests/e2e/theme.spec.ts:141:1 › the theme follows the OS by default and the control's choice persists across a reload (529ms)
  ✓   7 [chromium] › tests/e2e/app.spec.ts:85:1 › without a token the shell names the missing credentials from the label table (270ms)
  ✓   8 [chromium] › tests/e2e/theme.spec.ts:185:1 › the selected theme option and the API status meet AA as rendered (367ms)
  ✓   9 [chromium] › tests/e2e/app.spec.ts:97:1 › Monitor asks for the admin token, then shows live health, processes and the journal (471ms)
  ✓  10 [chromium] › tests/e2e/app.spec.ts:130:1 › a wrong token is named as invalid credentials (239ms)
  ✓  11 [chromium] › tests/e2e/app.spec.ts:141:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (492ms)
  ✓  12 [chromium] › tests/e2e/app.spec.ts:172:1 › the desktop launch token authorises the shell, the Monitor and the event channel (204ms)
  ✓  13 [chromium] › tests/e2e/app.spec.ts:257:3 › inside the desktop webview (custom URI scheme adapter) › requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket (258ms)

  13 passed (13.8s)
```

`npm run test:e2e` in `apps/imagesquid`:

```text
Running 5 tests using 1 worker

  ✓  1 [chromium] › tests/e2e/app.spec.ts:22:1 › ImageSquid brand, album vocabulary and FileSquid API health (276ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:79:1 › ImageSquid tokens override FileSquid defaults in light and dark (IS-S-008) (205ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:104:1 › ImageSquid's theme control keeps a dark choice across a reload (359ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:124:1 › ImageSquid names missing credentials in its own words (170ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:133:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (369ms)

  5 passed (6.7s)
```
