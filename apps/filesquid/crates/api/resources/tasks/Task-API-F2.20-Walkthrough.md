# Task API-F2.20 Walkthrough: Repair consent statement names the repair's history version

Branch `fix/repair-consent-wording`, module `filesquid-api` ([plan row](../plan.md)), with the UI label, the ImageSquid label table and mockup 42 changed in the same change. Contract **0.28.0** (main was 0.27.0 when the branch merged it; the branch first bumped to 0.27.0 while main was 0.26.0).

Binding owner decision 4 of 2026-10-23 (the owner adopted the orchestrator's recommendations, verbatim answer: "provide and implement recommendations for owner's decisions 1-6"): "The consent statement says what the repair now does. API-S-023's statement becomes: one version inventory and its sidecar, copies of the root inventory, will be added to each object's latest version directory, and a new version recording the repair will be committed to each object's history (API-S-028); no file content changes. The UI label `repairConsent` reads: 'One copy of the root inventory and its sidecar will be added to each listed object's latest version directory, and a new version recording this repair will be committed to each object's history. No file content changes.'"

## API-S-023: Version-inventory repair, consented and journaled

The consent names its statement by ID. The statement now also says that the repair commits a new version recording it to each object's history (API-S-028), which the v1 statement did not, so the ID moves to `version_inventory_repair.v2` and a consent to `version_inventory_repair.v1` is refused as `consent_mismatch` with nothing written, like any other stale statement. `CONSENT_STATEMENT` reads: "For each object named, one version inventory and its sidecar, copies of the root inventory, will be added to the object's latest version directory, and a new version recording the repair will be committed to each object's history (API-S-028); no file content changes." The repair itself is unchanged.

## UI-S-027: Library lifecycle screens (repair consent dialog)

`repairConsent` in [`labels.en.json`](../../../../ui/src/theme/labels.en.json) and the ImageSquid table ([`labels.en.json`](../../../../../imagesquid/src/theme/labels.en.json)) reads exactly the owner's string, and the dialog sends statement `version_inventory_repair.v2` (`REPAIR_STATEMENT_ID`). Mockup [`42-repair-consent.svg`](../../../../../../resources/ui-mockups/v3/42-repair-consent.svg) shows the same text on three lines; the object lines move down 22 px. The file still parses as XML, and measured in Chromium at 1440 × 900 the longest line ends at x = 935 (the dialog's inner edge is 992) and the last object line at y = 443, above the separator at 466.

| Constraint | Gate | State |
|---|---|---|
| The API's statement names the history version and says no file content changes | `the_consent_statement_names_what_changes` | Met |
| The statement ID moves to v2; a v1 consent is refused `consent_mismatch`, journaled once, nothing written | `the_consent_statement_id_is_pinned`, `the_command_is_refused_without_consent_and_writes_nothing` | Met |
| `repairConsent` reads the owner's string in FileSquid and ImageSquid | `library-repair.test.ts` ("states exactly what the repair adds"), the label tables | Met |
| The UI sends statement v2 | `library-model.test.ts`, `library-client.test.ts`, `library-repair.test.ts` ("Confirm calls the repair route once…"), e2e fixtures | Met |
| Mockup 42 shows the same text and fits its dialog | XML parse and the Chromium measurement above; `mockup-fidelity.spec.ts` "repair consent (42)" | Met |
| Contract 0.28.0 with regenerated types | `unit_contract`, Vitest `contract.test.ts` | Met |

## What changed

- [`repair.rs`](../../src/local_library/repair.rs): `CONSENT_STATEMENT_ID` and `CONSENT_STATEMENT`.
- [`openapi.json`](../../openapi.json): the repair route and `RepairConsent.statement_id` descriptions name v2 and the history version; 0.28.0. `ui/src/api/schema.d.ts` and `ui/src/api/contract-version.ts` regenerated.
- UI: `REPAIR_STATEMENT_ID`, `repairConsent`, the `fs-library-repair.ts` module doc, the UI README, the F-001 feature scenario and the tests; the desktop WebdriverIO spec sends v2.
- ImageSquid: `repairConsent`.
- Mockup 42: text only.
- API-S-023 and UI-S-027 are rewritten by the owner-decisions PR (#131), not here.

## Tests

TDD: the tests were changed first and failed against the old statement.

```text
     Running tests/unit/version_inventory_repair.rs (<target>/debug/deps/unit_version_inventory_repair-5cde95bde43513f8)
running 21 tests
test a_head_that_already_has_both_files_is_refused_with_nothing_to_repair ... ok
test a_held_reservation_is_a_retryable_refusal ... ok
test a_damaged_interrupted_repair_is_refused_without_writing_the_sidecar ... ok
test a_consented_repair_by_the_owner_makes_a_sha256_object_committable ... ok
test a_root_sidecar_that_does_not_commit_the_root_inventory_is_refused ... ok
test a_second_repair_is_refused_with_nothing_to_repair ... ok
test an_interrupted_repair_is_resumed ... ok
test an_unknown_object_is_refused_per_object ... ok
test an_unlisted_file_in_the_head_is_neither_offered_nor_repaired ... ok
test an_unrecordable_outcome_still_reaches_the_journal ... ok
test an_unrecordable_consent_repairs_nothing ... ok
test candidates_say_whether_the_repair_opens_the_object_to_commits ... ok
test object_lists_are_bounded_and_distinct ... ok
test consent_and_outcomes_share_one_repair_run_id ... ok
test consent_and_one_outcome_row_per_object_are_journaled_without_paths ... ok
test the_command_is_refused_for_another_caller ... ok
test open_copy_and_rebuild_never_reach_the_repair ... ok
test the_candidate_query_lists_heads_without_their_version_inventory ... ok
test the_consent_statement_id_is_pinned ... FAILED
test the_consent_statement_names_what_changes ... FAILED
test the_command_is_refused_without_consent_and_writes_nothing ... FAILED
---- the_consent_statement_id_is_pinned stdout ----
thread 'the_consent_statement_id_is_pinned' (29460) panicked at crates/api/tests/unit/version_inventory_repair.rs:348:5:
assertion `left == right` failed
  left: "version_inventory_repair.v1"
---- the_consent_statement_names_what_changes stdout ----
thread 'the_consent_statement_names_what_changes' (29461) panicked at crates/api/tests/unit/version_inventory_repair.rs:415:9:
---- the_command_is_refused_without_consent_and_writes_nothing stdout ----
thread 'the_command_is_refused_without_consent_and_writes_nothing' (29458) panicked at crates/api/tests/unit/version_inventory_repair.rs:179:50:
test result: FAILED. 18 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

```text
 ❯ tests/unit/library-model.test.ts (7 tests | 1 failed) 40ms
 ✓ tests/unit/library-client.test.ts (18 tests) 66ms
 ❯ tests/ui/library-repair.test.ts (22 tests | 2 failed) 364ms
 FAIL  tests/ui/library-repair.test.ts > <fs-library-repair> consent > opens a modal dialog that names the objects and states exactly what the repair adds
AssertionError: expected 'One copy of the root inventory and it…' to contain 'a new version recording this repair w…'
 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/ui/library-repair.test.ts:124:34
 FAIL  tests/ui/library-repair.test.ts > <fs-library-repair> consent > Confirm calls the repair route once with the consent for exactly the named objects
AssertionError: expected [ { library_id: 'lib-1', …(2) } ] to deeply equal [ { library_id: 'lib-1', …(2) } ]
- Expected
+ Received
-       "statement_id": "version_inventory_repair.v2",
+       "statement_id": "version_inventory_repair.v1",
 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/ui/library-repair.test.ts:254:28
 FAIL  tests/unit/library-model.test.ts > consentRequest > names exactly the listed objects in the request and in the consent
AssertionError: expected { library_id: 'lib-1', …(2) } to deeply equal { library_id: 'lib-1', …(2) }
- Expected
+ Received
-     "statement_id": "version_inventory_repair.v2",
+     "statement_id": "version_inventory_repair.v1",
 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/unit/library-model.test.ts:58:21
 Test Files  2 failed | 1 passed (3)
      Tests  3 failed | 44 passed (47)
```

After the change, `cargo test -p filesquid-api --locked --test unit_version_inventory_repair`:

```text
     Running tests/unit/version_inventory_repair.rs (<target>/debug/deps/unit_version_inventory_repair-5cde95bde43513f8)
running 21 tests
test a_damaged_interrupted_repair_is_refused_without_writing_the_sidecar ... ok
test a_held_reservation_is_a_retryable_refusal ... ok
test a_consented_repair_by_the_owner_makes_a_sha256_object_committable ... ok
test a_head_that_already_has_both_files_is_refused_with_nothing_to_repair ... ok
test a_root_sidecar_that_does_not_commit_the_root_inventory_is_refused ... ok
test a_second_repair_is_refused_with_nothing_to_repair ... ok
test an_interrupted_repair_is_resumed ... ok
test an_unknown_object_is_refused_per_object ... ok
test an_unlisted_file_in_the_head_is_neither_offered_nor_repaired ... ok
test an_unrecordable_consent_repairs_nothing ... ok
test candidates_say_whether_the_repair_opens_the_object_to_commits ... ok
test object_lists_are_bounded_and_distinct ... ok
test an_unrecordable_outcome_still_reaches_the_journal ... ok
test consent_and_one_outcome_row_per_object_are_journaled_without_paths ... ok
test consent_and_outcomes_share_one_repair_run_id ... ok
test the_command_is_refused_for_another_caller ... ok
test open_copy_and_rebuild_never_reach_the_repair ... ok
test the_consent_statement_id_is_pinned ... ok
test the_consent_statement_names_what_changes ... ok
test the_command_is_refused_without_consent_and_writes_nothing ... ok
test the_candidate_query_lists_heads_without_their_version_inventory ... ok
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

`npx vitest run tests/ui/library-repair.test.ts tests/unit/library-model.test.ts tests/unit/library-client.test.ts tests/unit/contract.test.ts` (in `apps/filesquid/ui`):

```text
 RUN  v4.1.12 <repo>/apps/filesquid/ui
 ✓ tests/unit/library-model.test.ts (7 tests) 24ms
 ✓ tests/unit/library-client.test.ts (18 tests) 63ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 392ms
 ✓ tests/unit/contract.test.ts (2 tests) 257ms
 Test Files  4 passed (4)
      Tests  49 passed (49)
   Start at  19:57:46
   Duration  2.98s (transform 2.39s, setup 282ms, import 3.42s, tests 737ms, environment 1.82s)
JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

`npx playwright test -c playwright.preview.config.ts tests/e2e/preview/mockup-fidelity.spec.ts -g "repair consent"` against the production build (run with a throwaway config that points Playwright at the preinstalled Chromium, as the environment requires; the config file was deleted afterwards):

```text
Running 2 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › light theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (945ms)
  ✓  2 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › dark theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (554ms)
  2 passed (10.4s)
```

## Sync Execution Report

- No upstream document changed in this branch: API-S-023 and UI-S-027 carry the decision through PR #131.
- `apps/filesquid/ui/README.md`: the repair dialog's description.

## Mocked Environment Variables

None.
