# Task API-F2.2 Plan: View operation, preferences, storage use, copy, move and ZIP download (part 1)

Branch `feat/api-f2.2-views-copy-move-zip`. Module `filesquid-api` ([plan row](../plan.md)), part of global Task FS-2.5. Scope agreed with the orchestrator on 2026-10-20: this PR delivers parts (1) to (4) plus the listing by navigation node in the API-F1.3 path model. The rest stays open, and the plan box stays unticked.

## Requirements

[API-S-011](../srs.md), [SRS-F-062](../../../../../../resources/srs.md#srs-f-062-drive-style-views-sorting-selection-and-layout-preferences), [UI-S-017](../../../../ui/resources/srs.md), [API-S-021](../srs.md) (node handles), [API-S-005](../srs.md), [API-S-027](../srs.md) (download names), [NFR-003](../../../../../../resources/prd.md#nfr-003-bounded-processing-and-migration-endurance).

## Constraints copied from the SRS

1. **Preferences**
   - Store per user through the API, and restore on every host the user signs in to:
     - start view (`home` or the personal library root);
     - sort key (`name`, `size`, `modified`, `modified_by_me`, `opened_by_me`) and direction;
     - folder placement (`on_top`, `mixed`);
     - layout (list, grid);
     - hover-card setting.
   - Include the navigation tree's pinned destinations, section order and collapse state (UI-S-017):
     - `version` 1;
     - at most 100 pinned destinations, each an opaque node handle and its kind;
     - the section order;
     - at most 1,000 collapsed node handles.
   - No name, title, logical path, token or content handle is stored. The UI's own bounds are 16 section ids of 1 to 64 characters and handles of at most 512 bytes.
2. **Storage use** per library, including history: `GET /files/library-history-size` (API-F2.5, PR #99) already reports the bytes a library keeps with history. This PR documents it as the storage-use route.
3. **Whole-file download.** Stream any size from ranged reads, never holding the file in memory (NFR-003). Answer with `Content-Disposition: attachment` and the display name (API-S-027 via the host's `DisplayNames`).
4. **Listings** must not show the reserved `provenance/` (PROV-O records) and `.filesquid/` (application data) folders; listing inside them is `not_found`.
5. **Listing by navigation node** (UI-F2.1b):
   - `GET /files/listing?node=<handle>` answers a `FolderPage` for a collection or folder node, through the host's mapping of a folder scope to its OCFL object.
   - The node must be readable to the caller through the navigation service: an unknown, unreadable or malformed node is `not_found`.
   - Library, virtual collection and repository nodes are refused (`invalid_request`); their files come from the view operation.
   - Exactly one of `node` or `library_id` + `object_id` must be given.
6. **Upload contract (9)** is a real conflict: write a Conflict Artifact.

## Remaining after this PR

- (5) Copy: waits on the record-codec decision (the new FileId's metadata record).
- (6) Listing by library and virtual-collection node in the canonical per-file model: waits on the record codec.
- (7) The view operation's per-file rows: waits on the record codec.
- (8) ZIP download with the outgoing policy per member: waits on an F-046 outgoing policy engine.
- (10) Storage use per collection: waits on (6)'s mapping.

## Verification

- Integration tests `tests/integration/drive.rs` on a real filesystem root.
- BDD feature `tests/features/preferences_downloads_listings_API-S-011.feature`.
- Contract drift tests.
- fmt, clippy, tests, rustdoc; regenerate `schema.d.ts`; `FULL=1 bash scripts/pre-commit.sh`.
