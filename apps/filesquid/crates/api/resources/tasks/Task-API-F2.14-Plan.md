# Task API-F2.14 Plan: Home collections for uploads, copies and moves

Module `filesquid-api` ([plan row](../plan.md)), part of global Task FS-2.5. Binding sources: [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md) Decisions 3, 4, 5 and 12; [API-S-011](../srs.md) **Destination** and **Same logical path**; [API-S-019](../srs.md); [API-S-021](../srs.md); [API-S-026](../srs.md); [API-S-036](../srs.md); [DE-S-009, DE-S-010](../../../desktop/resources/srs.md); PRD [F-004](../../../../../../resources/prd.md#f-004-collections-folders-and-navigation), [F-062](../../../../../../resources/prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-056](../../../../../../resources/prd.md#f-056-single-filesquid-api).

## Current state (mapped 2026-10-30)

- None of upload, copy, moves, renames or folders carries `collection_id`/`folder`, refuses `destination_required`/`collection_not_found`, numbers a taken path, registers the file in the collection snapshot, or publishes `navigation.change`.
- `FileService` (this crate) has no catalog port. The one registrar is `SqliteCatalog::register_file` in `filesquid-native`, used by the import job only.
- Journal names are `files.upload`, `files.copy`, `files.move`; the spec names `file.upload`, `file.copy`, `file.move`.

## Granular constraints (SRS and ADR)

1. Upload, copy and move MUST name `collection_id` (required) and MAY name `folder`, a collection logical path, not in or under `provenance` or `.filesquid` (API-S-011 **Destination**).
2. No `collection_id`: `400 invalid_request`, code `destination_required`, before any byte is staged, journaled once.
3. Unknown collection or one of another library: `404 not_found`, code `collection_not_found`, journaled once.
4. No default collection and no upload or copy to a library's top level.
5. Each upload and copy MUST register the file in the snapshot with its home collection, the logical path `<folder>/<name>` and its object ID, under the request's operation ID (API-S-026).
6. The last segment is the display name after the API-S-005 safe transformation, at most 255 bytes (for an upload the upload name; for a copy the source's display name).
7. A move publishes one snapshot version and rewrites no bytes; it keeps its name.
8. Same logical path: a taken path, a recoverably deleted file's included, MUST NOT be refused or overwritten; the new item takes `-N` (folder at the end; file before the first dot; dotfile keeps its leading dot), first free N from 1.
9. At most 9,999 numbers (`MAX_SUFFIX`); then `409 conflict`, code `path_suffix_exhausted`, no change, journaled `failed`.
10. A numbered segment over 255 bytes: `invalid_request`, code `name_too_long`.
11. The path used MUST be answered and journaled (as an opaque registration, never the path in the journal: API-S-005 forbids names in journal entries; the answer carries it).
12. Each operation journaled once (`file.upload`, `file.copy`, `file.move`) with opaque IDs; each refusal once with its code.
13. Each success publishes `navigation.change` for the parents it changed (API-S-021).
14. A crash between the file version and the snapshot version leaves the operation pending; a retry under the same operation ID completes it once (API-S-019, DE-S-010).
15. Contract minor bump, regenerated `ui/src/api/schema.d.ts`.
16. Close the API-F2.2 remainder "registering a copy's home collection in the catalog".

## Approach

- New port `files::HomeCollections` in this crate: register a file in a collection at a path under an operation ID and answer the path used or a refusal code; look up the file a pending operation committed. `MemoryHomeCollections` is its reference implementation for tests and hosts without a catalog; `filesquid-native`'s `SqliteCatalog` implements it over `register_file`, and the desktop installs it.
- The destination check (`collection_in`) moves from `imports/routes.rs` to a shared module used by the import, upload and copy routes.
- Upload and copy: check the destination first, then commit the object, then register; journal `file.upload`/`file.copy`; publish `navigation.change`.
- Part 2 (moves, renames, folders) waits on the orchestrator's answers on the route shapes.

## Verification

Failing tests first through the router (unit and integration over `FilesRig` with `MemoryHomeCollections`), the feature `home_collections_API-S-011.feature`, native unit tests for the adapter, `FULL=1 bash scripts/pre-commit.sh`.
