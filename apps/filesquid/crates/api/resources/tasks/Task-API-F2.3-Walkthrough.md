# Task API-F2.3 Walkthrough: Search operation and operator grammar

Branch `feat/api-f2.3-search-operators`, module `filesquid-api` ([plan row](../plan.md)), part of global Task FS-2.5. Contract 0.22.0. The plan box stays unticked: the server operators wait on SV-F1.4, and the remaining items are listed below and in the plan row's status line. Holistic UI check: the global search bar of UI-S-007 can call `GET /api/v1/files/search` and tell an unknown operator (`invalid_request`) from one this host cannot answer (`unavailable`). On the UI side this PR only regenerates `schema.d.ts`.

## SRS mapping

| SRS ID and title | Covered by |
|---|---|
| [API-S-012](../srs.md) Search operation and operator grammar | `integration_search` (10 tests), `search_operation_API-S-012.feature` (11 scenarios), `files::search*` unit tests (13), `unit_contract`, `unit_auth` |
| [SRS-F-063](../../../../../../resources/srs.md#srs-f-063-file-search-query-language-and-filter-chips) File search query language and filter chips | the grammar, the unknown-operator error, relevance as default with any view sort key, stable paging, declared case folding, date boundaries in the caller's zone without invented instants: `every_answered_operator_narrows_the_results`, `unknown_operators_are_refused_and_never_match_an_empty_filter`, `relevance_orders_by_default_and_any_view_key_replaces_it`, `case_folding_is_declared_and_diacritics_are_not_folded`, `capture_dates_without_a_zone_compare_by_calendar_date` |
| [SRS-NFR-004](../../../../../../resources/srs.md#srs-nfr-004-security-and-privacy) Security and privacy | permission filtering before ranking: `results_are_filtered_by_permission_before_ranking` |
| [API-S-011](../srs.md) View operation | results shaped and paged like the view, with the cursor conflict rule: `paging_is_stable_and_a_changed_result_set_refuses_the_cursor` |
| [API-S-027](../srs.md) Display and download names | results name files by their display name |
| [API-S-002](../srs.md) The versioned API contract owned by this crate | contract 0.22.0 with the grammar, bounds and refusals in the operation's description; regenerated `ui/src/api/schema.d.ts` |

## What changed

- **Grammar** (`src/files/search_query.rs`, pure).
  - Words, quoted phrases, a leading `-` and `name:value` operators with quoted values; `size>` and `size<` take their value without a colon. Operator names compare without case.
  - Bounds: `MAX_QUERY_BYTES` (1,024) and `MAX_QUERY_TERMS` (32).
  - Answered operators: `type:` (image, video, audio, text, document, folder), `in:`, `title:`, `owner:me`, `creator:`, `tag:`, `is:deleted`, `is:unfiled`, `before:`, `after:`, `createdbefore:`, `createdafter:`, `source:`, `size>` and `size<` (B, KB, MB, GB in powers of 1024).
  - An unknown operator is `invalid_request` naming it; `generator:` is unknown, since this build has no `c2pa` module.
  - An operator of the grammar this host cannot answer is `unavailable` naming it: `is:favorite`, `owner:` naming another user, `sharedwith:`, `to:`, `from:` and `pendingowner:`.
  - Comparison uses simple case folding; diacritics are not folded.
- **Matching** (`src/files/search_match.rs`, pure).
  - Free text reads the display name and every `dcterms:` value.
  - Relevance is 3 per term in the name, 2 in a `dcterms:title`, 1 in another `dcterms:` value.
  - `before:` and `after:` use the head version's date in the caller's `zone`.
  - `createdbefore:` and `createdafter:` read `exif:DateTimeOriginal`, else `dcterms:created`. A zoned value compares by its date in the caller's zone; a zoneless value compares by the calendar date it states, so no instant is invented.
  - Folder rows match by name, `type:folder` and `owner:me`.
- **Scope** (`src/files/search_scope.rs`).
  - The repository (by default), a library, a collection or a virtual collection, read from the host catalog through the navigation service.
  - Every library, collection and file the caller may not read is left out before anything is read or ranked.
  - Bounds: `MAX_SEARCH_LIBRARIES` (1,000) and `MAX_SEARCH_FILES` (10,000), with `complete: false` beyond them.
- **Operation** (`src/files/search.rs`, `drive_routes.rs`).
  - `GET /api/v1/files/search?q=&node=&sort=&direction=&folders=&zone=&cursor=&limit=`.
  - Sort is `relevance` (most relevant first) or any view sort key, with the same matches either way.
  - Rows, paging and cursor rules are the view's (`page_rows`, shared with `GET /api/v1/files/view`). A cursor of other parameters is 400; a cursor of results that changed is 409.
  - There is no separate index: each search reads the catalog and the OCFL records, so it always follows canonical state.
- **Shared code.** `FileFacts` now carries the record's statements. `page_rows`, `ranked` and `digest_parameters` are shared by the view and search. `NavigationService` gains a crate-internal `repository_id()`.

## Remaining (box unticked)

- The server operators `sharedwith:`, `to:`, `from:` and `pendingowner:` (after SV-F1.4), `is:favorite` (no favorites yet) and `owner:` naming another user (no accounts). Until then they are refused as `unavailable`.
- NFC normalisation of names and queries. Until the owner decides on a normalisation crate, comparison uses simple case folding only.
- Full-text content search of PDF and plain text, with its capability register entry.
- BYO-AI natural-language questions, and search suggestions.
- The multi-user scenarios of `file_search_query_language_F-063.feature`, which need `filesquid-server` users.

## Test output

```text
$ cargo test -p filesquid-api --locked --lib files::search
test files::search::tests::zones_are_fixed_offsets ... ok
test files::search_match::tests::folders_match_by_name_and_type_and_relevance_ranks_names_first ... ok
test files::search_match::tests::capture_dates_without_a_zone_compare_by_calendar_date ... ok
test files::search::tests::relevance_is_the_default_and_any_view_key_replaces_it ... ok
test files::search_match::tests::free_text_reads_the_name_and_dublin_core_values ... ok
test files::search_query::tests::folding_is_simple_case_folding ... ok
test files::search_match::tests::operators_read_their_statements_and_facts ... ok
test files::search_query::tests::every_answered_operator_parses ... ok
test files::search_match::tests::head_dates_use_the_callers_zone ... ok
test files::search_query::tests::operators_this_host_cannot_answer_are_unavailable ... ok
test files::search_query::tests::queries_are_bounded ... ok
test files::search_query::tests::words_phrases_and_exclusions ... ok
test files::search_query::tests::unknown_operators_and_bad_values_are_refused_and_never_ignored ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out; finished in 0.00s

$ cargo test -p filesquid-api --locked --test integration_search
test case_folding_is_declared_and_diacritics_are_not_folded ... ok
test free_text_phrases_and_exclusions_read_names_and_dublin_core ... ok
test operators_this_host_cannot_answer_are_refused_as_unavailable ... ok
test a_node_limits_the_search_and_dates_follow_the_callers_zone ... ok
test every_answered_operator_narrows_the_results ... ok
test relevance_orders_by_default_and_any_view_key_replaces_it ... ok
test paging_is_stable_and_a_changed_result_set_refuses_the_cursor ... ok
test unfiled_files_and_folder_rows_are_searchable ... ok
test results_are_filtered_by_permission_before_ranking ... ok
test unknown_operators_are_refused_and_never_match_an_empty_filter ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s

$ cargo test -p filesquid-api --locked --test unit_contract --test unit_auth
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.60s

$ cargo test -p filesquid-api --locked --test features (this feature and the run summary)
Feature: Search operation and operator grammar
  Scenario: Free text matches the name and Dublin Core values
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "harbour"
   ✔  Then the results should be "img_0001.jpg"
  Scenario: Quoted phrases match exactly and a leading minus excludes
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "\"Client deck\" -draft"
   ✔  Then the results should be "Client deck.pptx"
  Scenario Outline: Operators narrow the result set
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "title:budget"
   ✔  Then the results should be "budget.pdf"
  Scenario Outline: Operators narrow the result set
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "type:image"
   ✔  Then the results should be "img_0001.jpg"
  Scenario Outline: Operators narrow the result set
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "size<10 -type:folder"
   ✔  Then the results should be "Client deck.pptx"
  Scenario Outline: Operators narrow the result set
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "holi type:folder"
   ✔  Then the results should be "Holidays"
  Scenario: Unknown operators return an error, never an empty-filter match
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "colour:blue"
   ✔  Then the search should be refused as "invalid_request"
  Scenario: generator: is not part of the grammar without the c2pa module
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "generator:\"Camera App\""
   ✔  Then the search should be refused as "invalid_request"
  Scenario: Operators this host cannot answer are refused, not ignored
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "sharedwith:public"
   ✔  Then the search should be refused as "unavailable"
  Scenario: Results are filtered by permission before ranking
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  Given a file "Budget 2026 secret.pdf" in a collection the owner may not read
   ✔  When the owner searches for "budget"
   ✔  Then the results should be "budget.pdf"
  Scenario: Case folding follows the declared contract
   ✔> Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"
   ✔  When the owner searches for "BUDGET"
   ✔  Then the results should be "budget.pdf"
25 features
362 scenarios (260 passed, 102 skipped)
1253 steps (1151 passed, 102 skipped)
```
