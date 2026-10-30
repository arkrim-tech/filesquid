# Task API-F2.16 Plan: Upload review step and per-file job progress

Branch `feat/api-f2.16-upload-review`. Sources: the [plan row](../plan.md), [API-S-036](../srs.md), [API-S-037](../srs.md), [UI-S-028](../../../../ui/resources/srs.md), mockups `resources/ui-mockups/v3/18-import-review.svg` and `19-import-progress.svg`.

## Constraints copied from the SRS

API-S-036 **Review**:
1. `POST /api/v1/imports` (`files.write`) answers `{outcome: pending, import: planned, plan_id}` with the plan before any byte is staged: files found, new, already in this library, bytes to copy, destination collection and folder, count of names to be numbered, free bytes after the upload, staging limit.
2. `GET /api/v1/imports/{plan_id}/items` (`files.write`) pages the planned files, at most 1,000 per page with a cursor: display name (base name only), size, new or already present, numbered.
3. The job starts only on `POST /api/v1/imports/{plan_id}/start`; a repeated start answers the same job.
4. `DELETE /api/v1/imports/{plan_id}` and expiry after 1 hour discard the plan, changing nothing.
5. A changed source is refused `conflict` (`source_changed`).
6. Repeated operation ID: its open plan (`planned`, same `plan_id`), its started job (`started`), `cancelled` without a dialog; an expired or discarded plan answers `plan_not_found`.
7. Refusals: `library_not_found` 404, `dialog_open` 409, `unavailable` 503, `plan_not_found` 404.
8. Journal: `import.plan` exactly once per plan request (`succeeded` with the plan ID, `cancelled`, or `failed` with code); `import.start` once per start (`succeeded` with the job ID or `failed`); `import.discard` once per discard. No path, storage key or file or folder name in request, answer, error or journal; the plan items route's display name never in an error or journal entry.
9. Contract: routes, actions, schemas and codes under a minor version bump (0.31.0 above main's 0.30.0), regenerated TypeScript types.

API-S-037 **Per-file progress**:
10. An upload job's view carries its destination collection and folder.
11. `GET /api/v1/jobs/{job}/items` (`jobs.read`) pages at most 1,000 files per page with a cursor: display name (no path), destination folder, state (`queued`, `copying` with bytes done of total, `copied`, `skipped`, `failed` with error code).
12. `POST /api/v1/jobs/{job}/items/{item}/retry` and `POST /api/v1/jobs/{job}/retry-failed` (`jobs.manage`, no body, answer the job view) put a failed file, or every failed file, back to `queued` and hand the job to its runner; a paused job is refused 409 `job_paused` with no change (journaled `job.refused`); a retry that puts nothing back changes nothing and hands nothing to the runner; a file that is not `failed` is left as it is; an unknown item is 404; a finished or cancel-requested job refuses a retry as the controls do.
13. Each file put back is journaled once as `job.item_retried` under the job's operation ID; a retry that changes nothing is not journaled; item display names never in an error or journal entry.

Owner decisions: answer 5 "Keep the review"; answer 6 "Yes, report progress per file"; decision 2 of 2026-10-23 (controls and retries take no envelope).

## Design

- `imports` becomes a module: types, the `ImportStarter` port (every method with a default, so the desktop's current starter keeps compiling and a host without the review starts at once until DE-1.8 adopts the plan), `PlanBook` (the in-memory book of open, started and discarded plans with expiry and the replay rule, for hosts and tests) and the routes.
- `jobs` gains the `JobItems` port (destination, paged items, retry), `MemoryJobItems`, `Jobs::with_items`, `Jobs::view` (the view with the destination), `Jobs::retry` and the three routes; `JobView.destination`.
- `filesquid-server` has no starter and no items: the routes answer `503 unavailable`.
- Desktop: unchanged code; DE-1.8 implements the plan and the items over the native import plans.
