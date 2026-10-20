# Task API-F2.2 Conflict Artifact: the upload and file-write contract

Raised 2026-10-20 during [Task API-F2.2](../plan.md), per [sync-logic.md](../../../../../../.agents/rules/sync-logic.md), at the orchestrator's request. The orchestrator asked to settle the upload contract: `PUT /files/upload` takes raw `application/octet-stream`, while `FileSquidClient.upload()` and UI-S-012 describe multipart with an envelope. The disagreement is wider than upload. It covers every file-write route, so it needs an owner decision.

## 1. Conflict description

The requirements and the contract's stated convention say every mutation carries a `CommandEnvelope` and answers a `CommandOutcome`. The file-write routes that API-F1.3 shipped, and that API-F2.5 extends, follow a different convention:

| | Requirement and convention | File-write routes as built |
|---|---|---|
| Request | A `CommandEnvelope` (schema version 1, operation ID as idempotency key, expected revision, scope) as the `envelope` member of the JSON body. Upload: multipart with an `envelope` part and a `file` part | Plain JSON bodies (`MoveRequest`, `RemoveRequest`, `RestoreRequest`, `LabelRequest`); upload is `PUT /files/upload` with the bytes as `application/octet-stream` and identifiers in the query |
| Answer | A 2xx `CommandOutcome` (`committed`, `pending`, `conflict`, `error`) for the same operation ID; HTTP 409 resolves to `conflict` | A `FileCommit` (`library_id`, `object_id`, `version`, `created`); refusals, 409 included, as an `ApiError` |
| Idempotency | A repeated operation ID answers the first outcome with no second change | None: a retried write that already committed is refused as a no-op (`conflict`), or publishes again if the state differs |

The UI holds both sides:

- `FileSquidClient.upload()` (`ui/src/api/client.ts`) posts multipart with an envelope and expects a `CommandOutcome`. No server route accepts it.
- `FileSquidClient.putBytes()` performs the real upload against `PUT /files/upload`, and treats a 409 as an error, as the route answers.

## 2. PRD / SRS reference

- [UI-S-012](../../../../ui/resources/srs.md): "Every command and upload MUST carry a `CommandEnvelope` (schema version 1, non-empty operation id, expected revision and scope); a 2xx answer MUST resolve to a `CommandOutcome` (`committed`, `pending`, `conflict` or `error`) for the same operation id … HTTP 409 MUST resolve to `conflict`; any other non-2xx answer MUST reject with `ApiError`; only `committed` is completion evidence."
- The contract's own convention (`openapi.json` `info.description`): "a command (mutation) sends a CommandEnvelope as the `envelope` member of its JSON request body (operation_id is the idempotency key, expected_revision the revision last read) and answers with a CommandOutcome".
- [API-S-019](../srs.md) applies the envelope and repeated-operation-ID rule to the library domain commands.
- [API-S-008](../srs.md) and the API-F1.3 plan row describe the file writes with no envelope requirement. The routes were built and accepted (PR #82) as plain bodies answering `FileCommit`.

## 3. ADR reference

[ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 2 (versioned contract, typed commands and queries) and Decision 6 (typed errors). Its command model is the `CommandEnvelope` and `CommandOutcome` of the API's `operation` module.

## 4. Recommended resolution

**Option A (recommended): bring the file writes onto the command convention.**

- `POST /files/moves`, `/removals`, `/restorations`, `PUT /files/labels`, and the upload take `{ "envelope": CommandEnvelope, "input": … }`.
- The upload takes the bytes as a second multipart part, or uses a two-step stage-then-commit if the desktop scheme cannot carry multipart within its buffer cap (DE-S-005).
- They answer a `CommandOutcome` whose `committed` result carries the `FileCommit`, with HTTP 409 for `conflict`.
- A repeated operation ID answers the first outcome without a second version: the version message already carries the operation ID (ADR 040), so the receipt can be found in the object's history.
- This matches UI-S-012 and API-S-019, and gives file writes safe retries.
- Cost: a contract-wide change to five routes and the UI's file source, in its own task row (API plus UI).

**Option B: amend UI-S-012 and the convention.**

- File writes are an exception: plain bodies and a `FileCommit` answer, idempotent by "refuse a no-op" rather than by operation ID.
- Remove `FileSquidClient.upload()`'s multipart form and keep `putBytes()`.
- Cheaper, but a retried write after a lost answer can publish a second version when another change landed in between. That is the risk the envelope exists to remove.

**Option C: keep both.** Not recommended: the UI would carry an `upload()` that no host serves.

Artifacts to update under Option A:

- `crates/api/openapi.json` (five routes, schemas), `crates/api/src/files/` (routes, views, service idempotency lookup), and the API-F1.3 and API-F2.5 walkthrough notes.
- `ui/src/api/client.ts` and `ui/src/files/source.ts` (UI lane).
- [API-S-008](../srs.md) or a new API-S row stating the file-write command form.
- The plan rows: a new API row and a matching UI row.

Under Option B: UI-S-012, the contract `info.description`, and `client.ts`.

## 5. Impact assessment

- Unresolved, the UI's documented `upload()` cannot reach any host.
- Retried file writes have no idempotency key, so a lost answer followed by a retry can commit twice, or report `conflict` for a change that did commit.
- Every new write route (API-F2.2 copy and move, API-F1.4 metadata edits) has to pick a convention without a rule, so the split widens with each row.

API-F2.2 adds no new write route in this PR, so nothing here waits on the decision. Copy and the ZIP download are already deferred for other reasons.
