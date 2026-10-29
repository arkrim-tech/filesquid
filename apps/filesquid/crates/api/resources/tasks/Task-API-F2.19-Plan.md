# Task API-F2.19 Plan: Command envelopes on the library routes

Branch `feat/api-f2.19-library-envelopes`, module `filesquid-api` ([plan row](../plan.md)), with the default UI's library callers (`apps/filesquid/ui`, UI-1.1 and UI-1.3) and the desktop host's route tests moved to the envelope in the same change. Global Task FS-2.5. Owner decision 1 of 2026-10-23 (adopted by "provide and implement recommendations for owner's decisions 1-6").

## Traceability

- PRD [F-056](../../../../../../resources/prd.md#f-056-single-filesquid-api), SRS [SRS-F-056](../../../../../../resources/srs.md#srs-f-056-single-filesquid-api)
- [API-S-002](../srs.md), [API-S-019](../srs.md), [API-S-025](../srs.md), [API-S-034](../srs.md), [API-S-038](../srs.md), [UI-S-012](../../../../ui/resources/srs.md), [CORE-S-005](../../../core/resources/srs.md), [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md)
- Features: [`library_and_repair_routes_API-S-025.feature`](../../tests/features/library_and_repair_routes_API-S-025.feature), a new `library_command_envelopes_API-S-002.feature`, [`versioned_api_contract_F-056.feature`](../../tests/features/versioned_api_contract_F-056.feature), [`host_adapter_UI-S-012.feature`](../../../../ui/tests/features/host_adapter_UI-S-012.feature)

## Granular constraints (verbatim or near-verbatim from the SRS and the plan row)

| # | Constraint | Source |
|---|---|---|
| C1 | `POST /api/v1/libraries` (create), `/libraries/openings` (open), `/libraries/snapshot-copies` (copy), `/libraries/rebuilds` (rebuild) and the rename route API-F2.18 adds take a `filesquid-core` `CommandEnvelope` (schema version 1, non-empty operation ID, expected revision, scope) and answer a `CommandOutcome` (`committed`, `pending`, `conflict` or `error`) for the same operation ID | API-S-002, plan row |
| C2 | A conflict is HTTP 409 with a `conflict` outcome | API-S-002 |
| C3 | A repeated operation ID of a command that changed something answers its first outcome with no second change; the stored receipt is answered before the revision is checked | API-S-002, API-S-019 |
| C4 | A command that changed nothing stores no receipt | API-S-019 |
| C5 | Create-library sends expected revision `0`, and its commit must find the library absent; any other command whose expected revision is not the library's current revision is refused as `conflict` with no change | API-S-019, plan row |
| C6 | The rebuild route keeps answering its `library.rebuild` job as the `pending` outcome, so a repeated operation ID answers the same job; a rebuild request for a library whose job is unfinished answers that job and submits none | plan row, API-S-025 |
| C7 | The location-choices route (a dialog, not a mutation) and the repair route (consent carries its idempotency through the repair run ID) keep their bodies: not envelope routes; the job controls of API-S-037 take no envelope | API-S-002, API-S-034, API-S-023 |
| C8 | Envelope: protocol schema version, operation/idempotency key, expected revision and permitted scope | CORE-S-005 |
| C9 | Every command carries a `CommandEnvelope`; a 2xx answer resolves to a `CommandOutcome` for the same operation id; HTTP 409 resolves to `conflict`; any other non-2xx rejects with `ApiError`; only `committed` is completion evidence | UI-S-012 |
| C10 | Contract minor version bump, regenerated `ui/src/api/schema.d.ts`, the default UI's callers of these routes moved to `FileSquidClient.command()` so no client is left on plain bodies | plan row, API-S-002 |
| C11 | Every refused call journaled exactly once with its error code; answers carry opaque IDs only | API-S-025, API-S-017 |
| C12 | The preferences and the metadata set/remove routes keep their plain bodies pending the owner's open decision; the envelope helper stays reusable for them | orchestrator instruction 2026-10-29 |

## Design

- **Revision of a bound library.** The library routes have had no revision to compare so far. A library bound on this host (created or opened) is at revision `1`; later library commands that change the bound library (the rename of API-F2.18) advance it, and `LibraryView` answers it as `revision`. Expected revision rules: create `0` (the library must not exist); open `0` for a library not open on this host, or its current revision when it is already open (which answers the library again); copy and rebuild the library's current revision. Anything else is `conflict` (C5).
- **Scope.** `location:<location_id>` for create and open (they bind a location; the library ID may not exist yet), `library:<library_id>` for copy, rebuild and rename.
- **Receipts.** The in-memory `CommandReceipts` of API-F2.13 (one store per host process; an operation ID is one key across every command route). Create, a fresh open and a copy change something and store their answer; a repeated open of an open library, a rebuild that answered an unfinished job, and every refusal store none (C3, C4). A new rebuild job carries the envelope's operation ID and its `pending` answer is stored, so a repeat answers the same job (C6).
- **Helper.** A new crate module `envelope` holds the generic checks and answers (body shape, envelope checks against a route-given scope, claim, committed/pending/conflict answers), so the files routes can move onto it later and the preferences and metadata routes can take it in a small follow-up (C12). The files routes are not changed in this task.
- **UI.** `ClientLibrarySource` sends each of the four commands through `FileSquidClient.command()`; `LibraryView.revision` is kept in the registry and sent as the expected revision of copy and rebuild (a library this page did not open is at the initial revision until UI-1.3 reads the facts). A `conflict` outcome is raised as the `ApiError` the views already word; `rebuild()` resolves to the pending job answer (UI-1.3 owns the job progress UI).
- **Desktop.** The desktop host's route tests (integration, cucumber steps, WebdriverIO spec) send envelopes; no desktop code changes.

## Files

- `crates/api/src/envelope.rs` (new), `src/lib.rs` (module, `command_receipts()`)
- `crates/api/src/local_library/{routes,commands,locations,rebuild_job,mod}.rs`
- `crates/api/openapi.json`, `src/lib.rs` (`API_VERSION`), `tests/unit/contract.rs`
- `crates/api/tests/integration/library_routes.rs`, `tests/integration/library_commands.rs` (new), `tests/support/{library_routes_fixture,library_route_steps,library_contract}.rs`, `tests/unit/rebuild_job.rs`, `tests/features/library_command_envelopes_API-S-002.feature` (new), `tests/features.rs`
- `crates/desktop/tests/{integration/library_routes.rs,support/library_steps.rs,e2e/specs/libraries.e2e.mjs}`
- `ui/src/libraries/{client-source,model,contract,registry}.ts`, `ui/src/components/fs-library-{start,detail}.ts`, `ui/src/api/{schema.d.ts,contract-version.ts}`, `ui/tests/unit/library-client.test.ts`, `ui/tests/support/library-fakes.ts`, `ui/tests/ui/library-*.test.ts`, `ui/tests/e2e/support/library-api.ts`, `ui/tests/e2e/preview/library.spec.ts`, `ui/README.md`
- `crates/api/resources/{plan,srs}.md`, `resources/implementation-plan-checklist.md`

## Verification

TDD: the new integration test file and the changed route tests fail first against the plain-body routes. Then `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test -p filesquid-api --locked`, `cargo test -p filesquid-desktop --locked`, `cargo doc --workspace --no-deps`; in `ui/`: `npm run contract:types`, `npx eslint .`, `npx tsc --noEmit`, `npx vitest run`, Playwright `library.spec.ts` against the production build; `FULL=1 bash scripts/pre-commit.sh` on the head commit.
