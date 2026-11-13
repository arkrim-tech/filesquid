# Task UI-F2.19 Walkthrough: Preview top bar and zoom chip

Global Task FS-2.5. Branch `feat/ui-f2.19-preview-top-bar`. Row: [plan.md](../plan.md) Task UI-F2.19. Owner decision 2026-10-30, verbatim: "Okay implement all these recommendations" (recommendation 3: build the preview's top bar and zoom chip of mockup 22 in the MVP, defer its filmstrip, keep the media frame on the 8 px `--fs-radius` token).

## SRS mapping

| SRS requirement | What this task covers | Tests |
|---|---|---|
| UI-S-031 Preview and file panel composition | The preview's bar (back, collection, badge, display name, "{position} of {total}"), previous and next at the media's sides, the media fitted in the 8 px `--fs-radius` frame on `--fs-surface-raised`, the zoom chip "Fit · {percent}%" with "Loading full size" and its progress bar; no filmstrip | `tests/ui/file-browser.test.ts`, `tests/ui/file-preview.test.ts`, `tests/e2e/preview/preview-top-bar.spec.ts` |
| UI-S-024 Visual constants of the active mockup suite | Token colours in light and dark, 13 px and 14 px 600 type, 1 px `--fs-outline` lines, 16 px and 20 px line icons, the badge's 10 px 700 mono on its class colour | `tests/e2e/preview/preview-top-bar.spec.ts` (computed styles and boxes within 2 px), `tests/unit/component-styles.test.ts` |
| UI-S-019 Display names | The bar shows the API's display name; the collection and the badge are set as text | `tests/ui/file-browser.test.ts` |
| UI-S-005 Label tables | `fileZoomFit` and `fileLoadingFullSize` in the default and ImageSquid tables | `tests/unit/distribution-tables.test.ts` |
| UI-S-035 Production-build browsing performance | Prepared-image p95 79.3 ms (budget 100 ms), composition boxes of the bar and chip | `tests/performance/50-preview.perf.spec.ts`, `tests/performance/80-composition.perf.spec.ts` |

PRD: [F-003](../../../../../resources/prd.md#f-003-browsing-selection-and-viewing), [F-055](../../../../../resources/prd.md#f-055-default-ui-and-panel-composition), [NFR-002](../../../../../resources/prd.md#nfr-002-benchmark-envelope-and-responsiveness). Gherkin: [`preview_top_bar_and_zoom_chip_UI-S-031.feature`](../../tests/features/preview_top_bar_and_zoom_chip_UI-S-031.feature) (checked with the feature-writer skill; one scenario added for Back).

## What changed

- `src/components/file-browser-frame.ts`: the UI-F2.2 viewer header becomes the one top bar (back as a 16 px chevron named `fileBackToList`, the collection, the badge, `h2.viewer-title`, `.position`), and the previous and next text buttons become 40 px round icon controls in a stage beside the preview. While a file is shown the browser carries `viewing`, fills `main` edge to edge and hides the folder's trail, toolbar and name form, because mockup 22 draws the bar straight under the header.
- `src/components/fs-file-browser.ts`: sets `viewing`, the collection (from `location.collection_name`, left out when absent) and the badge (UI-S-026 class from the display name's extension).
- `src/files/model.ts`: optional `FileLocation.collection_name`, the collection's name as the API gave it to the route that placed the browser.
- `src/files/zoom.ts`: `fitPercent(shown, natural)`.
- `src/files/preview.ts`: the built-in image renderer adds the zoom chip: "Loading full size" and an indeterminate progress bar (an `<img>` reports no byte progress) while the image loads, "Fit · {percent}%" once it has decoded, kept in step with the shown width by a `ResizeObserver`, hidden when the image fails.
- `src/components/fs-file-preview.ts`: the preview fills its area as a grid of the media box, a 20 px gap and the 36 px chip row; the media keeps `--fs-radius` and `--fs-surface-raised`.
- Labels: `fileZoomFit` "Fit · {percent}%", `fileLoadingFullSize` "Loading full size" in `apps/filesquid/ui/src/theme/labels.en.json`, `apps/imagesquid/src/theme/labels.en.json` and `labels.ts`.
- `tests/performance/80-composition.perf.spec.ts`: the preview composition now asserts the bar and the chip; its old note about a 10 px frame is replaced.

## Mockup measurements relied on

From `resources/ui-mockups/v3/22-preview-image.svg` and `22-preview-image-dark.svg` (the webview has no 28 px title bar, so mockup y `n` is `n - 28`; the file panel is open, so the content area is 880 px):

| Element | Mockup value | SVG source | Asserted in the browser |
|---|---|---|---|
| Bar | x 240, y 89, 880 x 52, line at y 141 #E0E0E0 / #2A2A2A | `<rect x="240" y="89" width="880" height="52">`, `<line x1="240" y1="141" …>` | box (240, 61, 880, 52); 1 px `--fs-outline` bottom |
| Back icon | 16 px at x 256 | `translate(256 107) scale(0.666667)` | icon box x 256, 16 x 16 |
| Collection | x 280, 13 px 400, #555555 / #A0A0A0 | `<text x="280" y="120" …>Harbour project` | x 280, 13 px 400 `--fs-on-surface-variant` |
| Badge | 30 x 16, rx 3, #007AF9, 10 px 700 mono | `<rect x="400" y="107" width="30" height="16" rx="3">` | height 16, radius 3 px, media colours |
| Name | 14 px 600, #000000 / #F5F5F5 | `<text x="438" y="120" … font-weight="600">` | 14 px 600 `--fs-on-surface-heading` |
| Position | 13 px, #777777 / #6A6A6A | `<text x="562.453" y="120" …>3 of 11` | 13 px `--fs-on-surface-muted` |
| Previous, next | r 20 at (284, 391) and (1076, 391) | `<circle cx="284" cy="391" r="20">`, `<circle cx="1076" …>` | boxes (264, 343) and (1056, 343), 40 x 40 |
| Media frame | x 360, y 178, 640 x 426, rx 8 | `<clipPath id="fs-media-frame"><rect … rx="8"/>` | box (360, 150, 640, 426), radius 8 px |
| Backdrop | #F5F5F5 / #1C1C1C | `<rect x="240" y="89" width="880" height="811">` | `--fs-surface-container` |
| Chip | x 550, y 624, h 36, rx 18 | `<rect x="550" y="624" width="260" height="36" rx="18">` | y 596, height 36, centred under the media |
| Fit text | 13 px 500 #111111 / #E0E0E0 | `<text x="620" y="647" … font-weight="500">Fit · 38%` | 13 px 500 `--fs-on-surface` |
| Loading text | 13 px 400 #777777 / #6A6A6A | `<text x="746" y="647" …>Loading full size` | 13 px 400 `--fs-on-surface-muted` |
| Progress | 92 x 3, track #E0E0E0, fill #16AC49 | `<rect x="700" y="652" width="92" height="3" …>` | 92 x 3, `--fs-outline` track, `--fs-primary` fill |
| Filmstrip | 104 px band from y 796 | `<rect x="240" y="796" width="880" height="104">` | not built; its room is left empty |

Where an owner decision was applied over a measurement: the media frame. Both SVGs now draw it with `rx="8"` (redrawn 2026-10-31), the 8 px `--fs-radius` the plan row keeps (owner decisions 2026-10-30 and 2026-10-31). The 4 px corners in the SVG are the deferred filmstrip's thumbnails; the earlier 10 px reading did not match the SVG. The chip's 18 px radius is drawn with `--fs-radius-pill` (fully rounded, so 18 px on a 36 px chip, UI-S-024 "filter chips fully rounded"); its computed value is the token's.

The three disagreements between mockup 22 and the plan row are in [Task-UI-F2.19-Conflict.md](./Task-UI-F2.19-Conflict.md): the dark fill of the bar, chip and round controls (`#141414` drawn, `--fs-surface-card` `#1c1c1c` built), the zoom-out and zoom-in icons of the chip (not built: no zooming beyond Fit), and the collection, which no route supplies yet (built as `FileLocation.collection_name`; the bar leaves it out until a route sets it).

## Not built here

- The filmstrip (deferred, owner decision 2026-10-30) and zooming beyond Fit.
- The bar's Download, Show in Finder and panel buttons that mockup 22 draws at its right end: UI-F2.18's walkthrough assigns them to UI-F2.17, UI-F2.8 and UI-F2.15.
- A producer of `collection_name`: Conflict 3.

## How it was tested

- Vitest (`npx vitest run` in apps/filesquid/ui): red run first (13 new tests failing), then the whole suite green.
- Playwright preview suite (`-c playwright.preview.config.ts`, `vite build` plus `vite preview`, under the shared lock): the new spec, then the whole suite (283 passed).
- Not run: the server-backed Playwright suite (`playwright.config.ts`), which needs a `filesquid-server` build; this machine builds no Rust. The ImageSquid suites (not required during the MVP).
- `npm run test:perf` (under the lock): every budget this task touches passes (prepared image p95 79.3 ms, the error state at p95 63.7 ms, the composition boxes of the bar and chip), but 28 of 30 tests fail on `expect(errors).toEqual([])` because the page logs "Failed to load resource: the server responded with a status of 404 (Not Found)". The same failure happens on an unmodified `origin/main` export (`00-control.perf.spec.ts`, which loads no FileSquid view), so it predates this branch. Raw output below.
- No coverage tool is configured for `@filesquid/ui` (`vitest.config.ts` has none); the SRS mapping above lists the tests per requirement.

## Raw test output

### Red run (new tests before the implementation)

```
 FAIL  tests/unit/zoom.test.ts [ tests/unit/zoom.test.ts ]
⎯⎯⎯⎯⎯⎯ Failed Tests 13 ⎯⎯⎯⎯⎯⎯⎯
 FAIL  tests/ui/file-browser.test.ts > <fs-file-browser> preview top bar (UI-S-031, Task UI-F2.19) > completes the viewer header as the one bar: back, badge, display name and position
 FAIL  tests/ui/file-browser.test.ts > <fs-file-browser> preview top bar (UI-S-031, Task UI-F2.19) > gives the badge the class colours of UI-S-026, and none to a name without an extension
 FAIL  tests/ui/file-browser.test.ts > <fs-file-browser> preview top bar (UI-S-031, Task UI-F2.19) > leaves the collection out when the location names none, and never guesses one
 FAIL  tests/ui/file-browser.test.ts > <fs-file-browser> preview top bar (UI-S-031, Task UI-F2.19) > names the collection the location gives, as data
 FAIL  tests/ui/file-browser.test.ts > <fs-file-browser> preview top bar (UI-S-031, Task UI-F2.19) > puts previous and next as round icon controls at the media's sides, named from the label table
 FAIL  tests/ui/file-browser.test.ts > <fs-file-browser> preview top bar (UI-S-031, Task UI-F2.19) > draws no filmstrip and no zoom control beyond Fit
 FAIL  tests/ui/file-browser.test.ts > <fs-file-browser> preview top bar (UI-S-031, Task UI-F2.19) > fills the content area while a file is shown and hides the folder's trail and toolbar
 FAIL  tests/ui/file-browser.test.ts > <fs-file-browser> preview top bar (UI-S-031, Task UI-F2.19) > styles the bar from tokens: 52 px card bar, 1 px outline line, the type of UI-S-024
 FAIL  tests/ui/file-preview.test.ts > <fs-file-preview> zoom chip (UI-S-031, Task UI-F2.19) > says it is loading the full size, with a progress bar, until the image has loaded
 FAIL  tests/ui/file-preview.test.ts > <fs-file-preview> zoom chip (UI-S-031, Task UI-F2.19) > states the fitted scale once the image has loaded, and drops the loading line
 FAIL  tests/ui/file-preview.test.ts > <fs-file-preview> zoom chip (UI-S-031, Task UI-F2.19) > follows the shown width when the frame is resized
 FAIL  tests/ui/file-preview.test.ts > <fs-file-preview> zoom chip (UI-S-031, Task UI-F2.19) > hides the chip when the image cannot be shown
 FAIL  tests/ui/file-preview.test.ts > <fs-file-preview> zoom chip (UI-S-031, Task UI-F2.19) > keeps the media frame on the 8 px --fs-radius and --fs-surface-raised and draws the chip from tokens
 Test Files  3 failed (3)
      Tests  13 failed | 54 passed (67)
```

### Vitest (apps/filesquid/ui), green

```
 ✓ tests/ui/file-browser.test.ts (45 tests) 4803ms
 ✓ tests/unit/component-styles.test.ts (435 tests) 162ms
 ✓ tests/ui/file-preview.test.ts (22 tests) 284ms
 ✓ tests/unit/distribution-tables.test.ts (7 tests) 17ms
 ✓ tests/unit/zoom.test.ts (3 tests) 10ms
 Test Files  123 passed (123)
      Tests  2835 passed | 1 skipped (2836)
   Duration  78.68s (transform 6.24s, setup 5.54s, import 27.09s, tests 83.25s, environment 86.72s)
```

### Playwright preview suite, the new spec

```
  ✓   1 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:164:5 › light theme › the top bar names the collection, the badge, the file and its place (22, UI-S-031) (738ms)
  ✓   2 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:216:5 › light theme › previous and next are 40 px round controls at the media's sides (22, UI-S-031) (577ms)
  ✓   3 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:242:5 › light theme › the media is fitted in its 8 px frame and the chip states the scale (22, UI-S-031) (588ms)
  ✓   4 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:278:5 › light theme › while the full-size image loads the chip says so with its progress bar (22, UI-S-031) (480ms)
  ✓   5 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:164:5 › dark theme › the top bar names the collection, the badge, the file and its place (22, UI-S-031) (494ms)
  ✓   6 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:216:5 › dark theme › previous and next are 40 px round controls at the media's sides (22, UI-S-031) (500ms)
  ✓   7 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:242:5 › dark theme › the media is fitted in its 8 px frame and the chip states the scale (22, UI-S-031) (576ms)
  ✓   8 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:278:5 › dark theme › while the full-size image loads the chip says so with its progress bar (22, UI-S-031) (464ms)
  ✓   9 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:313:1 › next and previous step through the collection and keep the bar (589ms)
  ✓  10 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:334:1 › a location that names no collection shows none, never a guessed one (384ms)
  ✓  11 [chromium] › tests/e2e/preview/preview-top-bar.spec.ts:343:1 › content that cannot be read keeps the bar and shows the error within 2,000 ms (379ms)
  11 passed (17.2s)
```

### Playwright preview suite, whole suite

```
  283 passed (2.2m)
```

### npm run test:perf (budget table rows of this task, then the summary)

```
| control-idle-frames | 595 | 5 | 16.7 |  | 16.7 | 16.8 | 16.8 |  | 60 Hz, frame budget 16.7 ms (NFR-002); missed > 25.05 ms; at most 1% missed; at least 550 frames | pass |
| preview-failure-visible | 20 | 0 | 46.5 | 63.4 | 63.7 | 72.9 | 72.9 |  | UI-S-035 failure visible <= 2,000 ms (never a spinner) | pass |
| preview-large-ack | 120 | 20 | 29.7 | 30.3 | 30.3 | 30.4 | 30.6 |  | NFR-002 acknowledgment p95 <= 100 ms | pass |
| preview-large-image | 120 | 20 | 63 | 63.6 | 63.7 | 79.4 | 96.6 |  | UI-S-035 12-megapixel original warm p95 <= 2,000 ms | pass |
| preview-small-ack | 120 | 20 | 29.6 | 30.1 | 30.3 | 30.4 | 30.5 |  | NFR-002 acknowledgment p95 <= 100 ms | pass |
| preview-small-image | 120 | 20 | 63 | 63.7 | 79.3 | 79.9 | 80 |  | NFR-002 prepared viewer image paints p95 <= 100 ms | pass |
| preview-text-ack | 120 | 20 | 30 | 30.4 | 30.5 | 30.6 | 31 |  | NFR-002 acknowledgment p95 <= 100 ms | pass |
| preview-text-image | 120 | 20 | 46.6 | 47.1 | 47.2 | 62.9 | 63.2 |  | NFR-002 prepared view paints p95 <= 100 ms | pass |
  28) [chromium] › tests/performance/80-composition.perf.spec.ts:118:5 › dark theme › the preview with its file panel (mockups 22 and 35) as built, over generated images
    Error: expect(received).toEqual(expected) // deep equality
    + Array [
    +   "Failed to load resource: the server responded with a status of 404 (Not Found)",
  28 failed
  2 passed (14.5m)
```

The same 404 lines on an unmodified `origin/main` export:

```
  ✓  1 [chromium] › tests/performance/00-control.perf.spec.ts:27:1 › the browser build is recorded with the machine (10ms)
  ✘  2 [chromium] › tests/performance/00-control.perf.spec.ts:35:1 › control: an idle page keeps 60 Hz, so the machine can judge the app (11.0s)
  ✘  3 [chromium] › tests/performance/00-control.perf.spec.ts:59:1 › control: the route round trip under every request of the harness (1.2s)
    +   "Failed to load resource: the server responded with a status of 404 (Not Found)",
```

### Pre-commit hook

Ordinary run (`bash scripts/pre-commit.sh`) on the branch after merging `origin/main`:

```
ℹ️  Test scope: npm:apps/filesquid/ui
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (132s)
```
