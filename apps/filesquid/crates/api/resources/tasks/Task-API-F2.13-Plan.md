# Task API-F2.13 Plan: Command envelopes and operation-ID idempotency on the file write routes

Row: [Task API-F2.13](../plan.md), dispatched by global Task FS-2.5. Engineer role: platform engineer.

## Scope (owner decisions of 2026-10-23)

- The **file write routes only**: upload (`PUT /files/upload`), create folder (`POST /files/folders`), rename (`POST /files/renames`), move (`POST /files/moves`), copy (`POST /files/copies`), remove (`POST /files/removals`), restore (`POST /files/restorations`) and version labels (`PUT /files/labels`).
- The library mutation routes take the envelope in API-F2.19; the location-choices route (API-S-034), the repair route (API-S-023) and the job controls (API-S-037) are not envelope routes. The statement and namespace writes of API-S-035 are not in this row.
- UI side: `FileSquidClient` and the file browser's callers only (`ClientFileSource`, `MemoryFileSource`, `file-browser-writes.ts`, `files/ops.ts`), because UI-F2.18 (tokens and shell) is in flight in the same package.

## Constraints copied from the SRS

From [API-S-002](../srs.md):

- Every file write route MUST take a `filesquid-core` `CommandEnvelope` and answer a `CommandOutcome` for the same operation ID.
- Envelope: schema version 1, non-empty operation ID, expected revision, scope.
- A conflict MUST be HTTP 409 with a `conflict` outcome.
- A repeated operation ID of a command that changed something MUST answer its first outcome with no second change.
- Operations carry operation/idempotency key, expected revision, error code and retryability.
- The contract is OpenAPI 3.1 in `crates/api/openapi.json`; the TypeScript types are generated from it and a test fails on drift. Contract version one minor above main's at the last merge of `origin/main` (main is 0.24.0, so 0.25.0).

From [API-S-019](../srs.md) (the library precedent this row follows):

- A command whose expected revision is not the current revision MUST be refused as `conflict` with no change.
- The stored receipt is answered before the revision is checked.
- A command that changes nothing stores no receipt (owner decision 2026-10-19), so its retry is evaluated again.

From [UI-S-012](../../../../ui/resources/srs.md):

- Every command and upload carries a `CommandEnvelope` (schema version 1, non-empty operation id, expected revision and scope).
- A 2xx answer MUST resolve to a `CommandOutcome` (`committed`, `pending`, `conflict` or `error`) for the same operation id; HTTP 409 MUST resolve to `conflict`; any other non-2xx answer MUST reject with `ApiError`; only `committed` is completion evidence.
- `upload()` sends `multipart/form-data` with an `envelope` part (JSON) and a `file` part; `command()` sends `{ envelope, input }`.

From [DE-S-005](../../../desktop/resources/srs.md): the desktop scheme buffers at most 16 MiB of one **response**; requests are not capped by the scheme, so a multipart upload body (at most 64 MiB, `MAX_UPLOAD_BYTES`) passes through it and no stage-then-commit pair is needed.

From [API-S-004](../srs.md), [API-S-005](../srs.md), [API-S-008](../srs.md), [API-S-017](../srs.md): each route keeps its action (`files.write`) and the guard runs first; no answer, error or journal entry carries a path, storage key or name; each write publishes exactly one OCFL version or none; every refusal is journaled once.

## Design

### Request

- JSON routes: `{"envelope": CommandEnvelope, "input": <the route's former body>}`, unknown fields refused (`invalid_request`).
- Upload: `PUT /api/v1/files/upload` with `multipart/form-data`: `envelope` (JSON), `input` (JSON: the former query fields, `size` included) and `file` (the bytes, at most 64 MiB). No query string. axum's `multipart` feature (multer) is enabled for it.
- Envelope checks (`invalid_request` otherwise): `schema_version` 1 and `scope` = `library:<input.library_id>`.

### Expected revision

The head version of the OCFL object the command changes, as the client read it (`v1`, `v2`, ...), compared under the library's write lock, so a stale revision is a 409 `conflict` with no change:

| Route | Revision compared |
|---|---|
| folders, moves, removals, restorations | `input.object_id`'s head |
| renames, labels | the file object's head (`urn:uuid:<file_id>`) |
| upload with `file_id` | the file object's head |
| upload without `file_id` | `0`: the new file's object does not exist yet |
| copies | the source file object's head (the copy's own object is new) |

### Answer

- 2xx: `{operation_id, kind, error, result}`: the `CommandOutcome` fields plus `result`, the view the route answered before (`FileCommit`, `UploadCommit`, `FileCopy`, `FolderAnswer`, `MetadataCommit`); `kind` is `committed`.
- 409: the `ApiError` fields (`code` `conflict`, `message`, `retryable`) plus the conflict outcome fields (`operation_id`, `kind` `conflict`, `error` null, `result` null), schema `FileCommandConflict`. Stale revision, a no-op refusal, a concurrent write and an operation ID still in flight are conflicts.
- Other refusals: the typed `ApiError` as before.

### Receipts

- `files::CommandReceipts` in `ApiState`, in memory for the life of the host process, at most `MAX_RECEIPTS` (10,000) receipts, oldest evicted first.
- Claim before running: a stored receipt for the operation ID answers its first answer again (no second version, not journaled again); a receipt of another route, library or input refuses the request as `invalid_request` (`operation_id_reused`); an ID still in flight answers 409 `conflict`. The in-flight claim is released on every exit path (RAII guard).
- Only a command that changed something stores a receipt: create folder never does (it writes nothing), and a refusal never does.
- Known limit, recorded in the walkthrough: receipts do not survive a restart; after one, a replayed object write meets the moved head and is a `conflict`, while a replayed new upload or copy (revision `0` or an unchanged source) would run again. API-F2.12 records the operation ID in each version's message, which a durable store can rebuild from.

### Journal

Unchanged event names and redaction; each route entry also carries the envelope's operation ID.

## Files

- `crates/api/Cargo.toml`, `Cargo.lock` (axum `multipart`).
- `crates/api/src/files/command.rs` (new: envelope parsing, revision rule, answers), `receipts.rs` (new), `routes.rs`, `metadata_routes.rs`, `drive_routes.rs`, `handle.rs`, `service.rs`, `upload.rs`, `copy.rs`, `versions.rs`, `statements.rs`, `view.rs`, `mod.rs`; `src/lib.rs`.
- `crates/api/openapi.json` (0.25.0), `ui/src/api/schema.d.ts` (regenerated).
- Tests: `tests/integration/file_commands.rs` (new), the existing file route tests and steps moved to envelopes, `tests/features/file_command_envelopes_API-S-002.feature` (new).
- UI: `ui/src/api/client.ts`, `ui/src/api/commands.ts`, `ui/src/files/source.ts`, `ui/src/files/memory-source.ts`, `ui/src/files/ops.ts`, `ui/src/components/file-browser-writes.ts` (+ the browser's host hook for the shown revision), their unit tests.
- Docs: `crates/api/resources/srs.md` (API-S-002 wire format note only where needed), `plan.md`, checklist, walkthrough; UI plan note.

## Verification

- Failing tests first: every file write route refuses a body without an envelope; answers a `CommandOutcome` for the same operation ID; a stale expected revision is 409 `conflict` with no version; a repeated operation ID answers the first outcome with no second version; a no-op stores no receipt (a retry is evaluated again).
- UI: Vitest for `command()`/`upload()` result passthrough and the source's envelopes.
- `cargo fmt`, `cargo clippy -p filesquid-api --all-targets -D warnings`, `cargo test -p filesquid-api`, `npm run lint/typecheck/test` in `ui`, then `FULL=1 bash scripts/pre-commit.sh` (the scope is `full`: `Cargo.lock` and `openapi.json` change).
