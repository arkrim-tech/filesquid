# Task API-F2.18 Plan: Library facts: name, free space and recent libraries

Row: [Task API-F2.18](../plan.md), dispatched by global Task 1.2. Engineer role: platform engineer. Crates: `filesquid-api` (routes, port, contract), `filesquid-native` (the desktop's location registry implements the port), `filesquid-desktop` (wiring).

## Constraints copied from the SRS

From [API-S-038](../srs.md) (owner answer 10, 2026-10-22, "yes add library facts to v3"):

- A library MUST have a name, set when it is created and renamable by the owner, journaled.
- Name rules of API-S-019: 1 to 1,024 bytes, not blank; API-S-005 safe (no control characters; never a path in an answer, error or journal entry).
- `GET /api/v1/libraries/{library}/facts`, action `library.read`, answers `{library_id, name, free_bytes, free_bytes_reason}`: the free bytes of the volume holding the library's storage root, readable by anyone who may read the library (not admin-only).
- `free_bytes` is `null` with `free_bytes_reason` `volume_unreadable` or `not_local` when the volume cannot be read; otherwise `free_bytes_reason` is `null`.
- An unknown library, or one the caller may not read, is HTTP 404 `not_found`.
- `GET /api/v1/libraries/recent`, action `library.read`: first keep only libraries this host opened that the caller may read, then list at most 20, newest first, each with library ID, name, last opened time (RFC 3339 UTC), whether open now, and free bytes when its folder is reachable; never a path, storage key or location ID.
- The routes, fields and codes are in the contract under a minor version bump: main is 0.25.0, so 0.26.0.

From [API-S-004](../srs.md), [API-S-017](../srs.md), [API-S-025](../srs.md): each route declares its action and the guard runs first; every refusal is journaled once; the library routes call the application layer and do no file or SQL access themselves; hosts inject what they own.

## Owner decisions (2026-10-23, verbatim answer "provide and implement recommendations for owner's decisions 1-6")

- Decision 5: "Unavailable for the MVP, never invented, with no new crate: outside Linux and macOS the host records every platform fact as unavailable ... and answers free bytes as null with free_bytes_reason volume_unreadable (API-S-038). The Win32 reads are added by DE-F3.3". Free bytes use only crates already in the lockfile on Linux and macOS: `fs4::available_space` (already a dependency of `filesquid-native`).
- Decision 1: the library rename route takes a plain body now; API-F2.19 adds the envelope to the library routes later, so each route changes once.

## Dependency cycle with DE-1.8 (orchestrator, 2026-10-23)

This task defines the API side (the name at create and rename, the facts and recent routes, the host port that answers recent libraries and free bytes, and `filesquid-server`'s answer) and the desktop implementation of that port over the existing location registry (DE-1.1's `HostLocations`), keeping the name and last-opened time per library for the host's lifetime. DE-1.8 later adds what its row still claims (keeping the registry across launches). Recorded in both rows' Status and the walkthrough.

## Design

- `LocationRequest` gains an optional `name`. Create and open keep the name given; a library first seen without one is named `Untitled library`, so every library has a name. A name that breaks the rules is refused as `400 invalid_request` with the code `invalid_name`, before any binding.
- `POST /api/v1/libraries/renames` (`library.manage`, plain body `{library_id, name}`) renames a library this host opened; journaled `library.rename` with the library ID only; a library this host never opened is `404 library_not_found`; renaming to the current name changes nothing, answers the facts and is not journaled (owner decision 2026-10-19). Answers the facts.
- `GET /api/v1/libraries/{library}/facts` and `GET /api/v1/libraries/recent` as above. Readability: the installed navigation access (`NavigationAccess::may_read` of the library node), else the host access of this release.
- Port `LibraryRegistry` (in `filesquid-api`): remember a bound library, rename, one entry, all entries, free space of a location. `RegistryBook` keeps the per-library entries in memory, bounded at 256, forgetting the least recently opened. `MemoryLibraryRegistry` is the book with an injected free-space function (tests). The desktop's `HostLocations` holds a book and answers free space from its folders.
- `filesquid-server` installs no library commands, so the three routes answer `503 unavailable` there, as the other library routes do.

## Verification

Failing tests first (api integration, native unit, desktop integration, BDD), then the contract, `npm run contract:types`, fmt, clippy, tests, and `FULL=1 bash scripts/pre-commit.sh`.
