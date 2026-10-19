# Task PV-0.1 Walkthrough: Crate, PROV-O model and profile `filesquid-prov/1`

Branch `feat/pv-0.1-prov-crate`. Module `filesquid-prov` (`apps/filesquid/crates/prov`). Requirements: PROV-S-001 to PROV-S-005 of the module SRS, PRD F-067 and SRS-F-067, ADR 040 Decisions 2, 3 and 4. Those documents arrive with a separate documentation pull request, so this walkthrough names them without links.

## What changed

- New crate `filesquid-prov` (MIT, `publish = false`, edition 2021, `#![deny(missing_docs)]`), added to the workspace members in `apps/filesquid/Cargo.toml`; `Cargo.lock` updated by Cargo. Dependencies: `serde`, `serde_json`, `thiserror`. No features yet; PV-0.2 adds `ocfl`.
- `src/model.rs`: `Record`, `Activity`, `Entity`, `Agent`, `AgentKind` (`Installation`, `Host`, `Tool` reserved, `Unrecorded`), `AgentRole`, `HostFacts`, `ToolFacts`, `ObjectVersion`, `Derivation`.
- `src/scalar.rs`: `Timestamp` (RFC 3339 UTC), `Sha256Digest`, `InstallationId` (UUID v4), identifier rules. `src/ids.rs`: identifier forms and percent-encoding.
- `src/builder.rs`: `VersionFacts`, `OcflUser`, `Reconstruction`, `build_record`. `src/graph.rs`: canonical order and graph validation shared by the builder and the reader.
- `src/profile/`: `to_json_ld`, `parse_record`, `Profile`, a strict bounded JSON reader (`strict.rs`) and the `v1` writer and reader.
- `src/index.rs`: `HistoryIndex`, `HistoryQuery`, `PageRequest`, `Page`, `Cursor`.
- Tests: `tests/unit/{ids,builder,profile,parse,index,isolation}.rs`, the BDD harness `tests/features.rs` with `tests/features/prov_o_model_F-067.feature` (JUnit in `tests/allure-results/`, already gitignored by `**/allure-results/`), `tests/support/{fixtures,wasm}.rs`.
- Wiring: `filesquid-prov` in `MODULE_CRATES` and a `wasm32` step (`run_prov_wasm`) in `scripts/verify_tests.sh`; the crate in the `scripts/verify_changed_scope.sh` crate-graph fixture with one scope expectation; `filesquid-prov` in the forbidden lists of the core, metadata and c2pa isolation tests; the crate README; the root README "Run tests" paragraph and package list.

## PROV-S-001: Stand-alone module

Default dependencies exclude every other FileSquid crate; no feature names `filesquid-native` or `filesquid-c2pa`; no Axum, Tauri, SQLite, libvips or FFmpeg; no filesystem or network I/O; builds for `wasm32-unknown-unknown`; rustdoc on every public item.
Tests: `unit_isolation` (manifest rules, each also proven to fail on a synthetic manifest; source scan for `std::fs`, `std::net`, `std::process`, `std::env`; `#![deny(missing_docs)]`), BDD "The module stands alone" and `@wasm32` "The module builds for the browser"; `cargo check -p filesquid-prov --lib --target wasm32-unknown-unknown --locked`; `cargo doc` with `-D warnings`.

## PROV-S-002: Model

Entity, Activity, Agent and SoftwareAgent; the seven relations; RFC 3339 UTC times; entity object ID, version, logical path and SHA-256; activity operation ID, kind and journal entry IDs; no `prov:Person` and no display name; unrecorded agents.
Tests: `unit_builder` (installation agent, host acting on its behalf, unrecorded agents, OCFL user name never recorded, reconstruction marker), `unit_ids` (UUID v4, SHA-256, timestamps), `unit_profile` (reserved tool kind round trip, no Person), `unit_parse` (Person refused), BDD "The owner is an installation agent, never a person" and "Agents the caller did not observe are stated as unrecorded".

## PROV-S-003: Profile `filesquid-prov/1`

Embedded context (`prov`, `xsd`, `fsq` IRIs), `fsq:profile`, `@graph`; activity and entity identifier forms; deterministic bytes and node order; readers dispatch per profile.
Tests: `unit_profile` (byte-for-byte golden document, equal bytes, context, node order, round trip, member-order independence), `unit_ids` (identifier forms, upper-case percent-encoding), BDD "Equal facts give equal bytes" and "A record survives a parse round trip".

## PROV-S-004: Builder and validation

One activity per version; added, changed, moved and removed paths; used inputs; attribution and association; end time equals `created`; parse bounds with fixed codes naming no stored value.
Tests: `unit_builder` (every diff rule, refusals `PROV_INVALID_FACT`, `PROV_INVALID_MOVE`, `PROV_TIME_ORDER`), `unit_parse` (1 MiB boundary at exactly 1,048,576 and 1,048,577 bytes, depth 16 and 17, duplicate keys at root and in a node, unknown and missing profile, malformed JSON and UTF-8, forged identifier, wrong context, unknown member, non-UTC time, dangling reference, zero or two activities; every message checked for the planted secret), BDD "Parsing refuses a record that breaks a bound" (five examples) and the two diff scenarios.

## PROV-S-005: History index

Activities of an object, an entity, an operation, an agent and a time range; pages of 1 to 1000; cursor; ordered by end time then activity ID.
Tests: `unit_index` (page sizes 0, 1, 1000, 1001 and `usize::MAX`; every page size 1 to 5 yields the same sequence; equal end times ordered by ID; half-open time range; entity, agent and operation queries; cursor tokens; conflicting records), BDD "The history index pages an object's activities in order" and "Page sizes outside 1 to 1000 are refused".

## Design notes and decisions within the brief

- The owner decisions given with the task win over the PV-0.1 plan row and ADR 040 Decision 2, which still list `prov:Person`: there is no `prov:Person` kind, and the reader refuses one.
- Every record has exactly two agents from the builder: the installation agent and the host agent, each recorded or unrecorded (`urn:filesquid:agent:unrecorded:installation`, `urn:filesquid:agent:unrecorded:host`). Generated entities are attributed to the installation-role agent even when it is unrecorded. An unrecorded agent acts on no one's behalf; a recorded host acts on behalf of the installation-role agent.
- When no installation UUID is supplied, an OCFL `user.address` of `urn:uuid:<uuid v4>` names the installation agent (that address is what ADR 040 Decision 6 writes). The `user.name` is never written into a record.
- A move source is `used` and not `invalidated`, so a move stays distinguishable from a delete plus an add.
- An empty diff still builds one activity with no entities, for provenance-only versions (exports, repairs, backfill).
- Logical paths under `provenance/` (`RECORD_PREFIX`) are left out of records, since a record cannot hold its own digest.
- The activity keeps the version message as `fsq:message` when one is supplied.
- Object, operation and journal entry identifiers are limited to RFC 3986 path characters without `%` (1 to 1024 bytes), so they are used verbatim in identifiers; kinds are `[A-Za-z0-9._-]` (1 to 128 bytes); versions are `v` plus digits.
- The writer builds the JSON text from explicitly ordered members instead of a `serde_json::Map`, so the bytes do not depend on whether another crate in a build turns on `serde_json/preserve_order`.

## Coverage

ADR 017 defines no Rust line-coverage tool, so no coverage percentage is reported. Every public function and type of the crate is exercised by the unit tests and scenarios listed above; this is a list of covered requirements, not a measured percentage.

## Verification

Run from `apps/filesquid/` under the shared-target lock (`cargo_locked` from `scripts/lib/cargo_shared.sh`):

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy -p filesquid-prov --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test -p filesquid-prov --locked` | 71 unit tests, 18 scenarios (81 steps), 2 doctests passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p filesquid-prov --no-deps --locked` | exit 0 |
| `cargo check -p filesquid-prov --lib --target wasm32-unknown-unknown --locked` | exit 0 |
| `cargo test -p filesquid-core -p filesquid-metadata -p filesquid-c2pa --locked --test unit_isolation` | 10, 2 and 4 passed |
| `bash scripts/verify_tests.sh --scope rust:filesquid-prov` (from the repository root) | passed, including the new `wasm32 build (filesquid-prov)` step |
| `bash scripts/verify_changed_scope.sh` | 167 assertions passed |

The full pre-commit hook (`FULL=1 bash scripts/pre-commit.sh`) was not run on this branch: the owner runs it on the integrated branch.

## Mocked Environment Variables

None.

## TDD: failing run before implementation

The tests were written first and run against a compile-only skeleton of the public API whose bodies were `todo!()` (`cargo test -p filesquid-prov --no-fail-fast`). Every behaviour test failed; `unit_isolation` already passed because the manifest was written first, and its synthetic-manifest tests show each rule rejects a violation. In the BDD run only "The module stands alone" and the `@wasm32` scenario passed. Summary lines of that run:

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_prov-c8953c181ec6822d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs (<target>/debug/deps/features-c437955dd79c5c35)
18 scenarios (2 passed, 16 failed)
40 steps (24 passed, 16 failed)
  process didn't exit successfully: `<target>/debug/deps/features-c437955dd79c5c35` (exit status: 1)
     Running tests/unit/builder.rs (<target>/debug/deps/unit_builder-c85123737a05e471)
test result: FAILED. 0 passed; 21 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
     Running tests/unit/ids.rs (<target>/debug/deps/unit_ids-ce6fc1accb1454a0)
test result: FAILED. 0 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/unit/index.rs (<target>/debug/deps/unit_index-c2c2f7d616ea5953)
test result: FAILED. 0 passed; 10 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-f7ae67c8ad631b88)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/parse.rs (<target>/debug/deps/unit_parse-8bbe0772f8da3d5b)
test result: FAILED. 0 passed; 15 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/unit/profile.rs (<target>/debug/deps/unit_profile-c6589c848d29a7bb)
test result: FAILED. 0 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
error: 6 targets failed:
exit status: 101
      Step panicked. Captured output: not yet implemented
      Step panicked. Captured output: not yet implemented
      Step panicked. Captured output: not yet implemented
```

## Test output (implementation)

```text
Finished `test` profile [unoptimized] target(s) in 4.02s
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_prov-dc1ee66e11e926fd)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/features.rs (<target>/debug/deps/features-053bb28e26ac5f7e)
Feature: PROV-O model and profile filesquid-prov/1
  Scenario: One activity describes the committed version
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  When I build its PROV-O record
   ✔  Then the record should hold exactly one prov:Activity named after the operation, object and version
   ✔  And its end time should equal the version's created time
   ✔  And it should name the journal entry IDs of the commit
  Scenario: Added, changed, moved and removed paths become entities
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  When I build its PROV-O record
   ✔  Then the changed path should be generated by the activity and derived from its previous state
   ✔  And the moved path should be derived from the old path's previous state
   ✔  And the removed path's previous state should be invalidated by the activity
   ✔  And every previous state the activity replaced should be used by it
   ✔  And every entity should carry its object ID, version, logical path and SHA-256 digest
  Scenario: The owner is an installation agent, never a person
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  When I build its PROV-O record
   ✔  Then the activity should be associated with "urn:uuid:" followed by the installation UUID
   ✔  And the FileSquid host should be a prov:SoftwareAgent acting on behalf of the installation
   ✔  And the record should contain no prov:Person and no display name
  Scenario: Agents the caller did not observe are stated as unrecorded
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given the caller observed no installation and no host
   ✔  When I build its PROV-O record
   ✔  Then both agents should be stated as unrecorded
  Scenario: Equal facts give equal bytes
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  When I serialize the record twice from equal facts
   ✔  Then both documents should be byte-for-byte equal
   ✔  And the graph should list the activity first, then agents and entities sorted by identifier
  Scenario: A record survives a parse round trip
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  When I serialize the record and parse it again
   ✔  Then the parsed record should equal the original
  Scenario Outline: Parsing refuses a record that breaks a bound
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given a stored record that is larger than 1 MiB
   ✔  When I parse it
   ✔  Then parsing should fail with "PROV_TOO_LARGE"
   ✔  And the error message should name no stored value
  Scenario Outline: Parsing refuses a record that breaks a bound
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given a stored record that nests deeper than 16 levels
   ✔  When I parse it
   ✔  Then parsing should fail with "PROV_TOO_DEEP"
   ✔  And the error message should name no stored value
  Scenario Outline: Parsing refuses a record that breaks a bound
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given a stored record that repeats a key
   ✔  When I parse it
   ✔  Then parsing should fail with "PROV_DUPLICATE_KEY"
   ✔  And the error message should name no stored value
  Scenario Outline: Parsing refuses a record that breaks a bound
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given a stored record that names an unknown profile
   ✔  When I parse it
   ✔  Then parsing should fail with "PROV_UNKNOWN_PROFILE"
   ✔  And the error message should name no stored value
  Scenario Outline: Parsing refuses a record that breaks a bound
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given a stored record that holds an entity with a forged ID
   ✔  When I parse it
   ✔  Then parsing should fail with "PROV_INVALID_NODE"
   ✔  And the error message should name no stored value
  Scenario: The history index pages an object's activities in order
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given an index of 5 activities across 2 objects
   ✔  When I read the activities of the first object in pages of 2
   ✔  Then I should read 3 activities ordered by end time then activity ID
   ✔  And the last page should carry no cursor
  Scenario Outline: Page sizes outside 1 to 1000 are refused
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given an index of 5 activities across 2 objects
   ✔  When I ask for a page of 0 activities
   ✔  Then the page request should be refused
  Scenario Outline: Page sizes outside 1 to 1000 are refused
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given an index of 5 activities across 2 objects
   ✔  When I ask for a page of 1 activities
   ✔  Then the page request should be accepted
  Scenario Outline: Page sizes outside 1 to 1000 are refused
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given an index of 5 activities across 2 objects
   ✔  When I ask for a page of 1000 activities
   ✔  Then the page request should be accepted
  Scenario Outline: Page sizes outside 1 to 1000 are refused
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  Given an index of 5 activities across 2 objects
   ✔  When I ask for a page of 1001 activities
   ✔  Then the page request should be refused
  Scenario: The module stands alone
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  When I inspect the crate manifest
   ✔  Then no default dependency should be another FileSquid crate
   ✔  And no feature should name filesquid-native or filesquid-c2pa
  Scenario: The module builds for the browser
   ✔> Given the facts of version "v3" of an object whose base version is "v2"
   ✔  When I check the crate for wasm32-unknown-unknown
   ✔  Then the build should succeed
[Summary]
1 feature
18 scenarios (18 passed)
81 steps (81 passed)
     Running tests/unit/builder.rs (<target>/debug/deps/unit_builder-2fe5848df6f424ec)

running 21 tests
test activity_is_associated_with_each_agent ... ok
test an_empty_diff_still_records_one_activity ... ok
test added_path_is_generated_without_derivation ... ok
test ended_time_is_the_created_time_and_started_is_kept ... ok
test exactly_one_activity_named_after_operation_object_and_version ... ok
test generated_entities_are_attributed_to_the_installation_agent ... ok
test changed_path_is_generated_and_derived_from_its_previous_state ... ok
test inconsistent_moves_and_bases_are_refused ... ok
test installation_is_a_plain_agent_and_host_acts_on_its_behalf ... ok
test inputs_are_used_and_unchanged_paths_are_absent ... ok
test installation_uuid_is_rendered_lowercase ... ok
test invalid_facts_are_refused_with_fixed_codes ... ok
test journal_entries_are_sorted_and_deduplicated ... ok
test moved_path_derives_from_the_old_paths_base_entity ... ok
test ocfl_user_address_names_the_installation_but_its_name_is_never_recorded ... ok
test reconstruction_is_marked_with_its_own_recording_time ... ok
test record_paths_are_not_described_by_their_own_record ... ok
test ocfl_user_without_an_installation_urn_leaves_the_agent_unrecorded ... ok
test refusal_messages_name_no_stored_value ... ok
test removed_path_is_invalidated_by_the_activity ... ok
test unobserved_agents_are_stated_as_unrecorded ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/ids.rs (<target>/debug/deps/unit_ids-6bc7772d8ae1e7cb)

running 8 tests
test entity_identifier_form_encodes_the_logical_path ... ok
test activity_identifier_form ... ok
test every_other_byte_is_upper_case_percent_encoded_utf8 ... ok
test installation_ids_are_uuid_v4_rendered_as_urn_uuid ... ok
test sha256_digests_are_64_hex_characters_in_lower_case ... ok
test timestamps_must_be_rfc3339_utc ... ok
test timestamps_order_by_instant_not_by_text ... ok
test unreserved_characters_stay_as_they_are ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/index.rs (<target>/debug/deps/unit_index-f869cb5678945858)

running 10 tests
test activities_of_an_entity_include_its_generation_and_its_use ... ok
test activities_of_an_object_are_ordered_by_end_time ... ok
test activities_of_an_agent ... ok
test a_full_last_page_has_no_next_cursor ... ok
test page_size_must_be_between_one_and_a_thousand ... ok
test cursors_travel_as_tokens ... ok
test equal_end_times_are_ordered_by_activity_id ... ok
test every_page_size_yields_the_same_sequence_without_gaps ... ok
test time_range_includes_from_and_excludes_to ... ok
test records_are_kept_and_conflicting_duplicates_refused ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-1c5b55dec5713a29)

running 9 tests
test a_default_filesquid_dependency_would_fail ... ok
test a_feature_naming_native_or_c2pa_would_fail ... ok
test library_dependencies_are_serde_serde_json_and_thiserror ... ok
test the_crate_is_mit_licensed_and_unpublished_for_now ... ok
test hosts_sqlite_and_media_crates_would_fail ... ok
test the_crate_root_denies_missing_docs ... ok
test an_optional_filesquid_dependency_in_default_would_fail ... ok
test the_manifest_keeps_every_isolation_rule ... ok
test the_library_does_no_filesystem_network_or_process_io ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/parse.rs (<target>/debug/deps/unit_parse-2241bc6978a19d94)

running 15 tests
test a_forged_entity_identifier_breaks_the_profile ... ok
test a_person_agent_is_never_accepted ... ok
test bounds_are_one_mebibyte_and_sixteen_levels ... ok
test dangling_references_break_the_profile ... ok
test a_wrong_context_breaks_the_profile ... ok
test malformed_json_and_invalid_utf8_are_refused ... ok
test nesting_deeper_than_sixteen_is_refused ... ok
test nesting_of_exactly_sixteen_is_not_a_depth_refusal ... ok
test exactly_one_activity_is_required ... ok
test a_record_above_one_mebibyte_is_refused ... ok
test repeated_keys_are_refused_at_every_level ... ok
test non_utc_times_break_the_profile ... ok
test unknown_or_missing_profiles_are_refused ... ok
test unknown_members_and_wrong_types_break_the_profile ... ok
test a_record_of_exactly_one_mebibyte_is_read ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/unit/profile.rs (<target>/debug/deps/unit_profile-a546845d21984084)

running 8 tests
test context_and_profile_are_embedded ... ok
test graph_lists_activity_then_agents_then_entities_by_identifier ... ok
test equal_inputs_give_equal_bytes ... ok
test member_and_node_order_do_not_change_the_parsed_record ... ok
test serialization_matches_the_profile_byte_for_byte ... ok
test no_person_and_no_display_name_anywhere ... ok
test reserved_tool_agent_kind_round_trips ... ok
test parse_round_trip_returns_the_same_record ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests filesquid_prov

running 2 tests
test crates/prov/src/ids.rs - ids::percent_encode (line 23) ... ok
test crates/prov/src/lib.rs - (line 30) ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s

exit status: 0
```
