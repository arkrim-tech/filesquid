# Task API-F2.21 Plan: View and search row facts, the Recent view and destination identifiers

Global task FS-2.5. Requirements: [API-S-011](../srs.md) **Row facts**, **Recent** and **Destination**, [API-S-012](../srs.md), [API-S-021](../srs.md) **Destination**, [API-S-014](../srs.md) **History size**, [API-S-009](../srs.md); PRD [F-062](../../../../../../resources/prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-063](../../../../../../resources/prd.md#f-063-file-search-query-language-and-filter-chips), [F-004](../../../../../../resources/prd.md#f-004-collections-folders-and-navigation). Scenarios: [`row_facts_recent_and_destinations_API-S-011.feature`](../../tests/features/row_facts_recent_and_destinations_API-S-011.feature).

## Constraints copied from the SRS

1. Every file row of `GET /api/v1/files/view` and `GET /api/v1/files/search` carries `library_id`, the head version, and the home `collection_id` with that collection's title; both are `null` for a file registered in no collection.
2. Every folder row carries the number of files and the bytes beneath it, counted within the bounds of one view (10,000 files), and is marked incomplete when a bound or an unreadable file stopped the count.
3. A collection or folder row carries the `collection_id` of its collection.
4. `view=recent`: the files the caller changed, opened or uploaded, newest first; each row carries its action kind (`changed`, `opened` or `uploaded`) and its RFC 3339 UTC time; computed only from the caller's own succeeded journal entries; at most 20 journal pages per library. The views home, personal, shares, favorites and tags stay refused.
5. One view sorts at most 10,000 files and 1,000 folder rows and answers `complete: false` beyond that; cursors are refused as `invalid_request` for other parameters and `conflict` when the rows changed.
6. Navigation nodes of kind `collection`, and `folder` nodes inside a collection, carry `collection_id`; every other kind carries `null`.
7. `GET /api/v1/navigation/destination?node=<handle>` (action `navigation.read`) answers `library_id` and `collection_id` for a collection node, and also `folder` (the logical folder path inside the collection) for a folder node. Any other node kind is `invalid_request`. An unknown node, a malformed handle, an unreadable node, a node under an unreadable ancestor and a node that no longer exists all get the same `not_found` answer as the listing.
8. `GET /api/v1/files/library-history-size` answers `history_bytes` beside its total: the bytes held only by versions other than each object's head version, each distinct content once; within 10,000 objects and 100,000 contents, with the same `truncated` flag; zero for a library with one version per object.
9. `ApiError` carries a machine-readable sub-code beside the HTTP status and `code` for `destination_required` (400), `collection_not_found` (404), `path_suffix_exhausted` (409) and `restore_name_exhausted` (409), and may carry `name_too_long` and `invalid_path`, on every route that refuses with them.
10. No name, title or path reaches the journal (API-S-005); refusals are journaled once.
11. Contract minor version bump in `openapi.json` and regenerated `ui/src/api/schema.d.ts`.

## Approach

- `src/error.rs`: `SubCode` and `ApiError::with_rule`; `FileError`, `LibraryError` and the import destination refusal attach it.
- `src/files/versions.rs`: `history_bytes` in `add_object_size`.
- `src/navigation`: `collection_id` on `NodeView`; a `destination` service method and route.
- `src/files/drive*.rs`, `search*.rs`: row facts, folder counts after paging, the Recent scope from the journal.
- Tests first, through the router: `tests/integration/row_facts.rs`, additions to `navigation.rs`, `versions.rs`, `home_collections.rs`, `recently_deleted.rs`, `unit/errors.rs`, `unit/contract.rs`, and cucumber steps in `tests/support/row_facts_steps.rs`.

## Verification

`cargo test -p filesquid-api --locked`, Clippy, rustdoc, the UI package's typecheck and tests after regenerating the types, then `FULL=1 bash scripts/pre-commit.sh`.
