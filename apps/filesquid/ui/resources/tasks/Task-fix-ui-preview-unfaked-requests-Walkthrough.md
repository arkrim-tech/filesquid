# Task fix-ui-preview-unfaked-requests Walkthrough

## UI-S-013: Playwright proofs against the production build with strict DOM assertions

A fix, not a plan row; no plan box is ticked. Requirements: [UI-S-013](../srs.md) (strict DOM, no console errors), [UI-S-035](../srs.md) (production-build browsing performance) and [UI-S-023](../srs.md) (client event delivery, the route both fakes now answer). Only test code changes; no product code, no budget, no strict-DOM excused line.

## Causes

Each failure was a request from the built page that reached a fake host, or the `vite preview` proxy, with no route behind it. Diagnostic runs (throwaway copies of the specs that logged every response of 400 or more and every console line, deleted afterwards) named each one.

1. **Performance suite, every page:** the Jobs button reads `GET /api/v1/jobs?kind=import&limit=100` at start (UI-1.2). The fake host of the performance suite ([`host.ts`](../../tests/performance/host.ts), UI-F3.1) predates that read, so its catch-all answered 404. The browser logs that 404 as a console error, and the page forwards its own warning ("FileSquid could not read the upload jobs") to `POST /api/v1/client-events` (UI-F2.16), which the catch-all also refused with 404.
2. **Performance suite, Large catalogue:** the Large catalogue's levels are longer than the 50 pages the view loads, so the view warns by design ("FileSquid showed the first 50 pages of a navigation level only"). The page forwards that warning to `POST /api/v1/client-events`, which got the catch-all's 404.
3. **Preview suite, "View in Logs opens…"** ([`provenance-panel.spec.ts`](../../tests/e2e/preview/provenance-panel.spec.ts)): the click opens the Logs view (UI-F2.16), which reads `GET /api/v1/admin/journal?order=desc&names=true&limit=50&file=<id>`, the oldest entry and `GET /api/v1/libraries/recent`. The spec fakes none of these, so the preview proxy answers 502. The test ended right after its URL check, so it failed only when the 502 line arrived first. Adding a 2-second wait before the final assertion made it fail 3 of 3 with `RESP 502 …/api/v1/admin/journal?order=desc&names=true&limit=50&file=7f3c9a1e-…`.

## Changes

- [`host.ts`](../../tests/performance/host.ts): the shell routes answer `GET /api/v1/jobs` with an empty `JobPage` and acknowledge `POST /api/v1/client-events` as the host does (`acknowledged`, `journaled`, `principal`), like the preview suite's Logs fake. The catch-all still answers 404 to anything else and records it.
- [`provenance-panel.spec.ts`](../../tests/e2e/preview/provenance-panel.spec.ts): the test answers the Logs view's journal reads with an empty page and its library read with an empty list before it clicks. It now also asserts that the Logs view settles in its empty state and that its first journal read carries `file=<the file's ID>`.

TDD-ENFORCE: Not applicable - only test fakes change, and the failing tests are the proof (each failure was reproduced before the change and passes after it).

## Before

`00-control.perf.spec.ts` on the branch from `origin/main`, before the change: 1 passed, 2 failed. Excerpt (the raw file was lost when the machine restarted):

```text
Running 3 tests using 1 worker
  ✓  1 [chromium] › tests/performance/00-control.perf.spec.ts:27:1 › the browser build is recorded with the machine (15ms)
  1) [chromium] › tests/performance/00-control.perf.spec.ts:35:1 › control: an idle page keeps 60 Hz, so the machine can judge the app
    + Received  + 8
    +   "Failed to load resource: the server responded with a status of 404 (Not Found)",   (6 lines)
    > 56 |   expect(errors).toEqual([]);
  2) [chromium] › tests/performance/00-control.perf.spec.ts:59:1 › control: the route round trip under every request of the harness
    +   "Failed to load resource: the server responded with a status of 404 (Not Found)",   (4 lines)
    > 82 |   expect(session.errors).toEqual([]);
  2 failed
  1 passed (23.7s)
```

After the jobs fix alone, the whole performance suite still failed 13 of 30, all Large-catalogue tests, with 60 of the same 404 lines (cause 2).

The provenance test on its own passed 24 of 24 (8 + 16 repeats, then 24 more on the merged head) and did not fail in isolation:

```text
  24 passed (31.1s)
```

It failed in the whole preview suite run on the merged head, before the spec change:

```text
  1) [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation 
    Error: expect(received).toEqual(expected) // deep equality
    - Expected  - 1
    + Received  + 3
    - Array []
    + Array [
    +   "Failed to load resource: the server responded with a status of 502 (Bad Gateway)",
    + ]
    > 583 |   expect(errors).toEqual([]);
          |                  ^
      584 | });
      585 |
      586 | test("JSON-LD saves the file's history and announces it; a failure is an alert", async ({
        at tests/e2e/preview/provenance-panel.spec.ts:583:18
    Error Context: tests/test-results/provenance-panel-View-in-L-20625-he-file-and-to-an-operation-chromium/error-context.md
  1 failed
    [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation 
  288 passed (1.7m)
  1 failed
  288 passed (1.7m)
```

## After

### The provenance test, `--repeat-each=24`, one worker

```text
  ✓   1 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (489ms)
  ✓   2 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (533ms)
  ✓   3 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (512ms)
  ✓   4 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (468ms)
  ✓   5 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (503ms)
  ✓   6 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (516ms)
  ✓   7 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (530ms)
  ✓   8 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (555ms)
  ✓   9 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (492ms)
  ✓  10 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (494ms)
  ✓  11 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (483ms)
  ✓  12 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (488ms)
  ✓  13 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (511ms)
  ✓  14 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (508ms)
  ✓  15 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (489ms)
  ✓  16 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (474ms)
  ✓  17 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (477ms)
  ✓  18 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (511ms)
  ✓  19 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (460ms)
  ✓  20 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (550ms)
  ✓  21 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (504ms)
  ✓  22 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (542ms)
  ✓  23 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (501ms)
  ✓  24 [chromium] › tests/e2e/preview/provenance-panel.spec.ts:568:1 › View in Logs opens the Logs address filtered to the file and to an operation (501ms)
  24 passed (31.4s)
```

With the 2-second wait before the final assertion that failed 3 of 3 before the change, the same throwaway copy passes 3 of 3.

### The whole preview suite (`npx playwright test -c playwright.preview.config.ts`)

```text
  ✓  288 [chromium] › tests/e2e/preview/versions-panel.spec.ts:325:5 › dark theme › Download this version: a refused download shows a visible alert and saves nothing (415ms)
  ✓  289 [chromium] › tests/e2e/preview/versions-panel.spec.ts:356:5 › dark theme › File info: the history size row follows the size, from the history-size route (263ms)
  289 passed (1.5m)
```

### The whole performance suite (`npm run test:perf`)

```text
  ✓   1 [chromium] › tests/performance/00-control.perf.spec.ts:27:1 › the browser build is recorded with the machine (5ms)
  ✓   2 [chromium] › tests/performance/00-control.perf.spec.ts:35:1 › control: an idle page keeps 60 Hz, so the machine can judge the app (12.0s)
  ✓   3 [chromium] › tests/performance/00-control.perf.spec.ts:59:1 › control: the route round trip under every request of the harness (871ms)
  ✓   4 [chromium] › tests/performance/10-cold-start.perf.spec.ts:29:3 › first useful folder view, small catalog, cold (NFR-002 <= 3 s) (3.3s)
  ✓   5 [chromium] › tests/performance/10-cold-start.perf.spec.ts:29:3 › first useful folder view, medium catalog, cold (NFR-002 <= 3 s) (3.3s)
  ✓   6 [chromium] › tests/performance/10-cold-start.perf.spec.ts:29:3 › first useful folder view, large catalog, cold (NFR-002 <= 3 s) (3.2s)
  ✓   7 [chromium] › tests/performance/10-cold-start.perf.spec.ts:91:1 › a traced cold load of the Large catalog is recorded as evidence (395ms)
  ✓   8 [chromium] › tests/performance/20-browse.perf.spec.ts:70:3 › warm browse, Large catalog › selecting a row and moving with the keyboard (28.8s)
  ✓   9 [chromium] › tests/performance/20-browse.perf.spec.ts:85:3 › warm browse, Large catalog › sorting and switching the layout (32.2s)
  ✓  10 [chromium] › tests/performance/20-browse.perf.spec.ts:104:3 › warm browse, Large catalog › opening folders and going back (cache miss every time) (48.6s)
  ✓  11 [chromium] › tests/performance/20-browse.perf.spec.ts:133:3 › warm browse, Large catalog › narrowing with the sidebar filter (24.1s)
  ✓  12 [chromium] › tests/performance/30-scroll.perf.spec.ts:31:3 › list: frames while scrolling stay inside the 60 Hz limits (35.5s)
  ✓  13 [chromium] › tests/performance/30-scroll.perf.spec.ts:31:3 › grid: frames while scrolling stay inside the 60 Hz limits (35.4s)
  ✓  14 [chromium] › tests/performance/40-resources.perf.spec.ts:34:3 › medium: the DOM holds a bounded window with every page loaded (3.8s)
  ✓  15 [chromium] › tests/performance/40-resources.perf.spec.ts:34:3 › large: the DOM holds a bounded window with every page loaded (4.1s)
  ✓  16 [chromium] › tests/performance/40-resources.perf.spec.ts:102:1 › heap growth after warm-up stays within 10% over 600 mixed interactions (1.5m)
  ✓  17 [chromium] › tests/performance/50-preview.perf.spec.ts:149:1 › a prepared display-size image paints at p95 <= 100 ms, over every image of the folder (25.3s)
  ✓  18 [chromium] › tests/performance/50-preview.perf.spec.ts:177:1 › a 12-megapixel original: first useful equals final, within 2 s warm (26.5s)
  ✓  19 [chromium] › tests/performance/50-preview.perf.spec.ts:205:1 › text reads one ranged request of at most 64 KiB, painted at p95 <= 100 ms (21.6s)
  ✓  20 [chromium] › tests/performance/50-preview.perf.spec.ts:255:3 › cold: the first small preview after load, in a new context (20) (6.1s)
  ✓  21 [chromium] › tests/performance/50-preview.perf.spec.ts:255:3 › cold: the first large preview after load, in a new context (20) (5.3s)
  ✓  22 [chromium] › tests/performance/50-preview.perf.spec.ts:320:1 › content that cannot be read shows the error state within 2 s, never a spinner (3.3s)
  ✓  23 [chromium] › tests/performance/60-search.perf.spec.ts:26:1 › search: query feedback, live files and completed results over 120 measured queries (3.0m)
  ✘  24 [chromium] › tests/performance/70-load.perf.spec.ts:46:1 › background work: select and open-folder keep their budgets while events stream in (20.8s)
  ✘  25 [chromium] › tests/performance/70-load.perf.spec.ts:87:1 › background work: opening folders under the same stream (2.6m)
  ✓  26 [chromium] › tests/performance/70-load.perf.spec.ts:128:1 › resource pressure: select, sort and open-folder at 4x CPU slowdown are reported (2.3m)
  ✓  27 [chromium] › tests/performance/80-composition.perf.spec.ts:43:5 › light theme › the shell and the list of mockup 05 hold with the Large catalog loaded (1.1s)
  ✓  28 [chromium] › tests/performance/80-composition.perf.spec.ts:118:5 › light theme › the preview with its file panel (mockups 22 and 35) as built, over generated images (2.6s)
  ✓  29 [chromium] › tests/performance/80-composition.perf.spec.ts:43:5 › dark theme › the shell and the list of mockup 05 hold with the Large catalog loaded (1.0s)
  ✓  30 [chromium] › tests/performance/80-composition.perf.spec.ts:118:5 › dark theme › the preview with its file panel (mockups 22 and 35) as built, over generated images (302ms)
  2 failed
  28 passed (15.7m)
```

No test fails on a console line any more. The two remaining failures are not caused by unfaked requests:

```text
  1) [chromium] › tests/performance/70-load.perf.spec.ts:46:1 › background work: select and open-folder keep their budgets while events stream in 
    Error: sample 22 never reached its predicate within 5000 ms
       at harness.ts:153
    > 153 |       throw new Error(`sample ${i} never reached its predicate within ${spec.timeoutMs} ms`);
        at runSamples (tests/performance/harness.ts:153:13)
        at selectClass (tests/performance/browse-classes.ts:86:18)
        at tests/performance/70-load.perf.spec.ts:67:20
    Error Context: tests/test-results/70-load.perf-background-wo-f2de6-gets-while-events-stream-in-chromium/error-context.md
  2) [chromium] › tests/performance/70-load.perf.spec.ts:87:1 › background work: opening folders under the same stream 
    Error: sample 28 never reached its predicate within 20000 ms
       at harness.ts:153
    > 153 |       throw new Error(`sample ${i} never reached its predicate within ${spec.timeoutMs} ms`);
        at runSamples (tests/performance/harness.ts:153:13)
        at folderClass (tests/performance/browse-classes.ts:250:18)
        at tests/performance/70-load.perf.spec.ts:97:21
    Error Context: tests/test-results/70-load.perf-background-wo-554b8-lders-under-the-same-stream-chromium/error-context.md
  2 failed
    [chromium] › tests/performance/70-load.perf.spec.ts:46:1 › background work: select and open-folder keep their budgets while events stream in 
    [chromium] › tests/performance/70-load.perf.spec.ts:87:1 › background work: opening folders under the same stream 
  28 passed (15.7m)
```

A diagnostic run of the first one counted about 3,000 `GET /api/v1/navigation` reads in its first 15 seconds. Each `navigation.change` event (5 a second) makes the tree re-read the Large level, up to 50 pages. The navigation tree's cost on the Large catalogue is Task UI-F3.4's area (UI-S-035), so this fix leaves it alone.

## Suites run and not run

- Run: the preview suite (`playwright.preview.config.ts`) and the performance suite (`playwright.performance.config.ts`), one run at a time under the shared Playwright lock. The pre-commit hook runs Vitest, lint and typecheck.
- Not run: the server-backed suite (`playwright.config.ts`), which needs a `filesquid-server` build that this machine does not make.
