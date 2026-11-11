# Task UI-F2.21 Walkthrough: Monitor link and route by role

Global task: [Task FS-2.4](../../../../../resources/implementation-plan.md). Module row: [UI-F2.21](../plan.md). Branch: `feat/ui-f2.21-monitor-link-by-role`.

Owner decision 2026-10-31, verbatim: "accept recommendations" (recommendation 3: the Monitor link and route are offered only to the desktop owner or a server administrator).

## SRS mapping

| Requirement | Full title | Covered by |
|---|---|---|
| [UI-S-032](../srs.md) | Monitor and Logs composition (the Monitor button and route offered only to a caller whom the API grants the admin routes) | `tests/unit/admin-access.test.ts`, `tests/ui/monitor-by-role.test.ts`, `tests/e2e/preview/monitor-by-role.spec.ts` |
| [UI-S-026](../srs.md) | View states, row facts and the permission state (`stateDenied`) | `tests/ui/monitor-by-role.test.ts` (direct visit, own read refused), `tests/e2e/preview/monitor-by-role.spec.ts` (direct visit) |
| [UI-S-025](../srs.md) | Shell composition (header Monitor button, sidebar Logs row) | `tests/ui/monitor-by-role.test.ts`, `tests/e2e/preview/monitor-by-role.spec.ts` |
| [UI-S-021](../srs.md) | Logs view (for the desktop owner) | `offersLogs` unit tests; the Logs row hidden with the Monitor button |
| PRD [F-061](../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring) | Activity journal and live health monitoring (only the desktop owner or server administrators may open the Monitor) | all of the above |
| [SRS-F-061](../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring) | Activity journal and live health monitoring | all of the above |

BDD scenarios: [`monitor_by_role_UI-S-032.feature`](../../tests/features/monitor_by_role_UI-S-032.feature) (three scenarios kept, three added: the Logs row follows the same role check, a caller with no credential keeps the sign-in, a role refusal met by the Monitor's own read hides the button).

## What changed

- `src/components/admin-access.ts` (new): the one role check of the admin views. `accessAfter(error)` (pure): success grants, a `403` denies unless its reason is `invalid_credentials`, anything else leaves the role unknown. `offersMonitor(access)` and `offersLogs(hostType, access)` (pure). `AdminAccessStore` (an `EventTarget`): `probe()` reads `GET /api/v1/admin/health` once per call while the page holds a credential, `settled()` resolves to the probe in flight without sending anything, `deny()` records a refusal met elsewhere; a generation counter drops a stale answer.
- `<fs-app-shell>`: optional `access`; the header Monitor button is hidden unless `offersMonitor`, the Logs row unless `offersLogs` (one check, the Logs row also on the desktop host only, as before). The Logs row now stays hidden until the host badge has named the desktop host.
- `<fs-monitor>`: optional `access`; the first load waits for the role probe, a refused caller gets `<fs-states>` with `{ kind: "denied", reason: "not_permitted" }` ("You do not have permission to see this", "Not permitted") with no subscription and no admin call; a role refusal of its own load or poll is recorded with `deny()` and shows the same state instead of the sign-in form.
- `watchOwner` (View in Logs links of the file panel) and `bindFilesRoute`: the same `offersLogs` check.
- `src/app/main.ts`: one store for the shell, the Monitor and the files route; probed at start and after a token is entered on the Monitor.
- Docs: root README (Monitor dashboard paragraph), `@filesquid/ui` README (Admin views by role), the plan row Status, the checklist box and the MVP progress bar.

No contract field, route or label key was added: the only route used is `GET /api/v1/admin/health`, in the contract on main; `stateDenied` and `authNotPermitted` already exist in both label tables.

## Interpretation recorded for the owner

- API-S-004 answers invalid credentials with `403 invalid_credentials`. That is not a role refusal (no caller was identified), so it keeps the server host's sign-in form; only a `403` with another reason (`not_permitted`) hides the Monitor.
- Without a credential the page sends no probe (the API could only answer `401`): the button stays and the Monitor keeps its sign-in form. A token entered there is probed; if it is refused the button disappears and the Monitor shows `stateDenied` until the page is reloaded (the token lives in memory only).
- The desktop owner's per-launch token is always granted the admin routes, so the desktop owner always sees the Monitor.

## Tests run

### Failing first (before the code)

```text
⎯⎯⎯⎯⎯⎯ Failed Tests 19 ⎯⎯⎯⎯⎯⎯⎯
 FAIL  tests/ui/monitor-by-role.test.ts > Monitor link and route by role (UI-S-032) > shows no Monitor button and no Logs row when the admin health route answers 403
TypeError: AdminAccessStore is not a constructor
 FAIL  tests/unit/admin-access.test.ts > accessAfter > grants the admin views after a successful admin read
TypeError: accessAfter is not a function
 Test Files  2 failed (2)
      Tests  19 failed (19)
```

### After the code

```text
 ✓ tests/unit/admin-access.test.ts (12 tests) 25ms
 ✓ tests/ui/monitor-by-role.test.ts (7 tests) 235ms
 Test Files  2 passed (2)
      Tests  19 passed (19)
```

Whole package (`npm test` in `apps/filesquid/ui`), with `npx prettier --write .`, `npx eslint .` (exit 0) and `npx tsc --noEmit` (exit 0):

```text
 ✓ tests/ui/monitor-by-role.test.ts (7 tests) 418ms
 Test Files  124 passed (124)
      Tests  2843 passed | 1 skipped (2844)
```

### Playwright, preview suite (`vite build` plus `vite preview`)

`flock -w 5400 /dev/shm/filesquid-ui-playwright.lock env FILESQUID_E2E=1 npx playwright test -c playwright.preview.config.ts` (the whole preview suite):

```text
  ✓  192 [chromium] › tests/e2e/preview/monitor-by-role.spec.ts:124:5 › light theme › a caller the admin routes refuse sees no Monitor button and no Logs row (295ms)
  ✓  193 [chromium] › tests/e2e/preview/monitor-by-role.spec.ts:140:5 › light theme › a direct visit to #/monitor shows stateDenied with no further admin call (6.3s)
  ✓  210 [chromium] › tests/e2e/preview/monitor-by-role.spec.ts:166:5 › light theme › the owner sees the Monitor button and the dashboard as before (461ms)
  ✓  212 [chromium] › tests/e2e/preview/monitor-by-role.spec.ts:124:5 › dark theme › a caller the admin routes refuse sees no Monitor button and no Logs row (296ms)
  ✓  214 [chromium] › tests/e2e/preview/monitor-by-role.spec.ts:140:5 › dark theme › a direct visit to #/monitor shows stateDenied with no further admin call (6.4s)
  ✓  229 [chromium] › tests/e2e/preview/monitor-by-role.spec.ts:166:5 › dark theme › the owner sees the Monitor button and the dashboard as before (357ms)
  1 failed
  277 passed (1.9m)
```

The one failure is `provenance-panel.spec.ts` "View in Logs opens the Logs address filtered to the file and to an operation": a `502 (Bad Gateway)` console line from the Logs view's own read after the click, which the preview proxy answers. It is flaky on `origin/main` as well, unchanged by this task: repeated 8 times, it failed 5 of 8 on an export of `origin/main` and 2 of 8 on this branch. It belongs to the Logs view's preview spec (UI-F2.16 close-out).

The Playwright specs assert: custom elements defined, every word from the active label table, computed colours of the denied state (title `--fs-on-surface`, detail `--fs-on-surface-variant`) and of the owner's Monitor button (`--fs-on-surface` text, `--fs-surface-card` background, `--fs-outline-strong` border, 600 weight, 7 px radius) in light and dark, exactly one admin request (`/api/v1/admin/health`) for a refused caller even after a poll interval, and no console errors other than the browser's line for the deliberate 403.

### Not run

- The server-backed Playwright suite (`playwright.config.ts`): it needs a `filesquid-server` build and this machine builds no Rust. The change keeps its flows: without a credential no probe is sent (the sign-in specs), and with the admin launch token the probe answers 200.
- Coverage: `@filesquid/ui` has no coverage reporter configured; none was added.

## Pre-commit

See the PR for the hook's summary line on the head commit (`Test scope: npm:apps/filesquid/ui`).
