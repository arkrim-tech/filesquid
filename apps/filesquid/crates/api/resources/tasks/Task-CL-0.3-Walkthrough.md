# Task CL-0.3 Walkthrough: Replica, cache identity, and pending mutations

Module task [CL-0.3](../plan.md) of global [Task 0.10](../../../../../../resources/implementation-plan.md#task-0.10). Branch `feat/cl-0.3-replica-cache-identity`, from `main` at `6a02a11`. Dependency CL-0.2 was complete; this task completes global Task 0.10.

**Status: done.** `filesquid-api` gains the `replica` module: a permission-aware cache identity, visible coverage with `not_cached` and `evicted` recovery states, the injected `ReplicaCache` port with an in-memory reference, and the `PendingLedger` of pending browser changes, which become canonical only after a fenced head publication. The module does no storage I/O, uses no native, network or storage dependency (only `std`, `serde`, `serde_json`, `sha2`, `time`, `tracing` and `filesquid-core`) and adds no dependency, so the API-S-006 guards stay green with no pins. No route or contract path is added, so the OpenAPI contract and the generated UI types are unchanged. [CL-0.4](../plan.md) freezes the commit receipt; until then `HeadPublication` is the minimal proof of a fenced publication.

## API-S-024: Replica, cache identity and pending changes

Parents: [F-036 Browser and mobile-web experience](../../../../../../resources/prd.md#f-036-browser-and-mobile-web-experience), [TC-002 Endpoint processing and hosted encryption](../../../../../../resources/prd.md#tc-002-endpoint-processing-and-hosted-encryption), [ADR 008](../../../../../../resources/architecture/008-browser-wasm-scope.md), [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md).

| Constraint | Gate | State |
|---|---|---|
| C1 Identity names schema version 1, host type, viewer (1 to 256 bytes, no control character), library and grant digest | `viewer_ids_are_bounded_and_free_of_control_characters`, `keys_and_identities_carry_no_name_or_path` | Met |
| C2 Grant digest: SHA-256 of the sorted, de-duplicated scopes; the input list holds 1 to 64 names (counted before de-duplication) of 1 to 128 bytes each; order and repeats do not matter, membership does | `grant_digest_ignores_order_and_repeats_but_not_membership`, `grant_digest_bounds_the_scope_list` | Met |
| C3 Cache key: 64 lowercase hex over domain separator and length-prefixed components; any component changes it; no name or path | `equal_identities_give_equal_keys_and_every_component_changes_the_key`, `the_key_encoding_is_not_ambiguous_across_field_boundaries`, `keys_and_identities_carry_no_name_or_path` | Met |
| C4 Query key covers identity and the whole request | `query_keys_differ_by_identity_page_size_cursor_and_query` | Met |
| C5 An entry is served only to its own identity; a grant change makes old entries unreachable | `a_page_stored_for_another_identity_is_never_served`, `a_grant_change_makes_the_old_entries_unreachable`, `a_changed_grant_supersedes_the_old_identity_only_for_the_same_viewer_and_library`, scenarios "Another viewer never reads", "A changed grant", "made for another identity" | Met |
| C6 Coverage is visible; a miss is `not_cached` and an eviction `evicted`, both retryable and never an empty page | `a_cached_page_answers_with_its_coverage_and_key`, `incomplete_coverage_is_visible_on_the_answer`, `coverage_is_complete_only_when_every_shard_is_loaded`, `a_miss_is_not_cached_and_never_an_empty_page`, `an_evicted_entry_says_evicted`, `storing_again_after_eviction_recovers_the_entry`, scenarios "An evicted view" and "An incomplete view" | Met |
| C7 Cache bounded (default 1,024, at least 1), oldest evicted first, eviction memory bounded | `the_cache_is_bounded_evicts_oldest_first_and_remembers_it`, `the_eviction_memory_is_bounded_too`, `cache_capacity_must_be_positive_and_has_a_default` | Met |
| C8 A pending change keeps its command unchanged, an RFC 3339 UTC time, its state and attempts | `a_new_change_is_queued_with_its_command_unchanged`, `queued_at_must_be_rfc3339_utc`, `create_library_cannot_be_pending_because_it_names_no_library` | Met |
| C9 At most 1,000 changes per library and 64 libraries; a repeated operation ID is the same change | `pending_changes_are_bounded_per_library`, `the_number_of_libraries_with_pending_changes_is_bounded`, `a_repeated_operation_id_is_the_same_change` | Met |
| C10 Publish in queue order; a conflict or stalled head blocks later changes | `only_the_oldest_unfinished_change_may_begin_publishing`, `a_conflict_blocks_later_changes_until_rebased_or_discarded`, scenario "A lost race" | Met |
| C11 `committed` only through a fenced head publication, from `publishing`; a committed change cannot be discarded | `a_change_is_committed_only_by_a_fenced_head_publication`, `a_head_publication_names_a_revision`, `only_a_committed_change_can_be_acknowledged`, scenarios "A new change is pending" and "A change becomes canonical" | Met |
| C12 Retry then stall after 5 attempts; conflict and stall leave only by rebase or discard; publishing cannot be discarded or rebased | `a_retry_waits_then_publishes_again_and_stalls_after_the_attempt_bound`, `a_publishing_change_can_be_neither_discarded_nor_rebased`, `unknown_operations_are_not_found`, scenario "Repeated failures stall" | Met |
| C13 Snapshot and restore keep the bounds, refuse bad input and requeue an interrupted publication | `the_snapshot_restores_the_same_ledger_and_requeues_an_interrupted_publication`, `restore_refuses_duplicates_malformed_input_and_over_bound_snapshots`, `restore_refuses_a_committed_change_without_a_published_revision`, scenarios "survive a reload" and "damaged saved list" | Met |
| C14 Typed errors with stable codes and no names | `every_error_has_a_stable_code_and_a_message_without_names` | Met |
| API-S-006 no storage I/O or forbidden crate | `unit_isolation` (unchanged and green) | Met |
| Review round 1 (Cubic): validated restore of keys, identities and coverage; retryable `pending_full` and `not_next`; `drop_superseded`; `invalid_command`; stalled restore at the attempt cap; snapshot size bound; refreshed entries become newest; redacting `Debug` | `not_next_is_retryable_like_pending_full`, `a_superseded_identity_loses_its_entries_and_others_keep_theirs`, `restoring_state_validates_keys_identities_and_coverage`, `refreshing_an_entry_makes_it_the_newest`, `debug_output_never_prints_cached_answers`, `a_command_that_breaks_a_library_rule_cannot_be_pending`, `repeating_the_original_request_after_a_rebase_answers_the_change`, `restore_refuses_a_revision_on_an_uncommitted_change`, `an_interrupted_last_attempt_restores_as_stalled`, `an_oversized_snapshot_is_refused_before_parsing` | Met |

### Secondary SRS mapping

- **SRS-F-036: Browser and mobile-web experience.** A browser reads a replica scoped to its viewer and grants, sees coverage and recovery states instead of silent loss (T-022), and keeps unpublished changes visibly pending. The new feature is listed under SRS-F-036's tests.
- **API-S-006: Application meaning (catalog, search, replica, the FileSquid OCFL application profile) MUST be implemented here once for every host.** The replica meaning lives once in `filesquid-api`; the host injects the cache backend and stores the ledger snapshot.
- **API-S-010: The crate MUST build for `wasm32-unknown-unknown`.** The module uses no filesystem, network or native dependency (only `std`, `serde`, `serde_json`, `sha2`, `time`, `tracing` and `filesquid-core`). The whole crate does not build for `wasm32-unknown-unknown` yet: `cargo build -p filesquid-api --target wasm32-unknown-unknown` fails in `mio` (pulled in by Axum and Tokio), which predates this task and belongs to API-S-010's own work. The replica module itself adds no dependency that blocks that build.
- **API-S-008: Every content or organisation change MUST commit an OCFL version through the `filesquid-ocfl` change model.** A pending change is not a commit; it becomes one only when the host's fenced head publication succeeds ([ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md) Decision 5).

## What changed

- `crates/api/src/replica/`: `identity`, `coverage`, `cache`, `pending`, `error`; `lib.rs` exports it; `host.rs` derives `Deserialize` and `Hash` for `HostType`.
- Tests: `tests/unit/replica.rs`, `tests/support/replica_steps.rs` and `tests/features/replica_cache_identity_and_pending_changes_API-S-024.feature`.
- Docs: module SRS row API-S-024; the CL-0.3 plan row; product SRS test list and traceability row for SRS-F-036; root README test command; checklist boxes for CL-0.3 and global Task 0.10, with the progress bar redrawn.
- No new dependency; no contract change.

## Tests

TDD: the tests were written first. Before the module existed, `cargo test -p filesquid-api --locked --test unit_replica` failed to compile:

```text
error[E0432]: unresolved import `filesquid_api::replica`
  --> crates/api/tests/unit/replica.rs:12:20
   |
12 | use filesquid_api::replica::{
   |                    ^^^^^^^ could not find `replica` in `filesquid_api`
error: could not compile `filesquid-api` (test "unit_replica") due to 1 previous error
```

After the implementation, `cargo test -p filesquid-api --locked` gave for the replica target:

```text
     Running tests/unit/replica.rs 

running 48 tests
test a_changed_grant_supersedes_the_old_identity_only_for_the_same_viewer_and_library ... ok
test a_change_is_committed_only_by_a_fenced_head_publication ... ok
test a_cached_page_answers_with_its_coverage_and_key ... ok
test a_conflict_blocks_later_changes_until_rebased_or_discarded ... ok
test a_command_that_breaks_a_library_rule_cannot_be_pending ... ok
test a_grant_change_makes_the_old_entries_unreachable ... ok
test a_head_publication_names_a_revision ... ok
test a_miss_is_not_cached_and_never_an_empty_page ... ok
test a_new_change_is_queued_with_its_command_unchanged ... ok
test a_page_answer_type_round_trips_as_json ... ok
test a_publishing_change_can_be_neither_discarded_nor_rebased ... ok
test a_repeated_operation_id_is_the_same_change ... ok
test a_page_stored_for_another_identity_is_never_served ... ok
test a_retry_waits_then_publishes_again_and_stalls_after_the_attempt_bound ... ok
test an_interrupted_last_attempt_restores_as_stalled ... ok
test cache_capacity_must_be_positive_and_has_a_default ... ok
test coverage_is_complete_only_when_every_shard_is_loaded ... ok
test create_library_cannot_be_pending_because_it_names_no_library ... ok
test a_superseded_identity_loses_its_entries_and_others_keep_theirs ... ok
test an_evicted_entry_says_evicted ... ok
test debug_output_never_prints_cached_answers ... ok
test every_error_has_a_stable_code_and_a_message_without_names ... ok
test grant_digest_bounds_the_scope_list ... ok
test grant_digest_ignores_order_and_repeats_but_not_membership ... ok
test equal_identities_give_equal_keys_and_every_component_changes_the_key ... ok
test incomplete_coverage_is_visible_on_the_answer ... ok
test not_next_is_retryable_like_pending_full ... ok
test only_a_committed_change_can_be_acknowledged ... ok
test keys_and_identities_carry_no_name_or_path ... ok
test only_the_oldest_unfinished_change_may_begin_publishing ... ok
test query_keys_differ_by_identity_page_size_cursor_and_query ... ok
test queued_at_must_be_rfc3339_utc ... ok
test repeating_the_original_request_after_a_rebase_answers_the_change ... ok
test restore_refuses_a_committed_change_without_a_published_revision ... ok
test refreshing_an_entry_makes_it_the_newest ... ok
test restore_refuses_a_revision_on_an_uncommitted_change ... ok
test restoring_state_validates_keys_identities_and_coverage ... ok
test storing_again_after_eviction_recovers_the_entry ... ok
test the_cache_is_bounded_evicts_oldest_first_and_remembers_it ... ok
test the_eviction_memory_is_bounded_too ... ok
test the_key_encoding_is_not_ambiguous_across_field_boundaries ... ok
test the_number_of_libraries_with_pending_changes_is_bounded ... ok
test the_snapshot_restores_the_same_ledger_and_requeues_an_interrupted_publication ... ok
test unknown_operations_are_not_found ... ok
test viewer_ids_are_bounded_and_free_of_control_characters ... ok
test an_oversized_snapshot_is_refused_before_parsing ... ok
test pending_changes_are_bounded_per_library ... ok
test restore_refuses_duplicates_malformed_input_and_over_bound_snapshots ... ok

test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
```

The `features` harness runs the new feature (12 scenarios, all steps passed). The whole `filesquid-api` run reported 247 passed and 0 failed across its test binaries; the scenarios the harness skips are those of other features whose steps are not written yet.

`cargo fmt --all --check` and `cargo clippy -p filesquid-api --all-targets --locked -- -D warnings` were clean.

## Hook and full suite

`bash scripts/pre-commit.sh` passed on both commits of the branch. `FULL=1 bash scripts/pre-commit.sh` on the merged head (main at `5fa177e`) ended with:

```text
✅ All Pre-Commit Hooks Passed! Test scope: full (507s)
```

The desktop packaged-window e2e is skipped by default (`FILESQUID_DESKTOP_E2E=1` runs it).

## Mocked Environment Variables

None.
