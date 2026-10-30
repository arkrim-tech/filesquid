# Task UI-F3.1 Walkthrough: Production-build browsing performance

[Task UI-F3.1](../plan.md) is the UI module row of global [Task FS-3.4](../../../../../resources/implementation-plan.md#task-fs-3.4) (MVP performance). It measures the default UI's production build (`vite build` plus `vite preview`) in this container's Chromium with real input, real components and the v3 composition, and turns every budget into a failing assertion. The packaged macOS measurement stays with [Task DE-F3.2](../../../crates/desktop/resources/plan.md), so the global FS-3.4 box stays open.

Holistic review first ([holistic-ui.md](../../../../../.agents/rules/holistic-ui.md)): the measurements exercise the same drive view, navigation tree, search bar, file browser and file panel that UI-F2.x built, and the fixes below are changes inside those elements, not scaffolding a later task replaces.

## SRS mapping

| ID | Title | Covered by |
|---|---|---|
| [UI-S-035](../srs.md) | Production-build browsing performance (Task UI-F3.1) | `tests/performance/*.perf.spec.ts` (30 tests, one per class below), `tests/unit/perf-*.test.ts`, the feature file |
| [PRD NFR-002](../../../../../resources/prd.md#nfr-002-benchmark-envelope-and-responsiveness) | NFR-002: Benchmark envelope and responsiveness | warm feedback and result p95 and p99, first useful view, preview, search, scroll frames, with sample size, warm-up and percentiles per record |
| [PRD NFR-003](../../../../../resources/prd.md#nfr-003-bounded-processing-and-migration-endurance) | NFR-003: Bounded processing and migration endurance | `40-resources.perf.spec.ts` (bounded DOM window, heap growth after warm-up); the 72-hour soak is not run |
| [PRD RC-002](../../../../../resources/prd.md#rc-002-performance-and-resource-bounds) | RC-002: Performance and resource bounds | cold loads on three profiles, direction reversals and jumps in the scroll runs, background-event and CPU-pressure cases |
| [FileSquid U-11](../../../../../resources/product-vision.md#platform-ownership-and-access) | A large collection of large files feels as quick to browse and present as a small one, even while the app is processing other work | the Large catalog and the background-event case |
| [UI-S-013](../srs.md) | UI tests run against production artifacts (`vite build` plus `vite preview`) with strict DOM assertions | every spec runs on the built artifact; custom elements defined, labels from the active table, no console errors, computed values in light and dark |
| [UI-S-024](../srs.md) | Visual constants of the active mockup suite | `80-composition.perf.spec.ts` under the Large catalog, light and dark |
| [UI-S-025](../srs.md) | Shell composition | region boxes within 2 px; the list scrolls inside the content area (`40-resources`) |
| [UI-S-026](../srs.md) | Browser views, menus and dialogs | list header 38 px, rows 44 px, badge 30 x 16, layout control 72 x 32 |
| [UI-S-031](../srs.md) | Preview and file panel composition | file panel 320 px on card with a 1 px outline edge, as built |
| [ADR 007](../../../../../resources/architecture/007-durable-jobs-and-interactive-performance.md) / [ADR 017](../../../../../resources/architecture/017-test-runners-and-reporting.md) | Interactive performance; test runners and reporting | Playwright and Vitest with JUnit and Allure under `tests/allure-results/` |

## Environment, stated plainly

This is **not** the packaged macOS app. Headless Chromium 156 (Playwright 1.64) in a Linux container (4 vCPU Intel Xeon 2.10 GHz, 15.7 GiB), loopback, a host faked in process with no modelled latency, 1440 x 900 at device scale factor 2, UTC, one worker. The machine is not exclusive: another agent may use it, so an idle control page runs first and fails the run if it misses more than 1% of its frames (it passed: p99 frame 16.8 ms, slowest of 594 frames 33.3 ms). The harness floor (a route round trip) is 3.6 ms p50 and 6.2 ms p95 and is printed beside every result.

## What the measurements found, and what changed

Each fix had a failing test first. Findings 1 to 3 were fixed; Findings 4 and 5 stay open.

1. **The list was not bounded by the window.** The Large and Medium views measured a scrolling area 484,000 px tall (11,000 rows x 44 px) against a 900 px window: the navigation view had no height of its own, so the virtual window never windowed. Fixed in `fs-navigation-view.ts` and `fs-app-shell.ts` (a bounded flex column, `min-block-size: 0`). Red: `ui-f3.1-red-resources` below; green: `scroller-height-*` and `dom-rows-*` rows.
2. **A destination change searched the whole navigation tree.** Moving `aria-current` and the Tab stops by `querySelector` walked all 60,000 to 80,000 elements (about 5 ms a search). New `nav-tree-index.ts` indexes the drawn items once and `set current` touches only the items involved (`nav-tree-index.test.ts`, `nav-tree.test.ts`).
3. **An uncontained 10,000-node tree taxed every frame.** `contain: layout paint style` on the tree's scroller (`fs-app-shell.ts`); not size containment, which would collapse the tree in the stacked layout. Before: Large sort result p95 123.9 ms. After: 63.4 ms.
Also fixed, found later in the work:

- **Only a parent draws a chevron, and only the current row is positioned.** Each leaf lost two elements (the chevron svg and its path) and its own paint layer (`position: relative` moved to `a[aria-current="page"]`). The tree is now about 60,000 elements at the cap, not 80,000 (`nav-tree.test.ts`, `component-styles.test.ts`).
- **The list's column header shrank to 17 px on a long list** (38 px designed). With 11,000 rows the scroller's content is 484,000 px and the flex column took nearly all the header's height back. `.head { flex: none }` in `drive-row-view.ts`, found by the composition spec (`component-styles.test.ts`).
- **The fake file source blocked the fake host for 160 ms per history read** by hex-dumping every byte of a 700 KB image as its digest, so the first preview run measured the harness (227 ms p50 for any image). `digestOf` in `src/files/memory-source.ts` is now the length plus two FNV-1a sums (`memory-history.test.ts`). After: 60 ms p50. This is a correction of the fake, not of the product.

### Finding 4 (open): the Large catalog's open-folder and back miss the result gate

On the Large catalog, opening an unseen folder and going back miss the **warm browse result** gate: open p95 132.9 ms, back p95 123.1 ms against 100 ms (the feedback gates hold: open 99.7 ms, back 99.9 ms, which is within two milliseconds of the line). The cause is the navigation tree: it draws every node of a level up to its 10,000-node cap, and that costs about 50 ms of every frame that changes the current destination (the same run with the tree hidden measures 65 ms p95; the Small catalog, 100 collections, measures about 70 ms). Paint, layout, pre-paint and hit testing scale with the element count. Under 5 navigation events a second the same classes reach p95 248 ms. A virtualised navigation tree is the fix; it is a design change to UI-F2.1a's tree, so it is an **owner decision** (the alternative is a lower per-level cap with a "show more" row). The two Large open-folder tests are marked expected-to-fail (`test.fail`) so the budget stays an assertion and the suite turns red the day it passes.

### Finding 5: one garbage-collection stall while scrolling

Scrolling the Large list for 35 seconds showed one stall of 117 to 167 ms in the earlier runs (a V8 major collection whose phantom-handle finalisation took 156 ms in a trace); the head-commit run's longest frame was exactly 100 ms. Every other frame is inside the limits: p99 16.8 ms, about 0.3% missed. Limit defined by this task: stalls above 100 ms at most 0.1% of the frames (proposed, the owner confirms). Not fixed here; row-element pooling would lower the garbage.

### Gaps recorded, not fixed here

- **Mockup 22.** The preview's top bar (collection, badge, zoom chip) and its filmstrip are built by no task so far; the media frame's radius is the 8 px token, the mockup draws 10 px. The composition spec asserts what is built.
- **The navigation tree cap is 10,000 nodes** (1,000 a page, 50 pages), so the "50,000-album" Large tree shows its first 10,000 and says so.
- **Proposed limits** for the owner to confirm, all in [UI-S-035](../srs.md): N = 120 warm samples after a warm-up of 20 (100 required), 20 cold loads per profile, p99 <= 200 ms, a 5 s sample timeout, missed frame above 25.05 ms, at most 5% missed, frame p99 <= 50 ms, stalls above 100 ms at most 0.1%, a 12-megapixel original p95 <= 2,000 ms warm and 3,000 ms cold, failure state within 2,000 ms.

## Not measured here

The packaged macOS desktop, WKWebView, the Tauri scheme transport, SSD and I/O (DE-F3.2), `filesquid-server` (SV-F3.2), the encrypted browser host and the 4 GB mobile profile (UI-F5.1, UI-1.6), upload throughput and staging, and the 72-hour soak.

## Mocked Environment Variables

None. `FILESQUID_E2E_PREVIEW=1` is set by the Playwright configs to select the in-process host; `FILESQUID_E2E=1` enables the Playwright suites in the hook.

## Test output

### Red first

Failing unit tests before the harness existed (`ui-f3.1-red-unit`):

```text
 Test Files  5 failed (5)
      Tests  7 failed | 18 passed | 1 skipped (26)
 FAIL  tests/unit/perf-budgets.test.ts [ tests/unit/perf-budgets.test.ts ]
 FAIL  tests/unit/perf-fixtures.test.ts [ tests/unit/perf-fixtures.test.ts ]
 FAIL  tests/unit/perf-stats.test.ts [ tests/unit/perf-stats.test.ts ]
 FAIL  tests/unit/test-reporting.test.ts > Allure and JUnit output > writes Playwright Allure results and JUnit from playwright.performance.config.ts
 FAIL  tests/unit/test-reporting.test.ts > clean results before each run > clears only the Playwright results before an end-to-end run
 FAIL  tests/unit/test-reporting.test.ts > production artifact suite > runs the performance suite on the same built artifact, with the budgets as assertions
 FAIL  tests/unit/test-scripts.test.ts > run-e2e.mjs > fails when only the performance suite fails, so a missed budget fails the run
 FAIL  tests/unit/test-scripts.test.ts > run-e2e.mjs > passes extra arguments to every suite
 FAIL  tests/unit/test-scripts.test.ts > run-e2e.mjs > runs the server-backed suite, the preview suite, then the performance suite
 FAIL  tests/unit/test-scripts.test.ts > run-e2e.mjs > still runs the later suites when the server-backed suite fails, and fails
Error: Failed to resolve import "../performance/budgets" from "tests/unit/perf-budgets.test.ts". Does the file exist?
Error: Failed to resolve import "../performance/fixtures" from "tests/unit/perf-fixtures.test.ts". Does the file exist?
Error: Failed to resolve import "../performance/stats" from "tests/unit/perf-stats.test.ts". Does the file exist?
```

The Playwright DOM-bound test red against the unbounded list (Finding 1, `ui-f3.1-red-resources`):

```text
  ✘  1 [chromium] › tests/performance/40-resources.perf.spec.ts:34:3 › medium: the DOM holds a bounded window with every page loaded (1.8m)
  ✘  2 [chromium] › tests/performance/40-resources.perf.spec.ts:34:3 › large: the DOM holds a bounded window with every page loaded (1.8m)

    +   "scroller-height-medium: 484000 is above the limit 900 (UI-S-025: the list scrolls inside the content area (window 900 px))",
    +   "scroller-height-large: 484000 is above the limit 900 (UI-S-025: the list scrolls inside the content area (window 900 px))",
```

Before the tree fixes, the Large catalog against the Small and Medium ones (`ui-f3.1-scaling`):

```text
small: select p50=25.9 p95=26.8 max=27.3 | sort ack p50=25.9 p95=27.1 max=28.1 | sort result p50=42.8 p95=54.4 max=58.9
medium: select p50=19.5 p95=32.8 max=52.5 | sort ack p50=20.2 p95=34.3 max=54.8 | sort result p50=56.8 p95=76.0 max=88.0
large: select p50=40.8 p95=59.1 max=77.1 | sort ack p50=44.0 p95=61.9 max=93.0 | sort result p50=94.5 p95=123.9 max=143.1
```

Containment experiments on the Large tree (`ui-f3.1-tree3`):

```text
baseline: select p50=30.2 p95=45.2 | sort ack p50=42.5 p95=60.0 | sort result p50=84.2 p95=112.5
treeStrict: select p50=28.0 p95=31.1 | sort ack p50=27.3 p95=41.6 | sort result p50=61.7 p95=78.1
treeLayoutPaintStyle: select p50=29.7 p95=32.0 | sort ack p50=28.0 p95=42.8 | sort result p50=61.8 p95=78.4
treePaint: select p50=25.9 p95=32.3 | sort ack p50=27.5 p95=38.4 | sort result p50=61.4 p95=76.8
```

### Static checks (`apps/filesquid/ui`)

```text
$ npx prettier --check . ; npx eslint . ; npx tsc --noEmit
Checking formatting...
All matched files use Prettier code style!
prettier exit 0
eslint exit 0
tsc exit 0
```

### Unit and component tests (`npm test`)

```text
 Test Files  87 passed (87)
      Tests  2099 passed | 1 skipped (2100)
   Duration  38.81s (transform 5.65s, setup 3.29s, import 15.31s, tests 38.37s, environment 44.12s)
```

### Existing preview suite (regression check of the CSS and tree changes)

```text
$ FILESQUID_E2E=1 npx playwright test -c playwright.preview.config.ts
  164 passed (59.8s)
```

The server-backed suite (`playwright.config.ts`) starts `filesquid-server` with Cargo and was not run here; the head-commit hook run covers the UI scope.

### Performance suite (`FILESQUID_E2E=1 npm run test:perf`, the head-commit run)

`✘` marks the two expected-to-fail tests (Finding 4); Playwright counts them as passed.

```text
> @filesquid/ui@0.1.0 test:perf
> playwright test -c playwright.performance.config.ts

Running 30 tests using 1 worker

  ✓   1 [chromium] › tests/performance/00-control.perf.spec.ts:27:1 › the browser build is recorded with the machine (8ms)
  ✓   2 [chromium] › tests/performance/00-control.perf.spec.ts:35:1 › control: an idle page keeps 60 Hz, so the machine can judge the app (10.9s)
  ✓   3 [chromium] › tests/performance/00-control.perf.spec.ts:59:1 › control: the route round trip under every request of the harness (1.2s)
  ✓   4 [chromium] › tests/performance/10-cold-start.perf.spec.ts:29:3 › first useful folder view, small catalog, cold (NFR-002 <= 3 s) (5.0s)
  ✓   5 [chromium] › tests/performance/10-cold-start.perf.spec.ts:29:3 › first useful folder view, medium catalog, cold (NFR-002 <= 3 s) (4.9s)
  ✓   6 [chromium] › tests/performance/10-cold-start.perf.spec.ts:29:3 › first useful folder view, large catalog, cold (NFR-002 <= 3 s) (4.8s)
  ✓   7 [chromium] › tests/performance/10-cold-start.perf.spec.ts:91:1 › a traced cold load of the Large catalog is recorded as evidence (464ms)
  ✓   8 [chromium] › tests/performance/20-browse.perf.spec.ts:70:3 › warm browse, Large catalog › selecting a row and moving with the keyboard (31.3s)
  ✓   9 [chromium] › tests/performance/20-browse.perf.spec.ts:85:3 › warm browse, Large catalog › sorting and switching the layout (36.2s)
  ✘  10 [chromium] › tests/performance/20-browse.perf.spec.ts:104:3 › warm browse, Large catalog › opening folders and going back (cache miss every time) (1.0m)
  ✓  11 [chromium] › tests/performance/20-browse.perf.spec.ts:131:3 › warm browse, Large catalog › narrowing with the sidebar filter (28.6s)
  ✓  12 [chromium] › tests/performance/30-scroll.perf.spec.ts:31:3 › list: frames while scrolling stay inside the 60 Hz limits (35.6s)
  ✓  13 [chromium] › tests/performance/30-scroll.perf.spec.ts:31:3 › grid: frames while scrolling stay inside the 60 Hz limits (35.6s)
  ✓  14 [chromium] › tests/performance/40-resources.perf.spec.ts:34:3 › medium: the DOM holds a bounded window with every page loaded (4.1s)
  ✓  15 [chromium] › tests/performance/40-resources.perf.spec.ts:34:3 › large: the DOM holds a bounded window with every page loaded (4.4s)
  ✓  16 [chromium] › tests/performance/40-resources.perf.spec.ts:95:1 › heap growth after warm-up stays within 10% over 600 mixed interactions (1.6m)
  ✓  17 [chromium] › tests/performance/50-preview.perf.spec.ts:146:1 › a prepared display-size image paints at p95 <= 100 ms, over every image of the folder (30.2s)
  ✓  18 [chromium] › tests/performance/50-preview.perf.spec.ts:172:1 › a 12-megapixel original: first useful equals final, within 2 s warm (31.8s)
  ✓  19 [chromium] › tests/performance/50-preview.perf.spec.ts:200:1 › text reads one ranged request of at most 64 KiB, painted at p95 <= 100 ms (23.8s)
  ✓  20 [chromium] › tests/performance/50-preview.perf.spec.ts:248:3 › cold: the first small preview after load, in a new context (20) (7.3s)
  ✓  21 [chromium] › tests/performance/50-preview.perf.spec.ts:248:3 › cold: the first large preview after load, in a new context (20) (7.2s)
  ✓  22 [chromium] › tests/performance/50-preview.perf.spec.ts:313:1 › content that cannot be read shows the error state within 2 s, never a spinner (4.1s)
  ✓  23 [chromium] › tests/performance/60-search.perf.spec.ts:26:1 › search: query feedback, live files and completed results over 200 queries (4.1m)
  ✓  24 [chromium] › tests/performance/70-load.perf.spec.ts:46:1 › background work: select and open-folder keep their budgets while events stream in (16.6s)
  ✘  25 [chromium] › tests/performance/70-load.perf.spec.ts:87:1 › background work: opening folders under the same stream (1.2m)
  ✓  26 [chromium] › tests/performance/70-load.perf.spec.ts:127:1 › resource pressure: select, sort and open-folder at 4x CPU slowdown are reported (48.5s)
  ✓  27 [chromium] › tests/performance/80-composition.perf.spec.ts:44:5 › light theme › the shell and the list of mockup 05 hold with the Large catalog loaded (1.7s)
  ✓  28 [chromium] › tests/performance/80-composition.perf.spec.ts:119:5 › light theme › the preview with its file panel (mockups 22 and 35) as built, over generated images (402ms)
  ✓  29 [chromium] › tests/performance/80-composition.perf.spec.ts:44:5 › dark theme › the shell and the list of mockup 05 hold with the Large catalog loaded (1.7s)
  ✓  30 [chromium] › tests/performance/80-composition.perf.spec.ts:119:5 › dark theme › the preview with its file panel (mockups 22 and 35) as built, over generated images (501ms)

Performance summary (milliseconds; frame rows are frame intervals)

{
  "recorded": "2026-10-30T19:20:00.693Z",
  "host": "linux 6.18.44-fc-v185",
  "cpu": "Intel(R) Xeon(R) Processor @ 2.10GHz",
  "cores": 4,
  "memoryGiB": 15.7,
  "freeMemoryGiB": 14.6,
  "loadBefore": [1.35, 0.75, 0.53],
  "node": "v22.22.0",
  "note": "Headless Chromium in a container, loopback, fake host with no modelled latency; not the packaged macOS app.",
  "browser": "Chromium 156.0.8078.4 (headless, Playwright)",
  "loadAfter": [1.79, 1.51, 1.03]
}

| measurement | n | warm-up | p50 | p90 | p95 | p99 | max | budget | verdict |
|---|---|---|---|---|---|---|---|---|---|
| background-open-folder-feedback-large | 120 | 20 | 48.1 | 99.8 | 113.4 | 135.3 | 174.1 | NFR-002 p95 <= 100 ms; p99 <= 200 ms (same gates as idle) | FAIL |
| background-open-folder-result-large | 120 | 20 | 83.6 | 206.5 | 247.6 | 307.5 | 322.9 | NFR-002 p95 <= 100 ms; p99 <= 200 ms (same gates as idle) | FAIL |
| background-select-feedback-large | 120 | 20 | 26.5 | 31.1 | 32 | 32.4 | 32.5 | NFR-002 p95 <= 100 ms; p99 <= 200 ms (same gates as idle) | pass |
| cold-first-useful-large | 20 | 0 | 147.7 | 162.6 | 178.5 | 192.4 | 192.4 | NFR-002 first useful local viewport <= 3 s | pass |
| cold-first-useful-medium | 20 | 0 | 158.2 | 169.4 | 179.4 | 182.9 | 182.9 | NFR-002 first useful local viewport <= 3 s | pass |
| cold-first-useful-small | 20 | 0 | 150.7 | 194.1 | 195.3 | 197.5 | 197.5 | NFR-002 first useful local viewport <= 3 s | pass |
| content-height-large | 1 | 0 |  |  |  |  |  | UI-S-026: rows are 44 px tall; 11,000 rows (the footer counts them all) | pass |
| content-height-medium | 1 | 0 |  |  |  |  |  | UI-S-026: rows are 44 px tall; 11,000 rows (the footer counts them all) | pass |
| control-idle-frames | 594 | 5 | 16.7 |  | 16.7 | 16.8 | 33.3 | 60 Hz, frame budget 16.7 ms (NFR-002); missed > 25.05 ms | pass |
| control-route-round-trip | 200 | 20 | 3.6 | 5.5 | 6.2 | 7.4 | 8.7 | reported; the floor under every request in this harness | reported |
| dom-rows-large | 1 | 0 |  |  |  |  |  | NFR-003 bounded queues and caches: window rows + 2 x overscan of 5 | pass |
| dom-rows-medium | 1 | 0 |  |  |  |  |  | NFR-003 bounded queues and caches: window rows + 2 x overscan of 5 | pass |
| heap-growth-mixed | 1 | 0 |  |  |  |  |  | NFR-003 post-warm-up non-cache memory growth <= 10% (the 72-hour soak is not run here) | pass |
| pressure-select-feedback-large | 100 | 20 | 27.3 | 31.8 | 35.5 | 43 | 57.9 | reported at 4x CPU slowdown; budgets are for the reference machine | reported |
| pressure-sort-feedback-large | 100 | 20 | 80.5 | 107.8 | 110.7 | 129.5 | 150.8 | reported at 4x CPU slowdown; budgets are for the reference machine | reported |
| pressure-sort-result-large | 100 | 20 | 156.1 | 188.6 | 197.5 | 222.6 | 300.7 | reported at 4x CPU slowdown; budgets are for the reference machine | reported |
| preview-cold-large | 20 | 0 | 76.7 | 91.9 | 94.1 | 110.6 | 110.6 | UI-S-035 12-megapixel original cold <= 3,000 ms | pass |
| preview-cold-small | 20 | 0 | 76.1 | 77 | 77.4 | 77.5 | 77.5 | reported: cold behaviour is reported apart from the warm p95 (NFR-002) | reported |
| preview-failure-visible | 20 | 0 | 59.9 | 61 | 61.1 | 77.6 | 77.6 | UI-S-035 failure visible <= 2,000 ms (never a spinner) | pass |
| preview-large-ack | 120 | 20 | 27.1 | 28.1 | 28.3 | 28.5 | 28.7 | NFR-002 acknowledgment p95 <= 100 ms | pass |
| preview-large-image | 120 | 20 | 60.9 | 76.8 | 78 | 91.8 | 91.9 | UI-S-035 12-megapixel original warm p95 <= 2,000 ms | pass |
| preview-small-ack | 120 | 20 | 26.8 | 27.9 | 28.1 | 28.6 | 41.6 | NFR-002 acknowledgment p95 <= 100 ms | pass |
| preview-small-image | 120 | 20 | 60.4 | 74.5 | 77.7 | 120.5 | 125 | NFR-002 prepared viewer image paints p95 <= 100 ms | pass |
| preview-text-ack | 120 | 20 | 27.4 | 28 | 28.2 | 28.6 | 127.6 | NFR-002 acknowledgment p95 <= 100 ms | pass |
| preview-text-image | 120 | 20 | 44.1 | 44.8 | 45 | 61.3 | 144.3 | NFR-002 prepared view paints p95 <= 100 ms | pass |
| scroll-dom-rows-grid | 1 | 0 |  |  |  |  |  | NFR-003 bounded window: visible + 2 x overscan of 5 | pass |
| scroll-dom-rows-list | 1 | 0 |  |  |  |  |  | NFR-003 bounded window: visible + 2 x overscan of 5 | pass |
| scroll-frames-grid | 1896 | 5 | 16.7 |  | 16.7 | 16.8 | 83.3 | 60 Hz, frame budget 16.7 ms (NFR-002); missed > 25.05 ms | pass |
| scroll-frames-list | 1892 | 5 | 16.7 |  | 16.7 | 16.8 | 100 | 60 Hz, frame budget 16.7 ms (NFR-002); missed > 25.05 ms | pass |
| scroller-height-large | 1 | 0 |  |  |  |  |  | UI-S-025: the list scrolls inside the content area (window 900 px) | pass |
| scroller-height-medium | 1 | 0 |  |  |  |  |  | UI-S-025: the list scrolls inside the content area (window 900 px) | pass |
| search-completed-results | 120 | 20 | 94.5 | 110.5 | 126.3 | 140.6 | 209.5 | NFR-002 indexed metadata search p95 <= 2 s | pass |
| search-live-files | 120 | 20 | 300.7 | 302 | 302.2 | 318.1 | 318.7 | NFR-002 indexed metadata search p95 <= 2 s | pass |
| search-query-feedback | 120 | 20 | 18.2 | 19.4 | 29.3 | 31 | 31 | NFR-002 acknowledgment p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-back-feedback-large | 120 | 20 | 83 | 98.6 | 99.9 | 115.2 | 116.6 | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-back-result-large | 120 | 20 | 100.3 | 117.9 | 123.1 | 138.7 | 149.9 | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | FAIL |
| warm-filter-feedback-large | 120 | 20 | 55 | 63.4 | 76.8 | 96.4 | 161.3 | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-filter-result-large | 120 | 20 | 55 | 63.4 | 76.8 | 96.4 | 161.3 | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-keyboard-feedback-large | 120 | 20 | 18.8 | 31 | 32.2 | 32.9 | 35.6 | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-layout-feedback-large | 120 | 20 | 31.2 | 32.7 | 32.9 | 48.3 | 64.8 | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-layout-result-large | 120 | 20 | 31.2 | 32.7 | 32.9 | 48.3 | 64.8 | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-open-folder-feedback-large | 120 | 20 | 77.5 | 96.2 | 99.7 | 103.6 | 114 | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-open-folder-result-large | 120 | 20 | 101 | 126.7 | 132.9 | 158.4 | 163.6 | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | FAIL |
| warm-select-feedback-large | 120 | 20 | 30.1 | 32.3 | 32.6 | 33.1 | 49.9 | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-sort-feedback-large | 120 | 20 | 30.6 | 32.3 | 32.6 | 32.9 | 32.9 | NFR-002 warm interaction-to-visible-feedback p95 <= 100 ms; p99 <= 200 ms | pass |
| warm-sort-result-large | 120 | 20 | 48.6 | 55.1 | 63.4 | 82.5 | 122.7 | FS-3.4 warm browse p95 <= 100 ms; p99 <= 200 ms | pass |

  30 passed (14.4m)
```

The four `FAIL` rows are the open-folder and back classes of Finding 4 (idle and under events); their tests are expected to fail and did. The `warm-filter` and `warm-layout` rows repeat one sample for feedback and result because the control and the items appear in the same frame.

### Pre-commit hook (`bash scripts/pre-commit.sh`)

```text
✅ Vitest passed (46s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui).
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (84s)
```

## Sync Execution Report

Cascaded documents:

- `apps/filesquid/ui/resources/srs.md`: new [UI-S-035](../srs.md) (the budgets, profiles, defined limits, the known gap, what is not measured, verification).
- `resources/srs.md`: the SRS-NFR-002 tests list names the new suite and feature.
- `apps/filesquid/ui/resources/plan.md`: the UI-F3.1 row's Related Tests and Status; box ticked.
- `resources/implementation-plan-checklist.md`: the UI-F3.1 box ticked (global FS-3.4 left open).
- `README.md`: the Run tests section (`test:perf`, the third `test:e2e` suite, the 15-minute run time, the known gap).
- `apps/filesquid/ui/tests/features/production_build_browsing_performance_NFR-002.feature`: written with the feature-writer skill.

No conflict between the PRD, SRS, ADRs and plan needed a Conflict Artifact.
