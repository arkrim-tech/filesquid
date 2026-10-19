# Task docs-record-owner-decisions Walkthrough: three owner decisions recorded

Documentation-only task on branch `docs/record-owner-decisions`. TDD-ENFORCE: Not applicable - documentation update; no code changed.

## Owner decisions (2026-10-18)

1. The metadata `ocfl` store versions per edit: each metadata edit commits a new OCFL version through `filesquid-ocfl` (the IO-0.10 change model and write path) instead of rewriting `metadata.md` whole. Earlier versions are never modified.
2. The metadata `sidecar` store writes one `<filename>.metadata.md` beside each plain file, not one `metadata.md` per folder.
3. Until a host serves per-user preferences (API-S-011, delivered by API-F2.2 and UI-F2.8), several users of one `filesquid-server` who share a browser profile share its device-kept navigation pins. This is accepted.

## Changes

- [Metadata module plan](../../apps/filesquid/crates/metadata/resources/plan.md), Task MD-0.2 Description: the `ocfl` store MUST version per edit through the IO-0.10 change model and MUST NOT modify earlier versions. "MUST NOT reimplement inventories or `vN` publication" is kept. The row has no acceptance sentence, so none was added.
- [Metadata SRS](../../apps/filesquid/crates/metadata/resources/srs.md) MD-S-008: names the sidecar file `<filename>.metadata.md` (one per plain file) and states that with the `ocfl` store each edit MUST be a new OCFL version and MUST NOT rewrite an earlier one, matching ADR 027 Decision 2.
- [ADR 027](../architecture/027-filesquid-metadata-pluggable-stores.md) Decision 2: the `sidecar` bullet now reads one `<filename>.metadata.md` beside each plain file, matching the [metadata crate README](../../apps/filesquid/crates/metadata/README.md). A dated amendment note (2026-10-18, owner decision) is added under Related Decisions / Notes.
- [Product vision](../product-vision.md): the metadata module's store list and U-57 now name the sidecar file `<filename>.metadata.md`. The `ocfl` store's in-object `metadata.md` is unchanged.
- [UI SRS](../../apps/filesquid/ui/resources/srs.md) UI-S-017: one sentence accepting that a browser profile shared by several `filesquid-server` users shares the device-kept navigation pins until per-user preferences exist.
- [UI README](../../apps/filesquid/ui/README.md) "Navigation choices": the same note, so the package docs stay consistent. The UI plan rows UI-F2.1a, UI-F2.1b and UI-F2.8 already describe device storage and its move to the API, so they are unchanged.

No other document said one `metadata.md` per folder for the `sidecar` store (searched `resources/` and the metadata crate docs for `metadata.md`, "per folder", "beside" and "next to").

## Hook

The change touches files inside `apps/filesquid/crates/metadata/` and `apps/filesquid/ui/`, which belong to that crate and package, so the change scope is `rust:filesquid-cli,filesquid-desktop,filesquid-metadata npm:apps/filesquid/ui,apps/imagesquid`, not `docs`. The rule for a scope that includes `rust:` is a full-suite run: `FULL=1 bash scripts/pre-commit.sh`.

Raw step lines of `FULL=1 bash scripts/pre-commit.sh` on the branch after merging `origin/main` (2026-10-19 UTC), exit 0. Summary line: `✅ All Pre-Commit Hooks Passed! Test scope: full (227s)`. An earlier run on 2026-10-18 failed at Clippy on `crates/api/src/events.rs` (a Rust 1.99 deprecation in unchanged code); PR #64 on `main` fixed it.

```text
changed_scope: full (FULL=1 is set)
Running Pre-Commit Hooks... (test scope: full)
✅ Branch Validation passed.
Starting Port Isolation Validation...
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
Starting Document Sync Validation...
✅ Document Sync Validation passed.
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 95 PRD requirements, 82 resolved SRS references.
Starting Portable Documentation Validation...
✅ Portable Documentation validation passed.
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
✅ ./apps/filesquid is properly formatted.
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
Starting Documentation Validation...
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ FileSquid and ImageSquid document boundary passed.
✅ MVP queue and merge driver tests passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (166 assertions).
Starting Lint and Test Validation...
ℹ️  Test scope: full
✅ Clippy passed (40s).
✅ Workspace tests passed (55s).
✅ Module crate alone: filesquid-core passed (1s).
✅ Module crate alone: filesquid-ocfl passed (22s).
✅ Module crate alone: filesquid-metadata passed (0s).
✅ Module crate alone: filesquid-c2pa passed (4s).
✅ Module crate alone: filesquid-api passed (16s).
✅ Module crate alone: filesquid-native passed (2s).
✅ Clippy (filesquid-metadata, all features) passed (0s).
✅ Tests (filesquid-metadata, all features) passed (3s).
✅ wasm32 build (filesquid-metadata, default features off) passed (1s).
✅ wasm32 build (filesquid-metadata, ocfl feature) passed (0s).
✅ Rustdoc passed (4s).
✅ Clippy (desktop, test-instrumented) passed (11s).
✅ Desktop tests (test-instrumented) passed (23s).
✅ ESLint passed (3s).
✅ TypeScript passed (3s).
✅ Vitest passed (12s).
✅ ESLint passed (1s).
✅ TypeScript passed (3s).
✅ Vitest passed (2s).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (227s)
```
