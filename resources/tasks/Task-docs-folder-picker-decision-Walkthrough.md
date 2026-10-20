# Task docs-folder-picker-decision: Walkthrough

## Owner decision of 2026-10-20: how the desktop owner picks a library folder

Documentation-only task. No code, gate script or rule changed. It records the owner's decision of 2026-10-20 on how the desktop owner chooses the folder of a library, so that Task DE-1.1 can build it.

The owner's decision, verbatim: "go with A with rfd"

### Option A as presented to the owner

- Add an owner-only API route `POST /api/v1/libraries/location-choices` with the body `{purpose: create | open | copy_destination}` under the action `library.manage`.
- Through a new host port, `LocationChooser` (API side in `filesquid-api`), the desktop host shows a native folder dialog from Rust and answers an opaque `location_id`, or `cancelled`. `filesquid-server` answers `503 unavailable` until it has its own location registry.
- The UI then calls the existing library routes of API-F1.12 with that ID.
- Costs: a minor API contract bump and one new desktop dependency, `rfd`. It is MIT-licensed and is the crate `tauri-plugin-dialog` wraps. It is not a Tauri plugin, so the DE-S-002 plugin check still holds, and the webview never sees a path.
- WebDriver cannot drive a native dialog, so the packaged-window e2e registers its folders through an environment variable that only `test-webdriver` builds read, like `FILESQUID_E2E_LIBRARY`.
- Rejected: (B) native app menu items, which break API-S-001 reachability; (C) no picker in DE-1.1.

### Dependency verification (command-verification.md rule 3)

```
$ cargo search rfd --limit 3
rfd = "0.17.2"                    # Rusty File Dialog
brisk-rfd-components = "0.8.1"    # Components for rfd.
deka-rfd = "0.17.2"               # deka's fork of rfd 0.17.2 (PolyMeilex/rfd) adding fallible try_pick_file/try_save_file. Impor…
```

The crates.io API (`/api/v1/crates/rfd`) lists 0.17.2 as the newest stable version, published 2026-01-12, not yanked, licence `MIT`, repository `https://github.com/PolyMeilex/rfd`. The newest `tauri-plugin-dialog` (2.8.3) depends on `rfd ^0.16` for desktop targets, which confirms that the plugin wraps `rfd`. `rfd` is not yet in `apps/filesquid/Cargo.lock`. DE-1.1 pins it when it adds the dependency.

## Files changed

| File | Change |
|---|---|
| [ADR 002](../architecture/002-tauri2-desktop-shell.md) | Dated amendment in Status; a **Native folder choice (owner decision 2026-10-20)** decision; an assumption about `rfd` on the three desktops; `tauri-plugin-dialog`, option B and option C under Alternatives Considered; consequences; related links. The nine ISO 42010 items are kept |
| [ADR 030](../architecture/030-filesquid-api-and-transports.md) | Dated amendment in Status; **Library location choices (owner decision 2026-10-20)** under Decision 1 (route, body, action, `LocationChooser` port, `503 unavailable` on the server, minor contract bump); an assumption; options B and C under Alternatives Considered; consequences. The nine ISO 42010 items are kept |
| [ADR index](../architecture/README.md) | Rows 002 and 030 name the 2026-10-20 amendment |
| [API SRS](../../apps/filesquid/crates/api/resources/srs.md) | New **API-S-034 Library location choices**; API-S-025 says a folder reaches the library routes only as a host-registered location ID obtained through API-S-034 |
| [Desktop SRS](../../apps/filesquid/crates/desktop/resources/srs.md) | DE-S-002 says the dialog is shown by the Rust host through `rfd`, adding no capability, permission or command; new **DE-S-017 Native library folder choice** (rfd dialog, location registration, `cancelled`, no path to the webview, answers, errors or journal, `test-webdriver`-only e2e variable) |
| [Product SRS](../srs.md#srs-f-001-independent-local-library-and-ownership) | SRS-F-001: folders reach the library routes only as opaque location IDs, chosen on the desktop in a native dialog the host shows |
| [Desktop plan](../../apps/filesquid/crates/desktop/resources/plan.md) | DE-1.1 delivers the route, the `LocationChooser` port and the `rfd` backend; related requirements and agent prompt updated (`-p filesquid-api` added) |
| [API plan](../../apps/filesquid/crates/api/resources/plan.md) | API-F1.12 status notes that DE-1.1 adds the route and port to this crate |
| [UI plan](../../apps/filesquid/ui/resources/plan.md) | UI-1.1 calls the route, then the API-F1.12 route with the answered ID; handles `cancelled` and `503 unavailable`; depends on DE-1.1 |
| [Implementation plan](../implementation-plan.md) | Task 1.1 description names the decision and the DE-1.1 delivery |

[resources/implementation-plan-checklist.md](../implementation-plan-checklist.md) is unchanged because no box text changed. The PRD is unchanged because it does not say how a folder is chosen.

## Test scope

The hook's change-aware scope, from `bash scripts/changed_scope.sh` against `origin/main`:

```
rust:filesquid-api,filesquid-desktop,filesquid-native,filesquid-server npm:apps/filesquid/ui,apps/imagesquid
```

Crate and package resources count as the crate or package, so the scope includes `rust:`. The full suite was therefore run (`FULL=1 bash scripts/pre-commit.sh`, with a shared Cargo target, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and a private `TMPDIR`). Its summary line on the round-1 fixes (the first commit's run passed the same way, in 1067s):

```
✅ All Pre-Commit Hooks Passed! Test scope: full (433s)
```

## Cubic review round 1

- API-S-034 now says the route is a host interaction rather than a library command, so it takes no `CommandEnvelope`. A folder that is already registered is answered with its existing location ID, so retries do not pile up registrations.
- DE-S-017 and ADR 002 now require the dialog's parent only where the backend supports one. On Linux, only the XDG portal does; `rfd` can fall back to zenity.
- UI-1.1 passes the ID as `destination_location_id` to `snapshot-copies`, names Playwright coverage for the folder flow, and lists API-S-034. Global Task 1.1 lists API-S-034 and DE-S-017.
- DE-1.1 says it spans `filesquid-api` and `filesquid-desktop` as a follow-up to API-F1.12. Its test prompt covers redaction of paths, storage keys, folder names and content.
- ADR 030's consequence now limits the claim to hosts that implement `LocationChooser`.

## Sync Execution Report

- **ADR → SRS:** ADR 030's route and port went into API-S-034 and API-S-025 (API SRS). ADR 002's `rfd` dialog went into DE-S-002 and DE-S-017 (desktop SRS) and into SRS-F-001 (product SRS).
- **SRS → PRD:** checked [PRD F-001](../prd.md#f-001-independent-local-library-and-ownership), [F-009](../prd.md#f-009-multiple-libraries-and-filesystem-administration) and the Technology Stack section. None of them says how a folder is chosen, so the PRD is unchanged. The root README "About this project" section is unchanged because the vision and PRD are unchanged.
- **SRS → plans:** DE-1.1, API-F1.12, UI-1.1 and global Task 1.1. Requirements stay timeline-free: API-S-034 and DE-S-017 name no task or phase.
- **UI SRS:** no UI-S row says how a library folder is chosen (UI-1.1 has no UI-S row), so it is unchanged.
- **BDD scenarios:** no feature file changed, because this task makes no code or test changes. Task DE-1.1 writes the API-S-034 and DE-S-017 scenarios with `@feature-writer`.
- **Conflicts:** none found. Option A keeps ADR 002's no-TCP-listener rule and its capability policy, and keeps DE-S-006 (no path in the webview) and API-S-005 (no path in answers).

## Mocked Environment Variables

None.
