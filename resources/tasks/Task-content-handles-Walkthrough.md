# Task content-handles Walkthrough: short-lived content handles for media elements

Documentation-only task on branch `docs/content-handles`. TDD-ENFORCE: Not applicable - documentation update; [API-F1.3](../../apps/filesquid/crates/api/resources/plan.md) implements and tests it.

## API-S-018: Media elements read file bytes through short-lived, read-only content handles

Owner decision (2026-10-18): `<img>` and `<video>` cannot send `Authorization`, and [UI-S-014](../../apps/filesquid/ui/resources/srs.md) forbids tokens in URLs. An authenticated client therefore mints an opaque, read-only content handle for exactly one file version, and the media element loads `<api base>/api/v1/content/<handle>`, with `Range` support, on the desktop scheme and on `filesquid-server`. This resolves the Conflict Artifact recorded by [DE-F1.1](../../apps/filesquid/crates/desktop/resources/tasks/Task-DE-F1.1-Walkthrough.md) and in the S1 (a) row of [ADR 030](../architecture/030-filesquid-api-and-transports.md).

### Constants recorded

| Property | Value |
|---|---|
| Mint | `POST /api/v1/content-handles`, Bearer only, authorised exactly like the ranged content read of that file version; a handle cannot mint a handle |
| Scope | One file object, one OCFL version, one logical path; `GET` and `HEAD` only; nothing else |
| Entropy | 256 bits from the OS CSPRNG (at least 128 required), 43 base64url characters without padding, not derived from any token, key or file property |
| Storage | SHA-256 digest only, in memory only, never persisted |
| Idle timeout | 300 s, restarted at mint and at each accepted request start |
| Absolute lifetime | 3,600 s from mint |
| Expiry check | Only when a request starts; a started response is never cut off |
| Invalidation | When the issuing credential ends: desktop relaunch; server token rotation, logout, session end, restart; server policy re-evaluated at every request start |
| Bound | 1,024 live handles per issuing credential; least recently used evicted; expired removed at mint |
| Denial | One identical `403 forbidden`, `reason` `invalid_credentials`, not retryable; never 401 or 404; no file read before the handle is accepted |
| Redaction | Never logged, journaled, echoed, used as a metric label or rendered; path template `/api/v1/content/{handle}`; mint journaled as `api.content_handle.issued` without the handle |
| Headers | `Referrer-Policy: no-referrer`, `Cache-Control: private, no-store`, `X-Content-Type-Options: nosniff`, `Content-Security-Policy: default-src 'none'; sandbox` on every answer; `Accept-Ranges: bytes` and the stored media type on success |
| UI | Handle URL only as the `src` of `<img>`, `<video>`, `<audio>`, `<source>`; never in the address, history, storage, cookies, logs, rendered text, errors or a link to another window; one re-mint per media error, resuming at the same position |

### Why 300 seconds idle and 3,600 seconds absolute

A fixed 5-minute lifetime would break any video that plays for more than 5 minutes: a media element keeps issuing new `Range` requests as it plays and seeks, and through the desktop scheme each answer holds at most 16 MiB, so a 100 MB file needs at least 7 requests spread over the playback. Restarting a 300-second idle window at each accepted request keeps playback and seeking working, while an unused or leaked handle dies within 5 minutes. The 3,600-second absolute lifetime bounds how long any one handle can be kept alive by use. Because expiry is checked only when a request starts, a range read in flight is never cut off. A video paused for more than 5 minutes, or playing for more than 60 minutes, gets one refused request; the UI then mints a new handle and resumes at the same position (UI-S-014).

`Cache-Control: private, no-store` rather than a cacheable policy: every mint makes a new URL, so a cache would rarely hit, while private bytes would otherwise land in webview, browser or proxy disk caches. Media elements keep their in-memory buffer, so seeking within buffered data is unaffected; whether each webview seeks correctly under `no-store` is measured with S1 (a).

### Secondary SRS mapping

- **API-S-004: Every call MUST be authenticated and authorised before files, SQL or keys are touched.** The content route is the one other credential carrier; handles and tokens are never accepted in each other's place.
- **API-S-005: Responses MUST carry opaque handles, never raw filesystem paths or SQL.** A content handle and its URL encode no path, storage key, OCFL content path or token.
- **API-S-017: The activity journal MUST record every authorisation refusal and every credential issue.** Adds `api.content_handle.issued`.
- **DE-S-005: Every API request through the scheme MUST carry a per-launch token, and the event channel MUST sit behind the same client interface as the server's.** Exceptions named: the liveness probe and the content route; content answers pass through unchanged; no handle survives a relaunch.
- **UI-S-014: `FileSquidClient` MUST send `Authorization: Bearer <token>` on every HTTP request.** No token in URLs stays; content handles are the allowed exception and are not tokens.
- **SRS-F-056: Single FileSquid API.** New bullet for media elements and content handles.

## Changes

- [ADR 030](../architecture/030-filesquid-api-and-transports.md): Status amendment (2026-10-18T06:09:21Z); Decision 5 **Content handles for media elements**; Rationale, Assumptions, five rejected alternatives (token in the query string, cookies, `blob:` URLs, stateless signed URLs, loopback port for media), Consequences; the S1 (a) row marks the Conflict Artifact resolved and keeps the playback runs open under API-F1.3; the Open note.
- [ADR 002](../architecture/002-tauri2-desktop-shell.md) amendment note and the [ADR index](../architecture/README.md) row for ADR 030.
- [API SRS](../../apps/filesquid/crates/api/resources/srs.md): API-S-004, API-S-005, API-S-017, new API-S-018.
- [Desktop SRS](../../apps/filesquid/crates/desktop/resources/srs.md): DE-S-005. [UI SRS](../../apps/filesquid/ui/resources/srs.md): UI-S-014 and the parent note.
- [Product SRS](../srs.md): SRS-F-056 bullet and test list.
- [Architecture](../architecture.md): "One client, every host".
- [API plan](../../apps/filesquid/crates/api/resources/plan.md): API-F1.3 description, acceptance gates, agent prompt, dependencies (API-F1.7, already complete), related requirements and tests; 16 → 20 hours. [UI plan](../../apps/filesquid/ui/resources/plan.md): UI-F2.2 description, dependency note and related requirements (estimate unchanged).
- [Implementation plan](../implementation-plan.md): Task FS-1.2 description, 32 → 36 hours; phase F1 152 → 156; total 2,597 → 2,601; MVP 758 → 762.
- [DE-F1.1 walkthrough](../../apps/filesquid/crates/desktop/resources/tasks/Task-DE-F1.1-Walkthrough.md): open item marked resolved with the date and links.
- BDD: [`content_handles_F-056.feature`](../../apps/filesquid/crates/api/tests/features/content_handles_F-056.feature) (26 scenarios counting outline examples after review round 1, skipped until API-F1.3 adds step definitions), and three UI scenarios in [`client_authorisation_UI-S-014.feature`](../../apps/filesquid/ui/tests/features/client_authorisation_UI-S-014.feature) for content-handle previews.

The PRD needed no change: F-056 already requires deny-by-default authorisation and opaque handles, and a content handle is minted under that authorisation. No README change: no command, service or environment variable was added.

## Sync Execution Report

Owner decision → ADR 030 Decision 5 (and ADR 002 note, ADR index) → API-S-004, API-S-005, API-S-017, API-S-018, DE-S-005, UI-S-014 → SRS-F-056 → PRD F-056 checked, unchanged → architecture narrative → API-F1.3, UI-F2.2, Task FS-1.2 and the summary timeline → DE-F1.1 walkthrough → BDD feature.

## Verification

`bash scripts/pre-commit.sh` exit 0 (`CARGO_TARGET_DIR` shared, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, after `npm ci` in both UI packages and touching the crate sources). The gate summary lines, copied verbatim from the full run (several thousand lines of test logs), and the `filesquid-api` cucumber summary from the same run:

```text
========================================
========================================
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
✅ Document Sync Validation passed.
✅ Traceability Validation passed: 95 PRD requirements, 81 resolved SRS references.
✅ Portable Documentation validation passed.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
✅ ./apps/filesquid is properly formatted.
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ FileSquid and ImageSquid document boundary passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Clippy passed.
✅ Workspace tests passed.
✅ Module crate alone: filesquid-core passed.
✅ Module crate alone: filesquid-ocfl passed.
✅ Module crate alone: filesquid-metadata passed.
✅ Module crate alone: filesquid-c2pa passed.
✅ Module crate alone: filesquid-api passed.
✅ Rustdoc passed.
✅ Clippy (desktop, test-instrumented) passed.
✅ Desktop tests (test-instrumented) passed.
✅ ESLint passed.
✅ TypeScript passed.
✅ Vitest passed.
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
✅ ESLint passed.
✅ TypeScript passed.
✅ Vitest passed.
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed.
========================================
✅ All Pre-Commit Hooks Passed!
========================================

filesquid-api cucumber (tests/features, includes content_handles_F-056.feature):
[Summary]
7 features
106 scenarios (28 passed, 78 skipped)
180 steps (102 passed, 78 skipped)
```

## Review round 1 (Cubic)

All 13 findings were valid and fixed:

1. API-F1.3 acceptance named "other-file handles", which is not an invalid-handle case. It now lists unknown, malformed, expired, evicted, revoked and policy-failed handles, a proof that a handle serves only its own file version, and `HEAD` answers.
2. UI-F2.2 had no scenario for the media-error re-mint. Three scenarios were added to `client_authorisation_UI-S-014.feature` (single re-mint, resume position and paused state, error after a failed retry) and linked from UI-F2.2.
3. ADR 030 Consequences stated native playback as settled. It is now the expected outcome, to be confirmed by S1 (a).
4. There was no server-host scenario. A `filesquid-server` outline now covers logout, session end, token rotation, restart and permission withdrawal.
5. The refusal scenarios did not assert `api.auth.denied`. They now do.
6. There was no `HEAD` scenario and no check of the success headers. Added, with `Accept-Ranges: bytes` and the stored media type.
7. The unsatisfiable example did not assert `416`. A dedicated scenario now asserts `416` and `Content-Range: bytes */104857600`.
8. The DE-F1.1 walkthrough still read as open. The old paragraph is labelled historical, and only the playback measurements are said to remain open.
9. The ADR index row is now dated in its Status column, with ADR 030's RFC 3339 timestamp.
10. The architecture narrative now says that the client prefixes the host's API base.
11. The ADR 030 rationale now gives the real exposure: at most 60 minutes, and beyond 5 minutes only while the handle is in use.
12. This walkthrough now says "60 minutes", not just "60".
13. `HEAD` cannot carry a body. ADR 030 and API-S-018 now require identical status, headers and `ApiError` fields for every denial: the same body for `GET`, and an empty body for `HEAD`.

`bash scripts/pre-commit.sh` exit 0 after the fixes. The `filesquid-api` cucumber summary from that run (8 new scenarios, all skipped until API-F1.3):

```text
[Summary]
7 features
114 scenarios (28 passed, 86 skipped)
188 steps (102 passed, 86 skipped)
```

## Mocked Environment Variables

None.
