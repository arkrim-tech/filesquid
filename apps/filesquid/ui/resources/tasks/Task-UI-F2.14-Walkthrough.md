# Task UI-F2.14 Walkthrough: OCFL validation findings in the Monitor

[Task UI-F2.14](../plan.md) is a module row of global [Task 1.1](../../../../../resources/implementation-plan.md#task-1.1), dispatched with [API-F2.7](../../../crates/api/resources/plan.md) (done). The Monitor now has an **OCFL validation** section, the custom element `<fs-ocfl-findings>`, placed below Health. It reads `GET /api/v1/admin/ocfl/findings`, starts runs through `POST /api/v1/admin/ocfl/validation-runs` and updates live from `health.change` and from `ocfl.validation` process updates. The contract, the API and the Rust crates are unchanged.

## SRS mapping

| ID | Title | Covered by |
|---|---|---|
| [SRS-F-061](../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring) | Activity journal and live health monitoring | [`ocfl_validation_findings_monitor_F-061.feature`](../../tests/features/ocfl_validation_findings_monitor_F-061.feature), `tests/ui/ocfl-findings.test.ts`, `tests/ui/monitor.test.ts`, `tests/e2e/preview/ocfl-findings.spec.ts`, `tests/e2e/ocfl-findings.spec.ts` |
| [API-S-022](../../../crates/api/resources/srs.md) | OCFL validation findings MUST reach health and the Monitor | client methods (`tests/unit/admin-client.test.ts`); errors apart from warnings, withheld location, last and next run, schedule, truncated note, live refresh (Vitest and preview Playwright); real findings and run routes (server Playwright) |
| [UI-S-005](../srs.md) | A distribution's label table MUST define every key of the default table | the 37 new keys are in `REQUIRED_LABEL_KEYS` and in the FileSquid and ImageSquid tables (`apps/imagesquid` tests pass) |
| [UI-S-016](../srs.md) | Accessibility contracts | each findings table is named by its `h3` inside a named, focusable scroll region (`ocfl-findings.test.ts`); the accessibility audit and the 320 px / 200 % reflow test in `accessibility.spec.ts` pass with the new section |
| [UI-S-013](../srs.md) | UI tests against production artifacts with strict DOM assertions | preview and server-backed Playwright suites: elements defined, words from the label table, token colours in light and dark, no console errors |

## Constraint gates

| Constraint (from the Plan) | Code | Test |
|---|---|---|
| 1 `FindingsView`: at most 1,000 findings, `truncated`, `settings` (interval, default 7 days; cap, default 100,000), `libraries` | `FsOcflFindings.render`, `scheduleItems` | "says when more findings are open than listed", "shows the schedule in force"; server spec reads the real schedule |
| 2 finding fields; no location the API withholds | `findingRow` (`data-withheld`, "Not shown") | "renders no location the API withholds"; preview criterion (4) |
| 3 last run fields and outcomes | `runCells`, `outcomeText` | "shows the last and the next run…", "says Never and Not scheduled…", "names a failed run's error code and a capped run's unchecked objects"; preview criterion (5) |
| 4 run route: empty `RunRequest`, `RunAccepted`, refusals | `startOcflValidation`, `FsOcflFindings.startRun` | "starts a run of every open library…", "words a refused start…", admin-client 409 test; server spec posts to the real route |
| 5 health `ocfl_validation_errors` with codes and objects, worded from labels; warnings do not change health | `reasonText` in `fs-monitor.ts` | "words the OCFL validation reason…"; preview criteria (1) and (3) |
| 6 `health.change` and `ocfl.validation` process updates refresh the section | `FsMonitor.applyEvent` | "re-reads the findings on health.change and on an ocfl.validation process update only"; preview criterion (2) |
| 7 errors apart from warnings; W010 read-only note and repair link | `readOnlyNotes`, `repairHref` | "lists a W010 warning apart…", "shows the read-only note without a link…"; preview criterion (3) |
| 8 labels and tokens, light and dark | label tables, `STYLE` tokens | `component-styles.test.ts`, preview "error and warning codes take their colours from tokens in light and dark" |
| 9 named focusable scroll regions; focus kept | `tablePart`; refresh replaces rows only | "names every table by its heading…", "keeps focus on Validate now across a refresh" |
| 10 failure in the section only, with Retry | `showState`, `<fs-states>` | "shows the unavailable state with Retry…", "keeps the other sections up when the findings route is unavailable" |
| 11 coalesced refresh | `FsOcflFindings.refresh` | "coalesces refreshes: one request in flight and one queued behind it" |

## What changed

- **Client** ([`src/api/client.ts`](../../src/api/client.ts)): `adminOcflFindings()`, `startOcflValidation(request)`, and the contract types `FindingsView`, `FindingView`, `LibraryRuns`, `RunView`, `RunOutcome`, `ValidationSettings`, `RunRequest`, `RunAccepted` and `HealthReason`.
- **`<fs-ocfl-findings>`** ([`fs-ocfl-findings.ts`](../../src/components/fs-ocfl-findings.ts)), with pure row builders in [`ocfl-findings-view.ts`](../../src/components/ocfl-findings-view.ts): a Validation runs table, the schedule, an Errors table, a separate Warnings table, W010 read-only notes, a truncated note, and Validate now with a status line.
- **`<fs-monitor>`** ([`fs-monitor.ts`](../../src/components/fs-monitor.ts)): a new `ocfl` section after Health; `MonitorClient` gains the two methods; `MonitorOptions.repairHref`; it refreshes the section on `health.change` and on `ocfl.validation` process updates; it words the `ocfl_validation_errors` reason from `healthOcflErrors`, and each reason item carries `data-reason`.
- **Labels:** 37 new keys (`monitorFindings`, `healthOcflErrors`, `findings*`, `run*`) in [`labels.ts`](../../src/theme/labels.ts), [`labels.en.json`](../../src/theme/labels.en.json) and ImageSquid's table.
- **Docs:** the package README section "Monitor: OCFL validation findings", the root README "View logs" bullet, the plan row, and a note in UI-1.1 on passing `repairHref`.

## Holistic UI decisions

- **UI-1.1 (repair consent).** UI-1.1 has no route yet, so the Monitor takes an optional `repairHref(libraryId)` and shows no dead link until UI-1.1 passes one. The W010 note already says the files can be read but not changed, which matches the wording UI-1.1 requires.
- **UI-F2.16 (Logs view).** The findings tables use the same table and named-scroll-region structure as the Monitor's other tables. Nothing here would need to be replaced to link a finding to the Logs view later.
- **503 unavailable (UI-F2.1b).** The section uses `failureState` and `<fs-states>`, so a host without the validation service shows the unavailable state with Retry and does not take down the rest of the Monitor.

## Decisions and gaps for the platform engineer

- `filesquid-server` opens no OCFL library until SV-F1.2 (`start_validation` in `crates/server/src/lib.rs`). Against the real server, the findings route therefore answers no findings and no libraries. The server-backed Playwright suite proves that the real routes are wired (an empty findings answer, the real schedule, `POST` answering `{"started":[],"refused":[]}`). The E023, closure, W010, withheld-location and run-time criteria are proven against the production build with answers shaped like the contract. **Needed for a server-backed E023 proof:** a way for the server to open a library and validate it, for example a test-only fixture like the desktop's `FILESQUID_E2E_LIBRARY`, wired to `ValidationService::open_library`.
- No new route or field is assumed.

## How it was tested

1. Failing tests first: 27 failed before any implementation (below).
2. `npx prettier --write .`, `npx eslint .`, `npx tsc --noEmit` in `apps/filesquid/ui` and `apps/imagesquid`: clean.
3. `npm test` in both packages.
4. Playwright, both suites (`npm run test:e2e` runs them in this order). The installed Playwright expects a newer Chromium build than this machine has, so the suites ran through temporary configs that point `executablePath` at the machine's Chromium. The configs were deleted before committing. No other mocked environment values were used.

## Raw test output

### Red run before implementation

```text
⎯⎯⎯⎯⎯⎯ Failed Tests 27 ⎯⎯⎯⎯⎯⎯⎯
 Test Files  4 failed (4)
      Tests  27 failed | 163 passed (190)
```

### npm test (apps/filesquid/ui)

```text
 ✓ tests/ui/monitor.test.ts (18 tests) 475ms
 ✓ tests/ui/ocfl-findings.test.ts (18 tests) 319ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 18ms
 Test Files  49 passed (49)
      Tests  1035 passed (1035)
   Duration  17.03s (transform 2.92s, setup 1.92s, import 5.86s, tests 9.50s, environment 24.40s)
```

### npm test (apps/imagesquid)

```text
 Test Files  4 passed (4)
      Tests  37 passed (37)
```

### Playwright, built UI served by `filesquid-server`

```text
  ✓  29 [chromium] › tests/e2e/ocfl-findings.spec.ts:36:1 › the findings section reads the real findings route after sign-in (495ms)
  ✓  32 [chromium] › tests/e2e/ocfl-findings.spec.ts:67:1 › Validate now starts runs through the real run route and says nothing was open (479ms)
  42 passed (14.8s)
```

### Playwright, production build (`vite build` plus `vite preview`)

```text
  ✓  38 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:174:1 › (1) an E023 finding shows with its code beside the degraded health reason (383ms)
  ✓  40 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:200:1 › (2) the finding disappears after a clean revalidation's health.change, without a reload (386ms)
  ✓  42 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:220:1 › (3) a W010 warning is listed apart from errors and health stays ok (375ms)
  ✓  43 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:237:1 › (4) no location is rendered when the API withholds it (280ms)
  ✓  45 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:249:1 › (5) the last and next run times come from the findings route; Validate now starts a run (387ms)
  ✓  47 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:288:1 › error and warning codes take their colours from tokens in light and dark (379ms)
  49 passed (17.7s)
```

### Pre-commit hook on the head commit

Recorded in the pull request (`Test scope:` line).
