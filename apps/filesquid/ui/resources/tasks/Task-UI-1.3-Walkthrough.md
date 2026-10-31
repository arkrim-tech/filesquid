# Task UI-1.3 Walkthrough: First use, library facts, recent libraries and the rebuild job

[Task UI-1.3](../plan.md) completes the UI slice of global [Task 1.2](../../../../../resources/implementation-plan.md#task-1.2) that builds on API-F2.17 (rebuild job), API-F2.18 (library facts) and DE-1.8. Global Task 1.2 stays open: [Task UI-1.2](../plan.md) (import dialog, destination picker and job progress) is still in progress on its own branch, so only the module box and its row of the repository checklist are ticked. Library revisions restart at 1 after a host restart ([API-S-025](../../../crates/api/resources/srs.md); owner 2026-10-31 accepted "no change for the MVP"): no page here assumes a revision persists, and the library page and the rebuild start still read the current revision from the facts when this session did not create or open the library.

## What changed

- **First use** (`<fs-first-use>`, [mockup 57](../../../../../resources/ui-mockups/v3/57-first-use-library.svg)): `<fs-library-start>` reads `GET /api/v1/libraries/recent` when it renders; an empty list opens the 600 x 470 px dialog "Create your first library" over the welcome screen. Name, **Choose folder…** (location-choices, purpose `create`; only an opaque location ID reaches the UI), **Cancel** and **Create library**, which creates the library with its name (no name is sent when the field is blank, so the host names it) and then its first collection "untitled collection". A failure after the library exists keeps the library (`registry.remember`), says so (`libraryFirstUseCollectionFailed`) and offers Try again for the collection alone: the folder is not asked for again, the library is not created twice, and the collection command keeps its operation id. Cancel closes the dialog (it goes to the library page once the library exists); the Create card opens it again while no library is known. The dialog announces `fs-first-use-done` (bubbles, composed; detail `{ libraryId, collectionId }`) before it navigates: the hook for UI-1.2's import dialog. No import entry point was built.
- **Launch** (`redirectToFirstUse`): an app opened with no address of its own goes to `#/library` when the host knows no library; a host without a registry (503), a failure, or an address the owner chose is left alone.
- **Recent libraries** (`recent-libraries.ts`, [mockup 01](../../../../../resources/ui-mockups/v3/01-welcome.svg)): the RECENT LIBRARIES column beside the title, one row per library with its name, "Opened {when} · {free} free on its disk" (today, or the date; "free space unknown" when the volume cannot be read, never a figure) and a ghost **Open**, which goes to the page of a library the host has open and otherwise runs the Open flow (the API reopens a library only from a folder the owner chooses).
- **Library facts** ([mockups 41 and 09](../../../../../resources/ui-mockups/v3/41-library-page.svg)): the library page is titled with the name from `GET /api/v1/libraries/{library}/facts`, its breadcrumb is that name, and **Free space** reads "{free} on the disk that holds this library" ("Not known" when the volume cannot be read). The sidebar's "{free} free on disk" line already comes from `recent` (UI-F2.18) and is unchanged.
- **Rebuild job** (`<fs-library-rebuild>`, `ClientJobSource`, [mockup 04](../../../../../resources/ui-mockups/v3/04-recover-library.svg)): Recover now ends on `#/library/<id>/recover/<job>` (without the shell) and the library page's Rebuild on `#/library/<id>/rebuild/<job>` (inside it). One raised card shows the state chip (running, pausing, paused, cancelling, finished, failed, cancelled), the progress line, the bar, **Pause** or **Resume** and **Cancel**; a finished job shows its report and each object's validation errors. Live through `job.update`, with a bounded safety re-read (`refreshMs`, at most `MAX_REFRESHES`); a completed job is read again once for its report because the event carries none.
- **Words and tokens**: 40 label keys added to `REQUIRED_LABEL_KEYS`, the default table and ImageSquid's table (its collection words are albums). No token was added: every constant reuses the UI-S-024 tokens, so `distribution-tables.test.ts` passes.
- **Existing tests updated** (behaviour changed on purpose): the start page now reads `recent`, the library page reads its facts and is titled with the name, Recover and Rebuild go to the job page, the `.eyebrow` selectors of the start page became `.page > .eyebrow`, and the `facts` fixture in `library-client.test.ts` carries the contract's `free_bytes_reason`.

## SRS mapping

| ID | Title | Covered by |
|---|---|---|
| [UI-S-034](../srs.md) | First use (mockup 57-first-use-library.svg) | `tests/ui/first-use.test.ts`, `tests/unit/first-use-route.test.ts`, `tests/unit/library-facts-client.test.ts` (named create, first-collection command, retained operation id), `tests/e2e/preview/first-use.spec.ts` (regions, strings, computed styles, create then collection, failure kept with Try again, Cancel), [`first_use_library_UI-S-034.feature`](../../tests/features/first_use_library_UI-S-034.feature) |
| [UI-S-027](../srs.md) | Library lifecycle screens (welcome and create, Recover, library page) | `tests/ui/library-recent.test.ts`, `tests/ui/library-rebuild.test.ts`, `tests/ui/library-detail.test.ts`, `tests/unit/library-format.test.ts`, `tests/unit/library-job-routes.test.ts`, `tests/e2e/preview/first-use.spec.ts` (recent libraries, unreadable volume, library page, rebuild running, paused, resumed, cancelled and finished), [`library_rebuild_job_UI-S-027.feature`](../../tests/features/library_rebuild_job_UI-S-027.feature) |
| [UI-S-025](../srs.md) | Shell composition (the "{free} free on disk" line) | unchanged `storage-route.ts` (reads `recent`, API-S-038); `tests/unit/storage-library.test.ts` |
| [UI-S-024](../srs.md) | Visual constants of the active mockup suite | computed-style and region assertions in `first-use.spec.ts`, light and dark; `distribution-tables.test.ts` for the ImageSquid tables |
| [API-S-038](../../../crates/api/resources/srs.md) | Library facts (name, free space, recent libraries) | `parseLibraryFacts`, `parseRecentLibraries`, `ClientLibrarySource.facts` and `recent` |
| [API-S-037](../../../crates/api/resources/srs.md) | Job routes and job events | `ClientJobSource`, `parseJobView`, `parseJobControl`, `tests/unit/library-facts-client.test.ts` |
| [API-S-025](../../../crates/api/resources/srs.md) | Local library routes, rebuild as a durable job | `tests/ui/library-rebuild.test.ts`, `first-use.spec.ts` |
| [PRD F-001](../../../../../resources/prd.md#f-001-independent-local-library-and-ownership), [F-004](../../../../../resources/prd.md#f-004-collections-folders-and-navigation), [F-009](../../../../../resources/prd.md#f-009-multiple-libraries-and-filesystem-administration) | Library ownership, first collection, libraries and their facts | the scenarios above |

## Constraint gates

| Constraint | Code | Test |
|---|---|---|
| Dialog 600 x 470 px at x 420, y 162, radius 10 px, overlay surface, 1 px strong border, scrim 0.32 | `FIRST_USE_STYLE`, `.dialog` | `first-use.spec.ts` "first use (57...)" |
| Title 22 px 700 Georgia; lead 14 px; Name label 13 px 600 | `.dialog-title`, `.lead`, `.field-label` | same |
| Name input 544 x 36 px at y 314, 6 px radius, 2 px focus ring, focused on open | `input.input` | same |
| Location box 400 x 36 px, Choose folder 136 x 36 px (secondary), the rule line | `.location`, `.rule` | same |
| Note box 544 x 64 px, raised surface, 8 px radius, "First collection: untitled collection" 13 px 600 | `.collection-note` | same |
| Cancel 82 x 38 px secondary, Create library 130 x 38 px the one primary; disabled until a folder is chosen | `.dialog-actions` | same |
| Create the library, then the collection, each a command; library kept on a collection failure; Try again for that step | `FsFirstUse.create`, `retry` | `first-use.test.ts`; preview "a failure after the library exists..." |
| Strings from the label table, in both tables | 40 keys | `first-use.spec.ts`, `distribution-tables.test.ts` |
| Recent column 332 px at the page's right edge, rows 60 px apart, name 14 px 600, meta 13 px, Open ghost 76 x 24 px, eyebrow 11 px 400 upper case | `RECENT_STYLE` | preview "recent libraries (01...)" |
| Free space "{free} on the disk that holds this library", unknown without a figure | `freeSpaceText`, `FsLibraryDetail.showFacts` | `library-format.test.ts`, `library-detail.test.ts`, preview "the library page..." |
| Recover title 34 px; card 1040 x 112 px, raised, 8 px radius; chip 22 px tall, 11 px 500, 4 px radius; bar 760 x 4 px, 2 px radius, `--fs-outline` track and `--fs-primary` fill; Pause and Cancel 92 x 36 px | `REBUILD_STYLE` | preview "the rebuild job (04...)" |
| Error lines 1040 x 52 px, 1 px error border, 8 px radius, mono 13 px | `.errors li` | preview "the rebuild job finishes over the event channel..." |
| Pause, Resume and Cancel over the job routes with no body; one control at a time | `ClientJobSource`, `FsLibraryRebuild.control` | `library-rebuild.test.ts`, `library-facts-client.test.ts`, preview |
| No path, folder name, volume, job ID or library ID shown | all pages | `PATH` checks in `first-use.spec.ts`; `*.test.ts` |
| Loops bounded | `MAX_REFRESHES`, recent list at most 20 | "re-reads on its own while the job is active, bounded..." |

## Holistic UI decisions

- The first-use dialog is its own element with its own shadow root, so UI-1.2's import dialog and later first-run screens can reuse it or listen to its done event; the start page only mounts it.
- One page element serves both rebuild flows (`mode`), so the library page's Rebuild and Recover share the card, the controls and the report instead of two copies.
- `JobSource` is generic (any job by ID) and lives with the library sources; UI-1.2's job panel can use `ClientJobSource` rather than a second client for the same routes.
- The welcome screen's big logo (mockups 01 and 57) is not drawn: UI-1.1 left it out and no logo asset of that size exists in `public/`. The x positions and sizes match the mockups; the title and cards sit about 124 px higher than drawn.

## Proposed strings (not in the owner's approved list; for the owner to confirm)

`libraryFolderNone` "No folder chosen yet"; `libraryFolderChosen` "Folder chosen"; `libraryFirstUseCollectionFailed` "The library was created and is kept. Its first collection could not be created."; `libraryFirstUseCancel` "Cancel"; `libraryRecentMetaNoFree` "Opened {when} · free space unknown"; `libraryRecentToday` "today"; `libraryFactFreeTerm` "Free space"; `libraryFactFreeUnknown` "Not known: the disk cannot be read"; `libraryRebuildProgressNoTotal` "{done} objects read · started {time} · resumes from its checkpoint after you quit"; `libraryRebuildCancelled` "The rebuild was cancelled. Nothing in the library's files was changed."; `jobPause`, `jobResume`, `jobCancel`; `jobStatePending`, `jobStateRunning`, `jobStatePausing`, `jobStatePaused`, `jobStateCancelling`, `jobStateCompleted` ("finished"), `jobStateFailed`, `jobStateCancelled`. The strings of the Agent Prompt are exactly as approved; ImageSquid words the collection as an album ("First album: untitled album").

## Conflicts and what the platform engineer still has to deliver

[Task-UI-1.3-Conflict.md](./Task-UI-1.3-Conflict.md) is the Conflict Artifact for human review. In short:

1. **No create-collection route.** `ClientLibrarySource.createCollection` sends the assumed `POST /api/v1/libraries/collections` (command envelope with scope `library:<id>` and the library's revision, input `{ library_id, title }`, committed result `{ collection_id, title }`). Until it exists a host answers 404 or 405 and first use ends on its Try again state with the library kept.
2. **No folder name or volume in the location choice** (API-S-034 forbids it), so the Location row of mockup 57 says whether a folder is chosen, not "{folder} · on {volume}".
3. **The rebuild's report is null until the job completes**, so VALIDATION ERRORS SO FAR lists the errors with the finished report; the partial list "so far" cannot be shown live.
4. **Storage used and STORAGE BY COLLECTION** (UI-S-027, owner answer 16) are not built: no history split on `library-history-size`, no collection ID on navigation nodes.

## Sync Execution Report

Documents changed by this task: [`apps/filesquid/ui/resources/plan.md`](../plan.md) (the UI-1.3 row: ticked, status, related tests), [`resources/implementation-plan-checklist.md`](../../../../../resources/implementation-plan-checklist.md) and `resources/assets/mvp-progress.svg` (regenerated by `scripts/docs/update_mvp_progress.py`), [`apps/filesquid/ui/README.md`](../../README.md) (the element list and the library-flow section), the two new Gherkin features, and the Conflict Artifact. The SRS, PRD and ADRs are unchanged: the three conflicts above wait for the owner, and no requirement text was edited to fit the code. No ADR is affected.

## Mocked Environment Variables

None. The tests answer the API in the shapes of the contract (`page.route` in Playwright, fakes in Vitest); no dummy credentials or `.env` file were created.

## How it was tested

Failing tests first (below), then the implementation. Vitest units and jsdom DOM tests; Playwright against `vite build` plus `vite preview` at 1440 x 900 in light and dark. No coverage threshold is configured in this package, so no coverage report is attached; the Allure results under `tests/allure-results/` are regenerated by each run (`npm run report` builds the report). The Rust-served end-to-end suite and the hook run are recorded below.

## Raw test output

### Red run (before implementation)

The eight new test files and the changed ones, run against the code as it was (labels, tables and fakes already added so the tests compile; no component, source or parser written):

```text
     26|     const href = libraryRebuildHref("lib-1", "job-1");
       |                  ^
     27|     expect(href).toBe("#/library/lib-1/rebuild/job-1");
     28|     expect(parseLibraryHash(href)).toEqual({

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[95/97]⎯


 Test Files  11 failed (11)
      Tests  95 failed | 73 passed (168)
   Start at  16:47:30
   Duration  5.72s (transform 3.98s, setup 477ms, import 5.66s, tests 1.29s, environment 6.82s)

JUNIT report written to /home/user/filesquid/.claude/worktrees/agent-ae88a99b1c49f53e2/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### Prettier, ESLint and tsc (apps/filesquid/ui)

```text
Checking formatting...
All matched files use Prettier code style!
eslint exit: 0
tsc exit: 0
```

### Vitest (apps/filesquid/ui)

```text
 ✓ tests/ui/library-detail.test.ts (25 tests) 717ms
 ✓ tests/unit/library-client.test.ts (39 tests) 110ms
 ✓ tests/ui/library-start.test.ts (22 tests) 396ms
 ✓ tests/ui/library-rebuild.test.ts (24 tests) 448ms
 ✓ tests/ui/first-use.test.ts (13 tests) 383ms
 ✓ tests/unit/library-facts-client.test.ts (21 tests) 54ms
 ✓ tests/ui/library-recent.test.ts (13 tests) 568ms
 ✓ tests/ui/library-route.test.ts (9 tests) 544ms
 ✓ tests/unit/library-facts-contract.test.ts (31 tests) 108ms
 ✓ tests/unit/library-format.test.ts (22 tests) 96ms
 ✓ tests/unit/distribution-tables.test.ts (7 tests) 16ms
 ✓ tests/unit/first-use-route.test.ts (7 tests) 21ms
 ✓ tests/unit/library-job-routes.test.ts (10 tests) 26ms

 Test Files  96 passed (96)
      Tests  2293 passed | 1 skipped (2294)
   Duration  86.13s (transform 8.39s, setup 6.61s, import 26.44s, tests 90.56s, environment 97.89s)
```

### Playwright, production build plus vite preview (apps/filesquid/ui)

The new proofs, then the whole preview suite (existing tests included):

```text
✓   59 [chromium] › tests/e2e/preview/first-use.spec.ts:77:5 › light theme › first use (57, UI-S-034): the dialog over the welcome screen, region by region (980ms)
✓   63 [chromium] › tests/e2e/preview/first-use.spec.ts:222:5 › light theme › first use creates the library with its name and then its first collection (585ms)
✓   66 [chromium] › tests/e2e/preview/first-use.spec.ts:264:5 › light theme › a failure after the library exists keeps it and offers Try again for the collection (822ms)
✓   69 [chromium] › tests/e2e/preview/first-use.spec.ts:293:5 › light theme › Cancel closes the first-use dialog without a request, and Create opens it again (545ms)
✓   71 [chromium] › tests/e2e/preview/first-use.spec.ts:308:5 › light theme › recent libraries (01, UI-S-027): name, opened time and free space, and a volume that cannot be read (450ms)
✓   73 [chromium] › tests/e2e/preview/first-use.spec.ts:384:5 › light theme › Open on a recent library that is open now goes to its page; one that is not runs the Open flow (439ms)
✓   74 [chromium] › tests/e2e/preview/first-use.spec.ts:401:5 › light theme › the library page (41, 09): the library's name, and Free space from its facts (481ms)
✓   76 [chromium] › tests/e2e/preview/first-use.spec.ts:435:5 › light theme › the library page says so when the volume cannot be read, with no figure (312ms)
✓   78 [chromium] › tests/e2e/preview/first-use.spec.ts:446:5 › light theme › the rebuild job (04, UI-S-027): running, paused, resumed, cancelled (957ms)
✓   81 [chromium] › tests/e2e/preview/first-use.spec.ts:543:5 › light theme › the rebuild job finishes over the event channel and shows its report and errors (671ms)
✓   83 [chromium] › tests/e2e/preview/first-use.spec.ts:615:5 › light theme › a library's own rebuild is followed inside the shell and finishes (584ms)
✓   85 [chromium] › tests/e2e/preview/first-use.spec.ts:77:5 › dark theme › first use (57, UI-S-034): the dialog over the welcome screen, region by region (1.1s)
✓   89 [chromium] › tests/e2e/preview/first-use.spec.ts:222:5 › dark theme › first use creates the library with its name and then its first collection (471ms)
✓   91 [chromium] › tests/e2e/preview/first-use.spec.ts:264:5 › dark theme › a failure after the library exists keeps it and offers Try again for the collection (581ms)
✓   93 [chromium] › tests/e2e/preview/first-use.spec.ts:293:5 › dark theme › Cancel closes the first-use dialog without a request, and Create opens it again (462ms)
✓   95 [chromium] › tests/e2e/preview/first-use.spec.ts:308:5 › dark theme › recent libraries (01, UI-S-027): name, opened time and free space, and a volume that cannot be read (527ms)
✓   98 [chromium] › tests/e2e/preview/first-use.spec.ts:384:5 › dark theme › Open on a recent library that is open now goes to its page; one that is not runs the Open flow (455ms)
✓  100 [chromium] › tests/e2e/preview/first-use.spec.ts:401:5 › dark theme › the library page (41, 09): the library's name, and Free space from its facts (427ms)
✓  102 [chromium] › tests/e2e/preview/first-use.spec.ts:435:5 › dark theme › the library page says so when the volume cannot be read, with no figure (346ms)
✓  104 [chromium] › tests/e2e/preview/first-use.spec.ts:446:5 › dark theme › the rebuild job (04, UI-S-027): running, paused, resumed, cancelled (856ms)
✓  106 [chromium] › tests/e2e/preview/first-use.spec.ts:543:5 › dark theme › the rebuild job finishes over the event channel and shows its report and errors (510ms)
✓  108 [chromium] › tests/e2e/preview/first-use.spec.ts:615:5 › dark theme › a library's own rebuild is followed inside the shell and finishes (558ms)

  188 passed (1.4m)
```

### Pre-commit hook, ordinary run on the head commit (`bash scripts/pre-commit.sh`)

```text
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Change scope validation passed (188 assertions).
ℹ️  Test scope: npm:apps/filesquid/ui
⏭️  Skipped: Clippy (no Rust crate changed)
⏭️  Skipped: Clippy (desktop, test-instrumented) (no Rust crate changed)
✅ ESLint passed (19s).
✅ TypeScript passed (20s).
 Test Files  96 passed (96)
      Tests  2293 passed | 1 skipped (2294)
✅ Vitest passed (125s).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui).
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (208s)
```

### Pre-commit hook with `FILESQUID_E2E=1` (production build served by the real filesquid-server, then the performance suite)

Vitest, ESLint and tsc pass; the Rust-served suite passes 41 of 42 and the performance suite 29 of 30. The two failures are not in code this task touches: `tests/e2e/accessibility.spec.ts:67` compares the shell's accessibility tree to a stored snapshot that does not match the shell as built (it lacks the footer `contentinfo` and the Storage used caption paragraph, and Playwright 1.64 adds link URLs), and `tests/performance/70-load.perf.spec.ts:87` timed out at sample 275 while other agents loaded the machine (load average 7 to 10 on 4 cores). This task changes no shell file; I did not re-run either on origin/main. Both are reported to the orchestrator.

```text
ℹ️  Test scope: npm:apps/filesquid/ui
✅ ESLint passed (6s).
✅ TypeScript passed (9s).
 Test Files  96 passed (96)
      Tests  2293 passed | 1 skipped (2294)
✅ Vitest passed (53s).
  ✘   2 [chromium] › tests/e2e/accessibility.spec.ts:67:1 › landmarks, headings and control names come from the label table (5.3s)
    Error: expect(locator).toMatchAriaSnapshot(expected) failed
  1 failed
  41 passed (49.3s)
  188 passed (1.1m)
  ✘  25 [chromium] › tests/performance/70-load.perf.spec.ts:87:1 › background work: opening folders under the same stream (1.8m)
    Error: sample 275 never reached its predicate within 20000 ms
  1 failed
  29 passed (17.7m)
❌ Playwright e2e failed (1180s).
❌ pre-commit failed: Lint and Test Validation (test scope: npm:apps/filesquid/ui)
```
