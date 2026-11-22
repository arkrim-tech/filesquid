# Task UI-F3.4 Walkthrough: Virtualised navigation tree

[Task UI-F3.4](../plan.md) is a UI module row of global [Task FS-3.4](../../../../../resources/implementation-plan.md#task-fs-3.4) (MVP performance). It makes `<fs-nav-tree>` draw only its window, which closes the known gap that [Task UI-F3.1](./Task-UI-F3.1-Walkthrough.md) recorded as Finding 4. Global Task FS-3.4 stays open, because [Task DE-F3.2](../../../crates/desktop/resources/plan.md) still has to measure the packaged macOS app.

Owner decisions, verbatim. 2026-10-30: "Okay implement all these recommendations" (recommendation 1: virtualise the tree; recommendation 4: keep its cap of 10,000 nodes a level for the MVP). 2026-10-31: "accept recommendations" (the on-screen limit line).

## SRS mapping

| ID | Title | Covered by |
|---|---|---|
| [UI-S-035](../srs.md) | Production-build browsing performance (Task UI-F3.1): the navigation tree holds in the DOM at most the tree items in its window plus two overscans of 5, and keeps its tree semantics | `nav-tree-window.test.ts`, `nav-tree-rows.test.ts`, `nav-tree-viewport.test.ts`, `tests/ui/nav-tree-window.test.ts`, the preview spec `navigation-tree-window.spec.ts`, `40-resources.perf.spec.ts` (`dom-tree-items-large`, `tree-scroller-height-large`), the `20-browse` and `70-load` open-folder records gated strictly, `perf-known-gap.test.ts` |
| [UI-S-017](../srs.md) | The navigation tree: sections by kind with Recently deleted last, a level capped at its first 10,000 nodes with a console message and the `navigationLevelLimit` line, and pins and collapse state kept | `navigation-source.test.ts` (the cap by pages and by nodes), `tests/ui/nav-tree-window.test.ts` (the limit line, 100 pins, 1,000 collapsed nodes, a pin beyond the cap, a `not_found` pin), the preview spec (the limit line, a pin beyond the cap across a reload) |
| [UI-S-016](../srs.md) | Accessibility contracts: tree roles, keyboard, one Tab stop, focus recovery | `tests/ui/nav-tree-window.test.ts` (aria-level, aria-setsize, aria-posinset, aria-expanded and aria-current; the arrow keys, Home, End, expand, collapse, `*` and type-ahead over undrawn nodes; one Tab stop), `tests/ui/nav-tree.test.ts`, the preview spec (End and Home in a real layout) |
| [UI-S-025](../srs.md) | Shell composition: the tree scrolls inside the sidebar | the preview spec and `tree-scroller-height-large` (505 px, never taller than the 900 px window) |
| [UI-S-024](../srs.md) | Visual constants of the active mockup suite | the preview spec: 30 px rows, 13 px text, the active tint and text tokens and the muted limit line, in light and dark |
| [UI-S-005](../srs.md) | Label tables: every distribution defines every key | `navigationLevelLimit` in both tables; `distribution-tables.test.ts` |
| [UI-S-013](../srs.md) | UI tests run against production artifacts with strict DOM assertions | every Playwright test ran on `vite build` plus `vite preview` |
| [PRD NFR-002](../../../../../resources/prd.md#nfr-002-benchmark-envelope-and-responsiveness) | NFR-002: Benchmark envelope and responsiveness | the Large open-folder and back records below |
| [PRD NFR-003](../../../../../resources/prd.md#nfr-003-bounded-processing-and-migration-endurance) | NFR-003: Bounded processing and migration endurance | the tree's bounded DOM window |
| [PRD F-004](../../../../../resources/prd.md#f-004-collections-folders-and-navigation) | F-004: Collections, folders and navigation | the tree tests above |
| [PRD NFR-006](../../../../../resources/prd.md#nfr-006-accessibility-and-usability) | NFR-006: Accessibility and usability | the UI-S-016 row above |

## What changed

The holistic review came first ([holistic-ui.md](../../../../../.agents/rules/holistic-ui.md)). There is still one `<fs-nav-tree>`, and its data interface (`NavigationSource`, UI-F2.1a), its sections (UI-F2.8) and its item index (`nav-tree-index.ts`, UI-F3.1) are unchanged. The virtual window replaces the full render inside it.

- `nav-tree-rows.ts` (new, pure) turns a section's open levels into one flat list of rows in document order. Each row carries its level, its place in its level and that level's size (counted over the whole level), its parent row and its open state. A capped level ends in a limit row two rows tall.
- `nav-tree-window.ts` (new, pure) picks the rows of one tree that are in the window, with an overscan of 5 measured from the edges of the view, so all sections together stay within the bound.
- `nav-tree-viewport.ts` (new) reads the part of each tree that the window and every clipping ancestor leave in view, through the shell's slot and shadow root, and follows scrolls and size changes. It is injectable, so the jsdom tests need no layout.
- `nav-tree-list.ts` (new) keeps the drawn rows of one tree by row index. The focused row stays drawn until focus leaves it.
- `nav-tree-view.ts` makes a section's tree one flat `ul[role=tree]` as tall as all its rows. Each `li[role=none] > a[role=treeitem]` states `aria-level`, `aria-setsize` and `aria-posinset`. The nested `ul[role=group]` and `aria-owns` are gone, because a window cannot hold a group whose parent is not drawn. Rows are one line (1.875 rem); a long name ends in an ellipsis, and its full text is the row's tooltip.
- `fs-nav-tree.ts` builds the rows and draws each tree's window when the view moves. It runs the key rules of `nav-tree-keys.ts` (unchanged) over the rows, so the keys reach undrawn nodes: an undrawn node is drawn, then focused, then the window follows. The Tab stop is the active item when it is drawn, else the current destination, else the first row in view.
- `navigation/loader.ts` stops a level at 50 pages or 10,000 nodes (`MAX_LEVEL_NODES`), says so on the console and marks the level `capped`. The tree shows the `navigationLevelLimit` line under the last node shown and names the line as that node's description.
- The label key `navigationLevelLimit`, "Showing the first {count} only. Use search to find the rest.", is in the default and the ImageSquid tables.
- The known gap is removed. `treeGap` is gone from `known-gap.ts`, `report.ts`, `20-browse.perf.spec.ts`, `70-load.perf.spec.ts` and `perf-known-gap.test.ts`, which now fails if any spec declares a gap. The KNOWN GAP comment is gone from the performance feature file, and the README paragraph and UI-S-035 say that no gap is declared.
- `40-resources.perf.spec.ts` gains the tree's DOM bound on the Large catalog.

## Decisions

Two design changes come with the window, and both are owner decisions (2026-11-14, verbatim: "Accept both (Recommended)", the option "Fixed one-line rows and the flat ARIA tree are what let it draw only its window; the worker recorded both in UI-S-035."):

- **Fixed one-line rows.** Every row is 1.875 rem (30 px at the default text size), so a scroll position names the rows in view by arithmetic. A name too long for the sidebar ends in an ellipsis, and its full text is the row's tooltip. Before this task a long name wrapped onto more lines.
- **Flat ARIA tree.** A section's tree is one `ul[role=tree]`. Each item states its `aria-level`, `aria-setsize` and `aria-posinset`, and there is no nested `ul[role=group]` or `aria-owns`, because a window cannot hold a group whose parent is not drawn.

Both are recorded in [UI-S-035](../srs.md).

## Tests

### Failing first

The new tests ran before the code. This excerpt is from the 2026-11-11 run, when the two new pure modules did not exist yet:

```text
 FAIL  tests/unit/nav-tree-rows.test.ts [ tests/unit/nav-tree-rows.test.ts ]
Error: Failed to resolve import "../../src/components/nav-tree-rows" from "tests/unit/nav-tree-rows.test.ts". Does the file exist?
 FAIL  tests/unit/nav-tree-window.test.ts [ tests/unit/nav-tree-window.test.ts ]
Error: Failed to resolve import "../../src/components/nav-tree-window" from "tests/unit/nav-tree-window.test.ts". Does the file exist?
⎯⎯⎯⎯⎯⎯ Failed Tests 31 ⎯⎯⎯⎯⎯⎯⎯
 FAIL  tests/ui/nav-tree-window.test.ts > <fs-nav-tree> window (UI-S-035) > keeps at most the window plus two overscans of 5 tree items for 10,000 nodes
AssertionError: expected +0 to be 10000 // Object.is equality
     × shows the first 10,000 nodes of a longer level, in 50 pages of 200, and says so
     × End draws and focuses the last of 10,000 nodes, Home the first
     × shows the limit line under the last node, worded from the label table
     × opens a pin beyond the cap by its handle, and keeps it across a reload
```

The preview spec `navigation-tree-window.spec.ts` was written after the code. It passed on its first run.

### Prettier, ESLint, tsc and Vitest

`npx prettier --write`, `npx eslint .` and `npx tsc --noEmit` exit 0. `npm test`:

```text
 Test Files  126 passed (126)
      Tests  2905 passed | 1 skipped (2906)
```

This package has no coverage tool (`@vitest/coverage-*` is not installed, and no ADR adds one), so no coverage figure is reported.

### Playwright on the production build (`-c playwright.preview.config.ts`, under the machine's Playwright lock)

The new spec with the existing navigation spec:

```text
✓   2 [chromium] › tests/e2e/preview/navigation.spec.ts:65:1 › the live tree fills the shell's navigation slot, worded from the label table (2.1s)
✓   3 [chromium] › tests/e2e/preview/navigation.spec.ts:86:1 › a level is read page by page through next_cursor until it is null (379ms)
✓   4 [chromium] › tests/e2e/preview/navigation.spec.ts:124:1 › a destination opens with a breadcrumb filled from ancestors (449ms)
✓   5 [chromium] › tests/e2e/preview/navigation.spec.ts:145:1 › a deep link to a destination shows its breadcrumb after a reload (376ms)
✓   1 [chromium] › tests/e2e/preview/navigation-tree-window.spec.ts:130:1 › the tree holds only its window of a 10,000-node level, counted over the whole level (3.6s)
✓   6 [chromium] › tests/e2e/preview/navigation.spec.ts:157:1 › navigation.change reads only the named level again, without a reload (339ms)
✓   8 [chromium] › tests/e2e/preview/navigation.spec.ts:180:1 › a 503 shows the unavailable state with Retry, never an empty tree (354ms)
✓   9 [chromium] › tests/e2e/preview/navigation.spec.ts:197:1 › computed token values of the tree and breadcrumb are exact in light and dark (678ms)
✓   7 [chromium] › tests/e2e/preview/navigation-tree-window.spec.ts:163:1 › a capped level says so on screen under its last node, from the label table (1.2s)
✓  10 [chromium] › tests/e2e/preview/navigation-tree-window.spec.ts:185:1 › End reaches the last of 10,000 nodes and Home the first, with one Tab stop (816ms)
✓  11 [chromium] › tests/e2e/preview/navigation-tree-window.spec.ts:210:1 › a pin beyond the cap opens by its handle and survives a reload (2.1s)
✓  12 [chromium] › tests/e2e/preview/navigation-tree-window.spec.ts:228:3 › rows keep the look of mockup 55 in light (676ms)
✓  13 [chromium] › tests/e2e/preview/navigation-tree-window.spec.ts:228:3 › rows keep the look of mockup 55 in dark (681ms)
13 passed (30.0s)
```

The whole preview suite. The provenance-panel flake the orchestrator named did not occur in this run:

```text
✓  168 [chromium] › tests/e2e/preview/logs-view.spec.ts:709:3 › behaviour › a failed entry and a refusal are journal-only and offer no View provenance (648ms)
278 passed (2.2m)
```

Not run: the server-backed suite (`playwright.config.ts`), which needs a `filesquid-server` build, and this machine builds no Rust. Its `tests/e2e/navigation.spec.ts` assertions (an aria snapshot of closed nodes, and keys over a small tree) still hold for the flat markup on reading; no run confirms it.

### Performance suite (`npm run test:perf`) on the head-commit code

Run once on 2026-11-13, alone under the Playwright lock. The load average was high when the run started, because other work had just finished, and it fell during the run. The idle control held 60 Hz (595 frames, p99 16.8 ms), so the run counts.

Harness note: for this run the fake host also answered two start-up routes that the page now reads, `GET /api/v1/jobs` and `POST /api/v1/client-events`. Without them every performance test fails on "Failed to load resource: the server responded with a status of 404 (Not Found)" lines, as it does on main. On the orchestrator's instruction, that fix belongs to its own branch (`fix/ui-preview-unfaked-requests`) and is not part of this pull request. It changes the harness only, not the UI under measurement.

Machine:

```json
{
  "recorded": "2026-11-13T23:29:24.023Z",
  "host": "linux 6.18.44-fc-v445",
  "cpu": "Intel(R) Xeon(R) Processor @ 2.10GHz",
  "cores": 4,
  "memoryGiB": 15.7,
  "freeMemoryGiB": 14.2,
  "loadBefore": [
    9.11,
    5.73,
    3.26
  ],
  "node": "v22.23.4",
  "note": "Headless Chromium in a container, loopback, fake host with no modelled latency; not the packaged macOS app.",
  "browser": "Chromium 156.0.8078.4 (headless, Playwright)",
  "loadAfter": [
    1.45,
    1.86,
    2.27
  ]
}
```

Tests:

```text
✓   1 [chromium] › tests/performance/00-control.perf.spec.ts:27:1 › the browser build is recorded with the machine (15ms)
✓   2 [chromium] › tests/performance/00-control.perf.spec.ts:35:1 › control: an idle page keeps 60 Hz, so the machine can judge the app (11.0s)
✓   3 [chromium] › tests/performance/00-control.perf.spec.ts:59:1 › control: the route round trip under every request of the harness (1.1s)
✓   4 [chromium] › tests/performance/10-cold-start.perf.spec.ts:29:3 › first useful folder view, small catalog, cold (NFR-002 <= 3 s) (4.6s)
✓   5 [chromium] › tests/performance/10-cold-start.perf.spec.ts:29:3 › first useful folder view, medium catalog, cold (NFR-002 <= 3 s) (4.5s)
✓   6 [chromium] › tests/performance/10-cold-start.perf.spec.ts:29:3 › first useful folder view, large catalog, cold (NFR-002 <= 3 s) (4.6s)
✓   7 [chromium] › tests/performance/10-cold-start.perf.spec.ts:91:1 › a traced cold load of the Large catalog is recorded as evidence (449ms)
✓   8 [chromium] › tests/performance/20-browse.perf.spec.ts:70:3 › warm browse, Large catalog › selecting a row and moving with the keyboard (28.5s)
✓   9 [chromium] › tests/performance/20-browse.perf.spec.ts:85:3 › warm browse, Large catalog › sorting and switching the layout (31.2s)
✓  10 [chromium] › tests/performance/20-browse.perf.spec.ts:104:3 › warm browse, Large catalog › opening folders and going back (cache miss every time) (40.2s)
✓  11 [chromium] › tests/performance/20-browse.perf.spec.ts:126:3 › warm browse, Large catalog › narrowing with the sidebar filter (18.2s)
✓  12 [chromium] › tests/performance/30-scroll.perf.spec.ts:31:3 › list: frames while scrolling stay inside the 60 Hz limits (35.5s)
✓  13 [chromium] › tests/performance/30-scroll.perf.spec.ts:31:3 › grid: frames while scrolling stay inside the 60 Hz limits (35.5s)
✓  14 [chromium] › tests/performance/40-resources.perf.spec.ts:41:3 › medium: the DOM holds a bounded window with every page loaded (3.5s)
✓  15 [chromium] › tests/performance/40-resources.perf.spec.ts:41:3 › large: the DOM holds a bounded window with every page loaded (3.5s)
✓  16 [chromium] › tests/performance/40-resources.perf.spec.ts:109:1 › large: the navigation tree holds a bounded window of its 10,000 nodes (959ms)
✓  17 [chromium] › tests/performance/40-resources.perf.spec.ts:169:1 › heap growth after warm-up stays within 10% over 600 mixed interactions (1.5m)
✓  18 [chromium] › tests/performance/50-preview.perf.spec.ts:149:1 › a prepared display-size image paints at p95 <= 100 ms, over every image of the folder (27.9s)
✓  19 [chromium] › tests/performance/50-preview.perf.spec.ts:177:1 › a 12-megapixel original: first useful equals final, within 2 s warm (29.1s)
✓  20 [chromium] › tests/performance/50-preview.perf.spec.ts:205:1 › text reads one ranged request of at most 64 KiB, painted at p95 <= 100 ms (21.9s)
✓  21 [chromium] › tests/performance/50-preview.perf.spec.ts:255:3 › cold: the first small preview after load, in a new context (20) (7.2s)
✓  22 [chromium] › tests/performance/50-preview.perf.spec.ts:255:3 › cold: the first large preview after load, in a new context (20) (7.3s)
✓  23 [chromium] › tests/performance/50-preview.perf.spec.ts:320:1 › content that cannot be read shows the error state within 2 s, never a spinner (3.5s)
✓  24 [chromium] › tests/performance/60-search.perf.spec.ts:26:1 › search: query feedback, live files and completed results over 120 measured queries (1.3m)
✓  25 [chromium] › tests/performance/70-load.perf.spec.ts:46:1 › background work: select and open-folder keep their budgets while events stream in (15.1s)
✓  26 [chromium] › tests/performance/70-load.perf.spec.ts:87:1 › background work: opening folders under the same stream (45.7s)
✓  27 [chromium] › tests/performance/70-load.perf.spec.ts:125:1 › resource pressure: select, sort and open-folder at 4x CPU slowdown are reported (1.5m)
✓  28 [chromium] › tests/performance/80-composition.perf.spec.ts:44:5 › light theme › the shell and the list of mockup 05 hold with the Large catalog loaded (673ms)
✓  29 [chromium] › tests/performance/80-composition.perf.spec.ts:119:5 › light theme › the preview with its file panel (mockups 22 and 35) as built, over generated images (379ms)
✓  30 [chromium] › tests/performance/80-composition.perf.spec.ts:44:5 › dark theme › the shell and the list of mockup 05 hold with the Large catalog loaded (642ms)
✓  31 [chromium] › tests/performance/80-composition.perf.spec.ts:119:5 › dark theme › the preview with its file panel (mockups 22 and 35) as built, over generated images (371ms)
31 passed (11.0m)
```

Raw summary (milliseconds):

| measurement | n | warm-up | p50 | p90 | p95 | p99 | max | value | budget | verdict |
| background-open-folder-feedback-large | 120 | 20 | 42.7 | 49 | 49.5 | 75.7 | 89.1 |  | NFR-002 p95 <= 100 ms; p99 <= 200 ms (same gates as idle) | pass |
| background-open-folder-result-large | 120 | 20 | 59.9 | 76.4 | 80.7 | 122.4 | 125.6 |  | NFR-002 p95 <= 100 ms; p99 <= 200 ms (same gates as idle) | pass |
| background-select-feedback-large | 120 | 20 | 19.1 | 31.3 | 32.3 | 32.7 | 32.7 |  | NFR-002 p95 <= 100 ms; p99 <= 200 ms (same gates as idle) | pass |
| cold-first-useful-large | 20 | 0 | 154.2 | 171.6 | 178 | 189.8 | 189.8 |  | NFR-002 first useful local viewport <= 3 s | pass |
| cold-first-useful-medium | 20 | 0 | 153.5 | 170.7 | 172.1 | 180.1 | 180.1 |  | NFR-002 first useful local viewport <= 3 s | pass |
| cold-first-useful-small | 20 | 0 | 156.4 | 178.9 | 179.5 | 185.5 | 185.5 |  | NFR-002 first useful local viewport <= 3 s | pass |
| content-height-large | 1 | 0 |  |  |  |  |  | 484000 | UI-S-026: rows are 44 px tall; 11,000 rows (the footer counts them all) | pass |
| content-height-medium | 1 | 0 |  |  |  |  |  | 484000 | UI-S-026: rows are 44 px tall; 11,000 rows (the footer counts them all) | pass |
| control-idle-frames | 595 | 5 | 16.7 |  | 16.7 | 16.8 | 16.8 |  | 60 Hz, frame budget 16.7 ms (NFR-002); missed > 25.05 ms; at most 1% missed; at least 550 frames | pass |
| control-route-round-trip | 200 | 20 | 2.9 | 4.6 | 5.5 | 9.8 | 16.1 |  | reported; the floor under every request in this harness | reported |
| dom-rows-large | 1 | 0 |  |  |  |  |  | 20 | NFR-003 bounded queues and caches: window rows + 2 x overscan of 5 | pass |
| dom-rows-medium | 1 | 0 |  |  |  |  |  | 20 | NFR-003 bounded queues and caches: window rows + 2 x overscan of 5 | pass |
| dom-tree-items-large | 1 | 0 |  |  |  |  |  | 28 | UI-S-035: tree items in the window + 2 x overscan of 5 | pass |
| heap-growth-mixed | 1 | 0 |  |  |  |  |  | 0.04 | NFR-003 post-warm-up non-cache memory growth <= 10% (the 72-hour soak is not run here) | pass |
| pressure-back-result-large | 100 | 20 | 99.1 | 116.1 | 130.3 | 158 | 158.3 |  | reported at 4x CPU slowdown; budgets are for the reference machine | reported |
| pressure-open-folder-feedback-large | 100 | 20 | 64 | 74.6 | 80.6 | 82.4 | 82.7 |  | reported at 4x CPU slowdown; budgets are for the reference machine | reported |
| pressure-open-folder-result-large | 100 | 20 | 119.4 | 140.8 | 158.3 | 181.8 | 191.2 |  | reported at 4x CPU slowdown; budgets are for the reference machine | reported |
| pressure-select-feedback-large | 100 | 20 | 18.3 | 31.4 | 32.5 | 34.5 | 35.3 |  | reported at 4x CPU slowdown; budgets are for the reference machine | reported |
| pressure-sort-feedback-large | 100 | 20 | 32.1 | 34.7 | 49 | 50.1 | 60.4 |  | reported at 4x CPU slowdown; budgets are for the reference machine | reported |
| pressure-sort-result-large | 100 | 20 | 80 | 94.7 | 99.1 | 100.8 | 110.4 |  | reported at 4x CPU slowdown; budgets are for the reference machine | reported |
| preview-cold-large | 20 | 0 | 61.8 | 78 | 78.3 | 78.7 | 78.7 |  | UI-S-035 12-megapixel original cold <= 3,000 ms | pass |
| preview-cold-small | 20 | 0 | 62.3 | 78.6 | 78.7 | 78.7 | 78.7 |  | reported: cold behaviour is reported apart from the warm p95 (NFR-002) | reported |
| preview-failure-visible | 20 | 0 | 45.8 | 61.8 | 62.3 | 63.1 | 63.1 |  | UI-S-035 failure visible <= 2,000 ms (never a spinner) | pass |
| preview-large-ack | 120 | 20 | 29.5 | 30 | 30.1 | 30.3 | 30.4 |  | NFR-002 acknowledgment p95 <= 100 ms | pass |
| preview-large-image | 120 | 20 | 62.8 | 63.4 | 77.7 | 79.5 | 79.6 |  | UI-S-035 12-megapixel original warm p95 <= 2,000 ms | pass |
| preview-small-ack | 120 | 20 | 29.4 | 29.8 | 30 | 30.2 | 30.2 |  | NFR-002 acknowledgment p95 <= 100 ms | pass |
| preview-small-image | 120 | 20 | 62.7 | 77.7 | 79.6 | 79.8 | 96.1 |  | NFR-002 prepared viewer image paints p95 <= 100 ms | pass |
| preview-text-ack | 120 | 20 | 29 | 29.6 | 29.7 | 29.9 | 30.1 |  | NFR-002 acknowledgment p95 <= 100 ms | pass |
| preview-text-image | 120 | 20 | 45.7 | 46.3 | 46.3 | 59 | 63.4 |  | NFR-002 prepared view paints p95 <= 100 ms | pass |
| scroll-dom-rows-grid | 1 | 0 |  |  |  |  |  | 70 | NFR-003 bounded window: visible + 2 x overscan of 5 | pass |
| scroll-dom-rows-list | 1 | 0 |  |  |  |  |  | 26 | NFR-003 bounded window: visible + 2 x overscan of 5 | pass |
| scroll-frames-grid | 1903 | 5 | 16.7 |  | 16.7 | 16.8 | 33.2 |  | 60 Hz, frame budget 16.7 ms (NFR-002); missed > 25.05 ms; at most 5% missed; p99 <= 50 ms; stalls > 100 ms <= 0.1%; at least 1500 frames | pass |
| scroll-frames-list | 1903 | 5 | 16.7 |  | 16.7 | 16.8 | 33.2 |  | 60 Hz, frame budget 16.7 ms (NFR-002); missed > 25.05 ms; at most 5% missed; p99 <= 50 ms; stalls > 100 ms <= 0.1%; at least 1500 frames | pass |
| scroller-height-large | 1 | 0 |  |  |  |  |  | 654 | UI-S-025: the list scrolls inside the content area (window 900 px) | pass |
| scroller-height-medium | 1 | 0 |  |  |  |  |  | 654 | UI-S-025: the list scrolls inside the content area (window 900 px) | pass |
| search-completed-results | 120 | 20 | 62.1 | 62.6 | 62.6 | 62.9 | 79 |  | NFR-002 indexed metadata search p95 <= 2 s | pass |
| search-live-files | 120 | 20 | 287.5 | 288.1 | 288.4 | 301.3 | 303.6 |  | NFR-002 indexed metadata search p95 <= 2 s | pass |
| search-query-feedback | 120 | 20 | 20.8 | 21.4 | 21.6 | 31.8 | 32 |  | NFR-002 acknowledgment p95 <= 100 ms; p99 <= 200 ms | pass |
| tree-scroller-height-large | 1 | 0 |  |  |  |  |  | 505 | UI-S-035, UI-S-025: the tree scrolls inside the sidebar (window 900 px) | pass |
| warm-back-feedback-large | 120 | 20 | 35.9 | 36.9 | 46.3 | 49.6 | 49.7 |  | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-back-result-large | 120 | 20 | 36.7 | 52.6 | 53.5 | 66.4 | 69.1 |  | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-filter-feedback-large | 120 | 20 | 37.3 | 41.5 | 48.1 | 51.3 | 52.3 |  | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-filter-result-large | 120 | 20 | 37.3 | 41.5 | 48.1 | 51.3 | 52.3 |  | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-keyboard-feedback-large | 120 | 20 | 21.6 | 22 | 22.1 | 32.3 | 32.5 |  | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-layout-feedback-large | 120 | 20 | 19.2 | 20.3 | 20.5 | 32.9 | 33.1 |  | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-layout-result-large | 120 | 20 | 19.2 | 20.3 | 20.5 | 32.9 | 33.1 |  | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-open-folder-feedback-large | 120 | 20 | 41.5 | 43.4 | 44.8 | 59.1 | 63.7 |  | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-open-folder-result-large | 120 | 20 | 56.6 | 59.7 | 60.4 | 75.8 | 80.5 |  | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-select-feedback-large | 120 | 20 | 20.3 | 21.3 | 21.4 | 31.2 | 33.1 |  | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-sort-feedback-large | 120 | 20 | 19.6 | 25.5 | 31 | 32.9 | 33.1 |  | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-sort-result-large | 120 | 20 | 36.3 | 46.2 | 48.5 | 52.6 | 53.5 |  | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | pass |

### Before and after: Large catalog, open folder and go back

N = 120 after a warm-up of 20 in every row. Budget: p95 ≤ 100 ms and p99 ≤ 200 ms, for result and feedback alike.

| Record | Before p95 | Before p99 | After p95 | After p99 |
|---|---|---|---|---|
| warm-open-folder-result-large | 110.3 | 181.1 | 60.4 | 75.8 |
| warm-back-result-large | 119.5 | 150.2 | 53.5 | 66.4 |
| warm-open-folder-feedback-large | 83.0 | 114.5 | 44.8 | 59.1 |
| warm-back-feedback-large | 88.8 | 116.9 | 46.3 | 49.6 |
| background-open-folder-result-large | 106.6 | 143.0 | 80.7 | 122.4 |
| background-open-folder-feedback-large | 65.0 | 95.8 | 49.5 | 75.7 |

Before: origin/main at `3a0f620`, the two open-folder tests only, 2026-11-11, on the same machine (Intel Xeon 2.10 GHz, 4 cores, 15.7 GiB, Node v22.23.4; load average 4.46 before the run and 6.75 after it). The three result records missed their p95 (the declared gap), and both tests also failed on the 404 console lines above. Task UI-F3.1's head-commit run had recorded 115.8 ms and 117.6 ms.

After: the run above. With all 10,000 nodes of the level shown, the tree holds 28 tree items in a 505 px scrolling area (`dom-tree-items-large`, bound 28).

## Pre-commit hook

`bash scripts/pre-commit.sh` passed on each commit with the line `✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui`.

## Sync Execution Report

- [UI-S-035](../srs.md): the known-gap sentences now require the Large open-folder and back records to be gated strictly with no declared gap. They also record the row height, the one-line rows with a tooltip, and the limit line as the last node's description.
- Root `README.md`: the performance paragraph says no known gap is declared.
- BDD: [`production_build_browsing_performance_NFR-002.feature`](../../tests/features/production_build_browsing_performance_NFR-002.feature) lost its KNOWN GAP comment. [`virtualised_navigation_tree_UI-S-035.feature`](../../tests/features/virtualised_navigation_tree_UI-S-035.feature) already holds a scenario for each behaviour built here.
- PRD, product vision and ADRs are unchanged, because no library, route or architecture was added.
- The [UI plan](../plan.md) row UI-F3.4 and its [checklist](../../../../../resources/implementation-plan-checklist.md) row are ticked, and the MVP progress is redrawn. Global Task FS-3.4 stays open for DE-F3.2.

## Mocked Environment Variables

None. The Vitest and Playwright fakes answer in the contract's own shapes.
