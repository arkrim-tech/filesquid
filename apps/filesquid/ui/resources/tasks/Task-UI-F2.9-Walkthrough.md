# Task UI-F2.9 Walkthrough: Global search bar and filter chips

Branch `feat/ui-f2.9-search-bar-chips`. Module row of global Task FS-2.5. Plan row: [Task UI-F2.9](../plan.md). Mockups (active suite v3): [20](../../../../../resources/ui-mockups/v3/20-search-suggestions.svg), [21](../../../../../resources/ui-mockups/v3/21-search-results.svg), [53](../../../../../resources/ui-mockups/v3/53-search-unknown-operator.svg); composition UI-S-030, constants UI-S-024. Built on the view and selection model of [UI-F2.8](./Task-UI-F2.8-Walkthrough.md) and the search operation of API-F2.3 (contract 0.32.0 after the merge of `origin/main`; the search operation itself is as 0.31.0 delivered it).

## SRS mapping

| SRS ID and title | What was built | Proof |
|---|---|---|
| [UI-S-007](../srs.md) The global search bar MUST offer Type, People, Modified and Location chips with typed suggestions, chip-plus-text layering, per-chip remove and clear-all, drop-down results and Enter for the full list with column sorting; the sidebar filter MUST narrow the current view only; the active scope and result summary MUST show as chips; unsupported operators MUST be shown as unsupported | `<fs-search-bar>`: Type, Location, Modified and Title chips (People left out, UI-S-030), typed suggestions, chips plus text in one field, a remove control on each chip, Clear all, the drop-down with live FILES, Enter to `#/search/<query>`, the results in the Drive list with column heads, the scope and count chips (also while the sidebar filter holds text), and the refusal box for an operator the host cannot answer or does not know | `search-bar.test.ts`, `search-results.test.ts`, `search-route.test.ts`, `search.spec.ts` |
| [UI-S-030](../srs.md) Search composition: while typing (mockup 20), refused as unavailable (mockup 21), unknown operator (mockup 53) | Applied chips inside the field (24 px, `--fs-primary-container`, 13 px 500), Clear all, the field widened to 620 px with the focus border, the 620 x 520 px drop-down with FILTERS, SUGGESTIONS (`in:`, `type:`, `title:`), FILES, OPERATORS and "Enter shows all results"; "Search results" with "In: {library}" and "{n} files" chips, the error box "{operator} is not available on this device." or "{operator} is not a search operator." with their help lines, "Nothing was searched.", OPERATORS ON THIS DEVICE and the footer "0 results"; keys `searchUnavailable`, `searchUnavailableHelp`, `searchUnknownOperator`, `searchUnknownOperatorHelp`, `searchNothingSearched`, `searchOperatorsOnHost`, `searchEnterHint` and 31 more | the three mockup tests of `search.spec.ts` in light and dark (boxes within 2 px, computed styles, strings from the label table) |
| [UI-S-024](../srs.md) Visual constants of the active mockup suite | Every colour through a token in light and dark (field, chips, drop-down, rows, badge, refusal box, operator chips), Georgia title 30 px 700, 14, 13, 11 and 10 px text, eyebrows 11 px 400 upper case 0.1em, radii 6, 8, 5, 4 and 3 px, 36 px field, 1.5 px control border and 2 px focus border; spacing, type size and radius only from tokens (`component-styles.test.ts` covers the three new stylesheets); no new token | `search.spec.ts` computed-style assertions, `component-styles.test.ts` |
| [UI-S-025](../srs.md) Shell composition | The header's search field is now the bar (520 x 36 px at 24 px from the sidebar, placeholder `searchPlaceholder`); the shell's `search` slot sizes to it; the sidebar filter is unchanged | `mockup-fidelity.spec.ts` (shell frame), `production-build.spec.ts` |
| [UI-S-005](../srs.md) A distribution's label table MUST define every key of the default table | 38 new keys in `REQUIRED_LABEL_KEYS`, the default table and ImageSquid's table (words copied) | `distribution-tables.test.ts`, `theme.test.ts` |
| [UI-S-016](../srs.md) Accessibility contracts | Combobox over a listbox (`aria-expanded`, `aria-controls`, `aria-activedescendant`), arrow keys, Enter, Escape, Backspace, named remove controls and Clear all, the refusal box as `role="alert"`, `aria-sort` only on a chosen column | `search-bar.test.ts`, `search-results.test.ts`, the Playwright combobox test |
| [UI-S-013](../srs.md) UI tests run against production artifacts (`vite build` plus `vite preview`) with strict DOM assertions: custom elements defined, labels from the active label table, computed token values in light and dark, no console errors | 18 Playwright tests against the build at 1440 x 900, light and dark | `search.spec.ts` |

PRD: [F-063](../../../../../resources/prd.md#f-063-file-search-query-language-and-filter-chips) File search query language and filter chips. SRS: [SRS-F-063](../../../../../resources/srs.md#srs-f-063-file-search-query-language-and-filter-chips) File search query language and filter chips (its **Default UI** clause). API: [API-S-012](../../../crates/api/resources/srs.md) Search operation and operator grammar. Feature: [`global_search_bar_and_filter_chips_UI-S-030.feature`](../../tests/features/global_search_bar_and_filter_chips_UI-S-030.feature) (new, written with the feature-writer skill) and [`file_search_query_language_F-063.feature`](../../../crates/api/tests/features/file_search_query_language_F-063.feature) (the API's, whose Default UI scenarios this task satisfies).

## What changed

- **Search model** (`src/search/`): `model.ts` (chips, query composition and parsing so an address restores the bar, the Modified ranges in the caller's calendar and zone, the grammar's tokeniser as the API reads it, `refusalOf` that names the operator of a 503 or 400, type suggestions, the bounds 1,024 bytes, 32 terms, 8 chips, 3 live files), `source.ts` (`SearchSource` and `ClientSearchSource`), `memory-source.ts` (a fixture that answers and refuses as the operation does). `src/navigation/library-scope.ts` names the library a view and a search are scoped to.
- **Elements** (`src/components/`): `fs-search-bar.ts` with `search-dropdown.ts` (the drop-down's model and DOM); `fs-drive-view.ts` gained a search mode (`search` property: the same list, selection, bulk bar, menus and states, a local sort that starts at relevance, scope and count chips, the refusal box in `drive-search.ts`); `drive-row-view.ts` lets the column heads be sortable with no active key (`relevance`) and exports the badge style; `fs-navigation-view.ts` adds the node's name to `fs-destination-context` for the bar's `in:` suggestion.
- **Routes** (`src/app/`): `routes.ts` (`#/search/<query>`, `searchHash`, `parseSearchHash`), `drive-route.ts` (the one binder of the shell's drive view also owns the search route, the `fs-search` event and the bar's query and folder), `main.ts` wiring.
- **Holistic UI** ([holistic-ui.md](../../../../../.agents/rules/holistic-ui.md)): the results are the existing drive view in a search mode, not a second listing, so UI-F2.11 (versions) and UI-F2.17 (uploads) build on one list; the bar is its own element in a named slot and depends on no other panel.
- **Tests**: three existing specs now follow the bar (`mockup-fidelity.spec.ts`, `production-build.spec.ts` and `drive-shell.test.ts`, whose `fs-destination-context` detail now carries the collection's name), and the e2e helper `drive-api.ts` exports `viewRow`.

## Decisions to confirm

- **Focus border token**: UI-S-030 says the focused field has a "2 px `--fs-primary` focus border"; UI-S-024 (owner decision CA-1, verbatim "a") puts the focus ring and focused input border in `--fs-focus-ring` (#0f8a39 / #16ac49) because the guide's primary fails 3:1 on white. The field uses `--fs-focus-ring`, as every input does. UI-S-030's wording needs the same correction if the owner agrees.
- **Primary as text**: the mono operator of a suggestion row and Clear all are `--fs-primary` text, as the mockup and UI-S-030 draw them. On the light surfaces this is inside the exemption UI-S-024 (SRS-UI-007, CA-1) already lists (`--fs-primary` as text on `#ffffff`, 2.99:1); in the dark theme it passes 4.5:1 without one. The owner may want the exemption to name these two new uses.
- **Type values are the API's classes** (document, image, video, audio, text, folder): mockup 20's "Type: PDF" would need a per-format filter the grammar does not have; a PDF is a document. Typed words suggest a class ("pres" suggests documents, "photo" images).
- **Modified ranges** count back from the caller's calendar date: today, 7, 30 and 365 days (`after:` that date, with the `zone` parameter), so "last 30 days" asked on 16 October 2026 is `after:2026-09-16`.
- **Scope** is the first library the host lists, the one All files shows (the `singleOpenLibrary` stop-gap): "In: {library}". `in:` narrows to the collection shown and is offered as a suggestion; Location lists the library's collections.
- **The sidebar filter inside search results** narrows the rows already read by name (the search operation has no name filter), so the footer and the count chip follow the narrowed rows; it never changes the query.
- **Live files show only the answer to the query in the field**: while a newer query is in flight the FILES section is empty rather than listing files of an older one.
- **Relevance has no column**: a column head sorts, a second click reverses and a third returns to relevance, so no control the mockup does not draw was added.
- **Pixel fidelity within spacing tokens**: the component-style test lets margin, padding and gap use only the 4, 8, 16, 24 px tokens, so some insets land 1 to 2 px from the mockup (the results chips at y 118 against 120, the suggestion text at x 305 against 306); sizes use exact values. The operator chips wrap after a different chip than the mockup because the mockup's mono font is wider than the system mono; the test asserts the first row only. The results page lifts its title 8 px because the mockup draws it 7 px higher than the 24 px content padding the other pages use.
- **Partial operators are refused by the API while typing** (for example `type:` with no value is 400): the bar shows no files for it and no UI error, but the browser logs a network line per refused read.

## What is not built, and what the platform engineer must add

1. The view and search rows need `library_id` and the file's collection title (and a head `version`). Until then the live files show only the time (the mockup shows "{collection} · {time}"), and file actions on a result need exactly one open library.
2. There is no suggestions route (API-F2.3 lists "search suggestions" as remaining). The drop-down's FILES section uses the search operation with a page of three, relevance order and `folders=mixed`.
3. `is:unfiled` is still accepted by the built API until API-F2.15 retires it; the UI never offers it, and a later 400 for it is named as "is:unfiled is not a search operator."
4. The operators the host answers are a fixed list in the UI (the ten of the mockups). A host descriptor field naming them would let `filesquid-server` show its own after SV-F1.4; the refusal names its operator from the query because the API's `message` is the only place it appears (an `operator` member on the refusal would be sturdier).
5. Not built here, by the task's instruction: the People chip (no accounts), `sharedwith:`, `to:`, `from:` and `pendingowner:` (SV-F1.4), `is:favorite` and `owner:` of another user (refused as unavailable, and named as such), content search and natural-language questions.

## Tests

Failing first (before `src/search/` and `fs-search-bar` existed):

```
 Test Files  5 failed (5)
      Tests  no tests
Error: Failed to resolve import "../../src/search/memory-source" from "tests/unit/search-source.test.ts". Does the file exist?
```

Prettier, ESLint and `tsc` in `apps/filesquid/ui` (and Prettier over `apps/imagesquid`):

```
Checking formatting...
All matched files use Prettier code style!
eslint exit 0
tsc exit 0
```

Vitest (unit and DOM tests in jsdom), the new files and the total:

```
 ✓ tests/ui/search-bar.test.ts (35 tests)
 ✓ tests/ui/search-results.test.ts (24 tests)
 ✓ tests/unit/search-model.test.ts (47 tests)
 ✓ tests/unit/search-source.test.ts (17 tests)
 ✓ tests/ui/search-route.test.ts (11 tests)
 ✓ tests/unit/library-scope.test.ts (8 tests)

 Test Files  83 passed (83)
      Tests  1985 passed | 1 skipped (1986)
```

Playwright against `vite build` plus `vite preview` (`FILESQUID_E2E=1`, 1440 x 900, light and dark), the new spec:

```
  ✓   1 light theme › the field (mockup 20): size, border, placeholder and the widening on focus
  ✓   2 light theme › the drop-down (mockup 20): regions, sections and constants
  ✓   3 light theme › the active suggestion row is on the raised surface and named for assistive technology
  ✓   4 light theme › Enter shows the results in the Drive list, asked as the chips and text make it
  ✓   5 light theme › an operator the device cannot answer is refused and named (mockup 21)
  ✓   6 light theme › an operator that is not in the grammar is refused as an invalid request (mockup 53)
  ✓   7 dark theme › the field (mockup 20): size, border, placeholder and the widening on focus
  ✓   8 dark theme › the drop-down (mockup 20): regions, sections and constants
  ✓   9 dark theme › the active suggestion row is on the raised surface and named for assistive technology
  ✓  10 dark theme › Enter shows the results in the Drive list, asked as the chips and text make it
  ✓  11 dark theme › an operator the device cannot answer is refused and named (mockup 21)
  ✓  12 dark theme › an operator that is not in the grammar is refused as an invalid request (mockup 53)
  ✓  13 the sidebar filter shows its scope and count as chips and narrows only the current view
  ✓  14 Back and Forward bring the search and its chips back, and All files clears the bar
  ✓  15 a file chosen in the drop-down opens as a Drive row does
  ✓  16 Clear all empties the field and runs no search
  ✓  17 the Location chip limits the search to a collection
  ✓  18 the live files wait for a pause in typing and a refusal there is not a console error of ours
  18 passed
```

The whole preview suite after Cubic round 1:

```
  162 passed (1.0m)
```

The server-backed Playwright suite (`playwright.config.ts`) starts `filesquid-server` with Cargo and was not run: this change builds no Rust, and the change scope is npm-only. The pre-commit hook, ordinary `bash scripts/pre-commit.sh` (the change scope against `origin/main` is `npm:apps/filesquid/ui`, so no Cargo step runs):

```
ℹ️  Test scope: npm:apps/filesquid/ui
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (84s)
```

## Sync Execution Report

- PRD [F-063](../../../../../resources/prd.md#f-063-file-search-query-language-and-filter-chips), SRS [SRS-F-063](../../../../../resources/srs.md#srs-f-063-file-search-query-language-and-filter-chips), the module SRS rows UI-S-007, UI-S-024 and UI-S-030 and API-S-012 are followed as written; no upstream document was changed to match the code. Where the build reads differently (the focus border token, primary as text, the Type values) it is listed under Decisions to confirm for the owner, not edited into the SRS.
- Edited: the module plan row (status, Related Tests, box), the repository-root checklist row and the MVP progress bar, and the package README (the search section). No ADR is needed: the `#/search/<query>` route is a hash route like the others, and no library or architecture was added.
- BDD: [`global_search_bar_and_filter_chips_UI-S-030.feature`](../../tests/features/global_search_bar_and_filter_chips_UI-S-030.feature) carries the scenarios for the built behaviour.
- The module plan box and its checklist row are ticked; the global Task FS-2.5 stays open, so its box is unchanged.

## Mocked Environment Variables

None. The Playwright fakes answer the contract's own shapes. The Playwright Chromium build the installed Playwright expects (1248) was downloaded into the machine's browser directory with `npx playwright install chromium`; no credential or setting was faked.
